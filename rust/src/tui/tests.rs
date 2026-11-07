use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::json::parse_bytes;
use crate::path::Query;

const DOC: &str = concat!(
    r#"{
  "name": "jv",
  "deep": {"level1": {"level2": {"needle": "found me"}}},
  "items": [{"id": 1, "tag": "x"}, {"id": 2, "tag": "needle-ish"}],
  "payload": "{\"inner\": [1, 2]}",
  "long": ""#,
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaEND",
    "\"\n}"
);

type Copied = Rc<RefCell<Vec<String>>>;

fn new_model(opts: Options) -> (Model, Copied) {
    let mut doc = parse_bytes(DOC.as_bytes()).unwrap();
    let root = doc.single_root();
    let mut m = Model::new(doc, root, opts);
    let copied: Copied = Rc::default();
    let sink = copied.clone();
    m.set_copy_fn(move |s| {
        sink.borrow_mut().push(s.to_string());
        Ok(())
    });
    m.update(Msg::Resize {
        width: 60,
        height: 20,
    });
    (m, copied)
}

fn depth(d: usize) -> Options {
    Options {
        depth: d,
        ..Options::default()
    }
}

fn press(m: &mut Model, keys: &[&str]) {
    for k in keys {
        let key = match *k {
            "enter" => Key::Enter,
            "esc" => Key::Esc,
            s => Key::Char(s.chars().next().unwrap()),
        };
        m.update(Msg::Key(key));
    }
}

fn type_search(m: &mut Model, term: &str) {
    press(m, &["/"]);
    for c in term.chars() {
        m.update(Msg::Key(Key::Char(c)));
    }
    press(m, &["enter"]);
}

#[test]
fn search_finds_collapsed_nodes() {
    let (mut m, _) = new_model(depth(1));
    type_search(&mut m, "found me");
    assert_eq!(
        m.current_path(),
        "$.deep.level1.level2.needle",
        "status {:?}",
        m.status()
    );
    assert!(m.status().contains("Match 1/1"), "{}", m.status());
}

#[test]
fn search_next_prev_wraps() {
    let (mut m, _) = new_model(depth(1));
    type_search(&mut m, "needle");
    // key "needle" and value "needle-ish" both match, in document order
    assert_eq!(m.current_path(), "$.deep.level1.level2.needle");
    assert!(m.status().contains("1/2"));
    press(&mut m, &["n"]);
    assert_eq!(m.current_path(), "$.items[1].tag");
    assert!(m.status().contains("2/2"));
    press(&mut m, &["n"]);
    assert_eq!(m.current_path(), "$.deep.level1.level2.needle");
    press(&mut m, &["N"]);
    assert_eq!(m.current_path(), "$.items[1].tag");
    press(&mut m, &["esc"]);
    assert!(m.matches().is_empty());
    assert_eq!(m.status(), "Search cleared");
}

#[test]
fn search_by_path_and_not_found() {
    let (mut m, _) = new_model(depth(0));
    type_search(&mut m, "$.items[*].id");
    assert_eq!(m.current_path(), "$.items[0].id");
    assert_eq!(m.matches().len(), 2);
    type_search(&mut m, "zzz-none");
    assert!(m.status().contains("not found"), "{}", m.status());
    assert!(m.matches().is_empty());
    type_search(&mut m, "$.a[");
    assert!(m.status().contains("invalid path"), "{}", m.status());
}

#[test]
fn search_ignores_array_index_keys() {
    let (mut m, _) = new_model(depth(1));
    type_search(&mut m, "1");
    let doc = m.document();
    for &n in m.matches() {
        let p = doc.parent(n).unwrap();
        assert!(
            !(doc.node_type(p) == crate::json::NodeType::Array && doc.node_type(n).is_container()),
            "array element matched by its index: {}",
            doc.path(n)
        );
    }
    assert_eq!(m.matches().len(), 3); // "level1" key, id 1, and the payload string
}

#[test]
fn focus_option() {
    let mut doc = parse_bytes(DOC.as_bytes()).unwrap();
    let root = doc.single_root();
    let focus = Query::compile(".items[*].tag").unwrap().find(&doc, root);
    let mut m = Model::new(
        doc,
        root,
        Options {
            focus,
            focus_label: ".items[*].tag".into(),
            ..depth(0)
        },
    );
    assert_eq!(m.current_path(), "$.items[0].tag");
    assert!(m.status().contains("1/2"), "{}", m.status());
    press(&mut m, &["n"]);
    assert_eq!(m.current_path(), "$.items[1].tag");
}

#[test]
fn copy_value_and_path() {
    let (mut m, copied) = new_model(depth(2));
    type_search(&mut m, "$.items[0]");
    press(&mut m, &["y", "Y"]);
    {
        let c = copied.borrow();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], "{\n  \"id\": 1,\n  \"tag\": \"x\"\n}");
        assert_eq!(c[1], "$.items[0]");
    }
    assert_eq!(m.status(), "Copied path $.items[0]");
    m.set_copy_fn(|_| Err("no clipboard".into()));
    press(&mut m, &["y"]);
    assert_eq!(m.status(), "Copy failed: no clipboard");
}

#[test]
fn detail_view_shows_full_and_embedded_json() {
    let (mut m, copied) = new_model(depth(2));
    type_search(&mut m, "$.long");
    assert!(
        !m.view().contains("END"),
        "precondition: long value should be cut off"
    );
    press(&mut m, &["enter"]);
    assert!(m.is_detail_mode());
    assert!(
        m.view().contains("END"),
        "detail view should show full value:\n{}",
        m.view()
    );
    press(&mut m, &["esc"]);
    assert!(!m.is_detail_mode());
    type_search(&mut m, "$.payload");
    press(&mut m, &["v"]);
    let view = m.view();
    assert!(
        view.contains("Embedded JSON:") && view.contains("\"inner\": ["),
        "{view}"
    );
    press(&mut m, &["y"]);
    assert_eq!(copied.borrow().last().unwrap(), r#"{"inner": [1, 2]}"#);
}

#[test]
fn collapsed_container_is_one_line_with_summary() {
    let (m, _) = new_model(depth(1));
    let view = m.view();
    assert!(view.contains(r#""deep": {… 1 key},"#), "{view}");
    assert!(view.contains(r#""items": [… 2 items],"#), "{view}");
}

#[test]
fn expand_subtree_and_depth_keeps_cursor() {
    let (mut m, _) = new_model(depth(1));
    type_search(&mut m, "$.deep");
    press(&mut m, &["esc", "e"]);
    assert!(m.view().contains(r#""needle": "found me""#), "{}", m.view());
    type_search(&mut m, "found me");
    press(&mut m, &["1"]);
    assert_eq!(m.current_path(), "$.deep");
}

#[test]
fn navigation_basics() {
    let (mut m, _) = new_model(depth(1));
    press(&mut m, &["j", "j"]);
    assert_eq!(m.current_path(), "$.deep");
    press(&mut m, &["l", "j"]);
    assert_eq!(m.current_path(), "$.deep.level1");
    press(&mut m, &["h"]);
    assert_eq!(m.current_path(), "$.deep");
    press(&mut m, &["G"]);
    assert_eq!(m.current_path(), "$.long");
    press(&mut m, &["g", "?"]);
    assert!(m.view().contains("copy path"));
    press(&mut m, &["x"]);
    assert!(!m.is_help_mode());
    press(&mut m, &["n"]);
    assert_eq!(m.status(), "No active search (press /)");
}

// Additional coverage beyond the Go tests.

#[test]
fn view_layout_and_footer() {
    let (m, _) = new_model(depth(1));
    let view = m.view();
    let lines: Vec<&str> = view.lines().collect();
    assert_eq!(lines.len(), 20, "header + 18 body rows + footer");
    assert!(
        lines[0].starts_with("jv - JSON Viewer")
            && lines[0].ends_with("[/] Search  [?] Help  [q] Quit")
    );
    assert_eq!(lines[0].chars().count(), 60);
    assert_eq!(lines[1], "{");
    assert_eq!(lines[2], r#"  "name": "jv","#);
    assert_eq!(lines[7], "}");
    assert_eq!(lines[19], "Path: $  Lines: 1/6  Depth: 0");
}

#[test]
fn paging_and_scrolling() {
    let mut doc = parse_bytes(
        format!(
            "[{}]",
            (0..100)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
        .as_bytes(),
    )
    .unwrap();
    let root = doc.single_root();
    let mut m = Model::new(doc, root, depth(2));
    m.update(Msg::Resize {
        width: 40,
        height: 12,
    });
    m.update(Msg::Key(Key::PageDown));
    assert_eq!(m.current_path(), "$[9]");
    // the cursor row (10) is now the last body row
    assert_eq!(
        m.view().lines().nth(1).unwrap(),
        "  0,",
        "scrolled:\n{}",
        m.view()
    );
    assert!(m.view().lines().nth(10).unwrap().contains("9,"));
    m.update(Msg::Key(Key::End));
    assert_eq!(m.current_path(), "$[99]");
    assert!(
        m.view().contains("]"),
        "closing bracket visible at the bottom"
    );
    m.update(Msg::Key(Key::PageUp));
    assert_eq!(m.current_path(), "$[89]");
    m.update(Msg::Key(Key::Home));
    assert_eq!(m.current_path(), "$");
}

#[test]
fn type_hints_toggle_and_ctrl_c_quits() {
    let (mut m, _) = new_model(Options {
        show_types: true,
        ..depth(1)
    });
    assert!(
        m.view().contains(r#""name": "jv", (string)"#),
        "{}",
        m.view()
    );
    press(&mut m, &["t"]);
    assert_eq!(m.status(), "Types: off");
    assert!(!m.view().contains("(string)"));
    assert_eq!(m.update(Msg::Key(Key::Ctrl('c'))), Command::Quit);
    assert_eq!(m.update(Msg::Key(Key::Char('q'))), Command::Quit);
}

#[test]
fn detail_view_scrolls_and_rewraps() {
    let (mut m, _) = new_model(depth(2));
    type_search(&mut m, "$.deep");
    m.update(Msg::Resize {
        width: 60,
        height: 6,
    });
    press(&mut m, &["v"]);
    let first = m.view();
    assert!(
        first.contains("Path: $.deep") && first.contains("(1 key)"),
        "{first}"
    );
    m.update(Msg::Key(Key::Char('j')));
    assert!(!m.view().contains("Path: $.deep\n"), "scrolled one line");
    m.update(Msg::Resize {
        width: 30,
        height: 10,
    });
    assert!(
        m.view().lines().nth(1).unwrap().starts_with("Path: $.deep"),
        "resize re-opens at top"
    );
    press(&mut m, &["q"]);
    assert!(!m.is_detail_mode());
}

#[test]
fn search_input_editing_and_paste() {
    let (mut m, _) = new_model(depth(1));
    press(&mut m, &["/"]);
    m.update(Msg::Paste("found".into()));
    m.update(Msg::Key(Key::Char('x')));
    m.update(Msg::Key(Key::Backspace));
    m.update(Msg::Paste(" me".into()));
    assert!(
        m.view()
            .lines()
            .last()
            .unwrap()
            .contains("Search: > found me"),
        "{}",
        m.view()
    );
    // ctrl+c does not quit while typing (as in the Go version)
    assert_eq!(m.update(Msg::Key(Key::Ctrl('c'))), Command::None);
    press(&mut m, &["enter"]);
    assert_eq!(m.current_path(), "$.deep.level1.level2.needle");
    press(&mut m, &["/", "esc"]);
    assert_eq!(m.current_path(), "$.deep.level1.level2.needle");
}

#[test]
fn collapse_expand_and_open_close_all() {
    let (mut m, _) = new_model(depth(1));
    press(&mut m, &["o"]);
    assert!(m.view().contains(r#""needle": "found me""#));
    press(&mut m, &["O"]);
    assert_eq!(
        m.view().lines().filter(|l| !l.is_empty()).count(),
        9,
        "{}",
        m.view()
    );
    press(&mut m, &[" "]);
    assert!(
        m.view().contains("{… 5 keys}"),
        "root collapsed:\n{}",
        m.view()
    );
    press(&mut m, &["enter"]);
    assert!(m.view().contains(r#""name": "jv","#));
}

#[test]
fn match_highlight_styles() {
    use ratatui::style::Modifier;
    let (mut m, _) = new_model(depth(4));
    type_search(&mut m, "needle");
    let lines = m.view_lines();
    let needle = lines
        .iter()
        .find(|l| super::text::line_text(l).contains("\"needle\""))
        .unwrap();
    let key_span = needle
        .spans
        .iter()
        .find(|s| s.content.contains("needle"))
        .unwrap();
    // without color, matches are underlined (the selected line is bold too)
    assert!(key_span.style.add_modifier.contains(Modifier::UNDERLINED));
}
