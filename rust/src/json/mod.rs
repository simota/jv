//! JSON document model.
//!
//! A [`Document`] owns every node of one or more parsed JSON values in an
//! arena; nodes refer to each other with [`NodeId`]s. This mirrors the Go
//! implementation's pointer tree (parent links, ordered children, depth)
//! without reference counting or interior mutability.

mod encode;
mod scanner;
mod unquote;

use std::borrow::Cow;
use std::fmt;

pub use encode::{marshal, marshal_into, quote_string, write_quoted};
pub use scanner::{MAX_NESTING, parse_bytes};
pub(crate) use unquote::unquote;

/// Index of a node inside its [`Document`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The JSON type of a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeType {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
}

impl NodeType {
    pub fn as_str(self) -> &'static str {
        match self {
            NodeType::Object => "object",
            NodeType::Array => "array",
            NodeType::String => "string",
            NodeType::Number => "number",
            NodeType::Boolean => "boolean",
            NodeType::Null => "null",
        }
    }

    pub fn is_container(self) -> bool {
        matches!(self, NodeType::Object | NodeType::Array)
    }
}

impl fmt::Display for NodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A string stored in a [`Document`]'s string pool; resolve it with
/// [`Document::text`]. Keeping all key and string data in one buffer avoids
/// an allocation per string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Str {
    start: usize,
    len: usize,
}

/// How a node is addressed by its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// A top-level document (shown as `root`).
    Root,
    /// Position inside an array (or of a document inside a wrapped stream).
    Index(usize),
    /// Object member name.
    Name(Str),
}

/// Scalar payload of a node. Numbers keep their literal text so that
/// formatting never changes them (`1.50` stays `1.50`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Object,
    Array,
    String(Str),
    Number(Str),
    Bool(bool),
    Null,
}

impl Value {
    pub fn node_type(&self) -> NodeType {
        match self {
            Value::Object => NodeType::Object,
            Value::Array => NodeType::Array,
            Value::String(_) => NodeType::String,
            Value::Number(_) => NodeType::Number,
            Value::Bool(_) => NodeType::Boolean,
            Value::Null => NodeType::Null,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub key: Key,
    pub value: Value,
    pub children: Vec<NodeId>,
    pub parent: Option<NodeId>,
    pub depth: u32,
}

impl Node {
    pub fn node_type(&self) -> NodeType {
        self.value.node_type()
    }
}

/// A syntax error with a 1-based line and column (columns count bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError {
    pub msg: String,
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid JSON at line {}, column {}: {}",
            self.line, self.column, self.msg
        )
    }
}

impl std::error::Error for SyntaxError {}

/// Errors returned by [`parse_bytes`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Syntax(SyntaxError),
    /// The input contained only whitespace.
    Empty,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Syntax(e) => e.fmt(f),
            ParseError::Empty => f.write_str("no JSON value found in input"),
        }
    }
}

impl std::error::Error for ParseError {}

/// One or more parsed JSON documents.
#[derive(Clone, Debug, Default)]
pub struct Document {
    nodes: Vec<Node>,
    roots: Vec<NodeId>,
    /// String pool for keys, strings and number literals.
    strings: String,
}

impl Document {
    pub(crate) fn with_capacity(string_bytes: usize) -> Document {
        Document {
            strings: String::with_capacity(string_bytes),
            ..Document::default()
        }
    }

    /// Adds `s` to the string pool.
    pub(crate) fn intern(&mut self, s: &str) -> Str {
        let start = self.strings.len();
        self.strings.push_str(s);
        Str {
            start,
            len: s.len(),
        }
    }

    /// The text of a pooled string.
    pub fn text(&self, s: Str) -> &str {
        &self.strings[s.start..s.start + s.len]
    }

    /// The key of a node as text: `root`, the decimal index, or the member
    /// name.
    pub fn key_text(&self, id: NodeId) -> Cow<'_, str> {
        match self.nodes[id.index()].key {
            Key::Root => Cow::Borrowed("root"),
            Key::Index(i) => Cow::Owned(i.to_string()),
            Key::Name(s) => Cow::Borrowed(self.text(s)),
        }
    }

    /// The member name of a node inside an object.
    pub fn member_name(&self, id: NodeId) -> Option<&str> {
        match self.nodes[id.index()].key {
            Key::Name(s) => Some(self.text(s)),
            _ => None,
        }
    }

    /// The content of a string node.
    pub fn string(&self, id: NodeId) -> Option<&str> {
        match self.nodes[id.index()].value {
            Value::String(s) => Some(self.text(s)),
            _ => None,
        }
    }

    pub(crate) fn push(&mut self, node: Node) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).expect("more than u32::MAX JSON nodes"));
        self.nodes.push(node);
        id
    }

    /// The top-level documents, in input order.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Number of nodes allocated in the arena (an upper bound for ids).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.index()].children
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id.index()].parent
    }

    pub fn node_type(&self, id: NodeId) -> NodeType {
        self.nodes[id.index()].node_type()
    }

    pub fn depth(&self, id: NodeId) -> usize {
        self.nodes[id.index()].depth as usize
    }

    pub fn key(&self, id: NodeId) -> &Key {
        &self.nodes[id.index()].key
    }

    /// Returns the single document, or wraps several documents into a new
    /// array root (`$[0]`, `$[1]`, ...), as the Go `WrapDocuments` does.
    pub fn single_root(&mut self) -> NodeId {
        if self.roots.len() == 1 {
            return self.roots[0];
        }
        self.wrap_documents()
    }

    /// Combines all documents into a new array root.
    pub fn wrap_documents(&mut self) -> NodeId {
        let docs = self.roots.clone();
        let root = self.push(Node {
            key: Key::Root,
            value: Value::Array,
            children: docs.clone(),
            parent: None,
            depth: 0,
        });
        for (i, &doc) in docs.iter().enumerate() {
            let n = &mut self.nodes[doc.index()];
            n.key = Key::Index(i);
            n.parent = Some(root);
            self.set_depth(doc, 1);
        }
        root
    }

    fn set_depth(&mut self, id: NodeId, depth: u32) {
        let mut stack = vec![(id, depth)];
        while let Some((id, depth)) = stack.pop() {
            let n = &mut self.nodes[id.index()];
            n.depth = depth;
            stack.extend(n.children.iter().map(|&c| (c, depth + 1)));
        }
    }

    /// Sorts object keys (byte-wise) in the whole subtree.
    pub fn sort_keys(&mut self, id: NodeId) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            if self.node_type(id) == NodeType::Object {
                let mut children = std::mem::take(&mut self.nodes[id.index()].children);
                children.sort_by(|a, b| self.key_text(*a).cmp(&self.key_text(*b)));
                self.nodes[id.index()].children = children;
            }
            stack.extend_from_slice(&self.nodes[id.index()].children);
        }
    }

    /// Visits `id` and its descendants in document order. Returning `false`
    /// from `f` skips the children of the visited node.
    pub fn walk(&self, id: NodeId, mut f: impl FnMut(NodeId) -> bool) {
        let mut stack = vec![id];
        while let Some(id) = stack.pop() {
            if f(id) {
                stack.extend(self.children(id).iter().rev());
            }
        }
    }

    /// The jv path of a node (`$`, `$.a[0]`, `$["odd key"]`).
    pub fn path(&self, id: NodeId) -> String {
        let mut chain = vec![id];
        let mut cur = id;
        while let Some(p) = self.parent(cur) {
            chain.push(p);
            cur = p;
        }
        let mut out = String::from("$");
        for pair in chain.windows(2).rev() {
            out.push_str(&self.child_path_segment(pair[1], pair[0]));
        }
        out
    }

    /// The path suffix that addresses `child` within `parent`
    /// (`.key`, `["odd key"]` or `[3]`).
    pub fn child_path_segment(&self, parent: NodeId, child: NodeId) -> String {
        let key = self.key_text(child);
        if self.node_type(parent) == NodeType::Array {
            return format!("[{key}]");
        }
        if is_simple_key(&key) {
            format!(".{key}")
        } else {
            format!("[{}]", quote_string(&key))
        }
    }

    /// The value as JSON text for scalars. Containers yield `<nil>`, which is
    /// what the Go implementation prints for them (their payload is nil);
    /// callers only rely on it for search highlighting of empty containers.
    pub fn string_value(&self, id: NodeId) -> Cow<'_, str> {
        match self.node(id).value {
            Value::String(s) => Cow::Owned(quote_string(self.text(s))),
            Value::Number(n) => Cow::Borrowed(self.text(n)),
            Value::Bool(true) => Cow::Borrowed("true"),
            Value::Bool(false) => Cow::Borrowed("false"),
            Value::Null => Cow::Borrowed("null"),
            Value::Object | Value::Array => Cow::Borrowed("<nil>"),
        }
    }

    /// The unquoted content of string nodes, [`Self::string_value`] otherwise.
    pub fn raw_string(&self, id: NodeId) -> Cow<'_, str> {
        match self.string(id) {
            Some(s) => Cow::Borrowed(s),
            None => self.string_value(id),
        }
    }

    pub(crate) fn nodes_mut(&mut self) -> &mut Vec<Node> {
        &mut self.nodes
    }

    pub(crate) fn roots_mut(&mut self) -> &mut Vec<NodeId> {
        &mut self.roots
    }
}

/// `^[A-Za-z_][A-Za-z0-9_]*$`
pub fn is_simple_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    match bytes.next() {
        Some(c) if c.is_ascii_alphabetic() || c == b'_' => {}
        _ => return false,
    }
    bytes.all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

#[cfg(test)]
mod tests;
