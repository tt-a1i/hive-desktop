mod backend;
mod chat;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
actions!(hive_probe, [Quit]);

fn main() {
    println!("[probe] starting native chat window");
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.bind_keys([KeyBinding::new(
                if cfg!(target_os = "macos") {
                    "cmd-q"
                } else {
                    "ctrl-q"
                },
                Quit,
                None,
            )]);
            cx.on_action(|_: &Quit, cx| {
                println!("[probe] quit shortcut requested");
                cx.quit();
            });
            Theme::change(ThemeMode::Light, None, cx);
            cx.activate(true);
            gpui_kit::open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("Hive GPUI Probe".into()),
                        ..Default::default()
                    }),
                    window_bounds: Some(WindowBounds::centered(size(px(1100.), px(780.)), cx)),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| chat::Chat::new(window, cx)),
            )
            .expect("native chat window");
            println!("[probe] chat window opened");
        });
}
