use super::{Colorizer, Formatter, KEY, TYPE_HINT, scalar_color};
use crate::json::{Document, NodeId, NodeType, quote_string};

/// A tree view with a type hint on every line:
///
/// ```text
/// root {2}  object
/// |- "a" [1]  array
/// |  `- [0]: 1  number
/// `- "b c": "x"  string
/// ```
#[derive(Clone, Debug, Default)]
pub struct TypedFormatter {
    color: Colorizer,
}

impl TypedFormatter {
    pub fn new(color: bool) -> Self {
        TypedFormatter {
            color: Colorizer::new(color),
        }
    }

    fn walk(&self, out: &mut String, doc: &Document, root: NodeId) {
        // (node, prefix, is_last)
        let mut stack = vec![(root, String::new(), true)];
        while let Some((id, prefix, is_last)) = stack.pop() {
            let has_parent = doc.parent(id).is_some();
            out.push_str(&prefix);
            if has_parent {
                out.push_str(if is_last { "`- " } else { "|- " });
            }
            self.label(out, doc, id);
            if let Some(value) = self.value(doc, id) {
                out.push_str(": ");
                out.push_str(&value);
            }
            out.push_str("  ");
            self.color.push(out, TYPE_HINT, doc.node_type(id).as_str());
            out.push('\n');

            let children = doc.children(id);
            if children.is_empty() {
                continue;
            }
            let mut next = prefix;
            if has_parent {
                next.push_str(if is_last { "   " } else { "|  " });
            }
            let last = children.len() - 1;
            for (i, &child) in children.iter().enumerate().rev() {
                stack.push((child, next.clone(), i == last));
            }
        }
    }

    fn label(&self, out: &mut String, doc: &Document, id: NodeId) {
        match doc.parent(id) {
            None => self.color.push(out, KEY, "root"),
            Some(p) if doc.node_type(p) == NodeType::Array => {
                self.color
                    .push(out, KEY, &format!("[{}]", doc.key_text(id)));
            }
            Some(_) => self.color.push(out, KEY, &quote_string(&doc.key_text(id))),
        }
        let n = doc.children(id).len();
        match doc.node_type(id) {
            NodeType::Object => {
                out.push(' ');
                self.color.push(out, TYPE_HINT, &format!("{{{n}}}"));
            }
            NodeType::Array => {
                out.push(' ');
                self.color.push(out, TYPE_HINT, &format!("[{n}]"));
            }
            _ => {}
        }
    }

    fn value(&self, doc: &Document, id: NodeId) -> Option<String> {
        match doc.node_type(id) {
            NodeType::Object | NodeType::Array => None,
            NodeType::Null => Some(self.color.wrap(scalar_color(NodeType::Null), "null")),
            t => Some(self.color.wrap(scalar_color(t), &doc.string_value(id))),
        }
    }
}

impl Formatter for TypedFormatter {
    fn format_into(&self, out: &mut String, doc: &Document, id: NodeId) {
        self.walk(out, doc, id);
    }
}
