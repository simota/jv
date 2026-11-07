//! Interactive collapsible viewer.
//!
//! [`Model`] holds all state and is driven by [`Msg`]s, so it can be tested
//! without a terminal; [`run`] connects it to the terminal with ratatui and
//! crossterm.

mod clipboard;
mod input;
mod model;
mod styles;
mod text;
mod view;

use std::io;

use ratatui::crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEvent, KeyEventKind,
    KeyModifiers,
};
use ratatui::crossterm::execute;

pub use model::{Command, Model};

use crate::json::{Document, NodeId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

/// Viewer settings.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Initial expand depth (0 expands only the root).
    pub depth: usize,
    pub theme: Theme,
    pub color: bool,
    pub show_types: bool,
    /// Nodes to jump to on start (e.g. results of `--path`). They become the
    /// match list for `n`/`N`.
    pub focus: Vec<NodeId>,
    pub focus_label: String,
}

/// A key press, independent of the terminal library.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Ctrl(char),
    Alt(char),
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Enter,
    Esc,
    Backspace,
    Delete,
    Tab,
    AltBackspace,
    AltDelete,
    WordLeft,
    WordRight,
    Other,
}

/// Input events for the [`Model`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    Resize { width: u16, height: u16 },
    Key(Key),
    Paste(String),
}

/// Runs the viewer until the user quits.
pub fn run(doc: Document, root: NodeId, opts: Options) -> io::Result<()> {
    let mut model = Model::new(doc, root, opts);
    let mut terminal = ratatui::try_init()?;
    let result = (|| {
        execute!(io::stdout(), EnableBracketedPaste)?;
        let size = terminal.size()?;
        model.update(Msg::Resize {
            width: size.width,
            height: size.height,
        });
        loop {
            terminal.draw(|frame| model.render(frame))?;
            let msg = match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => Msg::Key(map_key(k)),
                Event::Resize(width, height) => Msg::Resize { width, height },
                Event::Paste(s) => Msg::Paste(s),
                _ => continue,
            };
            if model.update(msg) == Command::Quit {
                return Ok(());
            }
        }
    })();
    let _ = execute!(io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}

fn map_key(k: KeyEvent) -> Key {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    match k.code {
        KeyCode::Char(c) if ctrl => Key::Ctrl(c.to_ascii_lowercase()),
        KeyCode::Char(c) if alt => Key::Alt(c),
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::Backspace if alt => Key::AltBackspace,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete if alt => Key::AltDelete,
        KeyCode::Delete => Key::Delete,
        KeyCode::Left if ctrl || alt => Key::WordLeft,
        KeyCode::Right if ctrl || alt => Key::WordRight,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Esc,
        KeyCode::Tab => Key::Tab,
        _ => Key::Other,
    }
}

#[cfg(test)]
mod tests;
