use super::*;
use gpui_kit::{
    base::InteractiveElementExt as _,
    component::resizable::{HandleEdge, resize_handle},
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

const DEFAULT_WIDTH: f32 = 256.;
const MIN_WIDTH: f32 = 208.;
const MAIN_MIN_WIDTH: f32 = 640.;
const FILE: &str = "desktop-sidebar.json";

#[derive(Deserialize, Serialize)]
struct StoredWidth {
    width: f32,
}
struct ResizeOrigin {
    mouse_x: f32,
    width: f32,
    preview: f32,
}
enum WidthNotice {
    Read(String),
    Save { width: f32, error: String },
}
pub(crate) struct SidebarPreferences {
    width: f32,
    brand_width: f32,
    root: Option<PathBuf>,
    drag: Option<ResizeOrigin>,
    notice: Option<WidthNotice>,
    preview_only: bool,
}
impl Default for SidebarPreferences {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            brand_width: 0.,
            root: None,
            drag: None,
            notice: None,
            preview_only: cfg!(feature = "ui-preview")
                && std::env::var("HIVE_DESKTOP_PREVIEW_SCREEN")
                    .is_ok_and(|screen| !screen.trim().is_empty()),
        }
    }
}
fn minimum(brand: f32) -> f32 {
    MIN_WIDTH.max(brand.ceil())
}
fn maximum(viewport: f32, min: f32) -> f32 {
    min.max(viewport.floor() - MAIN_MIN_WIDTH)
}
fn clamped(width: f32, viewport: f32, brand: f32) -> f32 {
    let min = minimum(brand);
    width.clamp(min, maximum(viewport, min))
}
fn load(root: &Path) -> io::Result<f32> {
    let file = match fs::File::open(root.join(FILE)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(DEFAULT_WIDTH),
        Err(error) => return Err(error),
    };
    let stored: StoredWidth = serde_json::from_reader(file)?;
    if !stored.width.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid sidebar width",
        ));
    }
    Ok(stored.width.max(MIN_WIDTH))
}
fn save(root: &Path, width: f32) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(file.as_file_mut(), &StoredWidth { width })?;
    file.as_file().sync_all()?;
    file.persist(root.join(FILE))?;
    Ok(())
}
impl SidebarPreferences {
    /// The runtime owns data-directory discovery. A reconnect does not reload
    /// over a drag, a local choice, or its pending write-failure notice.
    pub(crate) fn restore(&mut self, root: &Path) {
        if self.root.is_some() {
            return;
        }
        self.root = Some(root.to_path_buf());
        self.reload();
    }
    fn reload(&mut self) {
        let Some(root) = &self.root else {
            return;
        };
        match load(root) {
            Ok(width) => {
                self.width = width;
                self.notice = None;
            }
            Err(error) => self.notice = Some(WidthNotice::Read(error.to_string())),
        }
    }
    fn commit(&mut self, width: f32) {
        self.drag = None;
        if self.preview_only {
            self.width = width;
            self.notice = None;
            return;
        }
        let result = self
            .root
            .as_ref()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, tr("后台尚未连接")))
            .and_then(|root| save(root, width));
        match result {
            Ok(()) => {
                self.width = width;
                self.notice = None;
            }
            Err(error) => {
                self.notice = Some(WidthNotice::Save {
                    width,
                    error: error.to_string(),
                })
            }
        }
    }
    fn retry(&mut self) {
        match self.notice.as_ref() {
            Some(WidthNotice::Save { width, .. }) => self.commit(*width),
            Some(WidthNotice::Read(_)) => self.reload(),
            None => (),
        }
    }
    pub(crate) fn resolved(&self, viewport: Pixels) -> Pixels {
        px(clamped(
            self.drag
                .as_ref()
                .map(|drag| drag.preview)
                .unwrap_or(self.width),
            viewport.into(),
            self.brand_width,
        ))
    }
    fn begin_drag(&mut self, mouse_x: Pixels, viewport: Pixels) {
        let width = self.resolved(viewport).into();
        self.drag = Some(ResizeOrigin {
            mouse_x: mouse_x.into(),
            width,
            preview: width,
        });
    }
    fn move_drag(&mut self, mouse_x: Pixels, viewport: Pixels) {
        if let Some(drag) = &mut self.drag {
            drag.preview = clamped(
                drag.width + f32::from(mouse_x) - drag.mouse_x,
                viewport.into(),
                self.brand_width,
            );
        }
    }
    fn finish_drag(&mut self) {
        if let Some(drag) = self.drag.take()
            && drag.preview != drag.width
        {
            self.commit(drag.preview);
        }
    }
}
#[derive(Clone)]
struct ResizeSidebar;
impl Render for ResizeSidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
impl HiveApp {
    pub(super) fn render_sidebar_resize(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        resize_handle("sidebar-resize", Axis::Horizontal)
            .inside(HandleEdge::Trailing)
            .role(Role::Splitter)
            .aria_label(tr("调整侧栏宽度，双击恢复默认"))
            .aria_orientation(accesskit::Orientation::Vertical)
            .aria_numeric_value(
                f32::from(self.sidebar.resolved(window.viewport_size().width)) as f64,
            )
            .aria_min_numeric_value(minimum(self.sidebar.brand_width) as f64)
            .aria_max_numeric_value(maximum(
                window.viewport_size().width.into(),
                minimum(self.sidebar.brand_width),
            ) as f64)
            .on_drag(ResizeSidebar, |drag, _, _, cx| cx.new(|_| drag.clone()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.sidebar
                        .begin_drag(event.position.x, window.viewport_size().width);
                    cx.stop_propagation();
                }),
            )
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<ResizeSidebar>, window, cx| {
                    this.sidebar
                        .move_drag(event.event.position.x, window.viewport_size().width);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.sidebar.finish_drag();
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.sidebar.finish_drag();
                    cx.notify();
                }),
            )
            .on_double_click(cx.listener(|this, _, _, cx| {
                this.sidebar.commit(DEFAULT_WIDTH);
                cx.notify();
            }))
    }

    pub(super) fn render_sidebar_brand_probe(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let app = cx.weak_entity();
        h_flex()
            .id("sidebar-brand-width-probe")
            .aria_hidden(true)
            .absolute()
            .top_0()
            .left_0()
            .opacity(0.)
            .pl(crate::chrome::traffic_light_inset(window) + px(8.))
            .pr_3()
            .gap_1p5()
            .items_center()
            .child(div().w(px(26.)).h(px(26.)))
            .child(div().text_sm().font_weight(FontWeight::BOLD).child("Hive"))
            .on_prepaint(move |bounds, _, cx| {
                let _ = app.update(cx, |this, cx| {
                    let width = f32::from(bounds.size.width).ceil();
                    if this.sidebar.brand_width != width {
                        this.sidebar.brand_width = width;
                        cx.notify();
                    }
                });
            })
    }

    pub(super) fn render_sidebar_width_notice(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let text = match self.sidebar.notice.as_ref()? {
            WidthNotice::Read(error) => format!(
                "{}: {}",
                tr("读取侧栏宽度失败"),
                crate::language::localize_owned_error(error)
            ),
            WidthNotice::Save { error, .. } => format!(
                "{}: {}",
                tr("保存侧栏宽度失败，原宽度仍生效"),
                crate::language::localize_owned_error(error)
            ),
        };
        Some(
            v_flex()
                .id("sidebar-width-error")
                .role(Role::Status)
                .aria_label(text.clone())
                .min_w_0()
                .flex_shrink_0()
                .p_2()
                .gap_1()
                .rounded_md()
                .bg(Color::ErrorSurface.get(cx))
                .child(div().text_xs().text_color(Color::Error.get(cx)).child(text))
                .child(
                    Button::new("retry-sidebar-width")
                        .outline()
                        .compact()
                        .small()
                        .label(tr("重试"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.sidebar.retry();
                            cx.notify();
                        })),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_WIDTH, FILE, SidebarPreferences, WidthNotice, clamped, load, save};
    use gpui_kit::px;
    use std::fs;
    #[test]
    fn width_constraints_preserve_main_space_and_temporary_clamps() {
        assert_eq!(clamped(DEFAULT_WIDTH, 1200., 0.), 256.);
        assert_eq!(clamped(999., 1000., 0.), 360.);
        assert_eq!(clamped(50., 1200., 0.), 208.);
        assert_eq!(clamped(256., 820., 0.), 208.);
        assert_eq!(clamped(208., 1000., 240.1), 241.);
        let preferences = SidebarPreferences {
            width: 400.,
            ..Default::default()
        };
        assert_eq!(preferences.resolved(px(900.)), px(260.));
        assert_eq!(preferences.resolved(px(1200.)), px(400.));
    }
    #[test]
    fn width_reopens_and_failed_atomic_write_retains_effective_width() {
        let root = tempfile::tempdir().unwrap();
        let mut preferences = SidebarPreferences::default();
        preferences.preview_only = false;
        preferences.restore(root.path());
        preferences.commit(320.);
        assert_eq!(load(root.path()).unwrap(), 320.);
        fs::remove_file(root.path().join(FILE)).unwrap();
        fs::create_dir(root.path().join(FILE)).unwrap();
        preferences.commit(380.);
        assert_eq!(preferences.width, 320.);
        assert!(matches!(
            preferences.notice,
            Some(WidthNotice::Save { width: 380., .. })
        ));
        assert!(root.path().join(FILE).is_dir());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn preview_and_reconnect_do_not_write_or_replace_active_drag() {
        let root = tempfile::tempdir().unwrap();
        save(root.path(), 300.).unwrap();
        let original = fs::read(root.path().join(FILE)).unwrap();
        let mut preferences = SidebarPreferences {
            preview_only: true,
            ..Default::default()
        };
        preferences.restore(root.path());
        preferences.commit(340.);
        assert_eq!(fs::read(root.path().join(FILE)).unwrap(), original);
        preferences.begin_drag(px(340.), px(1200.));
        preferences.move_drag(px(380.), px(1200.));
        preferences.restore(root.path());
        assert_eq!(preferences.resolved(px(1200.)), px(380.));
    }
}
