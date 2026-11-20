use crate::app::{ErrorNotice, HiveApp};
use crate::language::tr;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        menu::{DropdownMenu, PopupMenu, PopupMenuItem},
        *,
    },
    *,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

#[cfg_attr(target_os = "macos", path = "open_targets/macos.rs")]
#[cfg_attr(target_os = "windows", path = "open_targets/windows.rs")]
#[cfg_attr(
    not(any(target_os = "macos", target_os = "windows")),
    path = "open_targets/other.rs"
)]
mod platform;

macro_rules! open_targets {
    ($($name:ident => $wire:literal, $label:literal, $glyph:ident;)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
        pub enum OpenTarget {
            #[default]
            #[serde(rename = "finder")]
            Finder,
            $(#[serde(rename = $wire)] $name,)*
        }

        impl OpenTarget {
            pub fn label(self) -> &'static str {
                match self {
                    Self::Finder if cfg!(target_os = "macos") => tr("访达"),
                    Self::Finder if cfg!(target_os = "windows") => tr("文件资源管理器"),
                    Self::Finder => tr("文件管理器"),
                    $(Self::$name => $label,)*
                }
            }

            fn fallback_icon(self) -> Glyph {
                match self {
                    Self::Finder => Glyph::FolderOpen,
                    $(Self::$name => Glyph::$glyph,)*
                }
            }
        }
    }
}

open_targets! {
    Devin => "devin", "Devin Desktop", Code;
    Vscode => "vscode", "VS Code", Code;
    VscodeInsiders => "vscode-insiders", "VS Code Insiders", Code;
    Cursor => "cursor", "Cursor", Code;
    Zed => "zed", "Zed", Code;
    Windsurf => "windsurf", "Windsurf", Code;
    Antigravity => "antigravity", "Antigravity", Code;
    Qoder => "qoder", "Qoder", Code;
    SublimeText => "sublime-text", "Sublime Text", Code;
    IntelliJ => "intellij", "IntelliJ IDEA", Code;
    PyCharm => "pycharm", "PyCharm", Code;
    WebStorm => "webstorm", "WebStorm", Code;
    RustRover => "rustrover", "RustRover", Code;
    GoLand => "goland", "GoLand", Code;
    Rider => "rider", "Rider", Code;
    CLion => "clion", "CLion", Code;
    AndroidStudio => "android-studio", "Android Studio", Code;
    Xcode => "xcode", "Xcode", Code;
    Terminal => "terminal", "Terminal", Terminal;
    WindowsTerminal => "windows-terminal", "Windows Terminal", Terminal;
    ITerm => "iterm", "iTerm2", Terminal;
    Ghostty => "ghostty", "Ghostty", Terminal;
    WezTerm => "wezterm", "WezTerm", Terminal;
    Kitty => "kitty", "Kitty", Terminal;
    Alacritty => "alacritty", "Alacritty", Terminal;
    Warp => "warp", "Warp", Terminal;
    Cmux => "cmux", "cmux", Terminal;
}

use OpenTarget::*;

const EDITORS: &[OpenTarget] = &[
    Devin,
    Vscode,
    Cursor,
    Zed,
    Windsurf,
    Antigravity,
    Qoder,
    VscodeInsiders,
    SublimeText,
    Xcode,
    IntelliJ,
    PyCharm,
    WebStorm,
    GoLand,
    RustRover,
    Rider,
    CLion,
    AndroidStudio,
];
const TERMINALS: &[OpenTarget] = &[
    Terminal,
    WindowsTerminal,
    Ghostty,
    ITerm,
    Warp,
    Cmux,
    WezTerm,
    Kitty,
    Alacritty,
];

impl OpenTarget {
    pub fn for_platform() -> impl Iterator<Item = Self> {
        std::iter::once(Finder)
            .chain(EDITORS.iter().chain(TERMINALS).copied())
            .filter(|target| platform::supports(*target))
    }
}

#[derive(Clone)]
struct InstalledApp {
    target: OpenTarget,
    path: PathBuf,
    icon: Option<Arc<RenderImage>>,
}

impl InstalledApp {
    fn icon(&self, size: Pixels) -> AnyElement {
        match &self.icon {
            Some(image) => img(image.clone())
                .size(size)
                .flex_shrink_0()
                .into_any_element(),
            None => Icon::new(self.target.fallback_icon())
                .size(size)
                .into_any_element(),
        }
    }

    fn menu_item(&self, selected: bool) -> PopupMenuItem {
        let app = self.clone();
        PopupMenuItem::element(move |_, _| {
            h_flex()
                .flex_1()
                .items_center()
                .gap_2()
                .child(app.icon(px(18.)))
                .child(app.target.label())
        })
        .checked(selected)
    }
}

#[derive(Default)]
struct InstalledApps {
    apps: Vec<InstalledApp>,
    last_probe: Option<Instant>,
    probing: bool,
}

impl Global for InstalledApps {}

pub(crate) fn init(cx: &mut App) {
    cx.set_global(InstalledApps::default());
    refresh(cx);
}

pub(crate) fn refresh(cx: &mut App) {
    let Some(previous) = cx.update_global::<InstalledApps, _>(|state, _| {
        if state.probing
            || state
                .last_probe
                .is_some_and(|time| time.elapsed() < Duration::from_secs(2))
        {
            return None;
        }
        state.probing = true;
        Some(state.apps.clone())
    }) else {
        return;
    };
    let probe = cx.background_executor().spawn(async move {
        OpenTarget::for_platform()
            .filter_map(|target| {
                let path = platform::locate(target)?;
                if let Some(app) = previous
                    .iter()
                    .find(|app| app.target == target && app.path == path)
                {
                    return Some(app.clone());
                }
                let icon = platform::icon(target, &path).map(crate::attachments::render_image);
                Some(InstalledApp { target, path, icon })
            })
            .collect()
    });
    cx.spawn(async move |cx| {
        let apps: Vec<InstalledApp> = probe.await;
        cx.update(|cx| {
            let changed = cx.update_global::<InstalledApps, _>(|state, _| {
                state.probing = false;
                state.last_probe = Some(Instant::now());
                let current = state.apps.iter().map(|app| (app.target, &app.path));
                let updated = apps.iter().map(|app| (app.target, &app.path));
                if current.eq(updated) {
                    return false;
                }
                state.apps = apps;
                true
            });
            if changed {
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

fn installed(cx: &App) -> &[InstalledApp] {
    &cx.global::<InstalledApps>().apps
}

fn open(target: OpenTarget, path: &Path, application: Option<InstalledApp>) -> Result<(), String> {
    if !path.is_absolute() || !path.is_dir() {
        return Err(tr("项目文件夹不存在或无法访问").to_owned());
    }
    let app = application
        .map(|app| app.path)
        .filter(|path| path.exists())
        .or_else(|| platform::locate(target))
        .ok_or_else(|| format!("{} ({})", tr("目标应用未安装"), target.label()))?;
    platform::open(target, &app, path)
}

pub(crate) fn workspace_menu(
    mut menu: PopupMenu,
    owner: WeakEntity<HiveApp>,
    workspace: String,
    preferred: Option<OpenTarget>,
    cx: &App,
) -> PopupMenu {
    menu = menu.check_side(Side::Right);
    for app in installed(cx) {
        let owner = owner.clone();
        let workspace = workspace.clone();
        let target = app.target;
        menu = menu.item(
            app.menu_item(preferred == Some(target))
                .on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |app, cx| {
                        app.open_workspace_target(workspace.clone(), target, cx)
                    });
                }),
        );
    }
    menu
}

fn app_icon(target: OpenTarget, app: Option<&InstalledApp>, size: Pixels) -> AnyElement {
    match app {
        Some(app) => app.icon(size),
        None => Icon::new(target.fallback_icon())
            .size(size)
            .into_any_element(),
    }
}

pub(crate) fn settings_picker(preferred: OpenTarget, cx: &App) -> AnyElement {
    let selected = installed(cx).iter().find(|app| app.target == preferred);
    Button::new("general-open-target")
        .outline()
        .small()
        .accessibility_label(preferred.label())
        .child(
            h_flex()
                .gap_2()
                .child(app_icon(preferred, selected, px(16.)))
                .child(preferred.label())
                .child(Icon::new(Glyph::ChevronDown).xsmall()),
        )
        .disabled(installed(cx).is_empty())
        .dropdown_menu(move |mut menu, _, cx| {
            menu = menu.check_side(Side::Right);
            for app in installed(cx) {
                let target = app.target;
                menu = menu.item(
                    app.menu_item(target == preferred)
                        .on_click(move |_, _, cx| {
                            let mut next = crate::general::current(cx);
                            next.default_open_target = target;
                            crate::general::set(next, cx);
                        }),
                );
            }
            menu
        })
        .into_any_element()
}

impl HiveApp {
    pub(crate) fn render_project_open_controls(
        &self,
        workspace: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let apps = installed(cx);
        let preferred = crate::general::current(cx).default_open_target;
        let app = apps
            .iter()
            .find(|app| app.target == preferred)
            .or_else(|| apps.iter().find(|app| app.target == Finder));
        let target = app.map_or(Finder, |app| app.target);
        let target_label = target.label();
        let tooltip = if crate::language::english() {
            format!("Open project in {target_label}")
        } else {
            format!("在 {target_label} 中打开项目")
        };

        let workspace = workspace.to_owned();
        let menu_workspace = workspace.clone();
        let owner = cx.weak_entity();
        h_flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("topbar-open-folder")
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(28.))
                    .px_2()
                    .rounded_lg()
                    .child(
                        h_flex()
                            .gap_1p5()
                            .child(app_icon(target, app, px(16.)))
                            .child(target_label),
                    )
                    .tooltip(tooltip.clone())
                    .accessibility_label(tooltip)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_workspace_target(workspace.clone(), target, cx);
                    })),
            )
            .child(
                Button::new("topbar-open-target-menu")
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(28.))
                    .w(px(20.))
                    .rounded_lg()
                    .icon(Glyph::ChevronDown)
                    .tooltip(tr("选择打开方式"))
                    .accessibility_label(tr("选择打开方式"))
                    .disabled(apps.is_empty())
                    .dropdown_menu(move |menu, _, cx| {
                        workspace_menu(
                            menu,
                            owner.clone(),
                            menu_workspace.clone(),
                            Some(target),
                            cx,
                        )
                    }),
            )
    }

    pub(crate) fn open_workspace_target(
        &mut self,
        workspace: String,
        target: OpenTarget,
        cx: &mut Context<Self>,
    ) {
        let Some(folder) = self.workspaces.iter().find(|ws| ws.id == workspace) else {
            return;
        };
        let path = PathBuf::from(&folder.path);
        let app = installed(cx)
            .iter()
            .find(|app| app.target == target)
            .cloned();
        let launch = cx
            .background_executor()
            .spawn(async move { open(target, &path, app) });
        cx.spawn(async move |this, cx| {
            if let Err(error) = launch.await {
                let _ = this.update(cx, |this, cx| {
                    if this.workspace.as_ref() == Some(&workspace) {
                        this.error = Some(ErrorNotice::Detail("无法在外部应用中打开项目", error));
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }
}
