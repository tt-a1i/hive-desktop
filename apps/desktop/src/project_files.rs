use crate::{
    app::HiveApp,
    focus_scroll::{Reveal, Viewport},
    image_input::ImageSource,
    language::tr,
    palette::Color,
    protocol::AttachmentRef,
    transport::Runtime,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::{ops::Range, sync::Arc, time::Duration};

mod browser;

pub(crate) use browser::{FileBrowser, FilePreviewMode, FileTreeExpansions, ProjectFilePreviewText};

pub(crate) enum FileError {
    Owned(&'static str),
    Request(String),
}
impl FileError {
    pub(crate) fn preview(error: String) -> Self {
        match error.as_str() {
            "Project file exceeds preview limit" => Self::Owned(
                "文件超过 1 MiB，暂不在窗口中预览；可在文件管理器中查看。",
            ),
            "Project file is not text" => Self::Owned("文件包含二进制内容，无法作为文本预览。"),
            "Project file is not UTF-8 text" => Self::Owned("文件不是 UTF-8 文本，暂不支持此编码的预览。"),
            _ => Self::Request(error),
        }
    }

    pub(crate) fn text(&self) -> String {
        match self {
            Self::Owned(message) => tr(message).to_owned(),
            Self::Request(error) => crate::language::localize_owned_error(error),
        }
    }
}

#[derive(Default, Deserialize)]
struct Page {
    paths: Vec<String>,
    total: usize,
}
#[derive(Clone)]
struct Target {
    workspace: String,
    session: Option<String>,
    draft: String,
    connection: u64,
    open_request: u64,
}
impl Target {
    fn capture(app: &HiveApp) -> Option<Self> {
        Some(Self {
            workspace: app.workspace.clone()?,
            session: app.session.as_ref().map(|session| session.id.clone()),
            draft: app.draft_target.clone()?,
            connection: app.connection_generation,
            open_request: app.open_request,
        })
    }

    fn current(&self, app: &HiveApp) -> bool {
        app.connected
            && !app.closing
            && !app.loading
            && app.connection_generation == self.connection
            && app.open_request == self.open_request
            && app.workspace.as_ref() == Some(&self.workspace)
            && app.session.as_ref().map(|session| &session.id) == self.session.as_ref()
            && app.draft_target.as_ref() == Some(&self.draft)
    }
}
struct FilePicker {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    target: Target,
    source_directory: Option<String>,
    query: Entity<InputState>,
    generation: u64,
    page: Page,
    offset: usize,
    selected: usize,
    loading: bool,
    error: Option<FileError>,
    scroll: ScrollHandle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MentionQuery {
    pub start: usize,
    pub end: usize,
    pub query: String,
}

pub(crate) fn mention_token_at_cursor(text: &str, cursor: usize) -> Option<MentionQuery> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    if text.starts_with('/') && !text.chars().any(char::is_whitespace) {
        return None;
    }
    let before = &text[..cursor];
    for (idx, ch) in before.char_indices().rev() {
        if ch.is_whitespace() {
            return None;
        }
        if ch == '@' {
            let valid_boundary = idx == 0
                || before[..idx]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace);
            if !valid_boundary {
                return None;
            }
            return Some(MentionQuery {
                start: idx,
                end: cursor,
                query: before[idx + 1..].to_owned(),
            });
        }
    }
    None
}

#[derive(Default)]
pub struct MentionPicker {
    pub open: bool,
    pub loading: bool,
    generation: u64,
    query: String,
    token_range: Option<Range<usize>>,
    page: Page,
    selected: usize,
    error: Option<FileError>,
    scroll: ScrollHandle,
    target: Option<Target>,
}

impl MentionPicker {
    pub(crate) fn close(&mut self) {
        self.open = false;
        self.loading = false;
        self.generation = self.generation.wrapping_add(1);
        self.query.clear();
        self.token_range = None;
        self.page = Page::default();
        self.selected = 0;
        self.error = None;
        self.target = None;
    }
}

impl HiveApp {
    pub(crate) fn composer_mentions_changed(&mut self, cx: &mut Context<Self>) {
        if self.command_picker.open
            || !self.viewing_conversation()
            || !self.can_reference_project_file()
        {
            if self.mention_picker.open {
                self.mention_picker.close();
                cx.notify();
            }
            return;
        }
        let (text, cursor, selection_empty) = {
            let input = self.input.read(cx);
            (
                input.value().to_string(),
                input.cursor(),
                input.selected_range().is_empty(),
            )
        };
        if !selection_empty {
            if self.mention_picker.open {
                self.mention_picker.close();
                cx.notify();
            }
            return;
        }
        let Some(token) = mention_token_at_cursor(&text, cursor) else {
            if self.mention_picker.open {
                self.mention_picker.close();
                cx.notify();
            }
            return;
        };
        let Some(target) = Target::capture(self) else {
            if self.mention_picker.open {
                self.mention_picker.close();
                cx.notify();
            }
            return;
        };
        let same_query = self.mention_picker.open
            && self.mention_picker.query == token.query
            && self
                .mention_picker
                .target
                .as_ref()
                .is_some_and(|prev| prev.draft == target.draft && prev.workspace == target.workspace);
        self.mention_picker.open = true;
        self.mention_picker.token_range = Some(token.start..token.end);
        self.mention_picker.target = Some(target);
        if same_query {
            cx.notify();
            return;
        }
        self.mention_picker.query = token.query;
        self.mention_picker.selected = 0;
        self.mention_picker.scroll.set_offset(point(px(0.), px(0.)));
        self.load_inline_mentions(true, cx);
    }

    pub(crate) fn load_inline_mentions(&mut self, debounce: bool, cx: &mut Context<Self>) {
        let (Some(runtime), Some(target)) = (self.runtime.clone(), self.mention_picker.target.clone())
        else {
            return;
        };
        if !target.current(self) {
            self.mention_picker.close();
            cx.notify();
            return;
        }
        self.mention_picker.generation = self.mention_picker.generation.wrapping_add(1);
        let generation = self.mention_picker.generation;
        self.mention_picker.loading = true;
        self.mention_picker.error = None;
        let query = self.mention_picker.query.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if debounce {
                smol::Timer::after(Duration::from_millis(150)).await;
            }
            let still_valid = this
                .update(cx, |this, _| {
                    this.mention_picker.open
                        && this.mention_picker.generation == generation
                        && target.current(this)
                })
                .ok()
                == Some(true);
            if !still_valid {
                return;
            }
            let mut params = json!({
                "workspaceId": target.workspace,
                "query": query,
                "offset": 0
            });
            if let Some(session) = &target.session {
                params["sessionId"] = json!(session);
            }
            let result = runtime
                .request("project.files.search", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<Page>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if !this.mention_picker.open
                    || this.mention_picker.generation != generation
                    || !target.current(this)
                {
                    return;
                }
                this.mention_picker.loading = false;
                match result {
                    Ok(page) => {
                        this.mention_picker.page = page;
                        this.mention_picker.selected = 0;
                        this.mention_picker.error = None;
                    }
                    Err(error) => {
                        this.mention_picker.page = Page::default();
                        this.mention_picker.error = Some(FileError::Request(error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn accept_inline_mention(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(target) = self.mention_picker.target.clone() else {
            self.mention_picker.close();
            return false;
        };
        if !target.current(self) || !self.can_reference_project_file() {
            self.mention_picker.error = Some(FileError::Owned(
                "当前草稿暂不能添加引用，请关闭后重试。",
            ));
            cx.notify();
            return false;
        }
        let range = self.mention_picker.token_range.clone();
        self.mention_picker.close();
        if let Some(range) = range {
            let text = self.input.read(cx).value().to_string();
            if range.start < range.end
                && range.end <= text.len()
                && text.is_char_boundary(range.start)
                && text.is_char_boundary(range.end)
                && text[range.start..].starts_with('@')
            {
                let start = text[..range.start].encode_utf16().count();
                let end = start + text[range.start..range.end].encode_utf16().count();
                self.input.update(cx, |input, cx| {
                    input.replace_text_in_range(Some(start..end), "", window, cx);
                    input.focus(window, cx);
                });
            }
        }
        if !self.reference_project_file(&target, path, None, cx) {
            self.error = Some("当前草稿暂不能添加引用，请关闭后重试。".into());
        }
        self.schedule_draft_save(cx);
        cx.notify();
        true
    }

    pub(crate) fn accept_selected_mention(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return false;
        }
        if self.mention_picker.loading {
            return true;
        }
        if let Some(path) = self
            .mention_picker
            .page
            .paths
            .get(self.mention_picker.selected)
            .cloned()
        {
            self.accept_inline_mention(path, window, cx)
        } else {
            false
        }
    }

    pub(crate) fn move_mention(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.mention_picker.page.paths.len();
        if count > 0 {
            self.mention_picker.selected = self
                .mention_picker
                .selected
                .saturating_add_signed(delta)
                .min(count - 1);
            self.mention_picker
                .scroll
                .scroll_to_item(self.mention_picker.selected);
        }
        cx.notify();
    }

    pub(crate) fn render_inline_mentions(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let paths = &self.mention_picker.page.paths;
        v_flex()
            .w((window.viewport_size().width - px(48.)).min(px(500.)))
            .id("inline-mention-menu")
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.mention_picker.close();
                cx.notify();
            }))
            .occlude()
            .shadow(crate::palette::overlay_shadow(cx))
            .p_0()
            .rounded_xl()
            .border_1()
            .border_color(Color::BorderStrong.get(cx))
            .bg(Color::Overlay.get(cx))
            .overflow_hidden()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(Color::Divider.get(cx))
                    .child(
                        h_flex()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(Glyph::AtSign)
                                    .size(px(13.))
                                    .text_color(Color::Accent.get(cx))
                                    .flex_shrink_0(),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("引用项目文件")),
                            )
                            .when(!self.mention_picker.query.is_empty(), |row| {
                                row.child(
                                    div()
                                        .min_w_0()
                                        .max_w(px(180.))
                                        .truncate()
                                        .px_1p5()
                                        .py(px(1.))
                                        .rounded_md()
                                        .bg(Color::Inset.get(cx))
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_size(px(11.))
                                        .text_color(Color::Muted.get(cx))
                                        .child(format!("@{}", self.mention_picker.query)),
                                )
                            }),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_1()
                            .child(
                                Button::new("open-full-project-file-picker")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .px_2()
                                    .rounded_lg()
                                    .icon(Glyph::FolderOpen)
                                    .label(tr("浏览全部"))
                                    .accessibility_label(tr("引用项目文件"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.mention_picker.close();
                                        this.open_project_file_picker(window, cx);
                                    })),
                            )
                            .child(
                                Button::new("close-inline-mentions")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .w(px(26.))
                                    .rounded_lg()
                                    .icon(Glyph::X)
                                    .tooltip(tr("关闭"))
                                    .accessibility_label(tr("关闭"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mention_picker.close();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .when(self.mention_picker.loading, |view| {
                view.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("正在搜索…")),
                )
            })
            .when_some(
                self.mention_picker.error.as_ref().map(FileError::text),
                |view, error| {
                    view.child(
                        div()
                            .mx_3()
                            .my_2()
                            .p_2p5()
                            .rounded_lg()
                            .bg(Color::ErrorSurface.get(cx))
                            .text_xs()
                            .text_color(Color::Error.get(cx))
                            .child(error),
                    )
                },
            )
            .when(
                !self.mention_picker.loading
                    && self.mention_picker.error.is_none()
                    && paths.is_empty(),
                |view| {
                    view.child(
                        div()
                            .px_4()
                            .py_5()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("没有匹配的项目文件")),
                    )
                },
            )
            .when(!paths.is_empty(), |view| {
                view.child(
                    div().w_full().min_w_0().flex().child(Viewport::new(
                        "inline-mention-results-viewport",
                        &self.mention_picker.scroll,
                        v_flex()
                            .id("inline-mention-results")
                            .w_full()
                            .flex_1()
                            .min_w_0()
                            .max_h(px(220.))
                            .p_1p5()
                            .gap_0p5()
                            .overflow_y_scroll()
                            .track_scroll(&self.mention_picker.scroll)
                            .vertical_scrollbar(&self.mention_picker.scroll)
                            .children(paths.iter().enumerate().map(|(i, path)| {
                                let active = i == self.mention_picker.selected;
                                let path_click = path.clone();
                                Reveal::new(
                                    ("inline-mention-reveal", i),
                                    &self.mention_picker.scroll,
                                    Button::new(("inline-mention", i))
                                        .custom(crate::palette::sidebar_item_variant(active, cx))
                                        .compact()
                                        .w_full()
                                        .min_w_0()
                                        .h(px(30.))
                                        .px_2p5()
                                        .rounded_lg()
                                        .justify_start()
                                        .selected(active)
                                        .accessibility_label(format!(
                                            "{}: {path}",
                                            tr("引用项目文件")
                                        ))
                                        .tooltip(path.clone())
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_center()
                                                .justify_between()
                                                .gap_2()
                                                .child(
                                                    h_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .items_center()
                                                        .gap_2()
                                                        .child(
                                                            Icon::new(Glyph::FileText)
                                                                .size(px(13.))
                                                                .text_color(Color::Muted.get(cx))
                                                                .flex_shrink_0(),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .truncate()
                                                                .font_family(
                                                                    Theme::global(cx)
                                                                        .mono_font_family
                                                                        .clone(),
                                                                )
                                                                .text_xs()
                                                                .text_color(Color::Text.get(cx))
                                                                .child(path.clone()),
                                                        ),
                                                )
                                                .when(active, |row| {
                                                    row.child(
                                                        h_flex()
                                                            .flex_shrink_0()
                                                            .items_center()
                                                            .gap_1()
                                                            .child(crate::palette::keycap(
                                                                "Tab", cx,
                                                            ))
                                                            .child(
                                                                Icon::new(Glyph::CornerDownLeft)
                                                                    .size(px(13.))
                                                                    .text_color(
                                                                        Color::Accent.get(cx),
                                                                    )
                                                                    .flex_shrink_0(),
                                                            ),
                                                    )
                                                }),
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.accept_inline_mention(
                                                path_click.clone(),
                                                window,
                                                cx,
                                            );
                                        })),
                                )
                            })),
                    )),
                )
            })
            .child(crate::palette::overlay_footer(
                &[("↑↓", "选择"), ("Tab / Enter", "引用文件"), ("Esc", "关闭")],
                cx,
            ))
            .into_any_element()
    }
}
impl HiveApp {
    pub(crate) fn can_reference_project_file(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && self.runtime.is_some()
            && self.workspace.is_some()
            && self.draft_target.is_some()
            && self.workspace_writes_allowed()
            && !self.conversation_is_read_only()
            && !self
                .session
                .as_ref()
                .is_some_and(|session| self.inflight_inputs.contains_key(&session.id))
            && self.attachments.ready(self.draft_target.as_ref())
            && self.attachments.refs(self.draft_target.as_ref()).len() < 10
    }
    fn reference_project_file(
        &mut self,
        target: &Target,
        path: String,
        expected_sha256: Option<String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !target.current(self) || !self.can_reference_project_file() {
            return false;
        }
        self.import_image_sources(
            vec![ImageSource::ProjectFile {
                relative_path: path,
                session_id: target.session.clone(),
                expected_sha256,
            }],
            cx,
        );
        true
    }

    pub(crate) fn open_project_file_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_reference_project_file() || window.has_active_dialog(cx) {
            return;
        }
        let (Some(target), Some(runtime)) = (Target::capture(self), self.runtime.clone()) else {
            return;
        };
        let session = self.session.as_ref().filter(|session| session.workspace_id == target.workspace);
        let source_directory = if let Some(session) = session.filter(|session| session.scratch) {
            session.execution_path.as_ref().map(|path| format!("{}: {path}", tr("临时会话目录")))
        } else {
            session.and_then(|session| session.execution.as_ref())
                .filter(|execution| execution.kind == "worktree")
                .map(|execution| format!("{}: {}", tr("独立工作树执行目录"), execution.path))
                .or_else(|| self.workspaces.iter().find(|project| project.id == target.workspace)
                    .map(|project| format!("{}: {}", tr("项目路径"), project.path)))
        };
        let app = cx.weak_entity();
        let picker = cx.new(|cx| {
            let query =
                cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索项目相对路径…")));
            cx.subscribe(
                &query,
                |this: &mut FilePicker, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.offset = 0;
                        this.load(true, cx);
                    }
                },
            )
            .detach();
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            }
            let mut picker = FilePicker {
                app,
                runtime,
                target,
                source_directory,
                query,
                generation: 0,
                page: Page::default(),
                offset: 0,
                selected: 0,
                loading: false,
                error: None,
                scroll: ScrollHandle::new(),
            };
            picker.load(false, cx);
            picker
        });
        let focus = picker.read(cx).query.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(tr("引用项目文件"))
                .w(px(620.))
                .on_ok(|_, _, _| false)
                .child(picker.clone())
        });
        let dialog_focus = window.focused(cx);
        window.on_next_frame(move |window, cx| {
            if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                focus.focus(window, cx);
            }
        });
    }
}
impl FilePicker {
    fn current(&self, cx: &App) -> bool {
        self.app
            .upgrade()
            .is_some_and(|app| self.target.current(app.read(cx)))
    }
    fn load(&mut self, debounce: bool, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        self.page = Page::default();
        self.selected = 0;
        self.error = None;
        self.scroll = ScrollHandle::new();
        self.loading = true;
        cx.notify();
        let runtime = self.runtime.clone();
        let workspace = self.target.workspace.clone();
        let session = self.target.session.clone();
        let query = self.query.read(cx).value().to_string();
        let offset = self.offset;
        cx.spawn(async move |this, cx| {
            if debounce {
                smol::Timer::after(Duration::from_millis(150)).await;
            }
            if this
                .update(cx, |this, cx| {
                    this.generation == generation && this.current(cx)
                })
                .ok()
                != Some(true)
            {
                return;
            }
            let mut params = json!({"workspaceId":workspace,"query":query,"offset":offset});
            if let Some(session) = session {
                params["sessionId"] = json!(session);
            }
            let result = runtime
                .request(
                    "project.files.search",
                    params,
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<Page>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation || !this.current(cx) {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(page) => {
                        if page.total > 0 && this.offset >= page.total && page.paths.is_empty() {
                            this.offset = ((page.total - 1) / 100) * 100;
                            this.load(false, cx);
                            return;
                        }
                        if page.total == 0 {
                            this.offset = 0;
                        }
                        this.page = page;
                    }
                    Err(error) => this.error = Some(FileError::Request(error)),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn select(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading || !self.current(cx) {
            return;
        }
        let accepted = self
            .app
            .update(cx, |app, cx| app.reference_project_file(&self.target, path, None, cx))
            .unwrap_or(false);
        if accepted {
            let composer_focus = self
                .app
                .upgrade()
                .map(|app| app.read(cx).input.read(cx).focus_handle(cx));
            window.close_dialog(cx);
            if let Some(focus) = composer_focus {
                window.on_next_frame(move |window, cx| focus.focus(window, cx));
            }
        } else {
            self.error = Some(FileError::Owned("当前草稿暂不能添加引用，请关闭后重试。"));
            cx.notify();
        }
    }
    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control
            || modifiers.platform
            || modifiers.alt
            || (modifiers.shift && event.keystroke.key.as_str() != "tab")
            || self.query.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        {
            return;
        }
        match event.keystroke.key.as_str() {
            "up" | "down" | "tab" if !self.page.paths.is_empty() => {
                let len = self.page.paths.len();
                let forward = event.keystroke.key == "down"
                    || (event.keystroke.key == "tab" && !modifiers.shift);
                self.selected = if forward {
                    (self.selected + 1) % len
                } else {
                    (self.selected + len - 1) % len
                };
                self.scroll.scroll_to_item(self.selected);
                cx.notify();
            }
            "escape" => {
                let composer_focus = self
                    .app
                    .upgrade()
                    .map(|app| app.read(cx).input.read(cx).focus_handle(cx));
                window.close_dialog(cx);
                if let Some(focus) = composer_focus {
                    window.on_next_frame(move |window, cx| focus.focus(window, cx));
                }
            }
            "enter" => {
                if let Some(path) = self.page.paths.get(self.selected).cloned() {
                    self.select(path, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}
impl Render for FilePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.current(cx);
        let keys = cx.entity();
        let height = px(340.).min(window.viewport_size().height * 0.45);
        v_flex()
            .min_w_0()
            .gap_2()
            .when_some(self.source_directory.as_ref(), |view, source| {
                view.child(crate::text_reading::selectable_text("project-file-source-directory", source.clone()).text_xs())
            })
            .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr(
                "引用保存文件当前内容的副本。以后修改或删除源文件，不会改变已保存的快照。",
            )))
            .child(
                div()
                    .id("project-file-search-input")
                    .capture_key_down(move |event, window, cx| {
                        keys.update(cx, |this, cx| this.key(event, window, cx))
                    })
                    .child(
                        Input::new(&self.query)
                            .cleanable(true)
                            .aria_label(tr("搜索项目文件")),
                    ),
            )
            .when(!current, |view| {
                view.child(
                    div()
                        .id("project-file-context-changed")
                        .role(Role::Status)
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(tr("项目、会话或后台连接已变化，请关闭后重新选择。")),
                )
            })
            .when_some(self.error.as_ref(), |view, error| {
                view.child(
                    v_flex()
                        .gap_1()
                        .child(
                            div()
                                .id("project-file-search-error")
                                .role(Role::Status)
                                .text_sm()
                                .text_color(Color::Error.get(cx))
                                .child(error.text()),
                        )
                        .child(
                            Button::new("retry-project-file-search")
                                .outline()
                                .small()
                                .label(tr("重试"))
                                .disabled(!current || self.loading)
                                .on_click(cx.listener(|this, _, _, cx| this.load(false, cx))),
                        ),
                )
            })
            .child(
                div().h(height).min_h_0().flex().child(Viewport::new(
                    "project-file-viewport",
                    &self.scroll,
                    v_flex()
                        .id("project-file-results")
                        .role(Role::List)
                        .aria_label(tr("项目文件搜索结果"))
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .vertical_scrollbar(&self.scroll)
                        .when(self.loading && current, |list| {
                            list.child(div().p_2().text_sm().child(tr("正在搜索…")))
                        })
                        .when(
                            !self.loading && self.error.is_none() && self.page.paths.is_empty(),
                            |list| {
                                list.child(div().p_2().text_sm().child(tr("没有匹配的项目文件")))
                            },
                        )
                        .children(self.page.paths.iter().enumerate().map(|(i, path)| {
                            let path = path.clone();
                            let active = i == self.selected;
                            Reveal::new(
                                ("project-file-reveal", i),
                                &self.scroll,
                                Button::new(("project-file", i))
                                    .custom(crate::palette::sidebar_item_variant(active, cx))
                                    .compact()
                                    .w_full()
                                    .min_w_0()
                                    .rounded_lg()
                                    .justify_start()
                                    .selected(active)
                                    .disabled(!current)
                                    .accessibility_label(format!("{}: {path}", tr("引用项目文件")))
                                    .tooltip(path.clone())
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .child(div().flex_1().min_w_0().truncate().child(path.clone()))
                                            .when(active, |row| {
                                                row.child(crate::palette::keycap("Tab / Enter", cx))
                                            }),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.select(path.clone(), window, cx)
                                    })),
                            )
                        })),
                )),
            )
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(format!(
                                        "{} / {}",
                                        self.offset + self.page.paths.len(),
                                        self.page.total
                                    )),
                            )
                            .child(crate::palette::keycap("↑↓", cx))
                            .child(crate::palette::keycap("Tab", cx))
                            .child(crate::palette::keycap("Esc", cx)),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .child(
                                Button::new("project-file-prev")
                                    .outline()
                                    .small()
                                    .label(tr("上一页"))
                                    .disabled(!current || self.loading || self.offset == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.offset = this.offset.saturating_sub(100);
                                        this.load(false, cx);
                                    })),
                            )
                            .child(
                                Button::new("project-file-next")
                                    .outline()
                                    .small()
                                    .label(tr("下一页"))
                                    .disabled(
                                        !current
                                            || self.loading
                                            || self.offset + self.page.paths.len()
                                                >= self.page.total,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.offset += 100;
                                        this.load(false, cx);
                                    })),
                            ),
                    ),
            )
    }
}

struct ReferenceDetails {
    app: WeakEntity<HiveApp>,
    item: AttachmentRef,
    connection: u64,
    loading: bool,
    state: Option<SourceState>,
    error: Option<FileError>,
}
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum SourceState {
    Unchanged,
    Changed,
    Missing,
}
impl HiveApp {
    fn can_reveal_project_file_reference(&self, item: &AttachmentRef) -> bool {
        let Some(reference) = &item.project_file else {
            return false;
        };
        if !self.can_browse_project_files() || self.workspace.as_ref() != Some(&item.workspace_id) {
            return false;
        }
        let session = self.session.as_ref().filter(|session| session.workspace_id == item.workspace_id);
        match reference.source_session_id.as_deref() {
            Some(source) => session.is_some_and(|session| session.id == source),
            None => session.is_none_or(|session| {
                session.execution_path.as_deref().is_some_and(|directory| {
                    self.workspaces.iter().any(|workspace| {
                        workspace.id == item.workspace_id && workspace.path == directory
                    })
                })
            }),
        }
    }

    pub(crate) fn open_project_file_details(
        &mut self,
        item: AttachmentRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if item.project_file.is_none() || window.has_active_dialog(cx) {
            return;
        }
        let app = cx.weak_entity();
        let connection = self.connection_generation;
        let details = cx.new(|cx| {
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |_, _, cx| cx.notify()).detach();
            }
            ReferenceDetails {
                app,
                item,
                connection,
                loading: false,
                state: None,
                error: None,
            }
        });
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(tr("项目文件快照"))
                .w(px(620.))
                .child(details.clone())
        });
    }
}
impl ReferenceDetails {
    fn current(&self, cx: &App) -> bool {
        self.app.upgrade().is_some_and(|app| {
            let app = app.read(cx);
            app.connected
                && app.connection_generation == self.connection
                && app.workspace.as_ref() == Some(&self.item.workspace_id)
        })
    }

    fn reveal_source(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let _ = self.app.update(cx, |app, cx| {
            if !app.can_reveal_project_file_reference(&self.item) {
                return;
            }
            let path = self.item.project_file.as_ref().expect("validated project reference").relative_path.clone();
            window.close_dialog(cx);
            app.reveal_project_file(path, window, cx);
        });
    }
    fn check(&mut self, cx: &mut Context<Self>) {
        if self.loading || !self.current(cx) {
            return;
        }
        let Some(runtime) = self
            .app
            .upgrade()
            .and_then(|app| app.read(cx).runtime.clone())
        else {
            return;
        };
        self.loading = true;
        self.state = None;
        self.error = None;
        cx.notify();
        let item = self.item.clone();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "project.files.status",
                    json!({"workspaceId":item.workspace_id,"id":item.id}),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<SourceState>(value["state"].clone())
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                if !this.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(state) => this.state = Some(state),
                    Err(error) => this.error = Some(FileError::Request(error)),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for ReferenceDetails {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reference = self.item.project_file.as_ref().expect("project reference");
        let current = self.current(cx);
        let item = self.item.clone();
        let app = self.app.clone();
        let can_reveal = current && app.upgrade().is_some_and(|app| app.read(cx).can_reveal_project_file_reference(&item));
        let state = match self.state {
            Some(SourceState::Unchanged) => tr("源文件与快照一致"),
            Some(SourceState::Changed) => tr("源文件已更改，发送仍使用保存的快照"),
            Some(SourceState::Missing) => tr("源文件已删除，保存的快照仍可使用"),
            None => tr("尚未检查源文件"),
        };
        let source = reference.source_session_id.as_deref().map(|id| {
            let title = self.app.upgrade().and_then(|app| {
                let app = app.read(cx);
                app.session.iter().chain(app.sessions.iter())
                    .find(|session| session.id == id && session.workspace_id == self.item.workspace_id)
                    .map(|session| session.title.clone())
            });
            format!("{}: {}", tr("来源会话"), title.as_deref().unwrap_or(id))
        });
        let state = if current { state } else { tr("项目或后台连接已变化，请重新打开快照详情。") };
        v_flex()
            .min_w_0()
            .gap_3()
            .when_some(source, |view, source| {
                view.child(crate::text_reading::selectable_text("project-reference-source", source))
            })
            .child(crate::text_reading::selectable_text(
                "project-reference-path",
                reference.relative_path.clone(),
            ))
            .child(crate::text_reading::selectable_text(
                "project-reference-hash",
                format!("SHA-256: {}", reference.sha256),
            ))
            .child(div().text_sm().text_color(Color::Muted.get(cx)).child(tr(
                "引用保存文件当前内容的副本。以后修改或删除源文件，不会改变已保存的快照。",
            )))
            .child(
                div()
                    .id("project-reference-status")
                    .role(Role::Status)
                    .aria_label(state)
                    .text_sm()
                    .child(state),
            )
            .when_some(self.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error.text()),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("check-project-reference")
                            .outline()
                            .label(if self.loading {
                                tr("检查中…")
                            } else {
                                tr("检查源文件")
                            })
                            .disabled(!current || self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.check(cx))),
                    )
                    .child(
                        Button::new("preview-project-snapshot")
                            .outline()
                            .label(tr("预览附件副本"))
                            .disabled(!current)
                            .on_click(move |_, window, cx| {
                                let _ = app.update(cx, |app, cx| {
                                    app.open_attachment_preview(item.clone(), window, cx)
                                });
                            }),
                    )
                    .when(can_reveal, |actions| actions.child(
                        Button::new("reveal-project-reference-source")
                            .outline()
                            .label(tr("在文件面板中定位"))
                            .on_click(cx.listener(|this, _, window, cx| this.reveal_source(window, cx))),
                    )),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{MentionQuery, mention_token_at_cursor};

    #[test]
    fn mention_token_at_cursor_parses_boundary_cases() {
        assert_eq!(
            mention_token_at_cursor("@", 1),
            Some(MentionQuery {
                start: 0,
                end: 1,
                query: String::new(),
            })
        );
        assert_eq!(
            mention_token_at_cursor("check @src/app.rs", 17),
            Some(MentionQuery {
                start: 6,
                end: 17,
                query: "src/app.rs".into(),
            })
        );
        assert_eq!(
            mention_token_at_cursor("line1\n@readme", 13),
            Some(MentionQuery {
                start: 6,
                end: 13,
                query: "readme".into(),
            })
        );
        assert_eq!(mention_token_at_cursor("user@example.com", 16), None);
        assert_eq!(mention_token_at_cursor("@foo bar", 8), None);
        assert_eq!(mention_token_at_cursor("@foo bar", 4), Some(MentionQuery {
            start: 0,
            end: 4,
            query: "foo".into(),
        }));
        assert_eq!(mention_token_at_cursor("/cmd", 4), None);
        assert_eq!(mention_token_at_cursor("/@foo", 5), None);
        assert_eq!(
            mention_token_at_cursor("@foo\n/cmd", 4),
            Some(MentionQuery {
                start: 0,
                end: 4,
                query: "foo".into(),
            })
        );
        let text = "测试 @文件.rs";
        assert_eq!(
            mention_token_at_cursor(text, text.len()),
            Some(MentionQuery {
                start: "测试 ".len(),
                end: text.len(),
                query: "文件.rs".into(),
            })
        );
    }
}
