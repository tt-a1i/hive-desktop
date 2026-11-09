mod app;
mod appearance;
mod language;
mod artifacts;
mod attachments;
mod collaboration;
mod workflows;
mod composer;
mod composer_controls;
mod queued_inputs;
mod command_picker;
mod conversation;
mod conversation_find;
mod conversation_export;
mod file_save;
mod focus_scroll;
mod conversation_list;
mod drafts;
mod failures;
mod image_input;
mod image_viewer;
mod interactions;
mod members;
mod memory;
mod model_picker;
mod navigation;
mod palette;
mod process_supervisor;
mod protocol;
mod reasoning_picker;
mod roles;
mod runtime_location;
mod sessions;
mod settings;
#[cfg(target_os = "macos")]
mod shell_owner;
mod terminal;
mod thinking;
mod tool_message;
mod tool_diff;
mod text_reading;
mod file_viewer;
mod transport;
mod usage;
mod view;
mod workspace_actions;
mod welcome;

use gpui_kit::*;
actions!(
    hive_desktop,
    [
        Quit,
        OpenPreferences,
        ClosePreferences,
        CancelSessionRename,
        OpenSessionSearch,
        OpenConversationFind,
        CloseConversationFind,
        NextSessionResult,
        PreviousSessionResult
    ]
);

fn main() {
    #[cfg(target_os = "macos")]
    shell_owner::run_if_requested();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.set_global(appearance::Appearance::default());
            cx.set_global(language::Language::default());
            component::set_locale("zh-CN");
            cx.set_global(composer::ComposerPreferences::default());
            appearance::apply(None, cx);
            cx.bind_keys([
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-f"
                    } else {
                        "ctrl-f"
                    },
                    OpenConversationFind,
                    None,
                ),
                KeyBinding::new("escape", CloseConversationFind, Some("HiveConversation")),
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-k"
                    } else {
                        "ctrl-k"
                    },
                    OpenSessionSearch,
                    None,
                ),
                KeyBinding::new("down", NextSessionResult, Some("HiveSessionSearch")),
                KeyBinding::new("up", PreviousSessionResult, Some("HiveSessionSearch")),
                KeyBinding::new("escape", CancelSessionRename, Some("HiveSessionName")),
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-,"
                    } else {
                        "ctrl-,"
                    },
                    OpenPreferences,
                    None,
                ),
                KeyBinding::new("escape", ClosePreferences, Some("HivePreferences")),
                KeyBinding::new(
                    if cfg!(target_os = "macos") {
                        "cmd-q"
                    } else {
                        "ctrl-q"
                    },
                    Quit,
                    None,
                ),
            ]);
            cx.activate(true);
            gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Hive".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::centered(size(px(1180.), px(820.)), cx)),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    appearance::apply(Some(window), cx);
                    window
                        .observe_window_appearance(|window, cx| {
                            appearance::apply(Some(window), cx);
                        })
                        .detach();
                    cx.new(|cx| app::HiveApp::new(window, cx))
                },
            )
            .expect("open Hive window");
        });
}
