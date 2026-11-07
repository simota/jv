use std::collections::HashMap;

use super::{Colorizer, Formatter, KEY, TYPE_HINT};
use crate::json::{Document, NodeId, NodeType, quote_string};

/// Prints the inferred structure of one or more values. Array elements are
/// merged so that every key that appears in any element is listed; keys
/// missing from some elements are marked optional with `?` and values with
/// several observed types are shown as a union (`string | null`).
#[derive(Clone, Debug, Default)]
pub struct SchemaFormatter {
    color: Colorizer,
}

impl SchemaFormatter {
    pub fn new(color: bool) -> Self {
        SchemaFormatter {
            color: Colorizer::new(color),
        }
    }

    /// A single schema merged from several values (e.g. all lines of a JSON
    /// Lines file, or all results of a wildcard path).
    pub fn format_many(&self, doc: &Document, ids: &[NodeId]) -> String {
        let mut s = Schema::default();
        for &id in ids {
            s.merge(doc, id);
        }
        let mut out = String::new();
        self.write(&mut out, &s, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, s: &Schema, depth: usize) {
        let mut parts = 0;
        let mut sep = |out: &mut String| {
            if parts > 0 {
                out.push_str(" | ");
            }
            parts += 1;
        };
        if let Some(o) = &s.object {
            sep(out);
            self.write_object(out, o, depth);
        }
        if let Some(a) = &s.array {
            sep(out);
            self.write_array(out, a, depth);
        }
        for (i, t) in PRIMITIVE_ORDER.iter().enumerate() {
            if s.primitives[i] {
                sep(out);
                self.color.push(out, TYPE_HINT, t.as_str());
            }
        }
    }

    fn write_object(&self, out: &mut String, o: &ObjectSchema, depth: usize) {
        if o.fields.is_empty() {
            out.push_str("{}");
            return;
        }
        out.push_str("{\n");
        let indent = "  ".repeat(depth + 1);
        let last = o.fields.len() - 1;
        for (i, (key, field)) in o.fields.iter().enumerate() {
            out.push_str(&indent);
            self.color.push(out, KEY, &quote_string(key));
            if field.seen < o.count {
                self.color.push(out, TYPE_HINT, "?");
            }
            out.push_str(": ");
            self.write(out, &field.schema, depth + 1);
            if i < last {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str(&"  ".repeat(depth));
        out.push('}');
    }

    fn write_array(&self, out: &mut String, a: &ArraySchema, depth: usize) {
        let Some(items) = &a.items else {
            out.push_str("[]");
            return;
        };
        if items.is_inline() {
            out.push('[');
            self.write(out, items, depth);
            out.push(']');
            return;
        }
        out.push_str("[\n");
        out.push_str(&"  ".repeat(depth + 1));
        self.write(out, items, depth + 1);
        out.push('\n');
        out.push_str(&"  ".repeat(depth));
        out.push(']');
    }
}

impl Formatter for SchemaFormatter {
    fn format_into(&self, out: &mut String, doc: &Document, id: NodeId) {
        out.push_str(&self.format_many(doc, &[id]));
    }
}

const PRIMITIVE_ORDER: [NodeType; 4] = [
    NodeType::String,
    NodeType::Number,
    NodeType::Boolean,
    NodeType::Null,
];

/// A structural summary of one or more JSON values.
#[derive(Debug, Default)]
struct Schema {
    /// Observed primitive types, indexed like [`PRIMITIVE_ORDER`].
    primitives: [bool; 4],
    object: Option<ObjectSchema>,
    array: Option<Box<ArraySchema>>,
}

#[derive(Debug, Default)]
struct ObjectSchema {
    count: usize,
    /// Fields in first-seen order.
    fields: Vec<(String, FieldSchema)>,
    index: HashMap<String, usize>,
}

#[derive(Debug, Default)]
struct FieldSchema {
    seen: usize,
    schema: Schema,
}

#[derive(Debug, Default)]
struct ArraySchema {
    /// `None` when only empty arrays were observed.
    items: Option<Schema>,
}

impl Schema {
    fn merge(&mut self, doc: &Document, id: NodeId) {
        match doc.node_type(id) {
            NodeType::Object => {
                let o = self.object.get_or_insert_with(ObjectSchema::default);
                o.count += 1;
                for &child in doc.children(id) {
                    let key = doc.key(child).as_text();
                    let i = match o.index.get(key.as_ref()) {
                        Some(&i) => i,
                        None => {
                            o.index.insert(key.to_string(), o.fields.len());
                            o.fields.push((key.to_string(), FieldSchema::default()));
                            o.fields.len() - 1
                        }
                    };
                    let field = &mut o.fields[i].1;
                    field.seen += 1;
                    field.schema.merge(doc, child);
                }
            }
            NodeType::Array => {
                let a = self.array.get_or_insert_with(Box::default);
                for &child in doc.children(id) {
                    a.items
                        .get_or_insert_with(Schema::default)
                        .merge(doc, child);
                }
            }
            t => {
                let i = PRIMITIVE_ORDER
                    .iter()
                    .position(|&p| p == t)
                    .expect("primitive type");
                self.primitives[i] = true;
            }
        }
    }

    fn is_inline(&self) -> bool {
        if self.object.as_ref().is_some_and(|o| !o.fields.is_empty()) {
            return false;
        }
        if let Some(items) = self.array.as_ref().and_then(|a| a.items.as_ref())
            && !items.is_inline()
        {
            return false;
        }
        true
    }
}
