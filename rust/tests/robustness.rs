//! Property tests with generated input: no panics on arbitrary bytes or
//! paths, and invariants that must hold for every valid document.
//!
//! `JV_PROP_CASES` scales the number of cases (default 3000).

mod common;

use common::*;
use jv::diff;
use jv::format::{
    FlatFormatter, Formatter, PrettyFormatter, SchemaFormatter, TypedFormatter, flatten,
};
use jv::json::{Document, NodeId, marshal, parse_bytes};
use jv::path::Query;

fn cases() -> usize {
    std::env::var("JV_PROP_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3000)
}

fn parse(text: &[u8]) -> Option<(Document, NodeId)> {
    let mut doc = parse_bytes(text).ok()?;
    let root = doc.single_root();
    Some((doc, root))
}

#[test]
fn arbitrary_bytes_never_panic() {
    let mut rng = Rng(0xb17e_0001);
    for _ in 0..cases() {
        let len = rng.below(64);
        const ALPHABET: &[u8] = b"{}[],:\"\\ \n\t0123456789-+.eEtrufalsnx\x01\x7f\xff\xc3\xa9";
        let bytes: Vec<u8> = (0..len).map(|_| rng.pick(ALPHABET)).collect();
        if let Ok(mut doc) = parse_bytes(&bytes) {
            let root = doc.single_root();
            let _ = PrettyFormatter::new(true).format(&doc, root);
        }
        let valid = gen_document(&mut rng);
        let mutated = mutate(&mut rng, &valid);
        if let Some((doc, root)) = parse(&mutated) {
            let _ = TypedFormatter::new(false).format(&doc, root);
        }
    }
}

#[test]
fn arbitrary_paths_never_panic() {
    let mut rng = Rng(0xb17e_0002);
    let pieces = [
        "$",
        ".",
        "..",
        "[",
        "]",
        "*",
        "\"",
        "\\",
        "a",
        "é",
        "日",
        "-1",
        "0",
        "99999999999999999999999",
        " ",
        "\t",
        "x\"",
        "\\u",
        "\\ud800",
        "[\"",
        "\"]",
        "+",
        "_",
    ];
    for _ in 0..cases() {
        let doc_text = gen_document(&mut rng);
        let Some((doc, root)) = parse(doc_text.as_bytes()) else {
            continue;
        };
        let n = rng.below(8);
        let expr: String = (0..n).map(|_| rng.pick(&pieces)).collect();
        if let Ok(q) = Query::compile(&expr) {
            for id in q.find(&doc, root) {
                assert!(id.index() < doc.len());
            }
        }
    }
}

#[test]
fn marshal_round_trips() {
    let mut rng = Rng(0xb17e_0003);
    for _ in 0..cases() {
        let text = gen_document(&mut rng);
        let (doc, root) = parse(text.as_bytes()).expect("generated JSON is valid");
        let compact = marshal(&doc, root, "");
        let (doc2, root2) = parse(compact.as_bytes()).expect("marshal output is valid JSON");
        assert_eq!(
            marshal(&doc2, root2, ""),
            compact,
            "compact round trip of {text:?}"
        );
        let pretty = PrettyFormatter::new(false).format(&doc, root);
        let (doc3, root3) = parse(pretty.as_bytes()).expect("pretty output is valid JSON");
        assert_eq!(
            marshal(&doc3, root3, ""),
            compact,
            "pretty round trip of {text:?}"
        );
    }
}

#[test]
fn every_node_path_resolves_to_the_node() {
    let mut rng = Rng(0xb17e_0004);
    for _ in 0..cases() {
        let text = gen_document(&mut rng);
        let (doc, root) = parse(text.as_bytes()).expect("valid");
        doc.walk(root, |n| {
            let p = doc.path(n);
            let q = Query::compile(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
            assert_eq!(q.find(&doc, root), vec![n], "path {p} in {text:?}");
            true
        });
        // flat output paths are relative to the formatted node
        for entry in flatten(&doc, root) {
            let q = Query::compile(&entry.path).unwrap();
            assert_eq!(
                q.find(&doc, root),
                vec![entry.node],
                "flat path {}",
                entry.path
            );
        }
    }
}

#[test]
fn diff_invariants() {
    let mut rng = Rng(0xb17e_0005);
    for _ in 0..cases() {
        let text = gen_document(&mut rng);
        let (a, ra) = parse(text.as_bytes()).expect("valid");
        let (mut b, rb) = parse(text.as_bytes()).expect("valid");
        assert!(
            diff::compare(&a, ra, &b, rb).is_empty(),
            "self diff of {text:?}"
        );
        // key order does not matter
        b.sort_keys(rb);
        assert!(
            diff::compare(&a, ra, &b, rb).is_empty(),
            "sorted diff of {text:?}"
        );
        // comparing the other way round swaps additions and removals
        let other = gen_document(&mut rng);
        let (c, rc) = parse(other.as_bytes()).expect("valid");
        let count = |changes: &[diff::Change], k: diff::Kind| {
            changes.iter().filter(|c| c.kind() == k).count()
        };
        let fwd = diff::compare(&a, ra, &c, rc);
        let back = diff::compare(&c, rc, &a, ra);
        assert_eq!(
            count(&fwd, diff::Kind::Added),
            count(&back, diff::Kind::Removed),
            "{text:?} vs {other:?}"
        );
        assert_eq!(
            count(&fwd, diff::Kind::Removed),
            count(&back, diff::Kind::Added)
        );
        assert_eq!(
            count(&fwd, diff::Kind::Changed),
            count(&back, diff::Kind::Changed)
        );
    }
}

#[test]
fn formatters_handle_generated_documents() {
    let mut rng = Rng(0xb17e_0006);
    for _ in 0..cases() {
        let text = gen_document(&mut rng);
        let (doc, root) = parse(text.as_bytes()).expect("valid");
        let flat = FlatFormatter::new(false).format(&doc, root);
        assert_eq!(flat.lines().count(), flatten(&doc, root).len());
        let typed = TypedFormatter::new(false).format(&doc, root);
        let mut nodes = 0;
        doc.walk(root, |_| {
            nodes += 1;
            true
        });
        assert_eq!(typed.lines().count(), nodes, "one typed line per node");
        let _ = SchemaFormatter::new(true).format_many(&doc, doc.roots());
        let compact = PrettyFormatter::compact(false).format(&doc, root);
        assert_eq!(compact.lines().count(), 1);
    }
}
