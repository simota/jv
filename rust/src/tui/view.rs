use ratatui::Frame;
use ratatui::text::{Line, Span};

use super::model::{Model, Row, container_summary, is_object_member, lower};
use super::text::{display_safe, join_with_gap, line_text, styled, truncate, width, wrap};
use crate::json::{NodeId, NodeType, quote_string};

const HELP: &[&str] = &[
    "Navigate  Up/Down k/j  PgUp/PgDn Ctrl+u/d  g/G top/bottom",
    "  h / l         Collapse (or go to parent) / Expand",
    "  Space         Toggle",
    "  Enter         Toggle container, or view a leaf value",
    "  e             Expand everything under the cursor",
    "  o / O         Open all / Close all",
    "  1-9           Expand to depth",
    "Search",
    "  /text         Search keys and values in the whole document",
    "  /$.a.b[0]     Jump to a path (supports [*] and [-1])",
    "  n / N         Next / previous match     Esc  Clear highlight",
    "Values",
    "  v             Show full value (embedded JSON is pretty printed)",
    "  y / Y         Copy value as JSON / copy path (usable with jv -p)",
    "  t             Toggle type hints",
    "",
    "Press any key to close help, q to quit.",
];

/// Replaces the last visible help line when the help does not fit.
const HELP_MORE: &str = "… (enlarge the window to see all keys)";

const SEARCH_PROMPT: &str = "Search: ";

/// The fewest cells left for the title before the key hints are dropped
/// from a narrow header.
const MIN_HEADER_TITLE: usize = 8;

/// `left` and `right` on one line of `max` cells. When they do not fit,
/// `left` is shortened first so the key hints stay visible; on very narrow
/// screens only `left` is shown.
pub(super) fn fit_header(left: &str, right: &str, max: usize) -> String {
    if max == 0 || width(left) + 1 + width(right) <= max {
        return join_with_gap(left, right, max);
    }
    let budget = max.saturating_sub(width(right) + 1);
    if budget >= MIN_HEADER_TITLE {
        return join_with_gap(&truncate_str(left, budget, "…"), right, max);
    }
    truncate_str(left, max, "…")
}

/// The position info and the status message on one line of `max` cells.
/// The status is feedback for the last key press, so the info is shortened
/// first to keep it readable.
pub(super) fn fit_footer(info: &str, status: &str, max: usize) -> String {
    if status.is_empty() {
        return if max > 0 {
            truncate_str(info, max, "…")
        } else {
            info.to_string()
        };
    }
    let full = format!("{info}  {status}");
    if max == 0 || width(&full) <= max {
        return full;
    }
    match max.checked_sub(width(status) + 2) {
        Some(avail) if avail >= 2 => format!("{}  {status}", truncate_str(info, avail, "…")),
        _ => truncate_str(status, max, "…"),
    }
}

/// Width of the search field that fits the footer next to the prompt
/// (`Search: > ` and the cursor cell).
pub(super) fn search_input_width(max: usize) -> usize {
    if max == 0 {
        return 30;
    }
    max.saturating_sub(width(SEARCH_PROMPT) + 2 + 1).max(1)
}

fn truncate_str(s: &str, max: usize, tail: &str) -> String {
    line_text(&truncate(Line::raw(s.to_string()), max, tail))
}

impl Model {
    /// Every line of the screen: header, body (or help/detail) and footer.
    pub fn view_lines(&self) -> Vec<Line<'static>> {
        let mut lines = vec![self.render_header()];
        if self.help_mode {
            lines.extend(self.render_help());
        } else if self.detail_mode {
            lines.extend(self.render_detail());
        } else {
            lines.extend(self.render_body());
        }
        lines.push(self.render_footer());
        lines
    }

    /// The screen as plain text (no styles), one line per row.
    pub fn view(&self) -> String {
        self.view_lines()
            .iter()
            .map(line_text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Draws the screen into a ratatui frame.
    pub fn render(&self, frame: &mut Frame<'_>) {
        let area = frame.area();
        let mut lines = self.view_lines();
        // Like bubbletea, keep the bottom of a view taller than the screen.
        let h = area.height as usize;
        if lines.len() > h {
            lines.drain(..lines.len() - h);
        }
        let buf = frame.buffer_mut();
        for (y, line) in lines.iter().enumerate() {
            buf.set_line(area.x, area.y + y as u16, line, area.width);
        }
    }

    /// Only the rows inside the window are rendered.
    fn render_body(&self) -> Vec<Line<'static>> {
        let height = if self.body_height == 0 {
            self.rows.len()
        } else {
            self.body_height
        };
        let mut out: Vec<Line<'static>> = self
            .rows
            .iter()
            .skip(self.y_offset)
            .take(height)
            .map(|&r| {
                let line = self.render_row(r);
                if self.width > 0 {
                    truncate(line, self.width, "")
                } else {
                    line
                }
            })
            .collect();
        out.resize(height, Line::default());
        out
    }

    fn render_header(&self) -> Line<'static> {
        let (left, right) = if self.detail_mode {
            (
                format!("jv - Value: {}", display_safe(&self.current_path())),
                "[y] Copy  [Esc] Back",
            )
        } else {
            (
                "jv - JSON Viewer".to_string(),
                "[/] Search  [?] Help  [q] Quit",
            )
        };
        Line::from(styled(
            fit_header(&left, right, self.width),
            self.styles.header,
        ))
    }

    fn render_footer(&self) -> Line<'static> {
        let style = self.styles.footer;
        if self.search_mode {
            let mut spans = vec![styled(SEARCH_PROMPT, style)];
            spans.extend(self.search.view(style));
            return Line::from(spans);
        }
        let node = self.current_node();
        let lines = if self.detail_mode {
            let total = self.detail_lines.len();
            let bottom = (self.detail_offset + self.body_height).min(total);
            format!(
                "Lines: {}-{bottom}/{total}",
                (self.detail_offset + 1).min(total)
            )
        } else {
            format!("Lines: {}/{}", self.cursor + 1, self.flat_nodes.len())
        };
        let info = format!(
            "Path: {}  {lines}  Depth: {}",
            self.doc.path(node),
            self.doc.depth(node)
        );
        let text = fit_footer(
            &display_safe(&info),
            &display_safe(&self.status_msg),
            self.width,
        );
        Line::from(styled(text, style))
    }

    /// The key reference, wrapped to the width and cut to the body height so
    /// the header and footer stay on screen.
    fn render_help(&self) -> Vec<Line<'static>> {
        let mut lines = wrap(&HELP.join("\n"), self.width);
        if self.body_height > 0 {
            if lines.len() > self.body_height {
                lines.truncate(self.body_height - 1);
                lines.push(HELP_MORE.to_string());
            }
            lines.resize(self.body_height, String::new());
        }
        lines
            .into_iter()
            .map(|l| {
                let line = Line::from(styled(l.trim_end().to_string(), self.styles.help));
                if self.width > 0 {
                    truncate(line, self.width, "")
                } else {
                    line
                }
            })
            .collect()
    }

    fn render_detail(&self) -> Vec<Line<'static>> {
        let height = self.body_height.max(1);
        let mut out: Vec<Line<'static>> = self
            .detail_lines
            .iter()
            .skip(self.detail_offset)
            .take(height)
            .map(|l| {
                let line = Line::raw(l.clone());
                if self.width > 0 {
                    truncate(line, self.width, "")
                } else {
                    line
                }
            })
            .collect();
        out.resize(height, Line::default());
        out
    }

    fn render_row(&self, r: Row) -> Line<'static> {
        let doc = &self.doc;
        let s = &self.styles;
        let node = r.node;
        let mut spans: Vec<Span<'static>> = vec![Span::raw("  ".repeat(doc.depth(node)))];
        let comma = if self.is_last(node) { "" } else { "," };
        let t = doc.node_type(node);
        if r.closing {
            spans.push(Span::raw(format!("{}{comma}", close_bracket(t))));
            return Line::from(spans);
        }

        let (hl_key, hl_value) = self.highlight(node);
        if is_object_member(doc, node) {
            let key_style = if hl_key { s.matched } else { s.key };
            spans.push(styled(
                display_safe(&quote_string(&doc.key_text(node))).into_owned(),
                key_style,
            ));
            spans.push(Span::raw(": "));
        }
        let children = doc.children(node);
        if !t.is_container() {
            let value = if t == NodeType::Null {
                "null".to_string()
            } else {
                display_safe(&doc.string_value(node)).into_owned()
            };
            let style = if hl_value {
                s.matched
            } else {
                match t {
                    NodeType::String => s.string,
                    NodeType::Number => s.number,
                    NodeType::Boolean => s.boolean,
                    _ => s.null,
                }
            };
            spans.push(styled(value, style));
            spans.push(Span::raw(comma));
        } else {
            let open_style = if hl_value {
                s.matched
            } else {
                Default::default()
            };
            spans.push(styled(open_bracket(t), open_style));
            if !(self.expanded[node.index()] && !children.is_empty()) {
                if !children.is_empty() {
                    spans.push(styled(
                        format!("… {}", container_summary(doc, node)),
                        s.type_hint,
                    ));
                }
                spans.push(Span::raw(format!("{}{comma}", close_bracket(t))));
            }
        }
        if self.show_types {
            spans.push(Span::raw(" "));
            spans.push(styled(format!("({t})"), s.type_hint));
        }
        let line = Line::from(spans);
        if node == self.current_node() {
            line.patch_style(s.selected)
        } else {
            line
        }
    }

    fn is_last(&self, node: NodeId) -> bool {
        match self.doc.parent(node) {
            None => true,
            Some(p) => self.doc.children(p).last() == Some(&node),
        }
    }

    /// Whether the key and/or value of a matched node is highlighted: path
    /// matches highlight the key (or the value when the node has no key);
    /// text matches highlight whichever part contains the search term.
    fn highlight(&self, node: NodeId) -> (bool, bool) {
        if !self.match_set.contains(&node) {
            return (false, false);
        }
        let doc = &self.doc;
        let has_key = is_object_member(doc, node);
        if self.match_is_path {
            return (has_key, !has_key);
        }
        let term = lower(&self.match_label);
        let key = has_key && lower(&doc.key_text(node)).contains(term.as_ref());
        let value =
            doc.children(node).is_empty() && lower(&doc.raw_string(node)).contains(term.as_ref());
        (key, value)
    }
}

fn open_bracket(t: NodeType) -> &'static str {
    if t == NodeType::Array { "[" } else { "{" }
}

fn close_bracket(t: NodeType) -> &'static str {
    if t == NodeType::Array { "]" } else { "}" }
}
