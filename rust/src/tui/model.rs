use std::borrow::Cow;
use std::collections::HashSet;

use super::input::TextInput;
use super::styles::Styles;
use super::{Key, Msg, Options, clipboard};
use crate::json::{Document, NodeId, NodeType, marshal};
use crate::path::Query;

/// What the event loop should do after an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    None,
    Quit,
}

/// One rendered line: a node, or the closing bracket of an expanded
/// container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub node: NodeId,
    pub closing: bool,
}

pub(super) type CopyFn = Box<dyn FnMut(&str) -> Result<(), String>>;

/// State of the viewer.
pub struct Model {
    pub(super) doc: Document,
    pub(super) root: NodeId,
    /// Expanded state per node id.
    pub(super) expanded: Vec<bool>,
    /// Visible nodes; `rows` are the visible lines (nodes plus closing
    /// brackets) and `node_row[i]` is the row of `flat_nodes[i]`. Only rows
    /// inside the window are rendered, so large documents stay fast.
    pub(super) flat_nodes: Vec<NodeId>,
    pub(super) node_row: Vec<usize>,
    pub(super) rows: Vec<Row>,
    pub(super) cursor: usize,
    pub(super) y_offset: usize,
    pub(super) body_height: usize,
    pub(super) styles: Styles,
    pub(super) show_types: bool,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) status_msg: String,
    pub(super) search_mode: bool,
    pub(super) help_mode: bool,
    pub(super) search: TextInput,

    // search / path results
    pub(super) matches: Vec<NodeId>,
    pub(super) match_set: HashSet<NodeId>,
    pub(super) match_idx: usize,
    pub(super) match_label: String,
    pub(super) match_is_path: bool,

    // full-value view of the selected node
    pub(super) detail_mode: bool,
    pub(super) detail_lines: Vec<String>,
    pub(super) detail_offset: usize,

    pub(super) copy_fn: CopyFn,
}

impl Model {
    pub fn new(doc: Document, root: NodeId, opts: Options) -> Model {
        let mut m = Model {
            expanded: vec![false; doc.len()],
            doc,
            root,
            flat_nodes: Vec::new(),
            node_row: Vec::new(),
            rows: Vec::new(),
            cursor: 0,
            y_offset: 0,
            body_height: 0,
            styles: Styles::new(opts.theme, opts.color),
            show_types: opts.show_types,
            width: 0,
            height: 0,
            status_msg: String::new(),
            search_mode: false,
            help_mode: false,
            search: TextInput::new("text or $.path", 256, 30),
            matches: Vec::new(),
            match_set: HashSet::new(),
            match_idx: 0,
            match_label: String::new(),
            match_is_path: false,
            detail_mode: false,
            detail_lines: Vec::new(),
            detail_offset: 0,
            copy_fn: Box::new(clipboard::copy),
        };
        m.apply_expand_depth(opts.depth);
        if !opts.focus.is_empty() {
            m.set_matches(opts.focus, opts.focus_label, true);
            m.jump_to_match(0);
        }
        m.rebuild();
        m
    }

    /// Replaces the clipboard writer (for tests and embedding).
    pub fn set_copy_fn(&mut self, f: impl FnMut(&str) -> Result<(), String> + 'static) {
        self.copy_fn = Box::new(f);
    }

    pub fn status(&self) -> &str {
        &self.status_msg
    }

    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The node under the cursor.
    pub fn current_node(&self) -> NodeId {
        self.flat_nodes
            .get(self.cursor)
            .copied()
            .unwrap_or(self.root)
    }

    pub fn current_path(&self) -> String {
        self.doc.path(self.current_node())
    }

    pub fn matches(&self) -> &[NodeId] {
        &self.matches
    }

    pub fn is_detail_mode(&self) -> bool {
        self.detail_mode
    }

    pub fn is_help_mode(&self) -> bool {
        self.help_mode
    }

    pub(super) fn rebuild(&mut self) {
        self.layout();
        if self.cursor >= self.flat_nodes.len() {
            self.cursor = self.flat_nodes.len().saturating_sub(1);
        }
        self.ensure_cursor_visible();
    }

    fn layout(&mut self) {
        enum Step {
            Open(NodeId),
            Close(NodeId),
        }
        self.flat_nodes.clear();
        self.node_row.clear();
        self.rows.clear();
        let mut stack = vec![Step::Open(self.root)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Open(n) => {
                    self.flat_nodes.push(n);
                    self.node_row.push(self.rows.len());
                    self.rows.push(Row {
                        node: n,
                        closing: false,
                    });
                    let children = self.doc.children(n);
                    if self.expanded[n.index()] && !children.is_empty() {
                        stack.push(Step::Close(n));
                        stack.extend(children.iter().rev().map(|&c| Step::Open(c)));
                    }
                }
                Step::Close(n) => self.rows.push(Row {
                    node: n,
                    closing: true,
                }),
            }
        }
    }

    fn ensure_cursor_visible(&mut self) {
        if self.body_height == 0 {
            return;
        }
        let line = self.current_line_index();
        if line < self.y_offset {
            self.y_offset = line;
        }
        if line >= self.y_offset + self.body_height {
            self.y_offset = line + 1 - self.body_height;
        }
        let max_offset = self.rows.len().saturating_sub(self.body_height);
        self.y_offset = self.y_offset.min(max_offset);
    }

    fn current_line_index(&self) -> usize {
        self.node_row
            .get(self.cursor)
            .copied()
            .unwrap_or(self.cursor)
    }

    /// Expands nodes above `depth` (0: only the root).
    fn apply_expand_depth(&mut self, depth: usize) {
        let (doc, expanded) = (&self.doc, &mut self.expanded);
        doc.walk(self.root, |n| {
            expanded[n.index()] = if depth == 0 {
                doc.parent(n).is_none()
            } else {
                doc.depth(n) < depth
            };
            true
        });
    }

    fn expand_all(&mut self, expanded: bool) {
        let (doc, state) = (&self.doc, &mut self.expanded);
        doc.walk(self.root, |n| {
            state[n.index()] = doc.parent(n).is_none() || expanded;
            true
        });
    }

    fn expand_subtree(&mut self, id: NodeId) {
        let (doc, state) = (&self.doc, &mut self.expanded);
        doc.walk(id, |n| {
            if !doc.children(n).is_empty() {
                state[n.index()] = true;
            }
            true
        });
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.flat_nodes.is_empty() {
            return;
        }
        let max = self.flat_nodes.len() - 1;
        self.cursor = self.cursor.saturating_add_signed(delta).min(max);
    }

    fn move_page(&mut self, direction: isize) {
        if self.flat_nodes.is_empty() {
            return;
        }
        let step = if self.body_height == 0 {
            10
        } else {
            self.body_height
        } as isize;
        let target = self.current_line_index() as isize + direction * step;
        // node_row is increasing: find the first node at/after (down) or the
        // last node at/before (up) the target row.
        let mut idx = self.node_row.partition_point(|&r| (r as isize) < target) as isize;
        if direction < 0
            && (idx as usize >= self.node_row.len()
                || self.node_row[idx as usize] as isize > target)
        {
            idx -= 1;
        }
        self.cursor = idx.clamp(0, self.flat_nodes.len() as isize - 1) as usize;
    }

    /// Moves the cursor to `target` if it is visible.
    fn set_cursor_to_node(&mut self, target: NodeId) {
        if let Some(i) = self.flat_nodes.iter().position(|&n| n == target) {
            self.cursor = i;
        }
    }

    /// Expands every ancestor of `target` and moves the cursor to it.
    fn reveal_node(&mut self, target: NodeId) {
        let mut p = self.doc.parent(target);
        while let Some(id) = p {
            self.expanded[id.index()] = true;
            p = self.doc.parent(id);
        }
        self.layout();
        self.set_cursor_to_node(target);
    }

    fn visible(&self, n: NodeId) -> bool {
        let mut p = self.doc.parent(n);
        while let Some(id) = p {
            if !self.expanded[id.index()] {
                return false;
            }
            p = self.doc.parent(id);
        }
        true
    }

    /// Searches keys and values of the whole tree (collapsed parts too).
    /// Terms starting with `$` are evaluated as paths.
    pub(super) fn find_matches(&self, term: &str) -> Result<Vec<NodeId>, String> {
        if term.starts_with('$') {
            let q = Query::compile(term).map_err(|e| e.to_string())?;
            return Ok(q.find(&self.doc, self.root));
        }
        let needle = lower(term);
        let doc = &self.doc;
        let mut out = Vec::new();
        doc.walk(self.root, |n| {
            if is_object_member(doc, n) && lower(&doc.key_text(n)).contains(needle.as_ref()) {
                out.push(n);
                return true;
            }
            if doc.children(n).is_empty()
                && !doc.node_type(n).is_container()
                && lower(&doc.raw_string(n)).contains(needle.as_ref())
            {
                out.push(n);
            }
            true
        });
        Ok(out)
    }

    pub(super) fn set_matches(&mut self, nodes: Vec<NodeId>, label: String, is_path: bool) {
        self.match_set = nodes.iter().copied().collect();
        self.matches = nodes;
        self.match_is_path = is_path;
        self.match_label = label;
        self.match_idx = 0;
    }

    fn clear_matches(&mut self) {
        self.matches.clear();
        self.match_set.clear();
        self.match_label.clear();
        self.match_idx = 0;
    }

    /// Index of the first match after (or at, with `include_current`) the
    /// cursor in document order; 0 when there is none (wrap around).
    fn first_match_from_cursor(&self, include_current: bool) -> usize {
        let cur = self.current_node();
        let mut passed = false;
        let mut found: Option<usize> = None;
        let mut idx = 0;
        let matches = &self.matches;
        self.doc.walk(self.root, |n| {
            if found.is_some() || idx >= matches.len() {
                return false;
            }
            if n == cur {
                passed = true;
                if include_current && matches[idx] == n {
                    found = Some(idx);
                    return false;
                }
            } else if passed && matches[idx] == n {
                found = Some(idx);
                return false;
            }
            if matches[idx] == n {
                idx += 1;
            }
            true
        });
        found.unwrap_or(0)
    }

    fn jump_to_match(&mut self, i: isize) {
        if self.matches.is_empty() {
            return;
        }
        let n = self.matches.len() as isize;
        self.match_idx = i.rem_euclid(n) as usize;
        self.reveal_node(self.matches[self.match_idx]);
        self.status_msg = self.match_status();
    }

    fn match_status(&self) -> String {
        if self.matches.is_empty() {
            return String::new();
        }
        format!(
            "Match {}/{}: {}  (n/N)",
            self.match_idx + 1,
            self.matches.len(),
            self.match_label
        )
    }

    fn copy(&mut self, text: &str, what: &str) {
        self.status_msg = match (self.copy_fn)(text) {
            Ok(()) => format!("Copied {what}"),
            Err(e) => format!("Copy failed: {e}"),
        };
    }

    fn toggle_current(&mut self) {
        let n = self.current_node();
        if !self.doc.children(n).is_empty() {
            self.expanded[n.index()] = !self.expanded[n.index()];
        }
    }

    /// Applies one event.
    pub fn update(&mut self, msg: Msg) -> Command {
        let key = match msg {
            Msg::Resize { width, height } => {
                self.width = width.into();
                self.height = height.into();
                self.body_height = self.height.saturating_sub(2).max(1);
                if self.detail_mode {
                    self.open_detail();
                }
                self.rebuild();
                return Command::None;
            }
            Msg::Paste(s) => {
                if self.search_mode {
                    self.search.insert(&s);
                }
                return Command::None;
            }
            Msg::Key(k) => k,
        };
        if self.search_mode {
            self.update_search(key);
            return Command::None;
        }
        if self.detail_mode {
            return self.update_detail(key);
        }
        let quit_key = matches!(key, Key::Ctrl('c') | Key::Char('q'));
        if self.help_mode && !quit_key {
            self.help_mode = false;
            return Command::None;
        }
        match key {
            Key::Ctrl('c') | Key::Char('q') => return Command::Quit,
            Key::Up | Key::Char('k') => self.move_cursor(-1),
            Key::Down | Key::Char('j') => self.move_cursor(1),
            Key::PageUp | Key::Ctrl('u') => self.move_page(-1),
            Key::PageDown | Key::Ctrl('d') => self.move_page(1),
            Key::Left | Key::Char('h') => {
                let n = self.current_node();
                if !self.doc.children(n).is_empty() && self.expanded[n.index()] {
                    self.expanded[n.index()] = false;
                } else if let Some(p) = self.doc.parent(n) {
                    self.set_cursor_to_node(p);
                }
            }
            Key::Right | Key::Char('l') => {
                let n = self.current_node();
                if !self.doc.children(n).is_empty() {
                    self.expanded[n.index()] = true;
                }
            }
            Key::Char(' ') => self.toggle_current(),
            Key::Enter => {
                if self.doc.children(self.current_node()).is_empty() {
                    self.open_detail();
                    return Command::None;
                }
                self.toggle_current();
            }
            Key::Char('v') => {
                self.open_detail();
                return Command::None;
            }
            Key::Char('e') => {
                let n = self.current_node();
                self.expand_subtree(n);
                self.status_msg = format!("Expanded {}", self.doc.path(n));
            }
            Key::Char('o') => self.expand_all(true),
            Key::Char('O') => self.expand_all(false),
            Key::Char('g') | Key::Home => self.cursor = 0,
            Key::Char('G') | Key::End => {
                self.layout();
                self.cursor = self.flat_nodes.len().saturating_sub(1);
            }
            Key::Char('y') => {
                let json = marshal(&self.doc, self.current_node(), "  ");
                self.copy(&json, "value");
            }
            Key::Char('Y') => {
                let path = self.current_path();
                self.copy(&path, &format!("path {path}"));
            }
            Key::Char('t') => {
                self.show_types = !self.show_types;
                self.status_msg = if self.show_types {
                    "Types: on"
                } else {
                    "Types: off"
                }
                .into();
            }
            Key::Char('?') => {
                self.help_mode = true;
                return Command::None;
            }
            Key::Char('/') => {
                self.search_mode = true;
                self.search.focus();
                self.search.set_value("");
                return Command::None;
            }
            Key::Char('n') => {
                if self.matches.is_empty() {
                    self.status_msg = "No active search (press /)".into();
                } else {
                    let i = self.first_match_from_cursor(false);
                    self.jump_to_match(i as isize);
                }
            }
            Key::Char('N') => {
                if self.matches.is_empty() {
                    self.status_msg = "No active search (press /)".into();
                } else {
                    let i = self.first_match_from_cursor(true);
                    self.jump_to_match(i as isize - 1);
                }
            }
            Key::Esc => {
                if !self.matches.is_empty() {
                    self.clear_matches();
                    self.status_msg = "Search cleared".into();
                }
            }
            Key::Char(c @ '1'..='9') => {
                let current = self.current_node();
                self.apply_expand_depth(c as usize - '0' as usize);
                self.layout();
                // keep the cursor on the same node, or its nearest visible ancestor
                let mut n = Some(current);
                while let Some(id) = n {
                    if self.visible(id) {
                        self.set_cursor_to_node(id);
                        break;
                    }
                    n = self.doc.parent(id);
                }
            }
            _ => {}
        }
        self.rebuild();
        Command::None
    }

    fn update_search(&mut self, key: Key) {
        match key {
            Key::Esc => {
                self.search_mode = false;
                self.search.blur();
            }
            Key::Enter => {
                let value = self.search.value();
                let term = value.trim();
                self.search_mode = false;
                self.search.blur();
                if term.is_empty() {
                    self.status_msg = "Search: empty".into();
                    return;
                }
                let found = match self.find_matches(term) {
                    Ok(f) => f,
                    Err(e) => {
                        self.status_msg = format!("Search: {e}");
                        return;
                    }
                };
                if found.is_empty() {
                    self.clear_matches();
                    self.status_msg = format!("Search: not found: {term}");
                    self.rebuild();
                    return;
                }
                self.set_matches(found, term.to_string(), term.starts_with('$'));
                let i = self.first_match_from_cursor(true);
                self.jump_to_match(i as isize);
                self.rebuild();
            }
            k => self.search.handle(k),
        }
    }

    fn update_detail(&mut self, key: Key) -> Command {
        let page = self.body_height.max(1);
        let max_offset = self.detail_lines.len().saturating_sub(page);
        match key {
            Key::Ctrl('c') => return Command::Quit,
            Key::Esc | Key::Char('q' | 'v') | Key::Enter => self.detail_mode = false,
            Key::Char('y') => {
                let text = self.detail_copy_text();
                self.copy(&text, "value");
            }
            Key::Char('Y') => {
                let path = self.current_path();
                self.copy(&path, &format!("path {path}"));
            }
            Key::Down | Key::Char('j') => self.detail_offset += 1,
            Key::Up | Key::Char('k') => self.detail_offset = self.detail_offset.saturating_sub(1),
            Key::PageDown | Key::Char(' ' | 'f') => self.detail_offset += page,
            Key::PageUp | Key::Char('b') => {
                self.detail_offset = self.detail_offset.saturating_sub(page)
            }
            Key::Ctrl('d') | Key::Char('d') => self.detail_offset += page / 2,
            Key::Ctrl('u') | Key::Char('u') => {
                self.detail_offset = self.detail_offset.saturating_sub(page / 2)
            }
            _ => {}
        }
        self.detail_offset = self.detail_offset.min(max_offset);
        Command::None
    }

    /// Shows the full selected value in a scrollable, wrapped view.
    pub(super) fn open_detail(&mut self) {
        let n = self.current_node();
        let doc = &self.doc;
        let t = doc.node_type(n);
        let mut b = format!("Path: {}\nType: {}", doc.path(n), t);
        match t {
            NodeType::String => {
                let raw = doc.raw_string(n);
                b.push_str(&format!("  ({} chars)\n\n", raw.chars().count()));
                b.push_str(&raw);
                if let Some(embedded) = embedded_json(&raw) {
                    b.push_str("\n\nEmbedded JSON:\n");
                    b.push_str(&embedded);
                }
            }
            NodeType::Object | NodeType::Array => {
                b.push_str(&format!("  ({})\n\n", container_summary(doc, n)));
                b.push_str(&marshal(doc, n, "  "));
            }
            _ => {
                b.push_str("\n\n");
                b.push_str(&doc.string_value(n));
            }
        }
        // Tabs become four spaces, as lipgloss renders them in the Go viewer
        // (ratatui would drop them).
        let b = super::text::display_safe(&b).replace('\t', "    ");
        self.detail_lines = super::text::wrap(&b, self.width);
        self.detail_offset = 0;
        self.detail_mode = true;
    }

    fn detail_copy_text(&self) -> String {
        let n = self.current_node();
        match self.doc.node_type(n) {
            NodeType::String => self.doc.raw_string(n).into_owned(),
            _ => marshal(&self.doc, n, "  "),
        }
    }
}

pub(super) fn is_object_member(doc: &Document, n: NodeId) -> bool {
    doc.parent(n)
        .is_some_and(|p| doc.node_type(p) == NodeType::Object)
}

/// Lowercases per character (like Go's `strings.ToLower`), without
/// allocating for text that is already lowercase ASCII.
pub(super) fn lower(s: &str) -> Cow<'_, str> {
    if s.bytes().all(|b| b.is_ascii() && !b.is_ascii_uppercase()) {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.chars().flat_map(char::to_lowercase).collect())
    }
}

/// `1 key`, `3 keys`, `1 item`, `2 items`.
pub(super) fn container_summary(doc: &Document, n: NodeId) -> String {
    let len = doc.children(n).len();
    match doc.node_type(n) {
        NodeType::Object if len == 1 => "1 key".into(),
        NodeType::Object => format!("{len} keys"),
        NodeType::Array if len == 1 => "1 item".into(),
        NodeType::Array => format!("{len} items"),
        _ => String::new(),
    }
}

/// Pretty prints `s` if it is a JSON object or array encoded in a string
/// (common in logs and API payloads).
fn embedded_json(s: &str) -> Option<String> {
    let t = s.trim();
    if t.len() < 2 || !(t.starts_with('{') || t.starts_with('[')) {
        return None;
    }
    let doc = crate::json::parse_bytes(t.as_bytes()).ok()?;
    match doc.roots() {
        [one] => Some(marshal(&doc, *one, "  ")),
        _ => None,
    }
}
