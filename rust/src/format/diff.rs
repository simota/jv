use super::Colorizer;
use crate::diff::Change;
use crate::json::{Document, marshal_into};

/// Renders changes as one line per difference:
///
/// ```text
/// ~ $.age: 30 -> 31
/// + $.tags[3]: "new"
/// - $.legacy: {"a":1}
/// ```
pub fn format_diff(changes: &[Change], old: &Document, new: &Document, color: bool) -> String {
    let c = Colorizer::new(color);
    let mut out = String::new();
    for ch in changes {
        let mut line = format!("{} {}: ", ch.kind().symbol(), ch.path());
        let code = match *ch {
            Change::Added { new: n, .. } => {
                marshal_into(&mut line, new, n, "");
                32
            }
            Change::Removed { old: o, .. } => {
                marshal_into(&mut line, old, o, "");
                31
            }
            Change::Changed { old: o, new: n, .. } => {
                marshal_into(&mut line, old, o, "");
                line.push_str(" -> ");
                marshal_into(&mut line, new, n, "");
                33
            }
        };
        c.push(&mut out, code, &line);
        out.push('\n');
    }
    out
}
