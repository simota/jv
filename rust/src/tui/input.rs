//! A single-line text input with the key bindings of the bubbles
//! `textinput` used by the Go version.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use super::Key;

#[derive(Clone, Debug)]
pub struct TextInput {
    value: Vec<char>,
    cursor: usize,
    /// First visible character when the value is wider than `width`.
    offset: usize,
    pub placeholder: String,
    pub char_limit: usize,
    pub width: usize,
    focused: bool,
}

impl TextInput {
    pub fn new(placeholder: &str, char_limit: usize, width: usize) -> TextInput {
        TextInput {
            value: Vec::new(),
            cursor: 0,
            offset: 0,
            placeholder: placeholder.to_string(),
            char_limit,
            width,
            focused: false,
        }
    }

    pub fn value(&self) -> String {
        self.value.iter().collect()
    }

    pub fn set_value(&mut self, s: &str) {
        self.value = s.chars().take(self.char_limit).collect();
        self.cursor = self.value.len();
        self.fix_offset();
    }

    pub fn focus(&mut self) {
        self.focused = true;
    }

    pub fn blur(&mut self) {
        self.focused = false;
    }

    pub fn insert(&mut self, s: &str) {
        for c in s.chars().filter(|c| !c.is_control()) {
            if self.value.len() >= self.char_limit {
                break;
            }
            self.value.insert(self.cursor, c);
            self.cursor += 1;
        }
        self.fix_offset();
    }

    pub fn handle(&mut self, key: Key) {
        match key {
            Key::Backspace | Key::Ctrl('h') => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                }
            }
            Key::Delete | Key::Ctrl('d') => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
            }
            Key::Left | Key::Ctrl('b') => self.cursor = self.cursor.saturating_sub(1),
            Key::Right | Key::Ctrl('f') => self.cursor = (self.cursor + 1).min(self.value.len()),
            Key::Home | Key::Ctrl('a') => self.cursor = 0,
            Key::End | Key::Ctrl('e') => self.cursor = self.value.len(),
            Key::Ctrl('k') => self.value.truncate(self.cursor),
            Key::Ctrl('u') => {
                self.value.drain(..self.cursor);
                self.cursor = 0;
            }
            Key::Ctrl('w') | Key::AltBackspace => {
                let start = self.word_start();
                self.value.drain(start..self.cursor);
                self.cursor = start;
            }
            Key::AltDelete | Key::Alt('d') => {
                let end = self.word_end();
                self.value.drain(self.cursor..end);
            }
            Key::WordLeft | Key::Alt('b') => self.cursor = self.word_start(),
            Key::WordRight | Key::Alt('f') => self.cursor = self.word_end(),
            // like bubbles, other alt+key combinations insert the character
            Key::Char(c) | Key::Alt(c) => self.insert(&c.to_string()),
            _ => {}
        }
        self.fix_offset();
    }

    fn word_start(&self) -> usize {
        let mut i = self.cursor;
        while i > 0 && self.value[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !self.value[i - 1].is_whitespace() {
            i -= 1;
        }
        i
    }

    fn word_end(&self) -> usize {
        let mut i = self.cursor;
        while i < self.value.len() && self.value[i].is_whitespace() {
            i += 1;
        }
        while i < self.value.len() && !self.value[i].is_whitespace() {
            i += 1;
        }
        i
    }

    fn fix_offset(&mut self) {
        if self.width == 0 {
            self.offset = 0;
            return;
        }
        if self.cursor < self.offset {
            self.offset = self.cursor;
        } else if self.cursor > self.offset + self.width {
            self.offset = self.cursor - self.width;
        }
    }

    /// `> ` prompt, the visible part of the value and a block cursor.
    pub fn view(&self, base: Style) -> Vec<Span<'static>> {
        let cursor_style = base.add_modifier(Modifier::REVERSED);
        let mut spans = vec![Span::styled("> ", base)];
        if self.value.is_empty() {
            // Like the value, the placeholder takes `width` cells plus the
            // cursor cell.
            let shown = if self.width == 0 {
                usize::MAX
            } else {
                self.width + 1
            };
            let mut chars = self.placeholder.chars().take(shown);
            let first = chars.next().unwrap_or(' ');
            let dim = base.add_modifier(Modifier::DIM);
            if self.focused {
                spans.push(Span::styled(
                    first.to_string(),
                    cursor_style.add_modifier(Modifier::DIM),
                ));
            } else {
                spans.push(Span::styled(first.to_string(), dim));
            }
            spans.push(Span::styled(chars.collect::<String>(), dim));
            return spans;
        }
        let end = if self.width == 0 {
            self.value.len()
        } else {
            (self.offset + self.width).min(self.value.len())
        };
        let before: String = self.value[self.offset..self.cursor.min(end)]
            .iter()
            .collect();
        spans.push(Span::styled(before, base));
        if self.cursor < self.value.len() {
            let at = self.value[self.cursor].to_string();
            let after: String = self.value[(self.cursor + 1).min(end)..end].iter().collect();
            spans.push(Span::styled(
                at,
                if self.focused { cursor_style } else { base },
            ));
            spans.push(Span::styled(after, base));
        } else if self.focused {
            spans.push(Span::styled(" ", cursor_style));
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(keys: &[Key]) -> TextInput {
        let mut t = TextInput::new("ph", 256, 30);
        for &k in keys {
            t.handle(k);
        }
        t
    }

    #[test]
    fn editing_keys() {
        let k = |s: &str| s.chars().map(Key::Char).collect::<Vec<_>>();
        let mut keys = k("foo bar");
        keys.extend([Key::Ctrl('w')]);
        assert_eq!(typed(&keys).value(), "foo ");
        keys.extend([Key::Home, Key::Char('x'), Key::End, Key::Backspace]);
        assert_eq!(typed(&keys).value(), "xfoo");
        keys.extend([Key::Left, Key::Left, Key::Ctrl('k')]);
        assert_eq!(typed(&keys).value(), "xf");
        keys.extend([Key::Ctrl('u')]);
        assert_eq!(typed(&keys).value(), "");
    }

    #[test]
    fn char_limit() {
        let mut t = TextInput::new("", 3, 30);
        t.insert("abcdef");
        assert_eq!(t.value(), "abc");
    }
}
