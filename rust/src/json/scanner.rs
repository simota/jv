//! Single-pass JSON parser that builds [`Document`] nodes directly,
//! preserving key order and number literals, and reporting the same error
//! messages and positions as the Go implementation.
//!
//! The parser is iterative (an explicit stack instead of recursion), so
//! deeply nested input cannot overflow the call stack; nesting is still
//! limited to [`MAX_NESTING`] like Go's `encoding/json`.

use std::collections::HashMap;

use super::{Document, Key, Node, NodeId, ParseError, SyntaxError, Value, unquote};
use crate::gostr::{decode_rune, json_quote_char};

/// Maximum container nesting depth.
pub const MAX_NESTING: usize = 10000;

/// Parses every top-level JSON value in `data` (a single document, JSON
/// Lines, or concatenated documents).
pub fn parse_bytes(data: &[u8]) -> Result<Document, ParseError> {
    let mut sc = Scanner {
        data,
        pos: 0,
        doc: Document::default(),
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
}

enum Frame {
    Object {
        id: NodeId,
        /// Key -> child position; built once an object has more than 16
        /// members (smaller objects use a linear scan).
        index: Option<HashMap<Box<str>, usize>>,
    },
    Array {
        id: NodeId,
    },
}

impl Frame {
    fn id(&self) -> NodeId {
        match self {
            Frame::Object { id, .. } | Frame::Array { id } => *id,
        }
    }
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
                attach(&mut self.doc, top, child);
                self.skip_ws();
                let Some(c) = self.peek() else {
                    return Err(self.unexpected(""));
                };
                let is_object = matches!(top, Frame::Object { .. });
                match (is_object, c) {
                    (_, b',') => self.pos += 1,
                    (true, b'}') | (false, b']') => {
                        self.pos += 1;
                        let done = stack.pop().map(|f| f.id());
                        completed = done;
                    }
                    (true, _) => return Err(self.unexpected("after object key:value pair")),
                    (false, _) => return Err(self.unexpected("after array element")),
                }
            }
            // Read the next member/element of the innermost container.
            let top = stack.last().expect("an open container");
            let top_id = top.id();
            match top {
                Frame::Object { .. } => {
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
                    key = Key::Name(name.into_boxed_str());
                }
                Frame::Array { .. } => {
                    key = Key::Index(self.doc.children(top_id).len());
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
                let frame = if is_object {
                    Frame::Object { id, index: None }
                } else {
                    Frame::Array { id }
                };
                return Ok((id, Some(frame)));
            }
            b'"' => Value::String(self.string()?.into_boxed_str()),
            b'-' | b'0'..=b'9' => Value::Number(self.number()?.into()),
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

    fn string(&mut self) -> SResult<String> {
        let start = self.pos;
        self.pos += 1; // opening quote
        let mut escaped = false;
        while let Some(c) = self.peek() {
            match c {
                b'"' => {
                    self.pos += 1;
                    let raw = &self.data[start + 1..self.pos - 1];
                    if !escaped && let Ok(s) = std::str::from_utf8(raw) {
                        return Ok(s.to_owned());
                    }
                    return unquote(&self.data[start..self.pos])
                        .map_err(|e| self.error_at(start, format!("invalid string literal: {e}")));
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

    fn number(&mut self) -> SResult<&str> {
        let start = self.pos;
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
        // Only ASCII bytes were consumed.
        Ok(std::str::from_utf8(&self.data[start..self.pos]).expect("ASCII number literal"))
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
fn attach(doc: &mut Document, frame: &mut Frame, child: NodeId) {
    match frame {
        Frame::Array { id } => doc.nodes_mut()[id.index()].children.push(child),
        Frame::Object { id, index } => {
            let Key::Name(name) = doc.key(child).clone() else {
                unreachable!("object members always have a name");
            };
            let children = &doc.nodes()[id.index()].children;
            let dup = match index {
                Some(map) => map.get(&name).copied(),
                None => children
                    .iter()
                    .position(|&c| matches!(doc.key(c), Key::Name(k) if *k == name)),
            };
            let children = &mut doc.nodes_mut()[id.index()].children;
            match dup {
                Some(i) => children[i] = child,
                None => {
                    children.push(child);
                    let len = children.len();
                    match index {
                        Some(map) => {
                            map.insert(name, len - 1);
                        }
                        None if len > 16 => {
                            let ids = children.clone();
                            let map = ids
                                .iter()
                                .enumerate()
                                .map(|(i, &c)| (key_name(doc, c), i))
                                .collect();
                            *index = Some(map);
                        }
                        None => {}
                    }
                }
            }
        }
    }
}

fn key_name(doc: &Document, id: NodeId) -> Box<str> {
    match doc.key(id) {
        Key::Name(s) => s.clone(),
        other => other.as_text().into(),
    }
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
