//! Hive surfaces shared by chat, settings and collaboration views.
//! Terminal ANSI colors belong to the terminal emulator.
use gpui_kit::{App, Rgba, component::Theme, rgb};

pub enum Color {
    Surface,
    Sidebar,
    Card,
    Divider,
    Text,
    Muted,
    Secondary,
    Inset,
    Attention,
    Error,
    ErrorSurface,
}

impl Color {
    pub fn get(self, cx: &App) -> Rgba {
        let (light, dark) = match self {
            Self::Surface => (0xf8f8f6, 0x1f1f1e),
            Self::Sidebar => (0xf8f8f6, 0x1f1f1e),
            Self::Card => (0xffffff, 0x2c2c2a),
            Self::Divider => (0xd7d7d4, 0x3c3c3a),
            Self::Text => (0x262626, 0xd4d4d4),
            Self::Muted => (0x656562, 0xa6a6a2),
            Self::Secondary => (0x535550, 0xb7bab4),
            Self::Inset => (0xf0f0ed, 0x292929),
            Self::Attention => (0xf1eee4, 0x332f25),
            Self::Error => (0xab382b, 0xffa59a),
            Self::ErrorSurface => (0xffe9e4, 0x442a29),
        };
        rgb(if Theme::global(cx).is_dark() {
            dark
        } else {
            light
        })
    }
}
