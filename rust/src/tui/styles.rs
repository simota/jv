use ratatui::style::{Color, Modifier, Style};

use super::Theme;

/// Styles of the viewer. Without color only bold/underline are used.
#[derive(Clone, Copy, Debug, Default)]
pub struct Styles {
    pub key: Style,
    pub string: Style,
    pub number: Style,
    pub boolean: Style,
    pub null: Style,
    pub type_hint: Style,
    pub selected: Style,
    pub header: Style,
    pub footer: Style,
    pub help: Style,
    pub matched: Style,
}

impl Styles {
    pub fn new(theme: Theme, color: bool) -> Styles {
        let bold = Style::new().add_modifier(Modifier::BOLD);
        if !color {
            return Styles {
                selected: bold,
                header: bold,
                matched: Style::new().add_modifier(Modifier::UNDERLINED),
                ..Styles::default()
            };
        }
        let fg = |c: u8| Style::new().fg(Color::Indexed(c));
        let (selected_bg, matched) = match theme {
            Theme::Dark => (4, 11),
            Theme::Light => (12, 1),
        };
        Styles {
            key: fg(6),
            string: fg(2),
            number: fg(3),
            boolean: fg(5),
            null: fg(8),
            type_hint: fg(8),
            selected: Style::new()
                .bg(Color::Indexed(selected_bg))
                .fg(Color::Indexed(0)),
            header: bold.fg(Color::Indexed(8)),
            footer: fg(8),
            help: fg(8),
            matched: fg(matched).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        }
    }
}
