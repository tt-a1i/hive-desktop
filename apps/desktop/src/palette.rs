//! Hive surfaces shared by chat, onboarding, settings, and workspace views.
//! Tuned to T3 Code's cool-zinc light & obsidian-graphite dark hierarchy,
//! paired with Beluga's crisp local-readiness surfaces.
use gpui_kit::{
    App, Div, FontWeight, ParentElement as _, Rgba, Styled as _,
    component::{Theme, button::ButtonCustomVariant},
    div, px, rgb, rgba,
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
        let dark = Theme::global(cx).is_dark();
        match self {
            Self::Surface => rgb(if dark { 0x18181b } else { 0xfcfcfc }),
            Self::Sidebar => rgb(if dark { 0x131316 } else { 0xfafafa }),
            Self::Card => rgb(if dark { 0x1f1f23 } else { 0xffffff }),
            Self::Bubble => rgb(if dark { 0x27272c } else { 0xebebed }),
            Self::Divider => rgba(if dark { 0xffffff14 } else { 0x00000012 }),
            Self::Text => rgb(if dark { 0xf4f4f5 } else { 0x18181b }),
            Self::Secondary => rgb(if dark { 0xa1a1aa } else { 0x52525b }),
            Self::Muted => rgb(if dark { 0x8e8e98 } else { 0x71717b }),
            Self::Inset => rgb(if dark { 0x222227 } else { 0xf4f4f5 }),
            Self::Accent => rgb(if dark { 0x4d7ef5 } else { 0x1b4ed8 }),
            Self::AccentSoft => rgb(if dark { 0x1b2440 } else { 0xeef2ff }),
            Self::Attention => rgb(if dark { 0x2a2316 } else { 0xfef7e6 }),
            Self::Success => rgb(if dark { 0x00d492 } else { 0x009966 }),
            Self::Error => rgb(if dark { 0xf87171 } else { 0xdc2626 }),
            Self::ErrorSurface => rgb(if dark { 0x2d191c } else { 0xfef2f2 }),
        }
    }
}

/// T3 Code–style muted chrome button variant: calm grey icon/label at rest,
/// crisp foreground & soft inset surface on hover.
pub fn quiet_icon_variant(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(rgba(0).into())
        .foreground(Color::Muted.get(cx).into())
        .hover(Color::Inset.get(cx).into())
        .active(Color::Divider.get(cx).into())
}

/// Ultra-quiet icon variant for secondary row actions (`...`, copy, quote)
/// so dense lists and chat streams remain visually serene.
pub fn subtle_icon_variant(cx: &App) -> ButtonCustomVariant {
    ButtonCustomVariant::new(cx)
        .color(rgba(0).into())
        .foreground(Color::Muted.get(cx).opacity(0.68).into())
        .hover(Color::Inset.get(cx).into())
        .active(Color::Divider.get(cx).into())
}

/// T3 Code–style sidebar navigation item variant: avoids heavy selected washes
/// while keeping calm cool-zinc / soft-graphite contrast.
pub fn sidebar_item_variant(selected: bool, cx: &App) -> ButtonCustomVariant {
    let dark = Theme::global(cx).is_dark();
    if selected {
        let bg = rgb(if dark { 0x26262c } else { 0xebebee });
        let hover = rgb(if dark { 0x2c2c33 } else { 0xe4e4e7 });
        ButtonCustomVariant::new(cx)
            .color(bg.into())
            .foreground(Color::Text.get(cx).into())
            .hover(hover.into())
            .active(bg.into())
    } else {
        let hover = rgb(if dark { 0x1c1c21 } else { 0xf1f1f4 });
        let active = rgb(if dark { 0x26262c } else { 0xebebee });
        ButtonCustomVariant::new(cx)
            .color(rgba(0).into())
            .foreground(Color::Secondary.get(cx).into())
            .hover(hover.into())
            .active(active.into())
    }
}

/// Crisp idle/disabled fill color for the circular send button, applied via
/// Button's instance style so `disabled` doesn't wash it out to 15% alpha.
pub fn send_circle_disabled_bg(cx: &App) -> Rgba {
    rgb(if Theme::global(cx).is_dark() {
        0x27272b
    } else {
        0xededf0
    })
}

/// Crisp idle/disabled icon color for the circular send button.
pub fn send_circle_disabled_fg(_cx: &App) -> Rgba {
    rgb(0x71717a)
}

/// T3 Code circular send button: soft neutral zinc circle when idle/disabled,
/// high-contrast obsidian/alabaster ink circle when ready to send.
pub fn send_circle_variant(enabled: bool, cx: &App) -> ButtonCustomVariant {
    let dark = Theme::global(cx).is_dark();
    if enabled {
        if dark {
            ButtonCustomVariant::new(cx)
                .color(rgb(0xf4f4f5).into())
                .foreground(rgb(0x18181b).into())
                .hover(rgb(0xffffff).into())
                .active(rgb(0xd4d4d8).into())
        } else {
            ButtonCustomVariant::new(cx)
                .color(rgb(0x18181b).into())
                .foreground(rgb(0xffffff).into())
                .hover(rgb(0x27272a).into())
                .active(rgb(0x3f3f46).into())
        }
    } else if dark {
        ButtonCustomVariant::new(cx)
            .color(rgb(0x27272b).into())
            .foreground(rgb(0x71717a).into())
            .hover(rgb(0x27272b).into())
            .active(rgb(0x27272b).into())
    } else {
        ButtonCustomVariant::new(cx)
            .color(rgb(0xededf0).into())
            .foreground(rgb(0x71717a).into())
            .hover(rgb(0xededf0).into())
            .active(rgb(0xededf0).into())
    }
}

/// Computes a deterministic 1–2 character monogram matching T3 Code (`archify` -> `AY`,
/// `hive-desktop` -> `HD`, `CodeAnalysis` -> `CA`, `v2` -> `V2`).
pub fn project_monogram(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "PR".to_owned();
    }
    let parts: Vec<&str> = trimmed
        .split(|c: char| !c.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() >= 2 {
        let mut out = String::new();
        for part in [parts[0], parts[parts.len() - 1]] {
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
    let token = parts.first().copied().unwrap_or(trimmed);
    let chars: Vec<char> = token.chars().collect();
    if let Some(&first) = chars.first() {
        if !first.is_ascii() {
            return first.to_string();
        }
        // First digit in token (e.g. `v2` -> `V2`)
        if let Some(&digit) = chars.iter().skip(1).find(|c| c.is_ascii_digit()) {
            return format!("{}{digit}", first.to_ascii_uppercase());
        }
        // CamelCase second capital letter (e.g. `CodeAnalysis` -> `CA`)
        if let Some(&camel) = chars.iter().skip(1).find(|c| c.is_ascii_uppercase()) {
            return format!("{}{}", first.to_ascii_uppercase(), camel);
        }
        // Single-word monogram: first + last char (e.g. `archify` -> `AY`)
        if let Some(&last) = chars.last().filter(|_| chars.len() > 1) {
            return format!(
                "{}{}",
                first.to_ascii_uppercase(),
                last.to_ascii_uppercase()
            );
        }
        return first.to_ascii_uppercase().to_string();
    }
    "PR".to_owned()
}

/// Returns T3 Code–style ultra-low-saturation `(bg, fg)` badge colors for a project name.
/// Light uses Tailwind 600 ink over a 14% tint; Dark uses Tailwind 400 ink over a 14% tint.
pub fn project_badge_colors(name: &str, cx: &App) -> (Rgba, Rgba) {
    const TONES: &[(u32, u32)] = &[
        (0x52525b, 0xa1a1aa), // zinc / gray
        (0xdc2626, 0xf87171), // red
        (0xea580c, 0xfb923c), // orange
        (0xd97706, 0xfbbf24), // amber
        (0xd08700, 0xfdc700), // yellow
        (0x65a30d, 0xa3e635), // lime
        (0x16a34a, 0x4ade80), // green
        (0x059669, 0x34d399), // emerald
        (0x0d9488, 0x2dd4bf), // teal
        (0x0891b2, 0x22d3ee), // cyan
        (0x0284c7, 0x38bdf8), // sky
        (0x2563eb, 0x60a5fa), // blue
        (0x4f46e5, 0x818cf8), // indigo
        (0x7c3aed, 0xa78bfa), // violet
        (0x9333ea, 0xc084fc), // purple
        (0xc026d3, 0xe879f9), // fuchsia
        (0xdb2777, 0xf472b6), // pink
        (0xe11d48, 0xfb7185), // rose
    ];
    let hash = name
        .to_lowercase()
        .chars()
        .fold(0u32, |acc, ch| acc.wrapping_mul(31).wrapping_add(ch as u32));
    let (light_fg, dark_fg) = TONES[(hash % TONES.len() as u32) as usize];
    let fg = rgb(if Theme::global(cx).is_dark() {
        dark_fg
    } else {
        light_fg
    });
    let bg = fg.opacity(0.14);
    (bg, fg)
}

/// Renders a T3 Code–inspired rounded monogram avatar badge for a workspace.
pub fn project_badge(name: &str, size_px: f32, cx: &App) -> Div {
    let (bg, fg) = project_badge_colors(name, cx);
    let text = project_monogram(name);
    let font_px = if size_px <= 16.5 { 8.5 } else { 9.5 };
    let radius_px = if size_px <= 19. { 4. } else { 5. };
    div()
        .size(px(size_px))
        .flex_shrink_0()
        .rounded(px(radius_px))
        .bg(bg)
        .text_color(fg)
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
        assert_eq!(project_monogram("archify"), "AY");
        assert_eq!(project_monogram("CodeAnalysis"), "CA");
        assert_eq!(project_monogram("v2"), "V2");
        assert_eq!(project_monogram("桌面助手"), "桌");
    }
}
