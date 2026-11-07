use super::*;

fn parse_one(s: &str) -> (Document, NodeId) {
    let mut doc = parse_bytes(s.as_bytes()).unwrap_or_else(|e| panic!("parse {s:?}: {e}"));
    let root = doc.single_root();
    (doc, root)
}

fn keys(doc: &Document, id: NodeId) -> String {
    doc.children(id)
        .iter()
        .map(|&c| doc.key(c).as_text().into_owned())
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn preserves_key_order_and_sorts() {
    let (mut doc, root) = parse_one(r#"{"z":1,"a":2,"m":{"y":1,"b":2}}"#);
    assert_eq!(keys(&doc, root), "z,a,m");
    assert_eq!(keys(&doc, doc.children(root)[2]), "y,b");
    doc.sort_keys(root);
    assert_eq!(keys(&doc, root), "a,m,z");
    assert_eq!(keys(&doc, doc.children(root)[1]), "b,y");
}

#[test]
fn duplicate_keys_last_wins_first_position() {
    let (doc, root) = parse_one(r#"{"a":1,"b":2,"a":3}"#);
    let ch = doc.children(root);
    assert_eq!(ch.len(), 2);
    assert_eq!(doc.key(ch[0]).as_text(), "a");
    assert_eq!(doc.string_value(ch[0]), "3");
    assert_eq!(doc.parent(ch[0]), Some(root));
}

#[test]
fn duplicate_keys_in_large_objects() {
    // more than 16 members switches to the hash index
    let mut s = String::from("{");
    for i in 0..20 {
        s.push_str(&format!("\"k{i}\":{i},"));
    }
    s.push_str("\"k3\":\"x\",\"k19\":true}");
    let (doc, root) = parse_one(&s);
    let ch = doc.children(root);
    assert_eq!(ch.len(), 20);
    assert_eq!(doc.string_value(ch[3]), "\"x\"");
    assert_eq!(doc.string_value(ch[19]), "true");
}

#[test]
fn types_depth_and_verbatim_numbers() {
    let (doc, root) = parse_one(r#"{"s":"x","n":1.50,"b":false,"z":null,"a":[{"k":[]}]}"#);
    let want = [
        NodeType::String,
        NodeType::Number,
        NodeType::Boolean,
        NodeType::Null,
        NodeType::Array,
    ];
    for (i, w) in want.iter().enumerate() {
        assert_eq!(doc.node_type(doc.children(root)[i]), *w);
    }
    assert_eq!(doc.string_value(doc.children(root)[1]), "1.50");
    let deep = doc.children(doc.children(doc.children(root)[4])[0])[0];
    assert_eq!(doc.depth(deep), 3);
    assert_eq!(doc.path(deep), "$.a[0].k");
}

#[test]
fn multiple_documents() {
    let doc = parse_bytes(b"{\"a\":1}\n{\"a\":2}\n[3]").unwrap();
    assert_eq!(doc.roots().len(), 3);
    let (doc, root) = parse_one("{\"a\":1}\n{\"a\":2}");
    assert_eq!(doc.node_type(root), NodeType::Array);
    assert_eq!(doc.children(root).len(), 2);
    let leaf = doc.children(doc.children(root)[1])[0];
    assert_eq!(doc.path(leaf), "$[1].a");
    assert_eq!(doc.depth(leaf), 2);
}

#[test]
fn errors_have_position() {
    let cases = [
        ("{\n  \"a\": 1,\n  \"b\": }", 3, 8, "invalid character"),
        (r#"{"a":1,"#, 1, 8, "unexpected end"),
        ("{\"a\":1}\n{oops}", 2, 2, "invalid character"),
    ];
    for (input, line, col, msg) in cases {
        match parse_bytes(input.as_bytes()) {
            Err(ParseError::Syntax(e)) => {
                assert_eq!((e.line, e.column), (line, col), "{input:?}: {e}");
                assert!(e.to_string().contains(msg), "{input:?}: {e}");
            }
            other => panic!("{input:?}: expected syntax error, got {other:?}"),
        }
    }
    assert_eq!(parse_bytes(b"   ").unwrap_err(), ParseError::Empty);
}

#[test]
fn error_messages_match_go() {
    let cases: &[(&[u8], &str)] = &[
        (
            br#"{"a":"\x"}"#,
            "line 1, column 6: invalid string literal: invalid character 'x' in string escape code",
        ),
        (
            br#"{"a":"\u12g4"}"#,
            "line 1, column 6: invalid string literal: invalid character 'g' in \\u hexadecimal character escape",
        ),
        (
            b"[1,\xff]",
            "line 1, column 4: invalid character '\u{fffd}' looking for beginning of value",
        ),
        (
            b"[1,\x01]",
            "line 1, column 4: invalid character '\\x01' looking for beginning of value",
        ),
        (
            b"[1,\xc3\xa9]",
            "line 1, column 4: invalid character '\u{e9}' looking for beginning of value",
        ),
        (
            b"{\"a\":\"\\\xc3\xa9\"}",
            "line 1, column 6: invalid string literal: invalid character '\u{c3}' in string escape code",
        ),
        (
            b"[01]",
            "line 1, column 3: invalid character '1' after array element",
        ),
        (
            b"[1 2]",
            "line 1, column 4: invalid character '2' after array element",
        ),
        (
            br#"{"a" 1}"#,
            "line 1, column 6: invalid character '1' after object key",
        ),
        (
            br#"{"a":1 "b"}"#,
            "line 1, column 8: invalid character '\"' after object key:value pair",
        ),
        (
            b"{1:2}",
            "line 1, column 2: invalid character '1' looking for beginning of object key string",
        ),
        (
            b"{\"a\":1,}",
            "line 1, column 8: invalid character '}' looking for beginning of object key string",
        ),
        (
            b"[1,]",
            "line 1, column 4: invalid character ']' looking for beginning of value",
        ),
        (b"tru", "line 1, column 4: unexpected end of input"),
        (
            b"trux",
            "line 1, column 4: invalid character 'x' in literal true (expecting 'e')",
        ),
        (b"nul", "line 1, column 4: unexpected end of input"),
        (
            b"\"ab\x01\"",
            "line 1, column 4: invalid character '\\x01' in string literal",
        ),
        (b"1.", "line 1, column 3: unexpected end of input"),
        (
            b"1.x",
            "line 1, column 3: invalid character 'x' after decimal point in numeric literal",
        ),
        (b"-", "line 1, column 2: unexpected end of input"),
        (
            b"-x",
            "line 1, column 2: invalid character 'x' in numeric literal",
        ),
        (b"1e", "line 1, column 3: unexpected end of input"),
        (
            b"1e+x",
            "line 1, column 4: invalid character 'x' in exponent of numeric literal",
        ),
        (
            b"\xe3\x80\x80",
            "line 1, column 1: invalid character '\\u3000' looking for beginning of value",
        ),
        (
            b"[\xc2\xa0]",
            "line 1, column 2: invalid character '\\u00a0' looking for beginning of value",
        ),
        (
            b"'a'",
            "line 1, column 1: invalid character '\\'' looking for beginning of value",
        ),
        (b"\"abc", "line 1, column 5: unexpected end of input"),
        (b"\"abc\\", "line 1, column 6: unexpected end of input"),
    ];
    for (input, want) in cases {
        let err = parse_bytes(input).unwrap_err().to_string();
        assert_eq!(
            err,
            format!("invalid JSON at {want}"),
            "input {:?}",
            String::from_utf8_lossy(input)
        );
    }
}

#[test]
fn strings_are_decoded_like_go() {
    let (doc, root) =
        parse_one(r#"["\u0008\u000c\u001f\u007f\u2028<>&\ud800x\udc00\ud83d\ude00"]"#);
    let s = doc.children(root)[0];
    assert_eq!(
        doc.raw_string(s),
        "\x08\x0c\x1f\x7f\u{2028}<>&\u{fffd}x\u{fffd}😀"
    );
    assert_eq!(
        doc.string_value(s),
        "\"\\b\\f\\u001f\x7f\\u2028<>&\u{fffd}x\u{fffd}😀\""
    );
    let doc = parse_bytes(b"\"\xff\xe2\x82A\"").unwrap();
    assert_eq!(doc.raw_string(doc.roots()[0]), "\u{fffd}\u{fffd}\u{fffd}A");
}

#[test]
fn string_value_is_valid_json() {
    let (doc, root) = parse_one(r#"{"k":"a\u0000<b>\"日本"}"#);
    let k = doc.children(root)[0];
    assert_eq!(doc.string_value(k), r#""a\u0000<b>\"日本""#);
    assert_eq!(doc.raw_string(k), "a\x00<b>\"日本");
}

#[test]
fn path_quotes_special_keys() {
    let (doc, root) = parse_one(r#"{"key with space":{"a-b":[1]}}"#);
    let leaf = doc.children(doc.children(doc.children(root)[0])[0])[0];
    assert_eq!(doc.path(leaf), r#"$["key with space"]["a-b"][0]"#);
}

#[test]
fn marshal_round_trip() {
    let src = r#"{"z":[1,{"b":null,"a":true}],"e":{},"f":[],"s":"x<y"}"#;
    let (doc, root) = parse_one(src);
    assert_eq!(marshal(&doc, root, ""), src);
    let want = "{\n  \"z\": [\n    1,\n    {\n      \"b\": null,\n      \"a\": true\n    }\n  ],\n  \"e\": {},\n  \"f\": [],\n  \"s\": \"x<y\"\n}";
    assert_eq!(marshal(&doc, root, "  "), want);
}

#[test]
fn deep_nesting_is_rejected_without_overflowing() {
    let deep = "[".repeat(MAX_NESTING + 1) + &"]".repeat(MAX_NESTING + 1);
    let err = parse_bytes(deep.as_bytes()).unwrap_err().to_string();
    assert!(err.contains("max nesting"), "{err}");
    let ok = "[".repeat(MAX_NESTING) + &"]".repeat(MAX_NESTING);
    let doc = parse_bytes(ok.as_bytes()).unwrap();
    assert_eq!(doc.len(), MAX_NESTING);
    // an unterminated deep document fails cleanly too
    let open = "{\"a\":".repeat(MAX_NESTING - 1);
    assert!(parse_bytes(open.as_bytes()).is_err());
}

#[test]
fn walk_skips_children_when_asked() {
    let (doc, root) = parse_one(r#"{"a":{"b":1},"c":2}"#);
    let mut seen = Vec::new();
    doc.walk(root, |n| {
        seen.push(doc.path(n));
        doc.key(n).as_text() != "a"
    });
    assert_eq!(seen, ["$", "$.a", "$.c"]);
}
