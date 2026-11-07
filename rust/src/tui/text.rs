//! Width-aware text helpers (terminal cells, not bytes or chars).

use std::borrow::Cow;
use std::fmt::Write as _;

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub fn width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub fn line_width(line: &Line<'_>) -> usize {
    line.spans.iter().map(|s| width(&s.content)).sum()
}

/// Plain text of a line (for tests and width calculations).
pub fn line_text(line: &Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

/// Cuts `line` to at most `max` cells; when cut, `tail` (e.g. `…`) is
/// appended within the limit.
pub fn truncate(line: Line<'static>, max: usize, tail: &str) -> Line<'static> {
    if line_width(&line) <= max {
        return line;
    }
    let tail_w = width(tail);
    let budget = max.saturating_sub(tail_w);
    let mut used = 0;
    let mut spans: Vec<Span<'static>> = Vec::new();
    for span in line.spans {
        if used >= budget {
            break;
        }
        let w = width(&span.content);
        if used + w <= budget {
            used += w;
            spans.push(span);
            continue;
        }
        let mut cut = String::new();
        for c in span.content.chars() {
            let cw = c.width().unwrap_or(0);
            if used + cw > budget {
                break;
            }
            used += cw;
            cut.push(c);
        }
        spans.push(Span::styled(cut, span.style));
        break;
    }
    if !tail.is_empty() && tail_w <= max {
        spans.push(Span::raw(tail.to_string()));
    }
    Line::from(spans).style(line.style)
}

/// Word-wraps `text` to `max` cells per line: breaks at spaces where
/// possible, hard-breaks words that are too long, and keeps leading
/// indentation.
pub fn wrap(text: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        if max == 0 || width(line) <= max {
            out.push(line.to_string());
            continue;
        }
        let mut cur = String::new();
        let mut cur_w = 0;
        for token in tokens(line) {
            let tw = width(token);
            let is_space = token.starts_with(' ');
            if cur_w + tw <= max {
                cur.push_str(token);
                cur_w += tw;
                continue;
            }
            if is_space {
                // break here and drop the spaces
                out.push(std::mem::take(&mut cur));
                cur_w = 0;
                continue;
            }
            if cur_w > 0 && !cur.trim().is_empty() {
                out.push(cur.trim_end().to_string());
                cur.clear();
                cur_w = 0;
            }
            for c in token.chars() {
                let cw = c.width().unwrap_or(0);
                if cur_w + cw > max && cur_w > 0 {
                    out.push(std::mem::take(&mut cur));
                    cur_w = 0;
                }
                cur.push(c);
                cur_w += cw;
            }
        }
        out.push(cur);
    }
    out
}

/// Splits into alternating runs of spaces and non-spaces.
fn tokens(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut prev_space: Option<bool> = None;
    for (i, c) in line.char_indices() {
        let space = c == ' ';
        if prev_space.is_some_and(|p| p != space) {
            out.push(&line[start..i]);
            start = i;
        }
        prev_space = Some(space);
    }
    if start < line.len() {
        out.push(&line[start..]);
    }
    out
}

/// `left`, padding, `right` across `max` cells (at least one space).
pub fn join_with_gap(left: &str, right: &str, max: usize) -> String {
    if max == 0 {
        return format!("{left}  {right}");
    }
    let gap = max.saturating_sub(width(left) + width(right)).max(1);
    format!("{left}{}{right}", " ".repeat(gap))
}

pub fn styled(s: impl Into<String>, style: Style) -> Span<'static> {
    Span::styled(s.into(), style)
}

/// Makes document text safe and visible on the terminal. Control characters
/// other than newline and tab (C0, DEL and C1, e.g. ESC, BEL, CR or U+009B
/// CSI) are shown as escapes such as `\u001b` (`\r` for CR), the same as the
/// Go viewer. Quoted JSON strings stay valid: the escapes are the JSON ones.
pub fn display_safe(s: &str) -> Cow<'_, str> {
    if !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t') {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '\n' | '\t' => out.push(c),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_counts_cells() {
        let l = Line::from(vec![Span::raw("ab"), Span::raw("日本語")]);
        assert_eq!(line_text(&truncate(l.clone(), 5, "")), "ab日");
        assert_eq!(line_text(&truncate(l.clone(), 6, "…")), "ab日…");
        assert_eq!(line_text(&truncate(l, 8, "…")), "ab日本語");
    }

    #[test]
    fn wrap_words_and_long_tokens() {
        assert_eq!(wrap("hello world foo", 11), ["hello world", "foo"]);
        assert_eq!(wrap("aaaaaaaaaa", 4), ["aaaa", "aaaa", "aa"]);
        assert_eq!(
            wrap("    \"k\": \"some text\"", 12),
            ["    \"k\":", "\"some text\""]
        );
        assert_eq!(wrap("a\nb", 4), ["a", "b"]);
    }

    #[test]
    fn display_safe_escapes_controls() {
        for (input, want) in [
            ("plain", "plain"),
            ("日本語 ok", "日本語 ok"),
            ("tab\tnl\n", "tab\tnl\n"),
            ("\u{1b}[31m", r"\u001b[31m"),
            ("a\rb", r"a\rb"),
            ("\0\u{7}\u{7f}", r"\u0000\u0007\u007f"),
            ("\u{85}\u{9f}\u{a0}", "\\u0085\\u009f\u{a0}"),
        ] {
            assert_eq!(display_safe(input), want, "{input:?}");
        }
        assert!(matches!(display_safe("plain"), Cow::Borrowed(_)));
    }
}
