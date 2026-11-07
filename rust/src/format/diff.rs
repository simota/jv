use super::Colorizer;
use crate::diff::{Change, Kind};
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
        let mut line = format!("{} {}: ", ch.kind.symbol(), ch.path);
        let code = match ch.kind {
            Kind::Added => {
                marshal_into(&mut line, new, ch.new.expect("added value"), "");
                32
            }
            Kind::Removed => {
                marshal_into(&mut line, old, ch.old.expect("removed value"), "");
                31
            }
            Kind::Changed => {
                marshal_into(&mut line, old, ch.old.expect("old value"), "");
                line.push_str(" -> ");
                marshal_into(&mut line, new, ch.new.expect("new value"), "");
                33
            }
        };
        c.push(&mut out, code, &line);
        out.push('\n');
    }
    out
}
