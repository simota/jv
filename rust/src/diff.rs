//! Structural differences between two JSON documents.
//!
//! Objects are compared by key (key order is ignored), arrays by index and
//! numbers by value (`1`, `1.0` and `1e0` are equal).

use std::collections::{HashMap, HashSet};

use crate::json::{Document, NodeId, NodeType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Removed,
    Changed,
}

impl Kind {
    pub fn symbol(self) -> &'static str {
        match self {
            Kind::Added => "+",
            Kind::Removed => "-",
            Kind::Changed => "~",
        }
    }
}

/// One difference, addressed by a jv path (`$.a[0].b`). Node ids refer to
/// the old or the new document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Added {
        path: String,
        new: NodeId,
    },
    Removed {
        path: String,
        old: NodeId,
    },
    Changed {
        path: String,
        old: NodeId,
        new: NodeId,
    },
}

impl Change {
    pub fn kind(&self) -> Kind {
        match self {
            Change::Added { .. } => Kind::Added,
            Change::Removed { .. } => Kind::Removed,
            Change::Changed { .. } => Kind::Changed,
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Change::Added { path, .. }
            | Change::Removed { path, .. }
            | Change::Changed { path, .. } => path,
        }
    }
}

/// Returns the differences between `a` (in `da`) and `b` (in `db`) in
/// document order.
pub fn compare(da: &Document, a: NodeId, db: &Document, b: NodeId) -> Vec<Change> {
    let mut out = Vec::new();
    // Depth-first work list (no recursion, so deep documents cannot overflow
    // the stack). Items are pushed in reverse so that changes come out in
    // document order.
    let mut pending = vec![Work::Compare(a, b, "$".to_string())];
    let mut items = Vec::new();
    while let Some(work) = pending.pop() {
        match work {
            Work::Emit(change) => out.push(change),
            Work::Compare(a, b, path) => {
                compare_one(da, a, db, b, path, &mut items);
                pending.extend(items.drain(..).rev());
            }
        }
    }
    out
}

enum Work {
    Compare(NodeId, NodeId, String),
    Emit(Change),
}

fn compare_one(
    da: &Document,
    a: NodeId,
    db: &Document,
    b: NodeId,
    path: String,
    out: &mut Vec<Work>,
) {
    let changed = |path: String| {
        Work::Emit(Change::Changed {
            path,
            old: a,
            new: b,
        })
    };
    let ta = da.node_type(a);
    if ta != db.node_type(b) {
        out.push(changed(path));
        return;
    }
    match ta {
        NodeType::Object => {
            let b_index: HashMap<_, NodeId> = db
                .children(b)
                .iter()
                .map(|&c| (db.key_text(c), c))
                .collect();
            let mut seen = HashSet::with_capacity(da.children(a).len());
            for &ac in da.children(a) {
                let key = da.key_text(ac);
                let child_path = path.clone() + &da.child_path_segment(a, ac);
                out.push(match b_index.get(&key) {
                    Some(&bc) => Work::Compare(ac, bc, child_path),
                    None => Work::Emit(Change::Removed {
                        path: child_path,
                        old: ac,
                    }),
                });
                seen.insert(key);
            }
            for &bc in db.children(b) {
                if !seen.contains(&db.key_text(bc)) {
                    let path = path.clone() + &db.child_path_segment(b, bc);
                    out.push(Work::Emit(Change::Added { path, new: bc }));
                }
            }
        }
        NodeType::Array => {
            let (ac, bc) = (da.children(a), db.children(b));
            let common = ac.len().min(bc.len());
            for (&x, &y) in ac.iter().zip(bc) {
                out.push(Work::Compare(
                    x,
                    y,
                    path.clone() + &da.child_path_segment(a, x),
                ));
            }
            for &x in &ac[common..] {
                let path = path.clone() + &da.child_path_segment(a, x);
                out.push(Work::Emit(Change::Removed { path, old: x }));
            }
            for &y in &bc[common..] {
                let path = path.clone() + &db.child_path_segment(b, y);
                out.push(Work::Emit(Change::Added { path, new: y }));
            }
        }
        NodeType::Number => {
            if !numbers_equal(&da.string_value(a), &db.string_value(b)) {
                out.push(changed(path));
            }
        }
        _ => {
            if da.string_value(a) != db.string_value(b) {
                out.push(changed(path));
            }
        }
    }
}

/// Numeric equality of two JSON number literals (`1`, `1.0`, `1e0`, `10e-1`
/// are equal; `0` equals `-0`).
///
/// The comparison is exact. The Go implementation parses both literals into
/// 64-bit-mantissa `big.Float`s, so it also reports literals as equal when
/// they differ only beyond ~19 significant digits; see
/// `docs/rust/feature-parity.md`.
pub fn numbers_equal(x: &str, y: &str) -> bool {
    if x == y {
        return true;
    }
    match (Decimal::parse(x), Decimal::parse(y)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// A normalized decimal: `sign * 0.digits * 10^exp`, with no leading or
/// trailing zeros in `digits` (zero has no digits and is unsigned).
#[derive(Debug, PartialEq, Eq)]
struct Decimal {
    negative: bool,
    digits: Vec<u8>,
    exp: i128,
}

impl Decimal {
    fn parse(s: &str) -> Option<Decimal> {
        let (negative, s) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s.strip_prefix('+').unwrap_or(s)),
        };
        let (mantissa, exp) = match s.find(['e', 'E']) {
            Some(i) => {
                let e = &s[i + 1..];
                // Exponents with more than 30 significant digits fall back to
                // textual comparison (leading zeros do not count); Go uses
                // the same rule.
                if e.trim_start_matches(['+', '-'])
                    .trim_start_matches('0')
                    .len()
                    > 30
                {
                    return None;
                }
                (&s[..i], e.parse::<i128>().ok()?)
            }
            None => (s, 0),
        };
        let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        if int.is_empty() && frac.is_empty() {
            return None;
        }
        if !int.bytes().chain(frac.bytes()).all(|c| c.is_ascii_digit()) {
            return None;
        }
        let all: Vec<u8> = int.bytes().chain(frac.bytes()).map(|c| c - b'0').collect();
        let lead = all.iter().take_while(|&&d| d == 0).count();
        let digits = &all[lead..];
        let trail = digits.iter().rev().take_while(|&&d| d == 0).count();
        let digits = digits[..digits.len() - trail].to_vec();
        if digits.is_empty() {
            return Some(Decimal {
                negative: false,
                digits,
                exp: 0,
            });
        }
        // value = 0.d1d2... * 10^(int.len() - lead + exp)
        let exp = int.len() as i128 - lead as i128 + exp;
        Some(Decimal {
            negative,
            digits,
            exp,
        })
    }
}

/// `1 changed, 2 added, 1 removed`, or `no differences`.
pub fn summary(changes: &[Change]) -> String {
    let count = |k: Kind| changes.iter().filter(|c| c.kind() == k).count();
    let parts: Vec<String> = [
        (count(Kind::Changed), "changed"),
        (count(Kind::Added), "added"),
        (count(Kind::Removed), "removed"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect();
    if parts.is_empty() {
        "no differences".into()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::parse_bytes;

    fn describe(a: &str, b: &str) -> String {
        let da = parse_bytes(a.as_bytes()).unwrap();
        let db = parse_bytes(b.as_bytes()).unwrap();
        compare(&da, da.roots()[0], &db, db.roots()[0])
            .iter()
            .map(|c| format!("{}{}", c.kind().symbol(), c.path()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn compare_cases() {
        let cases = [
            (r#"{"a":1}"#, r#"{"a":1}"#, ""),
            (r#"{"a":1,"b":2}"#, r#"{"b":2,"a":1}"#, ""),
            (r#"{"n":1.0}"#, r#"{"n":1e0}"#, ""),
            (r#"{"n":1}"#, r#"{"n":"1"}"#, "~$.n"),
            (
                r#"{"a":{"x":1,"y":2}}"#,
                r#"{"a":{"x":3,"z":4}}"#,
                "~$.a.x -$.a.y +$.a.z",
            ),
            ("[1,2,3]", "[1,5]", "~$[1] -$[2]"),
            ("[1]", r#"[1,{"k":true}]"#, "+$[1]"),
            (r#"{"odd key":null}"#, "{}", r#"-$["odd key"]"#),
            (r#""x""#, r#""y""#, "~$"),
            (r#"{"a":[]}"#, r#"{"a":{}}"#, "~$.a"),
            (
                r#"{"a":[1,{"b":2}],"c":3}"#,
                r#"{"a":[1,{"b":3}],"c":4}"#,
                "~$.a[1].b ~$.c",
            ),
        ];
        for (a, b, want) in cases {
            assert_eq!(describe(a, b), want, "{a} vs {b}");
        }
    }

    #[test]
    fn number_equality() {
        for (x, y) in [
            ("1", "1.0"),
            ("1", "1e0"),
            ("10e-1", "1"),
            ("0", "-0"),
            ("0.0e5", "0"),
            ("120", "1.2e2"),
            ("-0.5", "-5E-1"),
            ("1e0000000000000000000000000000001", "10"),
            ("1e-0000000000000000000000000000000001", "0.1"),
            ("5E+0000000000000000000000000000000", "5"),
        ] {
            assert!(numbers_equal(x, y), "{x} == {y}");
        }
        for (x, y) in [("1", "2"), ("1", "-1"), ("0.1", "0.01"), ("1e400", "1e401")] {
            assert!(!numbers_equal(x, y), "{x} != {y}");
        }
    }

    #[test]
    fn summary_text() {
        let da = parse_bytes(br#"{"a":1,"b":1}"#).unwrap();
        let db = parse_bytes(br#"{"a":2,"c":1,"d":1}"#).unwrap();
        let ch = compare(&da, da.roots()[0], &db, db.roots()[0]);
        assert_eq!(summary(&ch), "1 changed, 2 added, 1 removed");
        assert_eq!(summary(&[]), "no differences");
    }
}
