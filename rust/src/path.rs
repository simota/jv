//! Path expressions such as `$.users[0].name`, `.items[*].id`,
//! `["key with space"]` or `list[-1]`.
//!
//! The syntax is the one jv shows in its TUI footer (and copies with `Y`),
//! plus a few conveniences:
//! - the leading `$` is optional (`.a.b`, `a.b` also work)
//! - `[*]` / `.*` match every child of an array or object
//! - negative indexes count from the end (`[-1]` is the last element)

use std::fmt;

use crate::gostr;
use crate::json::{Document, NodeId, NodeType, unquote};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Segment {
    Key(String),
    Index(i64),
    Wildcard,
}

/// A compiled path expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Query {
    expr: String,
    segments: Vec<Segment>,
}

/// A path that could not be compiled. The message matches the Go
/// implementation (`invalid path "$.a[": unterminated '['`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathError(String);

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PathError {}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.expr)
    }
}

impl Query {
    /// Parses a path expression.
    pub fn compile(expr: &str) -> Result<Query, PathError> {
        let s = expr.trim();
        let b = s.as_bytes();
        let qexpr = gostr::quote(expr);
        let err = |msg: String| Err(PathError(format!("invalid path {qexpr}: {msg}")));
        let mut segments = Vec::new();
        let mut i = usize::from(s.starts_with('$'));
        let mut first = true;
        while i < b.len() {
            let c = b[i];
            if c == b'.' {
                i += 1;
                match b.get(i) {
                    // `.[0]` (jq style) is the same as `[0]`
                    Some(b'[') => {}
                    Some(b'*') => {
                        segments.push(Segment::Wildcard);
                        i += 1;
                    }
                    Some(b'"') => {
                        let (end, key) = read_quoted(s, i)?;
                        segments.push(Segment::Key(key));
                        i = end;
                    }
                    _ => {
                        let end = scan_ident(b, i);
                        if end == i {
                            if i >= b.len() && segments.is_empty() {
                                // "." alone means the root
                                first = false;
                                continue;
                            }
                            return err(format!("expected key at position {}", i + 1));
                        }
                        segments.push(Segment::Key(s[i..end].to_string()));
                        i = end;
                    }
                }
            } else if c == b'[' {
                i += 1;
                match b.get(i) {
                    None => return err("unterminated '['".into()),
                    Some(b'*') => {
                        i += 1;
                        segments.push(Segment::Wildcard);
                    }
                    Some(b'"') => {
                        let (end, key) = read_quoted(s, i)?;
                        segments.push(Segment::Key(key));
                        i = end;
                    }
                    Some(_) => {
                        let Some(end) = s[i..].find(']') else {
                            return err("unterminated '['".into());
                        };
                        let raw = &s[i..i + end];
                        let Some(n) = parse_go_atoi(raw.trim()) else {
                            return err(format!("bad index {}", gostr::quote(raw)));
                        };
                        segments.push(Segment::Index(n));
                        i += end;
                    }
                }
                if b.get(i) != Some(&b']') {
                    return err("expected ']'".into());
                }
                i += 1;
            } else if first && c != b'$' {
                // bare leading key: `a.b`
                let end = scan_ident(b, i);
                if end == i {
                    return err(format!("unexpected {}", gostr::quote(&latin1(c))));
                }
                segments.push(Segment::Key(s[i..end].to_string()));
                i = end;
            } else {
                return err(format!(
                    "unexpected {} at position {}",
                    gostr::quote(&latin1(c)),
                    i + 1
                ));
            }
            first = false;
        }
        Ok(Query {
            expr: s.to_string(),
            segments,
        })
    }

    /// The (trimmed) source expression.
    pub fn as_str(&self) -> &str {
        &self.expr
    }

    /// Returns every node matched by the query below `root`, in document
    /// order.
    pub fn find(&self, doc: &Document, root: NodeId) -> Vec<NodeId> {
        let mut current = vec![root];
        for seg in &self.segments {
            let mut next = Vec::new();
            for &id in &current {
                let children = doc.children(id);
                match seg {
                    Segment::Wildcard => next.extend_from_slice(children),
                    Segment::Key(k) => {
                        if doc.node_type(id) != NodeType::Object {
                            continue;
                        }
                        if let Some(&c) = children
                            .iter()
                            .find(|&&c| doc.member_name(c) == Some(k.as_str()))
                        {
                            next.push(c);
                        }
                    }
                    Segment::Index(idx) => {
                        if doc.node_type(id) != NodeType::Array {
                            continue;
                        }
                        let len = children.len() as i64;
                        let idx = if *idx < 0 { idx + len } else { *idx };
                        if (0..len).contains(&idx) {
                            next.push(children[idx as usize]);
                        }
                    }
                }
            }
            current = next;
            if current.is_empty() {
                break;
            }
        }
        current
    }
}

/// Go's `string(byte)`: the byte as a Latin-1 character.
fn latin1(c: u8) -> String {
    char::from(c).to_string()
}

/// A bare key. Besides identifier characters it accepts `-` (and anything
/// else that is not a delimiter) so that kebab-case keys work unquoted.
fn scan_ident(b: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < b.len() && !matches!(b[j], b'.' | b'[' | b']' | b' ' | b'"') {
        j += 1;
    }
    j
}

fn read_quoted(s: &str, start: usize) -> Result<(usize, String), PathError> {
    let b = s.as_bytes();
    let mut j = start + 1;
    while j < b.len() {
        if b[j] == b'\\' {
            j += 2;
            continue;
        }
        if b[j] == b'"' {
            let lit = &b[start..=j];
            return match unquote(lit) {
                Ok(key) => Ok((j + 1, key)),
                Err(e) => Err(PathError(format!(
                    "invalid quoted key {}: {e}",
                    String::from_utf8_lossy(lit)
                ))),
            };
        }
        j += 1;
    }
    Err(PathError(format!(
        "invalid path {}: unterminated string",
        gostr::quote(s)
    )))
}

/// Go's `strconv.Atoi`: optional sign and decimal digits, 64-bit range.
fn parse_go_atoi(s: &str) -> Option<i64> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::{marshal, parse_bytes};

    const PATH_DOC: &str = r#"{
  "name": "jv",
  "items": [{"id": 1, "tags": ["a"]}, {"id": 2}, {"id": 3}],
  "misc": {"key with space": 1, "kebab-key": 2, "a.b": 3}
}"#;

    fn find_values(doc: &Document, root: NodeId, expr: &str) -> String {
        let q = Query::compile(expr).unwrap_or_else(|e| panic!("compile {expr:?}: {e}"));
        q.find(doc, root)
            .into_iter()
            .map(|n| marshal(doc, n, ""))
            .collect::<Vec<_>>()
            .join("|")
    }

    #[test]
    fn query_find() {
        let doc = parse_bytes(PATH_DOC.as_bytes()).unwrap();
        let root = doc.roots()[0];
        let whole = marshal(&doc, root, "");
        let cases: &[(&str, &str)] = &[
            ("$", &whole),
            (".", &whole),
            ("", &whole),
            ("$.name", r#""jv""#),
            (".name", r#""jv""#),
            ("name", r#""jv""#),
            ("items[1].id", "2"),
            ("$.items[-1].id", "3"),
            ("$.items[*].id", "1|2|3"),
            (".items.*.id", "1|2|3"),
            (r#"$.misc["key with space"]"#, "1"),
            ("$.misc.kebab-key", "2"),
            (r#"$.misc["a.b"]"#, "3"),
            (r#"$.misc."a.b""#, "3"),
            ("$.items[0].tags[0]", r#""a""#),
            ("$.missing", ""),
            ("$.items[10]", ""),
            ("$.name[0]", ""),
            ("$.items.id", ""),
            ("$.misc[*]", "1|2|3"),
            (".items.[1].id", "2"),
            ("$.items[ +1 ].id", "2"),
            ("$ab", ""),
        ];
        for (expr, want) in cases {
            assert_eq!(find_values(&doc, root, expr), *want, "expr {expr:?}");
        }
    }

    #[test]
    fn query_round_trips_node_path() {
        let doc = parse_bytes(PATH_DOC.as_bytes()).unwrap();
        let root = doc.roots()[0];
        doc.walk(root, |n| {
            let p = doc.path(n);
            let q = Query::compile(&p).unwrap();
            assert_eq!(q.find(&doc, root), vec![n], "path {p}");
            true
        });
    }

    #[test]
    fn compile_errors_match_go() {
        let cases = [
            ("$.a[", r#"invalid path "$.a[": unterminated '['"#),
            ("$.a[x]", r#"invalid path "$.a[x]": bad index "x""#),
            ("$.a[x", r#"invalid path "$.a[x": unterminated '['"#),
            (
                r#"$.a["b"#,
                r#"invalid path "$.a[\"b": unterminated string"#,
            ),
            ("$..a", r#"invalid path "$..a": expected key at position 3"#),
            (
                "$.a]",
                r#"invalid path "$.a]": unexpected "]" at position 4"#,
            ),
            ("$$", r#"invalid path "$$": unexpected "$" at position 2"#),
            ("]", r#"invalid path "]": unexpected "]""#),
            ("$.a[1 x", r#"invalid path "$.a[1 x": unterminated '['"#),
            ("$.a[1}", r#"invalid path "$.a[1}": unterminated '['"#),
            ("$.a[\"b\"x", r#"invalid path "$.a[\"b\"x": expected ']'"#),
            (
                ".\"a\tb\"",
                "invalid quoted key \"a\tb\": invalid character '\\t' in string literal",
            ),
            ("  $.a[ ", r#"invalid path "  $.a[ ": unterminated '['"#),
            (
                ".é]",
                "invalid path \".é]\": unexpected \"]\" at position 4",
            ),
        ];
        for (expr, want) in cases {
            assert_eq!(
                Query::compile(expr).unwrap_err().to_string(),
                want,
                "expr {expr:?}"
            );
        }
    }
}
