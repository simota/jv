//! Non-interactive output formats ("pipe mode").

mod diff;
mod flat;
mod pretty;
mod schema;
mod typed;

pub use diff::format_diff;
pub use flat::{FlatEntry, FlatFormatter, flatten};
pub use pretty::PrettyFormatter;
pub use schema::SchemaFormatter;
pub use typed::TypedFormatter;

use crate::json::{Document, NodeId};

/// Renders one value (and a trailing newline) into a string.
pub trait Formatter {
    fn format_into(&self, out: &mut String, doc: &Document, id: NodeId);

    fn format(&self, doc: &Document, id: NodeId) -> String {
        let mut out = String::new();
        self.format_into(&mut out, doc, id);
        out
    }
}

/// ANSI SGR coloring; a no-op when disabled.
#[derive(Clone, Copy, Debug, Default)]
pub struct Colorizer {
    pub enabled: bool,
}

impl Colorizer {
    pub const fn new(enabled: bool) -> Self {
        Colorizer { enabled }
    }

    pub fn push(&self, out: &mut String, code: u8, s: &str) {
        if self.enabled {
            out.push_str("\x1b[");
            out.push_str(&code.to_string());
            out.push('m');
            out.push_str(s);
            out.push_str("\x1b[0m");
        } else {
            out.push_str(s);
        }
    }

    pub fn wrap(&self, code: u8, s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 9);
        self.push(&mut out, code, s);
        out
    }
}

pub(crate) const KEY: u8 = 36;
pub(crate) const STRING: u8 = 32;
pub(crate) const NUMBER: u8 = 33;
pub(crate) const BOOLEAN: u8 = 35;
pub(crate) const NULL: u8 = 90;
pub(crate) const TYPE_HINT: u8 = 90;

/// Color code for a scalar node, used by several formatters.
pub(crate) fn scalar_color(t: crate::json::NodeType) -> u8 {
    use crate::json::NodeType;
    match t {
        NodeType::String => STRING,
        NodeType::Number => NUMBER,
        NodeType::Boolean => BOOLEAN,
        _ => NULL,
    }
}

#[cfg(test)]
mod tests;
