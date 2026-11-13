//! Hive surfaces shared by chat, onboarding, settings, and workspace views.
//! Tuned to T3 Code's cool-zinc light & obsidian-graphite dark hierarchy,
//! paired with Beluga's crisp local-readiness surfaces.
use gpui_kit::{
    App, Div, FontWeight, ParentElement as _, Rgba, Styled as _,
    component::{Theme, button::ButtonCustomVariant},
    div, px, rgb,
};
use std::time::{SystemTime, UNIX_EPOCH};

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
            Self::Surface => (0xffffff, 0x141417),
            Self::Sidebar => (0xf7f7f8, 0x0e0e11),
            Self::Card => (0xffffff, 0x1b1b20),
            Self::Bubble => (0xebebee, 0x222229),
            Self::Divider => (0xe8e9ed, 0x25252d),
            Self::Text => (0x18181b, 0xededf0),
            Self::Secondary => (0x52525b, 0xa1a1aa),
            Self::Muted => (0x76767f, 0x7c7c86),
            Self::Inset => (0xf1f2f5, 0x1e1e25),
            Self::Accent => (0x3b63f6, 0x6884fa),
            Self::AccentSoft => (0xeef2ff, 0x1c233d),
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

/// T3 Code–style muted chrome button variant: calm grey icon/label at rest,
/// crisp foreground & soft inset surface on hover.
pub fn quiet_icon_variant(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::rgba(0).into())
        .foreground(Color::Muted.get(cx).into())
        .hover(Color::Inset.get(cx).into())
        .active(Color::Divider.get(cx).into())
}

/// Ultra-quiet icon variant for secondary row actions (`...`, copy, quote)
/// so dense lists and chat streams remain visually serene.
pub fn subtle_icon_variant(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(gpui_kit::rgba(0).into())
        .foreground(Color::Muted.get(cx).opacity(0.62).into())
        .hover(Color::Inset.get(cx).into())
        .active(Color::Divider.get(cx).into())
}

/// T3 Code–style sidebar navigation item variant: avoids gpui-component's heavy
/// `Ghost` selected wash while keeping calm cool-zinc / obsidian contrast.
pub fn sidebar_item_variant(selected: bool, cx: &App) -> ButtonCustomVariant {
    let dark = Theme::global(cx).is_dark();
    if selected {
        let bg = rgb(if dark { 0x202028 } else { 0xe8e9ee });
        let hover = rgb(if dark { 0x24242d } else { 0xe3e4ea });
        ButtonCustomVariant::new(cx)
            .color(bg.into())
            .foreground(Color::Text.get(cx).into())
            .hover(hover.into())
            .active(bg.into())
    } else {
        let hover = rgb(if dark { 0x18181f } else { 0xeeeef2 });
        let active = rgb(if dark { 0x202028 } else { 0xe8e9ee });
        ButtonCustomVariant::new(cx)
            .color(gpui_kit::rgba(0).into())
            .foreground(Color::Secondary.get(cx).into())
            .hover(hover.into())
            .active(active.into())
    }
}

/// Crisp idle/disabled fill color for the circular send button, applied via
/// Button's instance style so `disabled` doesn't wash it out to 15% alpha.
pub fn send_circle_disabled_bg(cx: &App) -> Rgba {
    rgb(if Theme::global(cx).is_dark() {
        0x282832
    } else {
        0xe5e6eb
    })
}

/// Crisp idle/disabled icon color for the circular send button.
pub fn send_circle_disabled_fg(cx: &App) -> Rgba {
    rgb(if Theme::global(cx).is_dark() {
        0x8e8e98
    } else {
        0x71717a
    })
}

/// T3 Code circular send button: neutral zinc circle when idle/disabled,
/// high-contrast obsidian/alabaster ink circle when ready to send.
pub fn send_circle_variant(enabled: bool, cx: &App) -> ButtonCustomVariant {
    let dark = Theme::global(cx).is_dark();
    if enabled {
        if dark {
            ButtonCustomVariant::new(cx)
                .color(rgb(0xededf0).into())
                .foreground(rgb(0x141417).into())
                .hover(rgb(0xffffff).into())
                .active(rgb(0xd4d4d8).into())
        } else {
            ButtonCustomVariant::new(cx)
                .color(rgb(0x18181b).into())
                .foreground(rgb(0xffffff).into())
                .hover(rgb(0x2c2c31).into())
                .active(rgb(0x3f3f46).into())
        }
    } else if dark {
        ButtonCustomVariant::new(cx)
            .color(rgb(0x3b3b46).into())
            .foreground(rgb(0xe4e4ec).into())
            .hover(rgb(0x3b3b46).into())
            .active(rgb(0x3b3b46).into())
    } else {
        ButtonCustomVariant::new(cx)
            .color(rgb(0xcfcfd6).into())
            .foreground(rgb(0x18181b).into())
            .hover(rgb(0xcfcfd6).into())
            .active(rgb(0xcfcfd6).into())
    }
}

/// Computes a deterministic 1–2 character monogram like T3 Code (`archify` -> `AR`,
/// `hive-desktop` -> `HD`, `CodeAnalysis` -> `CA`).
pub fn project_monogram(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "PR".to_owned();
    }
    let parts: Vec<&str> = trimmed
        .split(|c: char| matches!(c, '-' | '_' | ' ' | '.' | '/'))
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() >= 2 {
        let mut out = String::new();
        for part in parts.into_iter().take(2) {
            if let Some(ch) = part.chars().next() {
                for up in ch.to_uppercase() {
                    out.push(up);
                }
            }
        }
        if !out.is_empty() {
            return out.chars().take(2).collect();
        }
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if let Some(&first) = chars.first() {
        if !first.is_ascii() {
            return first.to_string();
        }
        // Check camelCase second capital letter (e.g. CodeAnalysis -> CA)
        if let Some(&camel) = chars.iter().skip(1).find(|c| c.is_ascii_uppercase()) {
            return format!("{}{}", first.to_ascii_uppercase(), camel);
        }
        if let Some(&second) = chars.get(1).filter(|c| c.is_ascii_alphanumeric()) {
            return format!(
                "{}{}",
                first.to_ascii_uppercase(),
                second.to_ascii_uppercase()
            );
        }
        return first.to_ascii_uppercase().to_string();
    }
    "PR".to_owned()
}

/// Renders a T3 Code–inspired rounded monogram avatar badge for a workspace.
pub fn project_badge(name: &str, size_px: f32, cx: &App) -> Div {
    let hash = name.bytes().fold(0u32, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    });
    let palette_idx = (hash % 6) as usize;
    let ((light_bg, light_fg), (dark_bg, dark_fg)) = match palette_idx {
        0 => ((0xfef3c7, 0xb45309), (0x332611, 0xfbbf24)), // Warm Amber
        1 => ((0xe0e7ff, 0x4338ca), (0x1e223d, 0x818cf8)), // Soft Indigo
        2 => ((0xd1fae5, 0x047857), (0x122d24, 0x34d399)), // Mint Emerald
        3 => ((0xe0f2fe, 0x0369a1), (0x132938, 0x38bdf8)), // Sky Blue
        4 => ((0xf3e8ff, 0x6d28d9), (0x281c3a, 0xc084fc)), // Lavender Violet
        _ => ((0xffe4e6, 0xbe123c), (0x331820, 0xfb7185)), // Coral Rose
    };
    let (bg_hex, fg_hex) = if Theme::global(cx).is_dark() {
        (dark_bg, dark_fg)
    } else {
        (light_bg, light_fg)
    };
    let text = project_monogram(name);
    let font_px = if size_px <= 16.5 { 9.0 } else { 9.5 };
    div()
        .size(px(size_px))
        .flex_shrink_0()
        .rounded(px(5.))
        .bg(rgb(bg_hex))
        .text_color(rgb(fg_hex))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(font_px))
        .font_weight(FontWeight::BOLD)
        .child(text)
}

/// Formats millisecond epoch timestamps into compact T3 Code thread timestamps (`now`/`刚刚`, `5m`, `2h`, `4d`).
pub fn compact_relative_time(updated_at_ms: i64, english: bool) -> Option<String> {
    if updated_at_ms <= 0 {
        return None;
    }
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)?;
    let diff_secs = ((now_ms - updated_at_ms).max(0)) / 1000;
    Some(if diff_secs < 60 {
        if english { "now".into() } else { "刚刚".into() }
    } else if diff_secs < 3600 {
        format!("{}m", diff_secs / 60)
    } else if diff_secs < 86_400 {
        format!("{}h", diff_secs / 3600)
    } else if diff_secs < 86_400 * 30 {
        format!("{}d", diff_secs / 86_400)
    } else if diff_secs < 86_400 * 365 {
        format!("{}mo", diff_secs / (86_400 * 30))
    } else {
        format!("{}y", diff_secs / (86_400 * 365))
    })
}

/// Formats filesystem paths cleanly for UI display (`~/code/proj` or `…/parent/proj` for deep paths).
pub fn display_path(raw: &str) -> String {
    if let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .and_then(|s| s.into_string().ok())
        .filter(|s| !s.is_empty())
    {
        if raw == home {
            return "~".to_owned();
        }
        if let Some(rest) = raw.strip_prefix(&home) {
            if rest.starts_with('/') || rest.starts_with('\\') {
                return format!("~{rest}");
            }
        }
    }
    let parts: Vec<&str> = raw
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() > 3 {
        let tail = &parts[parts.len() - 2..];
        return format!("…/{}", tail.join("/"));
    }
    raw.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monogram_extracts_clean_two_letter_codes() {
        assert_eq!(project_monogram("hive-desktop"), "HD");
        assert_eq!(project_monogram("archify"), "AR");
        assert_eq!(project_monogram("CodeAnalysis"), "CA");
        assert_eq!(project_monogram("桌面助手"), "桌");
    }
}
