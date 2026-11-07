use super::{Colorizer, Formatter, KEY, scalar_color};
use crate::json::{Document, NodeId, NodeType};

/// One `path = value` line per leaf (and per empty container). Every printed
/// path can be fed back to `jv -p` or the TUI search.
#[derive(Clone, Debug, Default)]
pub struct FlatFormatter {
    color: Colorizer,
}

impl FlatFormatter {
    pub fn new(color: bool) -> Self {
        FlatFormatter {
            color: Colorizer::new(color),
        }
    }
}

impl Formatter for FlatFormatter {
    fn format_into(&self, out: &mut String, doc: &Document, id: NodeId) {
        for entry in flatten(doc, id) {
            self.color.push(out, KEY, &entry.path);
            out.push_str(" = ");
            match doc.node_type(entry.node) {
                NodeType::Array => out.push_str("[]"),
                NodeType::Object => out.push_str("{}"),
                NodeType::Null => self.color.push(out, scalar_color(NodeType::Null), "null"),
                t => self
                    .color
                    .push(out, scalar_color(t), &doc.string_value(entry.node)),
            }
            out.push('\n');
        }
    }
}

/// A leaf value together with its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatEntry {
    pub path: String,
    pub node: NodeId,
}

/// Lists every leaf value and empty container below `root` in document
/// order. Paths are relative to `root` (which itself is `$`).
pub fn flatten(doc: &Document, root: NodeId) -> Vec<FlatEntry> {
    let mut out = Vec::new();
    let mut stack = vec![(root, "$".to_string())];
    while let Some((id, path)) = stack.pop() {
        let children = doc.children(id);
        if children.is_empty() {
            out.push(FlatEntry { path, node: id });
            continue;
        }
        for &child in children.iter().rev() {
            stack.push((child, path.clone() + &doc.child_path_segment(id, child)));
        }
    }
    out
}
