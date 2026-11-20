mod app;
mod agent_profiles;
mod appearance;
mod chrome;
mod language;
mod local_notifications;
mod artifacts;
mod attachments;
mod collaboration;
mod workflows;
mod composer;
mod composer_controls;
mod queued_inputs;
mod command_palette;
mod command_picker;
mod conversation;
mod conversation_find;
mod conversation_export;
mod diffs_settings;
mod file_save;
mod focus_scroll;
mod conversation_list;
mod drafts;
mod diagnostics;
mod desktop_update;
mod failures;
mod general;
mod open_targets;
mod image_input;
mod project_files;
mod project_scripts;
mod git_workspace;
mod history_navigation;
mod keybindings;
mod image_viewer;
mod interactions;
mod maintenance;
mod members;
mod memory;
mod model_picker;
mod navigation;
mod palette;
mod process_supervisor;
mod protocol;
mod reasoning_picker;
mod mode_picker;
mod roles;
mod runtime_location;
mod sessions;
mod session_execution;
mod session_history;
mod session_handoff;
mod settings;
#[cfg(target_os = "macos")]
mod shell_owner;
#[cfg(target_os = "macos")]
mod cli_owner;
mod terminal;
mod thinking;
mod tool_message;
mod tool_diff;
mod text_reading;
mod file_viewer;
mod html_preview;
mod transport;
#[cfg(feature = "ui-preview")]
mod ui_preview;
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
        OpenCommandPalette,
        OpenConversationFind,
        CloseConversationFind,
        NextConversationMatch,
        PreviousConversationMatch,
        NextSessionResult,
        PreviousSessionResult,
        NewSessionShortcut,
        ToggleSidebar,
        ToggleWorkspaceTerminal,
        OpenGitWorkspaceShortcut,
        OpenProjectFilePicker,
        OpenProjectFileSelection,
        OpenModelPicker,
        ToggleSessionMode,
        SelectPreviousSession,
        SelectNextSession
    ]
);

fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--desktop-update-identity")
    {
        let platform = match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "win32",
            other => other,
        };
        let arch = match std::env::consts::ARCH {
            "aarch64" => "arm64",
            "x86_64" => "x64",
            other => other,
        };
        println!("{}", serde_json::json!({
            "schemaVersion": 1,
            "app": "com.hive.desktop.dev",
            "version": env!("CARGO_PKG_VERSION"),
            "platform": platform,
            "arch": arch,
            "installerProtocol": 2,
        }));
        return;
    }
    #[cfg(target_os = "macos")]
    {
        cli_owner::run_if_requested();
        shell_owner::run_if_requested();
    }
    if let Err(error) = desktop_update::authorize_candidate_startup() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            cx.set_app_identity("com.hive.desktop.dev", "Hive Desktop");
            gpui_kit::init(cx);
            appearance::prepare_terminal_fonts(cx);
            cx.set_global(local_notifications::Preferences::default());
            cx.set_global(appearance::Appearance::default());
            general::init(cx);
            open_targets::init(cx);
            diffs_settings::init(cx);
            cx.set_global(language::Language::default());
            component::set_locale("zh-CN");
            cx.set_global(composer::ComposerPreferences::default());
            keybindings::init(cx);
            appearance::apply(None, cx);
            cx.bind_keys(keybindings::startup_bindings(cx));
            cx.bind_keys([
                KeyBinding::new("escape", CloseConversationFind, Some("HiveConversation")),
                KeyBinding::new("enter", NextConversationMatch, Some("HiveConversationFind")),
                KeyBinding::new("shift-enter", PreviousConversationMatch, Some("HiveConversationFind")),
                KeyBinding::new("down", NextSessionResult, Some("HiveSessionSearch")),
                KeyBinding::new("up", PreviousSessionResult, Some("HiveSessionSearch")),
                KeyBinding::new("down", NextSessionResult, Some("HiveProjectFileSearch")),
                KeyBinding::new("up", PreviousSessionResult, Some("HiveProjectFileSearch")),
                KeyBinding::new("enter", OpenProjectFileSelection, Some("HiveProjectFileSearch")),
                KeyBinding::new("down", NextSessionResult, Some("HiveProjectFileTree")),
                KeyBinding::new("up", PreviousSessionResult, Some("HiveProjectFileTree")),
                KeyBinding::new("enter", OpenProjectFileSelection, Some("HiveProjectFileTree")),
                KeyBinding::new("escape", CancelSessionRename, Some("HiveSessionName")),
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
            #[cfg(target_os = "macos")]
            language::install_application_menu(cx);
            cx.activate(true);
            gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Hive".into()),
                        appears_transparent: cfg!(target_os = "macos"),
                        traffic_light_position: cfg!(target_os = "macos")
                            .then(|| point(px(15.), px(16.))),
                    }),
                    app_owns_titlebar_drag: cfg!(target_os = "macos"),
                    window_bounds: Some(WindowBounds::centered(size(px(1220.), px(820.)), cx)),
                    window_min_size: Some(size(px(640.), px(480.))),
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
                    cx.new(|cx| {
                        cx.observe_window_activation(window, |_, window, cx| {
                            if window.is_window_active() {
                                open_targets::refresh(cx);
                            }
                        })
                        .detach();
                        app::HiveApp::new(window, cx)
                    })
                },
            )
            .expect("open Hive window");
        });
}
