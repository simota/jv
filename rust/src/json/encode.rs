//! JSON encoding that preserves key order and number literals.

use super::{Document, NodeId, NodeType, Value};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Returns `s` as a JSON string literal, escaped like Go's `encoding/json`
/// with HTML escaping disabled: `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`,
/// other control characters as `\u00XX`, and U+2028/U+2029 as `\u2028`/`\u2029`.
pub fn quote_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    write_quoted(&mut out, s);
    out
}

/// Appends `s` as a JSON string literal to `out`.
pub fn write_quoted(out: &mut String, s: &str) {
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        let escape: Option<&str> = match b {
            b'"' => Some("\\\""),
            b'\\' => Some("\\\\"),
            0x08 => Some("\\b"),
            0x0c => Some("\\f"),
            b'\n' => Some("\\n"),
            b'\r' => Some("\\r"),
            b'\t' => Some("\\t"),
            _ => None,
        };
        if let Some(e) = escape {
            out.push_str(&s[start..i]);
            out.push_str(e);
            i += 1;
            start = i;
        } else if b < 0x20 {
            out.push_str(&s[start..i]);
            out.push_str("\\u00");
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0xf) as usize] as char);
            i += 1;
            start = i;
        } else if b == 0xe2
            && bytes.get(i + 1) == Some(&0x80)
            && matches!(bytes.get(i + 2), Some(0xa8 | 0xa9))
        {
            // U+2028 LINE SEPARATOR / U+2029 PARAGRAPH SEPARATOR
            out.push_str(&s[start..i]);
            out.push_str(if bytes[i + 2] == 0xa8 {
                "\\u2028"
            } else {
                "\\u2029"
            });
            i += 3;
            start = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[start..]);
    out.push('"');
}

/// Encodes the subtree at `id`. An empty `indent` produces compact output.
pub fn marshal(doc: &Document, id: NodeId, indent: &str) -> String {
    let mut out = String::new();
    marshal_into(&mut out, doc, id, indent);
    out
}

/// Like [`marshal`] but appends to `out`.
pub fn marshal_into(out: &mut String, doc: &Document, id: NodeId, indent: &str) {
    write_json(out, doc, id, indent, 0);
}

fn write_json(out: &mut String, doc: &Document, id: NodeId, indent: &str, depth: usize) {
    let node = doc.node(id);
    let (open, close) = match node.node_type() {
        NodeType::Object => ('{', '}'),
        NodeType::Array => ('[', ']'),
        _ => {
            write_scalar(out, &node.value);
            return;
        }
    };
    out.push(open);
    if node.children.is_empty() {
        out.push(close);
        return;
    }
    let is_object = node.node_type() == NodeType::Object;
    for (i, &child) in node.children.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        if !indent.is_empty() {
            out.push('\n');
            push_indent(out, indent, depth + 1);
        }
        if is_object {
            write_quoted(out, &doc.key(child).as_text());
            out.push(':');
            if !indent.is_empty() {
                out.push(' ');
            }
        }
        write_json(out, doc, child, indent, depth + 1);
    }
    if !indent.is_empty() {
        out.push('\n');
        push_indent(out, indent, depth);
    }
    out.push(close);
}

pub(crate) fn write_scalar(out: &mut String, value: &Value) {
    match value {
        Value::String(s) => write_quoted(out, s),
        Value::Number(n) => out.push_str(n),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Null => out.push_str("null"),
        Value::Object | Value::Array => out.push_str("<nil>"),
    }
}

pub(crate) fn push_indent(out: &mut String, indent: &str, depth: usize) {
    for _ in 0..depth {
        out.push_str(indent);
    }
}
