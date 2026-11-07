use super::{Colorizer, Formatter, KEY, scalar_color};
use crate::json::{Document, NodeId, NodeType, Value, quote_string};

/// Indented (or single-line) JSON that keeps the input key order.
#[derive(Clone, Debug, Default)]
pub struct PrettyFormatter {
    color: Colorizer,
    compact: bool,
    raw: bool,
}

impl PrettyFormatter {
    pub fn new(color: bool) -> Self {
        PrettyFormatter {
            color: Colorizer::new(color),
            ..Default::default()
        }
    }

    /// Renders each document on a single line.
    pub fn compact(color: bool) -> Self {
        PrettyFormatter {
            compact: true,
            ..Self::new(color)
        }
    }

    /// Makes top-level string values print without quotes or escaping
    /// (`jv -r -p .name`).
    pub fn with_raw(mut self, raw: bool) -> Self {
        self.raw = raw;
        self
    }

    fn write_node(&self, out: &mut String, doc: &Document, id: NodeId, depth: usize) {
        let node = doc.node(id);
        let (open, close) = match node.node_type() {
            NodeType::Object => ('{', '}'),
            NodeType::Array => ('[', ']'),
            t => {
                if t == NodeType::Null {
                    self.color.push(out, scalar_color(t), "null");
                } else {
                    self.color.push(out, scalar_color(t), &doc.string_value(id));
                }
                return;
            }
        };
        out.push(open);
        if node.children.is_empty() {
            out.push(close);
            return;
        }
        let is_object = node.node_type() == NodeType::Object;
        let last = node.children.len() - 1;
        for (i, &child) in node.children.iter().enumerate() {
            self.newline(out, depth + 1);
            if is_object {
                self.color
                    .push(out, KEY, &quote_string(&doc.key(child).as_text()));
                out.push(':');
                if !self.compact {
                    out.push(' ');
                }
            }
            self.write_node(out, doc, child, depth + 1);
            if i < last {
                out.push(',');
            }
        }
        self.newline(out, depth);
        out.push(close);
    }

    fn newline(&self, out: &mut String, depth: usize) {
        if self.compact {
            return;
        }
        out.push('\n');
        for _ in 0..depth {
            out.push_str("  ");
        }
    }
}

impl Formatter for PrettyFormatter {
    fn format_into(&self, out: &mut String, doc: &Document, id: NodeId) {
        if self.raw
            && let Value::String(s) = &doc.node(id).value
        {
            out.push_str(s);
            out.push('\n');
            return;
        }
        self.write_node(out, doc, id, 0);
        out.push('\n');
    }
}
