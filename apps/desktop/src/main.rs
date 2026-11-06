mod app;
mod collaboration;
mod conversation;
mod members;
mod navigation;
mod process_supervisor;
mod protocol;
mod roles;
mod runtime_location;
mod sessions;
mod settings;
mod transport;
mod view;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
actions!(hive_desktop, [Quit]);

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            cx.bind_keys([KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                Quit,
                None,
            )]);
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
                |window, cx| cx.new(|cx| app::HiveApp::new(window, cx)),
            )
            .expect("open Hive window");
        });
}
