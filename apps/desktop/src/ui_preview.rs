//! Development-only visual QA route selector (`--features ui-preview`).
use crate::app::{HiveApp, SettingsSection};
use gpui_kit::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

static APPLIED: AtomicBool = AtomicBool::new(false);

pub fn maybe_apply(app: &mut HiveApp, window: &mut Window, cx: &mut Context<HiveApp>) {
    if APPLIED.load(Ordering::Relaxed) {
        return;
    }
    let Ok(screen) = std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN") else {
        APPLIED.store(true, Ordering::Relaxed);
        return;
    };
    let screen = screen.trim().to_owned();
    if screen.is_empty() {
        APPLIED.store(true, Ordering::Relaxed);
        return;
    }
    if !app.connected || app.loading {
        return;
    }
    let needs_workspace = !screen.starts_with("settings:") && !screen.starts_with("session:");
    if needs_workspace && app.workspace.is_none() {
        return;
    }
    let needs_session = screen.starts_with("chat:");
    if needs_session && (app.session.is_none() || !app.conversation_ready) {
        return;
    }
    APPLIED.store(true, Ordering::Relaxed);
    let weak = cx.weak_entity();
    let handle = window.window_handle();
    cx.defer(move |cx| {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = weak.update(cx, |app, cx| {
                apply_screen(app, &screen, window, cx);
            });
        });
    });
}

fn apply_screen(
    app: &mut HiveApp,
    screen: &str,
    window: &mut Window,
    cx: &mut Context<HiveApp>,
) {
    match screen {
        "settings:appearance" => open_section(app, SettingsSection::Appearance, window, cx),
        "settings:chat" => open_section(app, SettingsSection::Chat, window, cx),
        "settings:accounts" => open_section(app, SettingsSection::PiAccounts, window, cx),
        "settings:models" => open_section(app, SettingsSection::Models, window, cx),
        "settings:team" => open_section(app, SettingsSection::Team, window, cx),
        "settings:notifications" => open_section(app, SettingsSection::Notifications, window, cx),
        "settings:remote" => open_section(app, SettingsSection::Remote, window, cx),
        "settings:programs" => open_section(app, SettingsSection::Programs, window, cx),
        "settings:search" => {
            app.open_settings(window, cx);
            app.settings_search
                .update(cx, |s, cx| s.set_value("模型", window, cx));
        }
        "members" => app.show_team(window, cx),
        "roles" => app.open_roles(cx),
        "collaboration" => app.open_collaboration(cx),
        "memory" => app.open_memory(cx),
        "workflows" => app.show_workflows(window, cx),
        "terminal" => app.show_terminal(window, cx),
        "session:search" => app.open_session_search(window, cx),
        "session:archives" => app.open_archived_sessions(window, cx),
        "chat:find" => app.open_conversation_find(window, cx),
        "chat:export" => app.open_conversation_export(window, cx),
        "chat:model-picker" => {
            app.choosing_model = true;
            app.load_reasoning(None, cx);
            cx.notify();
        }
        "chat:usage" => {
            app.usage.visible = true;
            app.load_usage(cx);
            cx.notify();
        }
        "chat:commands" => {
            app.input
                .update(cx, |input, cx| input.set_value("/", window, cx));
            app.composer_commands_changed(cx);
        }
        "chat:resources" => app.open_session_resources(window, cx),
        "chat:expanded-worklog" => {
            let mut ids = Vec::new();
            for message in &mut app.messages {
                for block in &mut message.thinking {
                    block.expanded = true;
                }
                if let Some(tool) = &mut message.tool {
                    tool.expanded = true;
                    ids.push(message.id.clone());
                }
            }
            for id in ids {
                app.expanded_tool_groups.insert(id);
            }
            app.sync_conversation_list(cx);
            cx.notify();
        }
        "chat:diff" => {
            let patch = concat!(
                "@@ -30,3 +30,4 @@\n",
                " impl Color {\n",
                "-    Self::Attention => (0xfef7e6, 0x2c2416),\n",
                "+    Self::Attention => (0xfef7e6, 0x2c2416),\n",
                "+    // foreground amber indicator uses high-contrast 0xd97706 / 0xfbbf24\n",
                "     Self::Divider => (0xe8e9ed, 0x25252d),\n",
            );
            app.open_tool_diff(
                "Edit".to_owned(),
                json!({ "path": "apps/desktop/src/palette.rs" }),
                Some(patch.to_owned()),
                "completed",
                window,
                cx,
            );
        }
        "chat:file-viewer" => {
            // Preview the selected project's actual file. Route selection must
            // never create or replace files in a user's workspace.
            app.open_file_preview("README.md".to_owned(), window, cx);
        }
        _ => {}
    }
}

fn open_section(
    app: &mut HiveApp,
    section: SettingsSection,
    window: &mut Window,
    cx: &mut Context<HiveApp>,
) {
    app.open_settings(window, cx);
    app.select_settings_section(section, window, cx);
}
