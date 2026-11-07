mod app;
mod appearance;
mod artifacts;
mod collaboration;
mod conversation;
mod conversation_find;
mod drafts;
mod interactions;
mod members;
mod memory;
mod model_picker;
mod navigation;
mod palette;
mod process_supervisor;
mod protocol;
mod roles;
mod runtime_location;
mod sessions;
mod settings;
#[cfg(target_os = "macos")]
mod shell_owner;
mod terminal;
mod tool_message;
mod transport;
mod view;
mod workspace_actions;

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
