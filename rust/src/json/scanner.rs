//! Single-pass JSON parser that builds [`Document`] nodes directly,
//! preserving key order and number literals, and reporting the same error
//! messages and positions as the Go implementation.
//!
//! The parser is iterative (an explicit stack instead of recursion), so
//! deeply nested input cannot overflow the call stack; nesting is still
//! limited to [`MAX_NESTING`] like Go's `encoding/json`.

use std::collections::HashMap;

use super::{Document, Key, Node, NodeId, ParseError, Str, SyntaxError, Value, unquote};
use crate::gostr::{decode_rune, json_quote_char};

/// Maximum container nesting depth.
pub const MAX_NESTING: usize = 10000;

/// Parses every top-level JSON value in `data` (a single document, JSON
/// Lines, or concatenated documents).
pub fn parse_bytes(data: &[u8]) -> Result<Document, ParseError> {
    let mut sc = Scanner {
        data,
        pos: 0,
        // String data is at most about the input size; reserving it up front
        // avoids regrowing the pool (untouched capacity is not resident).
        doc: Document::with_capacity(data.len()),
        scratch: Vec::new(),
    };
    loop {
        sc.skip_ws();
        if sc.pos >= data.len() {
            break;
        }
        let root = sc.document().map_err(ParseError::Syntax)?;
        sc.doc.roots_mut().push(root);
    }
    if sc.doc.roots().is_empty() {
        return Err(ParseError::Empty);
    }
    Ok(sc.doc)
}

struct Scanner<'a> {
    data: &'a [u8],
    pos: usize,
    doc: Document,
    /// Children of the open containers, innermost last. Each container
    /// moves its children into an exactly-sized `Vec` when it closes, which
    /// avoids repeated reallocation while children are added.
    scratch: Vec<NodeId>,
}

/// An open container.
struct Frame {
    id: NodeId,
    is_object: bool,
    /// Where this container's children start in `Scanner::scratch`.
    start: usize,
    /// Key -> child position; built once an object has more than 16
    /// members (smaller objects use a linear scan).
    index: Option<HashMap<Box<str>, usize>>,
}

type SResult<T> = std::result::Result<T, SyntaxError>;

impl Scanner<'_> {
    fn error_at(&self, pos: usize, msg: impl Into<String>) -> SyntaxError {
        let (line, column) = line_col(self.data, pos);
        SyntaxError {
            msg: msg.into(),
            offset: pos,
            line,
            column,
        }
    }

    fn unexpected(&self, context: &str) -> SyntaxError {
        if self.pos >= self.data.len() {
            return self.error_at(self.pos, "unexpected end of input");
        }
        let (r, _) = decode_rune(&self.data[self.pos..]);
        self.error_at(
            self.pos,
            format!("invalid character {} {}", json_quote_char(r), context),
        )
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.data.get(self.pos) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    /// Parses one top-level value.
    fn document(&mut self) -> SResult<NodeId> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut key = Key::Root;
        let mut parent: Option<NodeId> = None;
        let mut depth = 0usize;
        loop {
            // Parse the start of a value: a scalar is complete right away,
            // a non-empty container becomes a new frame.
            let (id, opened) = self.value_start(key, parent, depth)?;
            let mut completed = match opened {
                Some(frame) => {
                    stack.push(frame);
                    None
                }
                None => Some(id),
            };
            // Attach completed values to their parents and consume
            // separators until the next member/element can be read.
            while let Some(child) = completed.take() {
                let Some(top) = stack.last_mut() else {
                    return Ok(child);
                };
                attach(&self.doc, &mut self.scratch, top, child);
                self.skip_ws();
                let Some(c) = self.peek() else {
                    return Err(self.unexpected(""));
                };
                match (top.is_object, c) {
                    (_, b',') => self.pos += 1,
                    (true, b'}') | (false, b']') => {
                        self.pos += 1;
                        let frame = stack.pop().expect("open container");
                        let children = self.scratch[frame.start..].to_vec();
                        self.scratch.truncate(frame.start);
                        self.doc.nodes_mut()[frame.id.index()].children = children;
                        completed = Some(frame.id);
                    }
                    (true, _) => return Err(self.unexpected("after object key:value pair")),
                    (false, _) => return Err(self.unexpected("after array element")),
                }
            }
            // Read the next member/element of the innermost container.
            let top = stack.last().expect("an open container");
            let top_id = top.id;
            match top.is_object {
                true => {
                    self.skip_ws();
                    if self.peek() != Some(b'"') {
                        return Err(self.unexpected("looking for beginning of object key string"));
                    }
                    let name = self.string()?;
                    self.skip_ws();
                    if self.peek() != Some(b':') {
                        return Err(self.unexpected("after object key"));
                    }
                    self.pos += 1;
                    key = Key::Name(name);
                }
                false => {
                    key = Key::Index(self.scratch.len() - top.start);
                }
            }
            parent = Some(top_id);
            depth = self.doc.depth(top_id) + 1;
        }
    }

    fn value_start(
        &mut self,
        key: Key,
        parent: Option<NodeId>,
        depth: usize,
    ) -> SResult<(NodeId, Option<Frame>)> {
        self.skip_ws();
        let Some(c) = self.peek() else {
            return Err(self.unexpected(""));
        };
        let value = match c {
            b'{' | b'[' => {
                if depth >= MAX_NESTING {
                    return Err(self.error_at(self.pos, "exceeded max nesting depth"));
                }
                self.pos += 1;
                let is_object = c == b'{';
                let id = self.new_node(
                    key,
                    if is_object {
                        Value::Object
                    } else {
                        Value::Array
                    },
                    parent,
                    depth,
                );
                self.skip_ws();
                let close = if is_object { b'}' } else { b']' };
                if self.peek() == Some(close) {
                    self.pos += 1;
                    return Ok((id, None));
                }
                let frame = Frame {
                    id,
                    is_object,
                    start: self.scratch.len(),
                    index: None,
                };
                return Ok((id, Some(frame)));
            }
            b'"' => Value::String(self.string()?),
            b'-' | b'0'..=b'9' => {
                let start = self.pos;
                self.number()?;
                let literal =
                    std::str::from_utf8(&self.data[start..self.pos]).expect("ASCII number literal");
                Value::Number(self.doc.intern(literal))
            }
            b't' => {
                self.literal(b"true")?;
                Value::Bool(true)
            }
            b'f' => {
                self.literal(b"false")?;
                Value::Bool(false)
            }
            b'n' => {
                self.literal(b"null")?;
                Value::Null
            }
            _ => return Err(self.unexpected("looking for beginning of value")),
        };
        Ok((self.new_node(key, value, parent, depth), None))
    }

    fn new_node(&mut self, key: Key, value: Value, parent: Option<NodeId>, depth: usize) -> NodeId {
        self.doc.push(Node {
            key,
            value,
            children: Vec::new(),
            parent,
            depth: depth as u32,
        })
    }

    /// Reads a string literal into the document's string pool.
    fn string(&mut self) -> SResult<Str> {
        let start = self.pos;
        self.pos += 1; // opening quote
        let mut escaped = false;
        while let Some(c) = self.peek() {
            match c {
                b'"' => {
                    self.pos += 1;
                    let raw = &self.data[start + 1..self.pos - 1];
                    if !escaped && let Ok(s) = std::str::from_utf8(raw) {
                        return Ok(self.doc.intern(s));
                    }
                    let decoded = unquote(&self.data[start..self.pos]).map_err(|e| {
                        self.error_at(start, format!("invalid string literal: {e}"))
                    })?;
                    return Ok(self.doc.intern(&decoded));
                }
                b'\\' => {
                    escaped = true;
                    self.pos += 2;
                }
                0..0x20 => return Err(self.unexpected("in string literal")),
                _ => self.pos += 1,
            }
        }
        Err(self.error_at(self.data.len(), "unexpected end of input"))
    }

    /// Consumes a number literal.
    fn number(&mut self) -> SResult<()> {
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        if self.peek() == Some(b'0') {
            self.pos += 1;
        } else if self.digits() == 0 {
            return Err(self.unexpected("in numeric literal"));
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.unexpected("after decimal point in numeric literal"));
            }
        }
        if let Some(b'e' | b'E') = self.peek() {
            self.pos += 1;
            if let Some(b'+' | b'-') = self.peek() {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.unexpected("in exponent of numeric literal"));
            }
        }
        Ok(())
    }

    fn digits(&mut self) -> usize {
        let start = self.pos;
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        self.pos - start
    }

    fn literal(&mut self, word: &[u8]) -> SResult<()> {
        for &expected in word {
            if self.peek() != Some(expected) {
                let word = std::str::from_utf8(word).expect("ASCII literal");
                return Err(self.unexpected(&format!(
                    "in literal {word} (expecting '{}')",
                    expected as char
                )));
            }
            self.pos += 1;
        }
        Ok(())
    }
}

/// Adds `child` to the container of `frame`. Duplicate object keys keep the
/// position of the first occurrence and the value of the last one, as in
/// Go's `encoding/json`.
fn attach(doc: &Document, scratch: &mut Vec<NodeId>, frame: &mut Frame, child: NodeId) {
    if !frame.is_object {
        scratch.push(child);
        return;
    }
    let name = member_name(doc, child);
    let members = &scratch[frame.start..];
    let dup = match &frame.index {
        Some(map) => map.get(name).copied(),
        None => members.iter().position(|&c| member_name(doc, c) == name),
    };
    if let Some(i) = dup {
        scratch[frame.start + i] = child;
        return;
    }
    scratch.push(child);
    let len = scratch.len() - frame.start;
    match &mut frame.index {
        Some(map) => {
            map.insert(name.into(), len - 1);
        }
        None if len > 16 => {
            let map = scratch[frame.start..]
                .iter()
                .enumerate()
                .map(|(i, &c)| (Box::from(member_name(doc, c)), i))
                .collect();
            frame.index = Some(map);
        }
        None => {}
    }
}

fn member_name(doc: &Document, id: NodeId) -> &str {
    doc.member_name(id)
        .expect("object members always have a name")
}

fn line_col(data: &[u8], offset: usize) -> (usize, usize) {
    let offset = offset.min(data.len());
    let mut line = 1;
    let mut col = 1;
    for &b in &data[..offset] {
        if b == b'\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}
