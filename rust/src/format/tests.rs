use super::*;
use crate::diff::compare;
use crate::json::{Document, NodeId, parse_bytes};

fn parse(s: &str) -> (Document, NodeId) {
    let mut doc = parse_bytes(s.as_bytes()).unwrap();
    let root = doc.single_root();
    (doc, root)
}

#[test]
fn pretty_keeps_order_and_escapes() {
    let (doc, root) = parse(r#"{"b":"<\u0001>","a":[1,{}],"c":[]}"#);
    let got = PrettyFormatter::new(false).format(&doc, root);
    assert_eq!(
        got,
        "{\n  \"b\": \"<\\u0001>\",\n  \"a\": [\n    1,\n    {}\n  ],\n  \"c\": []\n}\n"
    );
}

#[test]
fn compact_and_raw() {
    let (doc, root) = parse(r#"{"a": [1, 2], "b": {"c": "x"}}"#);
    assert_eq!(
        PrettyFormatter::compact(false).format(&doc, root),
        "{\"a\":[1,2],\"b\":{\"c\":\"x\"}}\n"
    );
    let (sdoc, s) = parse(r#""line1\nline2""#);
    assert_eq!(
        PrettyFormatter::new(false).with_raw(true).format(&sdoc, s),
        "line1\nline2\n"
    );
    let arr = doc.children(root)[0];
    assert_eq!(
        PrettyFormatter::new(false).with_raw(true).format(&doc, arr),
        "[\n  1,\n  2\n]\n"
    );
}

#[test]
fn color_output() {
    let (doc, root) = parse(r#"{"a":null}"#);
    assert_eq!(
        PrettyFormatter::compact(true).format(&doc, root),
        "{\x1b[36m\"a\"\x1b[0m:\x1b[90mnull\x1b[0m}\n"
    );
}

#[test]
fn flat() {
    let (doc, root) = parse(r#"{"a":{"b":[1,"x"]},"odd key":{},"e":[],"n":null}"#);
    let want = "$.a.b[0] = 1\n$.a.b[1] = \"x\"\n$[\"odd key\"] = {}\n$.e = []\n$.n = null\n";
    assert_eq!(FlatFormatter::new(false).format(&doc, root), want);
    let (doc, root) = parse("5");
    assert_eq!(FlatFormatter::new(false).format(&doc, root), "$ = 5\n");
}

#[test]
fn schema_merges_array_elements() {
    let (doc, root) = parse(
        r#"{
        "items": [{"id": 1, "name": "a"}, {"id": 2, "name": null, "extra": true}],
        "mixed": [1, "x", [true]],
        "empty": [],
        "maybe": [{"x": 1}, null]
    }"#,
    );
    let want = r#"{
  "items": [
    {
      "id": number,
      "name": string | null,
      "extra"?: boolean
    }
  ],
  "mixed": [[boolean] | string | number],
  "empty": [],
  "maybe": [
    {
      "x": number
    } | null
  ]
}
"#;
    assert_eq!(SchemaFormatter::new(false).format(&doc, root), want);
}

#[test]
fn schema_format_many_merges_documents() {
    let doc = parse_bytes(b"{\"a\":1}\n{\"a\":\"x\",\"b\":2}").unwrap();
    let got = SchemaFormatter::new(false).format_many(&doc, doc.roots());
    assert_eq!(got, "{\n  \"a\": string | number,\n  \"b\"?: number\n}\n");
}

#[test]
fn schema_edge_cases() {
    let (doc, root) = parse(r#"[{}, {"a": []}, [], {"a": [[{"b": 1}]]}]"#);
    let got = SchemaFormatter::new(false).format(&doc, root);
    let want = "[\n  {\n    \"a\"?: [\n      [\n        {\n          \"b\": number\n        }\n      ]\n    ]\n  } | []\n]\n";
    assert_eq!(got, want);
}

#[test]
fn typed_formatter() {
    let (doc, root) = parse(r#"{"a":[1],"b c":"x"}"#);
    let want =
        "root {2}  object\n|- \"a\" [1]  array\n|  `- [0]: 1  number\n`- \"b c\": \"x\"  string\n";
    assert_eq!(TypedFormatter::new(false).format(&doc, root), want);
}

#[test]
fn typed_formatter_on_subtree_keeps_branch_prefix() {
    let (doc, root) = parse(r#"{"x":{"a":1,"b":[true,null]}}"#);
    let sub = doc.children(root)[0];
    let want = "`- \"x\" {2}  object\n   |- \"a\": 1  number\n   `- \"b\" [2]  array\n      |- [0]: true  boolean\n      `- [1]: null  null\n";
    assert_eq!(TypedFormatter::new(false).format(&doc, sub), want);
}

#[test]
fn diff_format() {
    let (a, ra) = parse(r#"{"a":1,"b":[1]}"#);
    let (b, rb) = parse(r#"{"a":2,"c":true}"#);
    let changes = compare(&a, ra, &b, rb);
    assert_eq!(
        format_diff(&changes, &a, &b, false),
        "~ $.a: 1 -> 2\n- $.b: [1]\n+ $.c: true\n"
    );
    assert_eq!(
        format_diff(&changes[..1], &a, &b, true),
        "\x1b[33m~ $.a: 1 -> 2\x1b[0m\n"
    );
}
