//! Hive surfaces shared by chat, onboarding, settings, and workspace views.
//! Tuned to a crisp cool-neutral zinc (light) and obsidian-graphite (dark) hierarchy
//! inspired by T3 Code and Beluga. Terminal ANSI colors belong to the terminal emulator.
use gpui_kit::{App, Rgba, component::Theme, rgb};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Surface,
    Sidebar,
    Card,
    Bubble,
    Divider,
    Text,
    Muted,
    Secondary,
    Inset,
    Accent,
    AccentSoft,
    Attention,
    Success,
    Error,
    ErrorSurface,
}

impl Color {
    pub fn get(self, cx: &App) -> Rgba {
        let (light, dark) = match self {
            Self::Surface => (0xffffff, 0x16161a),
            Self::Sidebar => (0xf6f6f8, 0x111114),
            Self::Card => (0xffffff, 0x1d1d23),
            Self::Bubble => (0xf0f1f5, 0x26262f),
            Self::Divider => (0xe5e6eb, 0x282831),
            Self::Text => (0x18181b, 0xededf0),
            Self::Secondary => (0x52525b, 0xa1a1aa),
            Self::Muted => (0x71717a, 0x7c7c86),
            Self::Inset => (0xf2f3f6, 0x212128),
            Self::Accent => (0x3b63f6, 0x5b7cfa),
            Self::AccentSoft => (0xeef2ff, 0x1e2540),
            Self::Attention => (0xfef7e6, 0x2c2416),
            Self::Success => (0x16a34a, 0x4ade80),
            Self::Error => (0xdc2626, 0xf87171),
            Self::ErrorSurface => (0xfef2f2, 0x2d191c),
        };
        rgb(if Theme::global(cx).is_dark() {
            dark
        } else {
            light
        })
    }
}
