use super::{FileError, Target};
mod content_results;
mod image_preview;
mod markdown_links;
mod reader_find;
mod search_snippet;
use crate::{
    app::HiveApp,
    git_workspace::{
        FILE_TREE_STATUS_LIMIT,
        file_tree::{ChangeMark, FileTreeChanges},
    },
    language::tr,
    open_targets::{self, OpenTarget, ResolvedEditorFile},
    palette::{self, Color},
    text_reading::{self, TextPages},
    transport::Runtime,
};
use content_results::{ContentQuery, ContentResults, Pick as ContentPick, Row as ContentRow};
use gpui_kit::{
    assets::IconName as Glyph,
    base::InteractiveElementExt as _,
    component::{
        button::{Button, ButtonVariants},
        dialog::Cancel,
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenuItem},
        resizable::{HandleEdge, resize_handle},
        scroll::ScrollableElement,
        text::{TextView, TextViewState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use image_preview::{ExpandedImageViewport, ImageViewport, ZoomAction};
use markdown_links::{MarkdownHeadings, is_markdown_file, relative_markdown_target};
use reader_find::ReaderFind;
use search_snippet::{SearchSnippet, path_highlights};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    ops::Range,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, OnceLock},
    time::Duration,
};

const PAGE_SIZE: usize = 100;
const TREE_PREVIEW_DELAY: Duration = Duration::from_millis(120);
const MARKDOWN_MAX_BYTES: usize = 64 * 1024;
const MARKDOWN_MAX_BLOCK_BYTES: usize = 8 * 1024;
const MARKDOWN_MAX_LINES: usize = 1000;
const DOCK_WIDTH: f32 = 320.;
const CHAT_MIN_WIDTH: f32 = 600.;
const DOCK_WIDTH_STEP: f32 = 24.;
const FILE_SPLIT_HANDLE: f32 = 8.;
const FILE_SPLIT_STEP: f32 = 24.;
const FILE_SPLIT_MIN_LIST: f32 = 104.;
const FILE_SPLIT_MIN_PREVIEW: f32 = 148.;
const COMPACT_PREVIEW_CONTEXT: &str = "HiveCompactFilePreview";

fn tree_key_context(has_preview: bool) -> &'static str {
    if has_preview {
        "HiveProjectFileTree HiveCompactFilePreview"
    } else {
        "HiveProjectFileTree"
    }
}

#[derive(Clone, Copy)]
struct FileSplit {
    available: f32,
    minimum: f32,
    maximum: f32,
    list: f32,
}

impl FileSplit {
    fn new(height: Pixels, preferred: Option<u8>, docked: bool) -> Self {
        let available = (f32::from(height) - FILE_SPLIT_HANDLE).max(1.);
        let minimum = FILE_SPLIT_MIN_LIST.min(available / 3.).max(available / 5.);
        let preview_minimum = FILE_SPLIT_MIN_PREVIEW
            .min(available / 3.)
            .max(available / 5.);
        let maximum = available - preview_minimum;
        let share = f32::from(preferred.unwrap_or(if docked { 50 } else { 40 })) / 100.;
        Self {
            available,
            minimum,
            maximum,
            list: (available * share).clamp(minimum, maximum),
        }
    }

    fn percent(self, height: f32) -> u8 {
        (100. * height.clamp(self.minimum, self.maximum) / self.available).round() as u8
    }
}

fn dock_width(preferred: Option<Pixels>, available: Pixels) -> Option<Pixels> {
    let maximum = available - px(CHAT_MIN_WIDTH);
    (maximum >= px(DOCK_WIDTH)).then(|| {
        preferred
            .unwrap_or(px(DOCK_WIDTH))
            .clamp(px(DOCK_WIDTH), maximum)
    })
}

#[derive(Clone)]
struct ResizeFileDock {
    width: Pixels,
    starting_x: Cell<Pixels>,
}

impl Render for ResizeFileDock {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Clone)]
struct ResizeFilePreview {
    list: f32,
    starting_y: Cell<Pixels>,
    generation: u64,
}

impl Render for ResizeFilePreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Deserialize)]
struct DirectoryEntry {
    path: String,
    directory: bool,
}

#[derive(Default)]
struct Directory {
    entries: Vec<DirectoryEntry>,
    loading: bool,
    loaded: bool,
    next_offset: usize,
    total: usize,
    error: Option<FileError>,
}

#[derive(Deserialize)]
struct DirectoryPage {
    entries: Vec<DirectoryEntry>,
    total: usize,
}

struct FileReveal {
    path: String,
}

#[derive(Debug, PartialEq, Eq)]
enum RevealStep<'a> {
    Expand(&'a str),
    Load(&'a str),
    Wait,
    Select,
    Missing,
}

impl FileReveal {
    fn step(
        &self,
        directories: &HashMap<String, Directory>,
        expanded: Option<&HashSet<String>>,
    ) -> RevealStep<'_> {
        if self.path.contains(['\\', '\0'])
            || self
                .path
                .split('/')
                .any(|part| matches!(part, "" | "." | ".." | ".git"))
        {
            return RevealStep::Missing;
        }
        let mut parent = "";
        for end in self
            .path
            .match_indices('/')
            .map(|(index, _)| index)
            .chain([self.path.len()])
        {
            let target = &self.path[..end];
            let last = end == self.path.len();
            let expects_directory = !last;
            let Some(folder) = directories.get(parent) else {
                return RevealStep::Load(parent);
            };
            if let Some(entry) = folder.entries.iter().find(|entry| entry.path == target) {
                if entry.directory != expects_directory {
                    return RevealStep::Missing;
                }
                if expects_directory && !expanded.is_some_and(|expanded| expanded.contains(target))
                {
                    return RevealStep::Expand(target);
                }
                parent = target;
            } else if folder.loading || folder.error.is_some() {
                return RevealStep::Wait;
            } else if !folder.loaded || folder.next_offset < folder.total {
                return RevealStep::Load(parent);
            } else {
                return RevealStep::Missing;
            }
        }
        RevealStep::Select
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct Match {
    path: String,
    line: Option<usize>,
}

#[derive(Deserialize)]
struct SearchHit {
    #[serde(flatten)]
    selection: Match,
    #[serde(default)]
    snippet: Option<SearchSnippet>,
    #[serde(skip)]
    path_highlights: Vec<Range<usize>>,
}

#[derive(Default, Deserialize)]
struct SearchPage {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default, rename = "ranges")]
    path_ranges: Value,
    #[serde(default)]
    matches: Vec<SearchHit>,
    total: usize,
    #[serde(default)]
    limited: bool,
}

impl SearchPage {
    fn append_name_paths(&mut self) {
        let mut ranges = match std::mem::take(&mut self.path_ranges) {
            Value::Array(entries) if entries.len() == self.paths.len() => entries.into_iter(),
            _ => Vec::new().into_iter(),
        };
        self.matches.extend(self.paths.drain(..).map(|path| {
            SearchHit {
                path_highlights: ranges
                    .next()
                    .map_or_else(Vec::new, |ranges| path_highlights(&path, ranges)),
                selection: Match { path, line: None },
                snippet: None,
            }
        }));
    }
}

struct Preview {
    selection: Match,
    execution_directory: Option<String>,
    loading: bool,
    text: Option<TextPages>,
    image: Option<ImageViewport>,
    sha256: Option<String>,
    markdown: Option<MarkdownPreview>,
    error: Option<FileError>,
    scroll: ScrollHandle,
    editor_task: Option<Task<()>>,
    editor_error: Option<String>,
    path_request: Option<FilePathRequest>,
    path_status: Option<FilePathStatus>,
}

impl Preview {
    fn loaded(&self) -> bool {
        self.text.is_some() || self.image.is_some()
    }

    fn reading_position(&self) -> Option<PreviewReadingPosition> {
        Some(PreviewReadingPosition {
            page: self.text.as_ref()?.page,
            source_scroll: self.scroll.offset(),
            markdown: self
                .markdown
                .as_ref()
                .map(|markdown| (markdown.mode, markdown.scroll_offset())),
        })
    }
}

#[derive(Clone, Copy)]
struct PreviewReadingPosition {
    page: usize,
    source_scroll: Point<Pixels>,
    markdown: Option<(FilePreviewMode, Point<Pixels>)>,
}

impl PreviewReadingPosition {
    fn restore_page(self, text: &mut TextPages) {
        text.show(self.page.min(text.count() - 1));
    }
}

#[derive(Clone)]
struct RememberedPreview {
    owner: EntityId,
    tree: TreeScope,
    selection: Match,
    directory: Option<String>,
    reading: Option<PreviewReadingPosition>,
}

impl RememberedPreview {
    fn in_tree(&self, tree: &TreeScope) -> bool {
        self.tree == *tree && self.directory.is_some()
    }
}

enum PreparedPreview {
    Text(TextPages),
    Image(Arc<RenderImage>),
}

struct MarkdownNavigation {
    execution_directory: String,
    fragment: Option<String>,
}

enum PreviewRead {
    Open(Option<MarkdownNavigation>),
    Restore(RememberedPreview),
    Tree,
}

struct FilePathRequest {
    reader: Option<WeakEntity<ExpandedFilePreview>>,
    request_id: Rc<()>,
    _task: Task<()>,
}

#[derive(Clone, Copy)]
enum FilePathAction {
    Reveal,
    CopyAbsolute,
    CopyRelative,
}

#[derive(Clone, Copy)]
enum FilePathStatus {
    RevealFailed,
    CopyFailed,
    AbsoluteCopied,
    RelativeCopied,
}

enum TreePreview {
    Waiting { _task: Task<()> },
    Reading { previous: Option<Preview> },
}

#[derive(Clone, PartialEq, Eq)]
struct PreviewIdentity {
    scope: Scope,
    preview_generation: u64,
    selection: Match,
}

impl PreviewIdentity {
    fn file_params(&self) -> Value {
        self.scope
            .params(json!({"relativePath":self.selection.path}))
    }
}

#[derive(Clone)]
enum FileDiffTarget {
    Tree(String),
    Preview {
        identity: PreviewIdentity,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
    },
}

impl FileDiffTarget {
    fn path(&self) -> &str {
        match self {
            Self::Tree(path) => path,
            Self::Preview { identity, .. } => &identity.selection.path,
        }
    }

    fn matches_preview(&self, current: Option<&PreviewIdentity>) -> bool {
        match self {
            Self::Tree(_) => true,
            Self::Preview { identity, .. } => current == Some(identity),
        }
    }
}

struct PreviewAttachment {
    identity: PreviewIdentity,
    target: Target,
    sha256: String,
}

struct ExpandedFilePreview {
    browser: WeakEntity<FileBrowser>,
    app: WeakEntity<HiveApp>,
    scope: Scope,
    scope_generation: u64,
    preview_generation: u64,
    selection: Match,
    text: Option<TextPages>,
    image: Option<ExpandedImageViewport>,
    markdown: Option<MarkdownPreview>,
    markdown_start_offset: Option<Point<Pixels>>,
    scroll: ScrollHandle,
    find: Option<ReaderFind>,
    copied_text: Option<Task<()>>,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    dialog_focus: Option<FocusHandle>,
    dismissed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilePreviewMode {
    Source,
    Markdown,
}

struct MarkdownPreview {
    mode: FilePreviewMode,
    document: Option<Entity<TextViewState>>,
    headings: Arc<OnceLock<MarkdownHeadings>>,
    scroll: ScrollHandle,
    pending_scroll: Cell<Option<Point<Pixels>>>,
}

impl MarkdownPreview {
    fn new(mode: FilePreviewMode, text: &TextPages, cx: &mut App) -> Self {
        let mut preview = Self {
            mode: FilePreviewMode::Source,
            document: None,
            headings: Arc::default(),
            scroll: ScrollHandle::new(),
            pending_scroll: Cell::new(None),
        };
        preview.switch(mode, text, cx);
        preview
    }

    fn switch(&mut self, mode: FilePreviewMode, text: &TextPages, cx: &mut App) {
        if mode == FilePreviewMode::Markdown && self.document.is_none() {
            self.document = Some(cx.new(|cx| TextViewState::markdown(&text.full, cx)));
        }
        self.mode = mode;
    }

    fn active(&self) -> bool {
        self.mode == FilePreviewMode::Markdown
    }

    fn scroll_offset(&self) -> Point<Pixels> {
        self.pending_scroll
            .get()
            .unwrap_or_else(|| self.scroll.offset())
    }

    fn reveal_initial_heading(&self, fragment: String, cx: &mut App) {
        let Some(document) = &self.document else {
            return;
        };
        let headings = self.headings.clone();
        document.update(cx, |document, cx| {
            if !document.rendered_text().source().is_empty() {
                Self::reveal_heading(document, &headings, &fragment, cx);
                return;
            }

            // Larger documents are parsed in the background. Wait for this
            // document's parser once before looking up the heading.
            let mut pending = Some(fragment);
            cx.observe_self(move |document, cx| {
                if let Some(fragment) =
                    pending.take_if(|_| !document.rendered_text().source().is_empty())
                {
                    Self::reveal_heading(document, &headings, &fragment, cx);
                }
            })
            .detach();
        });
    }

    fn reveal_heading(
        document: &mut TextViewState,
        headings: &OnceLock<MarkdownHeadings>,
        fragment: &str,
        cx: &mut Context<TextViewState>,
    ) {
        let rendered = document.rendered_text();
        let headings = headings.get_or_init(|| MarkdownHeadings::parse(rendered.source()));
        if let Some(range) = headings.range(fragment) {
            Self::reveal_heading_range(document, range, cx);
        }
    }

    fn reveal_heading_range(
        document: &mut TextViewState,
        source_range: Range<usize>,
        cx: &mut Context<TextViewState>,
    ) {
        if let Some(range) = document.rendered_text().range_for_source(source_range) {
            let _ = document.reveal_range(range, cx);
        }
    }

    fn render(
        &self,
        id: &'static str,
        open_project_link: impl Fn(&str, &mut Window, &mut App) + Send + Sync + 'static,
        cx: &App,
    ) -> Stateful<Div> {
        let document = self.document.as_ref().expect("active Markdown document");
        let document_read = document.read(cx).rendered_text();
        if !document_read.source().is_empty()
            && let Some(offset) = self.pending_scroll.take()
        {
            self.scroll.set_offset(offset);
        }
        let heading_document = document.downgrade();
        let headings = self.headings.clone();
        let scroll = self.scroll.clone();
        div()
            .id(id)
            .min_w_0()
            .role(Role::Label)
            .aria_value(document_read.as_str().to_owned())
            .child(
                TextView::new(document)
                    .selectable(true)
                    .on_reveal(move |line, window, _| {
                        let offset = scroll.offset();
                        let top = scroll.bounds().top() + px(8.);
                        let next =
                            (offset.y + top - line.top()).clamp(-scroll.max_offset().y, px(0.));
                        if next != offset.y {
                            let scroll = scroll.clone();
                            window.on_next_frame(move |window, _| {
                                scroll.set_offset(point(offset.x, next));
                                window.refresh();
                            });
                        }
                    })
                    // Block all document images, including HTML and data URLs.
                    // A terminal result also avoids GPUI's pending-image redraw timer.
                    .image_source(|_| {
                        ImageSource::Custom(Arc::new(|_, _| {
                            Some(Err(ImageCacheError::Asset(
                                "Project Markdown images are disabled".into(),
                            )))
                        }))
                    })
                    .on_link_click(move |destination, event, window, cx| {
                        let activate = match event {
                            ClickEvent::Mouse(click) => {
                                matches!(click.up.button, MouseButton::Left | MouseButton::Middle)
                            }
                            ClickEvent::Keyboard(_) => true,
                            ClickEvent::Touch(click) => !click.long_press,
                        };
                        if !activate {
                            return;
                        }
                        if let Some(fragment) = destination.strip_prefix('#') {
                            let _ = heading_document.update(cx, |document, cx| {
                                Self::reveal_heading(document, &headings, fragment, cx);
                            });
                        } else if let Some(url) = external_markdown_url(destination) {
                            cx.open_url(url.as_str());
                        } else {
                            open_project_link(destination, window, cx);
                        }
                    }),
            )
    }
}

fn supports_markdown_preview(text: &str) -> bool {
    if text.is_empty() || text.len() > MARKDOWN_MAX_BYTES {
        return false;
    }
    let bytes = text.as_bytes();
    // CommonMark treats LF, CRLF and CR as physical line endings.
    let endings = bytes
        .iter()
        .enumerate()
        .filter_map(|(index, byte)| {
            (*byte == b'\n' || *byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'))
                .then_some(index)
        })
        .chain(std::iter::once(bytes.len()));
    let mut line_start = 0;
    let mut block_start = 0;
    for (line, line_end) in endings.enumerate() {
        if line_start == bytes.len() {
            break;
        }
        if line >= MARKDOWN_MAX_LINES {
            return false;
        }
        // A paragraph with only soft line breaks can be shaped as a single run.
        if bytes[line_start..line_end]
            .iter()
            .all(u8::is_ascii_whitespace)
        {
            block_start = line_end + 1;
        } else if line_end - block_start > MARKDOWN_MAX_BLOCK_BYTES {
            return false;
        }
        line_start = line_end + 1;
    }
    true
}

fn initial_markdown_mode(selection: &Match) -> FilePreviewMode {
    if selection.line.is_some() {
        FilePreviewMode::Source
    } else {
        FilePreviewMode::Markdown
    }
}

fn external_markdown_url(destination: &str) -> Option<url::Url> {
    let url = url::Url::parse(destination).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.has_host()).then_some(url)
}

fn markdown_mode_controls<T: 'static>(
    current: FilePreviewMode,
    set_mode: fn(&mut T, FilePreviewMode, &mut Context<T>),
    cx: &Context<T>,
) -> impl IntoElement {
    h_flex().gap_1().children(
        [
            (
                "show-markdown-preview",
                tr("预览"),
                FilePreviewMode::Markdown,
            ),
            ("show-markdown-source", tr("源码"), FilePreviewMode::Source),
        ]
        .into_iter()
        .map(|(id, label, mode)| {
            Button::new(id)
                .ghost()
                .small()
                .label(label)
                .selected(mode == current)
                .on_click(cx.listener(move |this, _, _, cx| set_mode(this, mode, cx)))
        }),
    )
}

fn markdown_limit_notice(cx: &App) -> Stateful<Div> {
    let notice = tr("此 Markdown 超出排版预览限制，正在显示源码。");
    div()
        .id("markdown-preview-limit")
        .role(Role::Status)
        .aria_label(notice)
        .text_xs()
        .text_color(Color::Muted.get(cx))
        .child(notice)
}

fn is_expand_preview_chord(keystroke: &Keystroke, mac: bool) -> bool {
    keystroke.key == "enter"
        && keystroke.modifiers
            == (Modifiers {
                platform: mac,
                control: !mac,
                ..Modifiers::default()
            })
}

fn is_close_preview_key(event: &KeyDownEvent) -> bool {
    !event.is_held
        && !event.prefer_character_input
        && event.keystroke.key == "escape"
        && event.keystroke.modifiers == Modifiers::default()
}

fn expand_preview_hint(cx: &App) -> String {
    let shortcut = if cx.platform_info().is_mac() {
        "Cmd+Enter"
    } else {
        "Ctrl+Enter"
    };
    format!("{} ({shortcut})", tr("展开预览"))
}

fn preview_text(
    id: &'static str,
    text: &TextPages,
    wrap: bool,
    active_match: Option<text_reading::SourceMatch>,
    cx: &App,
) -> Stateful<Div> {
    let appearance = cx.global::<crate::appearance::Appearance>();
    let font = appearance
        .code_font_family()
        .font_family_name()
        .map(SharedString::from)
        .unwrap_or_else(|| Theme::global(cx).mono_font_family.clone());
    let view = div()
        .id(SharedString::from(format!("{id}-container")))
        .text_sm()
        .font_family(font)
        // Let the source's intrinsic width reach the two-axis scroll container.
        .when(!wrap, |view| {
            view.whitespace_nowrap().self_start().flex_none()
        });
    if text.full.is_empty() {
        view.child(text_reading::selectable_text(id, tr("此文件为空")))
    } else {
        view.child(text_reading::numbered_source(
            id,
            text,
            wrap,
            active_match,
            cx,
        ))
    }
}

fn step_preview(text: &mut TextPages, scroll: &mut ScrollHandle, step: isize) -> bool {
    let changed = text
        .page
        .checked_add_signed(step)
        .is_some_and(|page| text.show(page));
    if changed {
        *scroll = ScrollHandle::new();
    }
    changed
}

pub(crate) struct ProjectFilePreviewText {
    text: TextPages,
    markdown: Option<MarkdownPreview>,
    markdown_limited: bool,
    scroll: ScrollHandle,
}

impl ProjectFilePreviewText {
    pub(crate) fn new(path: &str, text: TextPages, cx: &mut App) -> Self {
        let markdown_file = !text.full.is_empty() && is_markdown_file(path);
        let markdown = (markdown_file && supports_markdown_preview(&text.full))
            .then(|| MarkdownPreview::new(FilePreviewMode::Markdown, &text, cx));
        Self {
            text,
            markdown_limited: markdown_file && markdown.is_none(),
            markdown,
            scroll: ScrollHandle::new(),
        }
    }

    pub(crate) fn set_mode(&mut self, mode: FilePreviewMode, cx: &mut App) {
        if let Some(markdown) = &mut self.markdown {
            markdown.switch(mode, &self.text, cx);
        }
    }

    pub(crate) fn has_text(&self, text: &TextPages) -> bool {
        self.text.full == text.full
    }

    pub(crate) fn step(&mut self, delta: isize) -> bool {
        !self.markdown.as_ref().is_some_and(MarkdownPreview::active)
            && step_preview(&mut self.text, &mut self.scroll, delta)
    }

    pub(crate) fn view<T: 'static>(
        &self,
        path: &str,
        height: Pixels,
        set_mode: fn(&mut T, FilePreviewMode, &mut Context<T>),
        step: fn(&mut T, isize, &mut Context<T>),
        cx: &Context<T>,
    ) -> impl IntoElement {
        let rendered = self.markdown.as_ref().filter(|markdown| markdown.active());
        let scroll = rendered.map_or(&self.scroll, |markdown| &markdown.scroll);
        v_flex()
            .min_w_0()
            .gap_2()
            .when_some(self.markdown.as_ref(), |view, markdown| {
                view.child(markdown_mode_controls(markdown.mode, set_mode, cx))
            })
            .when(self.markdown_limited, |view| {
                view.child(markdown_limit_notice(cx))
            })
            .child(
                text_reading::region(
                    "git-current-file".into(),
                    path.to_owned(),
                    scroll,
                    height,
                    cx,
                )
                .child(rendered.map_or_else(
                    || preview_text("git-current-source", &self.text, true, None, cx),
                    |markdown| markdown.render("git-current-markdown", |_, _, _| {}, cx),
                )),
            )
            .when(rendered.is_none() && self.text.count() > 1, |view| {
                view.child(
                    h_flex()
                        .justify_end()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("git-current-prev")
                                .ghost()
                                .small()
                                .label(tr("上一页"))
                                .disabled(self.text.page == 0)
                                .on_click(cx.listener(move |this, _, _, cx| step(this, -1, cx))),
                        )
                        .child(div().text_xs().child(format!(
                            "{} / {}",
                            self.text.page + 1,
                            self.text.count()
                        )))
                        .child(
                            Button::new("git-current-next")
                                .ghost()
                                .small()
                                .label(tr("下一页"))
                                .disabled(self.text.page + 1 >= self.text.count())
                                .on_click(cx.listener(move |this, _, _, cx| step(this, 1, cx))),
                        ),
                )
            })
    }
}

fn editor_open_error(error: &str, cx: &App) -> Stateful<Div> {
    div()
        .id("project-file-editor-error")
        .role(Role::Status)
        .text_xs()
        .text_color(Color::Error.get(cx))
        .child(format!(
            "{}: {}",
            tr("无法在编辑器中打开文件"),
            crate::language::localize_owned_error(error)
        ))
}

fn file_path_status(status: FilePathStatus, cx: &App) -> Stateful<Div> {
    let (message, color, id) = match status {
        FilePathStatus::RevealFailed => (
            "无法在文件管理器中显示此文件。",
            Color::Error,
            "project-file-manager-reveal-error",
        ),
        FilePathStatus::CopyFailed => (
            "无法复制此文件的绝对路径。",
            Color::Error,
            "project-file-copy-path-error",
        ),
        FilePathStatus::AbsoluteCopied => {
            ("已复制绝对路径", Color::Muted, "project-file-copied-path")
        }
        FilePathStatus::RelativeCopied => {
            ("已复制相对路径", Color::Muted, "project-file-copied-path")
        }
    };
    div()
        .id(id)
        .role(Role::Status)
        .text_xs()
        .text_color(color.get(cx))
        .child(tr(message))
}

#[derive(Default)]
enum TreeGitStatus {
    #[default]
    Idle,
    Busy,
    Ready(FileTreeChanges),
    NotRepository,
    Failed(String),
}

fn git_mark_appearance(mark: ChangeMark, cx: &App) -> (&'static str, &'static str, Rgba) {
    let diff_palette = crate::diffs_settings::current_palette(cx);
    match mark {
        ChangeMark::Added => ("A", "Git：已添加", diff_palette.added),
        ChangeMark::Deleted => ("D", "Git：已删除", diff_palette.removed),
        ChangeMark::Renamed => ("R", "Git：已重命名", Color::Secondary.get(cx)),
        ChangeMark::Conflicted => ("U", "Git：有冲突", Color::Error.get(cx)),
        ChangeMark::Modified => ("M", "Git：已修改", Color::Accent.get(cx)),
        ChangeMark::Descendant => ("•", "包含 Git 改动", Color::Accent.get(cx)),
    }
}

fn mark_allows_file_diff(mark: ChangeMark) -> bool {
    !matches!(mark, ChangeMark::Deleted | ChangeMark::Descendant)
}

#[derive(Clone, Hash, PartialEq, Eq)]
enum TreeSource {
    Directory(String),
    Session(String),
    Project,
}

impl TreeSource {
    fn new(session: Option<&crate::protocol::Session>, project_path: Option<&str>) -> Self {
        match session {
            Some(session) => session
                .execution_path
                .as_deref()
                .filter(|path| !path.is_empty())
                .or_else(|| {
                    session
                        .execution
                        .as_ref()
                        .filter(|execution| {
                            execution.kind == "worktree" && !execution.path.is_empty()
                        })
                        .map(|execution| execution.path.as_str())
                })
                .map(|path| Self::Directory(path.to_owned()))
                .unwrap_or_else(|| Self::Session(session.id.clone())),
            None => project_path
                .map(|path| Self::Directory(path.to_owned()))
                .unwrap_or(Self::Project),
        }
    }

    fn directory(&self) -> Option<&str> {
        match self {
            Self::Directory(path) => Some(path),
            _ => None,
        }
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct TreeScope {
    workspace: String,
    source: TreeSource,
}

#[derive(Default)]
pub(crate) struct FileTreeExpansions {
    paths: HashMap<TreeScope, HashSet<String>>,
    // The dock and the Files dialog share search choices in this window.
    search_flags: SearchFlags,
    last_preview: Option<RememberedPreview>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct SearchFlags {
    case_sensitive: bool,
    whole_word: bool,
    regex: bool,
}

impl FileTreeExpansions {
    fn restore_preview(&mut self, tree: &TreeScope, owner: EntityId) -> Option<RememberedPreview> {
        let saved = self
            .last_preview
            .as_mut()
            .filter(|saved| saved.in_tree(tree))?;
        saved.owner = owner;
        Some(saved.clone())
    }

    fn forget_preview(&mut self, tree: &TreeScope) {
        self.last_preview.take_if(|saved| saved.tree == *tree);
    }

    fn toggle(&mut self, scope: TreeScope, path: String) -> bool {
        let paths = self.paths.entry(scope.clone()).or_default();
        if paths.remove(&path) {
            if paths.is_empty() {
                self.paths.remove(&scope);
            }
            return false;
        }
        paths.insert(path);
        true
    }

    fn expand(&mut self, scope: TreeScope, path: String) {
        self.paths.entry(scope).or_default().insert(path);
    }

    fn collapse_all(&mut self, scope: &TreeScope) -> bool {
        self.paths.remove(scope).is_some()
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Scope {
    tree: TreeScope,
    session: Option<String>,
    connection: u64,
    open_request: u64,
}

impl Scope {
    fn params(&self, mut params: Value) -> Value {
        params["workspaceId"] = json!(self.tree.workspace);
        if let Some(session) = &self.session {
            params["sessionId"] = json!(session);
        }
        params
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum TreeItem {
    Entry(String),
    LoadMore(String),
}

impl TreeItem {
    fn file_path<'a>(&'a self, directories: &HashMap<String, Directory>) -> Option<&'a str> {
        let TreeItem::Entry(path) = self else {
            return None;
        };
        tree_entry(directories, path)
            .is_some_and(|entry| !entry.directory)
            .then_some(path.as_str())
    }
}

fn tree_entry<'a>(
    directories: &'a HashMap<String, Directory>,
    path: &str,
) -> Option<&'a DirectoryEntry> {
    let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    directories
        .get(parent)?
        .entries
        .iter()
        .find(|entry| entry.path == path)
}

enum TreeRow<'a> {
    Entry(&'a DirectoryEntry, usize),
    Status(&'a Directory, usize),
    LoadMore(&'a str, &'a Directory, usize),
}

impl TreeRow<'_> {
    fn item(&self) -> Option<TreeItem> {
        match self {
            Self::Entry(entry, _) => Some(TreeItem::Entry(entry.path.clone())),
            Self::LoadMore(path, _, _) => Some(TreeItem::LoadMore((*path).to_owned())),
            Self::Status(..) => None,
        }
    }

    fn matches_item(&self, item: &TreeItem) -> bool {
        match (self, item) {
            (Self::Entry(entry, _), TreeItem::Entry(path)) => entry.path == *path,
            (Self::LoadMore(path, ..), TreeItem::LoadMore(item_path)) => *path == item_path,
            _ => false,
        }
    }
}

fn visible_tree<'a>(
    directories: &'a HashMap<String, Directory>,
    expanded: Option<&HashSet<String>>,
) -> Vec<TreeRow<'a>> {
    fn append<'a>(
        directories: &'a HashMap<String, Directory>,
        expanded: Option<&HashSet<String>>,
        path: &'a str,
        depth: usize,
        rows: &mut Vec<TreeRow<'a>>,
    ) {
        let Some(folder) = directories.get(path) else {
            return;
        };
        for entry in &folder.entries {
            rows.push(TreeRow::Entry(entry, depth));
            if entry.directory && expanded.is_some_and(|paths| paths.contains(&entry.path)) {
                append(directories, expanded, &entry.path, depth + 1, rows);
            }
        }
        if folder.loading || folder.error.is_some() || folder.entries.is_empty() {
            rows.push(TreeRow::Status(folder, depth));
        }
        if (!folder.loading && folder.error.is_some()) || folder.next_offset < folder.total {
            rows.push(TreeRow::LoadMore(path, folder, depth));
        }
    }
    let mut rows = Vec::new();
    append(directories, expanded, "", 0, &mut rows);
    rows
}

fn collapsed_tree_selection(
    directories: &HashMap<String, Directory>,
    selected: &TreeItem,
) -> Option<(usize, TreeItem)> {
    let item = match selected {
        TreeItem::Entry(path) | TreeItem::LoadMore(path) if !path.is_empty() => {
            TreeItem::Entry(path.split('/').next()?.to_owned())
        }
        _ => selected.clone(),
    };
    visible_tree(directories, None)
        .iter()
        .position(|row| row.matches_item(&item))
        .map(|index| (index, item))
}

fn unrequested_expansions(
    directories: &HashMap<String, Directory>,
    expanded: &HashSet<String>,
) -> Vec<String> {
    // Follow fresh parent pages. A remembered path might be hidden by a
    // collapsed parent, moved, or on a page that hasn't been loaded yet.
    visible_tree(directories, Some(expanded))
        .into_iter()
        .filter_map(|row| match row {
            TreeRow::Entry(entry, _)
                if entry.directory
                    && expanded.contains(&entry.path)
                    && !directories.contains_key(&entry.path) =>
            {
                Some(entry.path.clone())
            }
            _ => None,
        })
        .collect()
}

pub(crate) struct FileBrowser {
    app: WeakEntity<HiveApp>,
    docked: bool,
    dismissed: bool,
    scope: Option<Scope>,
    scope_generation: u64,
    runtime: Option<Arc<Runtime>>,
    query: Entity<InputState>,
    contents: bool,
    search_flags: SearchFlags,
    directories: HashMap<String, Directory>,
    expansions: Entity<FileTreeExpansions>,
    root_focus: FocusHandle,
    browser_focus: FocusHandle,
    preview_focus: FocusHandle,
    dialog_focus: Option<FocusHandle>,
    resize_focus: FocusHandle,
    split_focus: FocusHandle,
    split_height: Pixels,
    split_drag: Option<u8>,
    tree_focus: FocusHandle,
    content_results_focus: FocusHandle,
    tree_selected: Option<TreeItem>,
    reveal: Option<FileReveal>,
    reveal_missing: Option<String>,
    tree_scroll: ScrollHandle,
    search_scroll: ScrollHandle,
    search: SearchPage,
    search_offset: usize,
    search_selected: usize,
    content_results: ContentResults,
    search_loading: bool,
    search_error: Option<FileError>,
    search_generation: u64,
    search_task: Option<Task<()>>,
    preview: Option<Preview>,
    preview_generation: u64,
    preview_task: Option<Task<()>>,
    tree_preview: Option<TreePreview>,
    expanded_preview: Option<WeakEntity<ExpandedFilePreview>>,
    git_generation: u64,
    git_status: TreeGitStatus,
    git_task: Option<Task<()>>,
}

impl HiveApp {
    pub(crate) fn can_browse_project_files(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && !self.current_worktree_released()
            && self.runtime.is_some()
            && self.workspace.is_some()
    }

    fn project_file_scope(&self) -> Option<Scope> {
        if !self.can_browse_project_files() {
            return None;
        }
        let workspace = self.workspace.clone()?;
        let session = self
            .session
            .as_ref()
            .filter(|session| session.workspace_id == workspace);
        let project_path = self
            .workspaces
            .iter()
            .find(|project| project.id == workspace)
            .map(|project| project.path.as_str());
        Some(Scope {
            tree: TreeScope {
                workspace,
                source: TreeSource::new(session, project_path),
            },
            session: session.map(|session| session.id.clone()),
            connection: self.connection_generation,
            open_request: self.open_request,
        })
    }

    fn project_file_dock_space(&self, window: &Window) -> Pixels {
        let viewport = window.viewport_size().width;
        viewport
            - if self.sidebar_open {
                self.sidebar.resolved(viewport)
            } else {
                px(0.)
            }
    }

    fn can_dock_project_files(&self, window: &Window) -> bool {
        let viewport = window.viewport_size().width;
        self.viewing_conversation()
            && !self.showing_welcome_guide
            && (!self.sidebar_open || viewport >= px(820.))
            && !self.git_drawer_open
            && !(self.split_peek.is_some() && viewport >= px(960.))
            && dock_width(None, self.project_file_dock_space(window)).is_some()
    }

    pub(crate) fn project_files_workspace_focus(
        &self,
        cx: &App,
    ) -> Option<(FocusHandle, FocusHandle)> {
        let scope = self.project_file_scope()?;
        let browser = self.project_file_dock.as_ref()?.read(cx);
        (!browser.dismissed && browser.scope.as_ref() == Some(&scope)).then(|| {
            (
                browser.root_focus.clone(),
                browser.query.read(cx).focus_handle(cx),
            )
        })
    }

    pub(crate) fn project_file_browser_button(
        &self,
        id: &'static str,
        window: &Window,
        cx: &Context<Self>,
    ) -> Button {
        let docked = self.project_file_dock.is_some() && self.can_dock_project_files(window);
        let label = if docked {
            "隐藏项目文件"
        } else {
            "浏览项目文件"
        };
        Button::new(id)
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(26.))
            .w(px(26.))
            .rounded_md()
            .icon(Glyph::FolderOpen)
            .selected(docked)
            .tooltip(tr(label))
            .accessibility_label(tr(label))
            .disabled(!docked && !self.can_browse_project_files())
            .on_click(cx.listener(|this, _, window, cx| this.open_project_file_browser(window, cx)))
    }

    pub(crate) fn open_project_file_browser(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_project_file_browser(None, window, cx);
    }

    pub(crate) fn reveal_project_file(
        &mut self,
        path: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_project_file_browser(Some(FileReveal { path }), window, cx);
    }

    fn show_project_file_browser(
        &mut self,
        reveal: Option<FileReveal>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) {
            return;
        }
        let docked = self.can_dock_project_files(window);
        if docked && reveal.is_none() && self.project_file_dock.is_some() {
            self.close_project_file_dock(window, cx);
            return;
        }
        let (Some(scope), Some(runtime)) = (self.project_file_scope(), self.runtime.clone()) else {
            return;
        };
        if let Some(browser) = self.project_file_dock.clone().filter(|_| docked) {
            browser.update(cx, |browser, cx| {
                // Observe current files even when this dock already visited the path.
                browser.set_scope(Some(scope), Some(runtime), cx);
                browser
                    .query
                    .update(cx, |query, cx| query.set_value("", window, cx));
                browser.reveal = reveal;
            });
            Self::focus_file_tree(&browser, window);
            cx.notify();
            return;
        }
        if let Some(browser) = &self.project_file_dock {
            browser.update(cx, |browser, cx| browser.set_scope(None, None, cx));
        }
        let revealing = reveal.is_some();
        let app = cx.weak_entity();
        let expansions = self.project_file_tree_expansions.clone();
        let search_flags = expansions.read(cx).search_flags;
        let browser = cx.new(|cx| {
            let query = cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索项目文件…")));
            let tree_focus = cx.focus_handle();
            cx.on_focus_out(&tree_focus, window, |this: &mut FileBrowser, _, _, cx| {
                this.cancel_tree_preview(cx);
            })
            .detach();
            cx.observe_window_activation(window, |this: &mut FileBrowser, window, cx| {
                if !window.is_window_active() {
                    this.cancel_tree_preview(cx);
                }
            })
            .detach();
            cx.subscribe(
                &query,
                |this: &mut FileBrowser, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.reveal = None;
                        this.reveal_missing = None;
                        this.search_offset = 0;
                        this.load_search(true, cx);
                    }
                },
            )
            .detach();
            cx.observe(&expansions, |this: &mut FileBrowser, state, cx| {
                let flags = state.read(cx).search_flags;
                if this.search_flags != flags {
                    let refresh =
                        this.contents || this.search_flags.case_sensitive != flags.case_sensitive;
                    this.search_flags = flags;
                    if refresh {
                        this.search_offset = 0;
                        this.load_search(false, cx);
                    }
                }
                this.restore_visible_expansions(cx);
                cx.notify();
            })
            .detach();
            if !docked {
                if let Some(parent) = app.upgrade() {
                    cx.observe(&parent, |this: &mut FileBrowser, _, cx| {
                        if !this.dismissed && !this.current(cx) {
                            this.dismiss(cx);
                        }
                        cx.notify();
                    })
                    .detach();
                }
            }
            let mut browser = FileBrowser {
                app,
                docked,
                dismissed: false,
                scope: None,
                scope_generation: 0,
                runtime: None,
                query,
                contents: false,
                search_flags,
                directories: HashMap::new(),
                expansions,
                root_focus: cx.focus_handle(),
                browser_focus: cx.focus_handle(),
                preview_focus: cx.focus_handle(),
                dialog_focus: None,
                resize_focus: cx.focus_handle(),
                split_focus: cx.focus_handle(),
                split_height: px(0.),
                split_drag: None,
                tree_focus,
                content_results_focus: cx.focus_handle().tab_stop(true),
                tree_selected: None,
                reveal: None,
                reveal_missing: None,
                tree_scroll: ScrollHandle::new(),
                search_scroll: ScrollHandle::new(),
                search: SearchPage::default(),
                search_offset: 0,
                search_selected: 0,
                content_results: ContentResults::default(),
                search_loading: false,
                search_error: None,
                search_generation: 0,
                search_task: None,
                preview: None,
                preview_generation: 0,
                preview_task: None,
                tree_preview: None,
                expanded_preview: None,
                git_generation: 0,
                git_status: TreeGitStatus::Idle,
                git_task: None,
            };
            browser.set_scope(Some(scope), Some(runtime), cx);
            browser.reveal = reveal;
            browser
        });
        let focus = if revealing {
            browser.read(cx).tree_focus.clone()
        } else {
            browser.read(cx).query.read(cx).focus_handle(cx)
        };
        if docked {
            if revealing {
                Self::focus_file_tree(&browser, window);
            }
            self.project_file_dock = Some(browser);
            // Keep typing in the same place when opening the side panel.
            cx.notify();
            return;
        }

        let width = (window.viewport_size().width - px(44.)).min(px(1040.));
        let dismiss = browser.downgrade();
        let dialog_browser = browser.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let dismiss = dismiss.clone();
            dialog
                .title(tr("浏览项目文件"))
                .w(width)
                .on_ok(|_, _, _| false)
                .on_close(move |_, _, cx| {
                    let _ = dismiss.update(cx, |this, cx| this.dismiss(cx));
                })
                .child(dialog_browser.clone())
        });
        let dialog_focus = window.focused(cx);
        browser.update(cx, |browser, _| browser.dialog_focus = dialog_focus.clone());
        window.on_next_frame(move |window, cx| {
            if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                focus.focus(window, cx);
            }
        });
    }

    fn focus_file_tree(browser: &Entity<FileBrowser>, window: &mut Window) {
        let browser = browser.downgrade();
        window.on_next_frame(move |window, cx| {
            if let Some(browser) = browser.upgrade() {
                let browser = browser.read(cx);
                let focus = browser.current(cx).then(|| browser.tree_focus.clone());
                if let Some(focus) = focus.filter(|_| !window.has_active_dialog(cx)) {
                    focus.focus(window, cx);
                }
            }
        });
    }

    fn close_project_file_dock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(browser) = self.project_file_dock.take() {
            if browser.read(cx).root_focus.contains_focused(window, cx) {
                self.input.read(cx).focus_handle(cx).focus(window, cx);
            }
            browser.update(cx, |browser, cx| browser.dismiss(cx));
            cx.notify();
        }
    }

    fn resize_project_file_dock(
        &mut self,
        target: Pixels,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let space = self.project_file_dock_space(window);
        let width = dock_width(Some(target), space);
        if width != dock_width(self.project_file_dock_width, space) {
            self.project_file_dock_width = width;
            cx.notify();
        }
    }

    fn project_file_dock_divider(
        &self,
        focus: FocusHandle,
        width: Pixels,
        space: Pixels,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let paint_focus = focus.clone();
        resize_handle("project-file-dock-resize", Axis::Horizontal)
            .inside(HandleEdge::Leading)
            .role(Role::Splitter)
            .aria_label(tr(
                "调整文件面板宽度，使用左右方向键，双击或按 Home 恢复默认",
            ))
            .aria_orientation(accesskit::Orientation::Vertical)
            .aria_numeric_value(f64::from(f32::from(width)))
            .aria_min_numeric_value(f64::from(DOCK_WIDTH))
            .aria_max_numeric_value(f64::from(f32::from(space) - CHAT_MIN_WIDTH))
            .track_focus(&focus)
            .tab_stop(true)
            .with_appearance(Rc::new(move |_, window, cx| {
                paint_focus.is_focused(window).then(|| {
                    div()
                        .h_full()
                        .w(px(2.))
                        .flex_none()
                        .bg(Color::Secondary.get(cx))
                        .into_any_element()
                })
            }))
            .on_drag(
                ResizeFileDock {
                    width,
                    starting_x: Cell::default(),
                },
                |drag, _, window, cx| {
                    drag.starting_x.set(window.mouse_position().x);
                    cx.new(|_| drag.clone())
                },
            )
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<ResizeFileDock>, window, cx| {
                    let drag = event.drag(cx);
                    let change = drag.starting_x.get() - event.event.position.x;
                    this.resize_project_file_dock(drag.width + change, window, cx);
                }),
            )
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if modifiers.alt || modifiers.control || modifiers.platform || modifiers.shift {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "left" => {
                        this.resize_project_file_dock(width + px(DOCK_WIDTH_STEP), window, cx)
                    }
                    "right" => {
                        this.resize_project_file_dock(width - px(DOCK_WIDTH_STEP), window, cx)
                    }
                    "home" => {
                        this.project_file_dock_width = None;
                        cx.notify();
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .on_double_click(cx.listener(|this, _, _, cx| {
                this.project_file_dock_width = None;
                cx.notify();
            }))
    }

    pub(crate) fn project_file_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let browser = self.project_file_dock.clone()?;
        let previous = browser.read(cx).scope.clone();
        let scope = self.project_file_scope();
        let visible = self.can_dock_project_files(window)
            && self.workspace.is_some()
            && !(previous.is_none() && window.has_active_dialog(cx));
        let next = scope.filter(|scope| visible || previous.as_ref() == Some(scope));
        if previous != next {
            let runtime = self.runtime.clone();
            browser.update(cx, |browser, cx| browser.set_scope(next, runtime, cx));
        }
        if !visible {
            return None;
        }
        let space = self.project_file_dock_space(window);
        let width = dock_width(self.project_file_dock_width, space)?;
        let root_focus = browser.read(cx).root_focus.clone();
        let divider =
            self.project_file_dock_divider(browser.read(cx).resize_focus.clone(), width, space, cx);
        Some(
            v_flex()
                .id("project-file-dock")
                .track_focus(&root_focus)
                .tab_stop(false)
                .relative()
                .w(width)
                .h_full()
                .min_h_0()
                .flex_shrink_0()
                .border_l_1()
                .border_color(Color::Divider.get(cx))
                .bg(Color::Surface.get(cx))
                .child(
                    h_flex()
                        .h(px(crate::chrome::TOPBAR_HEIGHT))
                        .flex_shrink_0()
                        .px_3()
                        .justify_between()
                        .items_center()
                        .gap_2()
                        .border_b_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(tr("项目文件")),
                        )
                        .child(
                            Button::new("close-project-file-dock")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .size(px(26.))
                                .icon(Glyph::X)
                                .tooltip(tr("隐藏项目文件"))
                                .accessibility_label(tr("隐藏项目文件"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.close_project_file_dock(window, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .p_2()
                        .child(browser),
                )
                .child(divider)
                .into_any_element(),
        )
    }
}

impl FileBrowser {
    fn current(&self, cx: &App) -> bool {
        !self.dismissed
            && self.scope.is_some()
            && self
                .app
                .upgrade()
                .is_some_and(|app| self.scope == app.read(cx).project_file_scope())
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.dismissed = true;
        self.search_task = None;
        self.save_preview_position(cx);
        self.clear_preview();
        self.reset_git_status();
    }

    fn clear_preview(&mut self) {
        self.split_drag = None;
        self.preview_generation += 1;
        self.preview_task = None;
        self.tree_preview = None;
        self.preview = None;
    }

    fn remember_preview_selection(&self, cx: &mut Context<Self>) {
        let (Some(scope), Some(preview)) = (&self.scope, &self.preview) else {
            return;
        };
        let directory = preview
            .execution_directory
            .clone()
            .or_else(|| scope.tree.source.directory().map(str::to_owned));
        let saved = RememberedPreview {
            owner: cx.entity_id(),
            tree: scope.tree.clone(),
            selection: preview.selection.clone(),
            directory,
            reading: preview.reading_position(),
        };
        self.expansions
            .update(cx, |state, _| state.last_preview = Some(saved));
    }

    fn save_preview_position(&self, cx: &mut Context<Self>) {
        let preview = match &self.tree_preview {
            Some(TreePreview::Reading { previous }) => previous.as_ref(),
            _ => self.preview.as_ref(),
        };
        let (Some(scope), Some(preview)) = (&self.scope, preview) else {
            return;
        };
        let owner = cx.entity_id();
        self.expansions.update(cx, |state, _| {
            if let Some(saved) = &mut state.last_preview
                && saved.owner == owner
                && saved.tree == scope.tree
                && saved.selection == preview.selection
            {
                if let Some(directory) = &preview.execution_directory {
                    saved.directory = Some(directory.clone());
                }
                if let Some(reading) = preview.reading_position() {
                    saved.reading = Some(reading);
                }
            }
        });
    }

    fn restore_compact_preview(&mut self, cx: &mut Context<Self>) {
        if self.preview.is_some() || self.reveal.is_some() || !self.current(cx) {
            return;
        }
        let scope = self.scope.as_ref().expect("current file scope");
        let owner = cx.entity_id();
        let saved = self
            .expansions
            .update(cx, |state, _| state.restore_preview(&scope.tree, owner));
        if let Some(saved) = saved {
            self.read_preview(saved.selection.clone(), PreviewRead::Restore(saved), cx);
        }
    }

    fn has_compact_preview(&self) -> bool {
        self.preview.is_some() || self.tree_preview.is_some()
    }

    fn close_compact_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.current(cx)
            || !self.has_compact_preview()
            || if self.docked {
                window.has_active_dialog(cx)
            } else {
                !self
                    .dialog_focus
                    .as_ref()
                    .is_some_and(|focus| focus.contains_focused(window, cx))
            }
        {
            return false;
        }
        let tree = &self.scope.as_ref().expect("current file scope").tree;
        self.expansions
            .update(cx, |state, _| state.forget_preview(tree));
        self.clear_preview();
        let query = self.query.read(cx);
        let focus = if query.value().trim().is_empty() {
            self.tree_focus.clone()
        } else if self.contents {
            self.content_results_focus.clone()
        } else {
            query.focus_handle(cx)
        };
        focus.focus(window, cx);
        cx.notify();
        true
    }

    fn close_preview_from_key(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        (self.tree_focus.contains_focused(window, cx)
            || self.preview_focus.contains_focused(window, cx))
            && self.close_compact_preview(window, cx)
    }

    fn set_scope(
        &mut self,
        scope: Option<Scope>,
        runtime: Option<Arc<Runtime>>,
        cx: &mut Context<Self>,
    ) {
        self.save_preview_position(cx);
        if self.scope != scope {
            self.reset_git_status();
        }
        self.scope = scope;
        self.runtime = runtime;
        self.scope_generation += 1;
        self.directories.clear();
        self.tree_selected = None;
        self.reveal = None;
        self.reveal_missing = None;
        self.tree_scroll = ScrollHandle::new();
        self.search = SearchPage::default();
        self.search_offset = 0;
        self.search_selected = 0;
        self.content_results.set_query(None);
        self.search_loading = false;
        self.search_error = None;
        self.search_scroll = ScrollHandle::new();
        self.search_task = None;
        self.clear_preview();
        if self.scope.is_some() {
            // Scope changes can happen during the parent view update.
            let generation = self.scope_generation;
            let pending = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = pending.update(cx, |this, cx| {
                    if this.scope_generation == generation {
                        this.load_directory(String::new(), cx);
                        this.load_search(false, cx);
                        this.restore_compact_preview(cx);
                        if matches!(this.git_status, TreeGitStatus::Idle) {
                            this.load_git_status(cx);
                        }
                    }
                });
            });
        }
    }

    fn reset_git_status(&mut self) {
        self.git_generation += 1;
        self.git_status = TreeGitStatus::Idle;
        self.git_task = None;
    }

    fn git_busy(&self, cx: &App) -> bool {
        self.app
            .upgrade()
            .is_some_and(|app| app.read(cx).file_tree_git_busy(cx))
    }

    fn load_git_status(&mut self, cx: &mut Context<Self>) {
        if self.git_task.is_some() || !self.current(cx) {
            return;
        }
        self.reset_git_status();
        if self.git_busy(cx) {
            self.git_status = TreeGitStatus::Busy;
            cx.notify();
            return;
        }
        let scope = self.scope.as_ref().expect("current file scope").clone();
        let params = scope.params(json!({"offset":0,"limit":FILE_TREE_STATUS_LIMIT}));
        let runtime = self.runtime.clone().expect("current file runtime");
        let generation = self.git_generation;
        self.git_task = Some(cx.spawn(async move |this, cx| {
            let result = runtime
                .request("git.status", params)
                .await
                .and_then(FileTreeChanges::from_wire);
            let _ = this.update(cx, |this, cx| {
                if this.git_generation != generation || this.scope.as_ref() != Some(&scope) {
                    return;
                }
                this.git_task = None;
                if !this.current(cx) {
                    return;
                }
                this.git_status = if this.git_busy(cx) {
                    TreeGitStatus::Busy
                } else {
                    match result {
                        Ok(Some(changes)) => TreeGitStatus::Ready(changes),
                        Ok(None) => TreeGitStatus::NotRepository,
                        Err(error) => TreeGitStatus::Failed(error),
                    }
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn load_directory(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let folder = self.directories.entry(path.clone()).or_default();
        if folder.loading {
            return;
        }
        folder.loading = true;
        folder.error = None;
        let offset = folder.next_offset;
        let scope = self.scope.as_ref().expect("current file scope");
        let params = scope.params(json!({"directory":path, "offset":offset}));
        let runtime = self.runtime.clone().expect("current file runtime");
        let generation = self.scope_generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("project.files.list", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<DirectoryPage>(value)
                        .map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.scope_generation != generation || !this.current(cx) {
                    return;
                }
                let Some(folder) = this.directories.get_mut(&path) else {
                    return;
                };
                folder.loading = false;
                let original_len = folder.entries.len();
                match result {
                    Ok(page) => {
                        folder.loaded = true;
                        folder.total = page.total;
                        folder.next_offset = if page.entries.is_empty() {
                            page.total
                        } else {
                            folder.next_offset + page.entries.len()
                        };
                        for entry in page.entries {
                            // The page may have shifted after files were added.
                            if !folder
                                .entries
                                .iter()
                                .any(|previous| previous.path == entry.path)
                            {
                                folder.entries.push(entry);
                            }
                        }
                    }
                    Err(error) => folder.error = Some(FileError::Request(error)),
                }
                if matches!(this.tree_selected.as_ref(), Some(TreeItem::LoadMore(selected)) if selected == &path)
                    && let Some(path) = folder.entries.get(original_len).map(|entry| entry.path.clone())
                {
                    this.select_tree_item(TreeItem::Entry(path), cx);
                }
                this.advance_reveal(cx);
                this.restore_visible_expansions(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_directory(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        self.cancel_tree_preview(cx);
        self.reveal = None;
        self.reveal_missing = None;
        let scope = self
            .scope
            .as_ref()
            .expect("current file scope")
            .tree
            .clone();
        let opened = self.expansions.update(cx, |expansions, cx| {
            let opened = expansions.toggle(scope, path.clone());
            cx.notify();
            opened
        });
        if opened
            && !self
                .directories
                .get(&path)
                .is_some_and(|folder| folder.loaded)
        {
            self.load_directory(path, cx);
        } else {
            cx.notify();
        }
    }

    fn expanded_paths<'a>(&self, cx: &'a App) -> Option<&'a HashSet<String>> {
        self.expansions
            .read(cx)
            .paths
            .get(&self.scope.as_ref()?.tree)
    }

    fn collapse_all_folders(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.current(cx) || !self.query.read(cx).value().trim().is_empty() {
            return;
        }
        let scope = &self.scope.as_ref().expect("current file scope").tree;
        let collapsed = self.expansions.update(cx, |expansions, cx| {
            let collapsed = expansions.collapse_all(scope);
            if collapsed {
                cx.notify();
            }
            collapsed
        });
        if !collapsed {
            return;
        }
        self.cancel_tree_preview(cx);
        self.reveal = None;
        self.reveal_missing = None;
        let selection = self
            .tree_selected
            .as_ref()
            .and_then(|item| collapsed_tree_selection(&self.directories, item));
        self.tree_selected = selection.map(|(index, item)| {
            self.tree_scroll.scroll_to_item(index);
            item
        });
        self.tree_focus.focus(window, cx);
        cx.notify();
    }

    fn directory_expanded(&self, path: &str, cx: &App) -> bool {
        self.expanded_paths(cx)
            .is_some_and(|expanded| expanded.contains(path))
    }

    fn restore_visible_expansions(&mut self, cx: &mut Context<Self>) {
        let Some(expanded) = self.expanded_paths(cx).filter(|paths| !paths.is_empty()) else {
            return;
        };
        if !self.current(cx) {
            return;
        }
        for path in unrequested_expansions(&self.directories, expanded) {
            self.load_directory(path, cx);
        }
    }

    fn advance_reveal(&mut self, cx: &mut Context<Self>) {
        while let Some(reveal) = self.reveal.as_ref() {
            match reveal.step(&self.directories, self.expanded_paths(cx)) {
                RevealStep::Expand(path) => {
                    let scope = self
                        .scope
                        .as_ref()
                        .expect("current file scope")
                        .tree
                        .clone();
                    let path = path.to_owned();
                    self.expansions.update(cx, |expansions, cx| {
                        expansions.expand(scope, path);
                        cx.notify();
                    });
                }
                RevealStep::Load(path) => {
                    self.load_directory(path.to_owned(), cx);
                    return;
                }
                RevealStep::Wait => return,
                RevealStep::Select => {
                    let item = TreeItem::Entry(reveal.path.clone());
                    self.reveal = None;
                    self.select_tree_item(item, cx);
                }
                RevealStep::Missing => {
                    self.reveal_missing = Some(reveal.path.clone());
                    self.reveal = None;
                }
            }
        }
    }

    fn load_search(&mut self, debounce: bool, cx: &mut Context<Self>) {
        self.cancel_tree_preview(cx);
        self.search_task = None;
        self.search_generation += 1;
        self.search = SearchPage::default();
        self.search_selected = 0;
        self.search_error = None;
        self.search_loading = false;
        self.search_scroll = ScrollHandle::new();
        cx.notify();
        let input = self.query.read(cx).value();
        let query = if self.search_flags.regex && !input.trim().is_empty() {
            input.to_string()
        } else {
            input.trim().to_owned()
        };
        self.content_results
            .set_query((self.contents && !query.is_empty()).then(|| ContentQuery {
                scope_generation: self.scope_generation,
                flags: self.search_flags,
                text: query.clone(),
            }));
        if query.is_empty() || !self.current(cx) {
            return;
        }
        self.search_loading = true;
        let scope = self.scope.as_ref().expect("current file scope");
        let runtime = self.runtime.clone().expect("current file runtime");
        let contents = self.contents;
        let offset = self.search_offset;
        let mut params = scope.params(json!({
            "query":query,"offset":offset,"caseSensitive":self.search_flags.case_sensitive,
            "regex":self.search_flags.regex,"wholeWord":self.search_flags.whole_word,
        }));
        if !contents {
            params["includeRanges"] = json!(true);
        }
        let scope_generation = self.scope_generation;
        let search_generation = self.search_generation;
        self.search_task = Some(cx.spawn(async move |this, cx| {
            if debounce {
                smol::Timer::after(Duration::from_millis(180)).await;
            }
            let result = runtime
                .search_project_files(contents, params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<SearchPage>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.scope_generation != scope_generation
                    || this.search_generation != search_generation
                    || !this.current(cx)
                {
                    return;
                }
                this.search_loading = false;
                match result {
                    Ok(mut page) => {
                        page.append_name_paths();
                        if offset >= page.total && offset > 0 {
                            this.search_offset =
                                page.total.saturating_sub(1) / PAGE_SIZE * PAGE_SIZE;
                            this.load_search(false, cx);
                            return;
                        }
                        this.search = page;
                        if contents {
                            this.content_results.reconcile(&this.search.matches);
                            this.scroll_to_content_cursor();
                        }
                    }
                    Err(error) => {
                        this.search_error = Some(match error.as_str() {
                            "Invalid regular expression." => FileError::Owned("无效的正则表达式。"),
                            "Regular expression search timed out." => {
                                FileError::Owned("正则搜索超时，请缩小搜索范围或简化表达式。")
                            }
                            "Regular expression search is temporarily unavailable." => {
                                FileError::Owned("正则搜索暂不可用，请重试。")
                            }
                            _ => FileError::Request(error),
                        })
                    }
                }
                cx.notify();
            });
        }));
    }

    fn toggle_search_case(
        &mut self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_search_flags(
            SearchFlags {
                case_sensitive: !self.search_flags.case_sensitive,
                ..self.search_flags
            },
            event,
            window,
            cx,
        );
    }

    fn toggle_search_whole_word(
        &mut self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_search_flags(
            SearchFlags {
                whole_word: !self.search_flags.whole_word,
                ..self.search_flags
            },
            event,
            window,
            cx,
        );
    }

    fn toggle_search_regex(
        &mut self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_search_flags(
            SearchFlags {
                regex: !self.search_flags.regex,
                ..self.search_flags
            },
            event,
            window,
            cx,
        );
    }

    fn set_search_flags(
        &mut self,
        flags: SearchFlags,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.current(cx) {
            return;
        }
        self.search_flags = flags;
        self.expansions.update(cx, |state, cx| {
            state.search_flags = flags;
            cx.notify();
        });
        self.search_offset = 0;
        self.load_search(false, cx);
        if matches!(event, ClickEvent::Mouse(_) | ClickEvent::Touch(_)) {
            self.query.read(cx).focus_handle(cx).focus(window, cx);
        }
    }

    fn search_composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.query.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        })
    }

    fn move_search_selection(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.query.read(cx).focus_handle(cx).is_focused(window)
            || self.search_composing(window, cx)
        {
            cx.propagate();
            return;
        }
        if self.query.read(cx).value().trim().is_empty() {
            self.tree_focus.focus(window, cx);
            self.move_tree_cursor(delta, window, cx);
            return;
        }
        if self.contents {
            self.move_content_cursor(delta, true, cx);
        } else if !self.search.matches.is_empty() {
            self.search_selected = self
                .search_selected
                .saturating_add_signed(delta)
                .min(self.search.matches.len() - 1);
            self.search_scroll.scroll_to_item(self.search_selected);
            cx.notify();
        }
    }

    fn open_search_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_composing(window, cx) {
            cx.propagate();
        } else if self.contents {
            if let Some(ContentPick::Hit(hit)) = self.content_results.cursor.clone() {
                if self.content_results.hit_visible(&self.search.matches, &hit) {
                    self.select(hit, cx);
                }
            }
        } else if let Some(hit) = self.search.matches.get(self.search_selected) {
            self.select(hit.selection.clone(), cx);
        }
    }

    fn move_content_cursor(&mut self, delta: isize, hits_only: bool, cx: &mut Context<Self>) {
        if let Some(index) =
            self.content_results
                .move_cursor(&self.search.matches, delta, hits_only)
        {
            self.search_scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn scroll_to_content_cursor(&self) {
        if let Some(cursor) = &self.content_results.cursor
            && let Some(index) =
                ContentResults::position(&self.content_results.rows(&self.search.matches), cursor)
        {
            self.search_scroll.scroll_to_item(index);
        }
    }

    fn toggle_content_group(&mut self, path: &str, expand: Option<bool>, cx: &mut Context<Self>) {
        if !self.contents
            || !self.current(cx)
            || !self
                .search
                .matches
                .iter()
                .any(|hit| hit.selection.path == path)
        {
            return;
        }
        self.content_results.toggle(path, expand);
        self.scroll_to_content_cursor();
        cx.notify();
    }

    fn content_results_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control
            || modifiers.platform
            || modifiers.alt
            || modifiers.shift
            || !self.current(cx)
            || !self.content_results_focus.is_focused(window)
        {
            return;
        }
        match event.keystroke.key.as_str() {
            "down" => self.move_content_cursor(1, false, cx),
            "up" => self.move_content_cursor(-1, false, cx),
            "left" => {
                if let Some(cursor) = self.content_results.cursor.clone() {
                    self.toggle_content_group(cursor.path(), Some(false), cx);
                }
            }
            "right" => {
                if let Some(ContentPick::File(path)) = self.content_results.cursor.clone() {
                    self.toggle_content_group(&path, Some(true), cx);
                }
            }
            "enter" => match self.content_results.cursor.clone() {
                Some(ContentPick::File(path)) => self.toggle_content_group(&path, None, cx),
                Some(ContentPick::Hit(hit))
                    if self.content_results.hit_visible(&self.search.matches, &hit) =>
                {
                    self.select(hit, cx)
                }
                _ => {}
            },
            "escape" => self.query.read(cx).focus_handle(cx).focus(window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn move_tree_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let rows = self.tree_rows(cx);
        let selectable = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| !matches!(row, TreeRow::Status(..)))
            .collect::<Vec<_>>();
        let selected = self.tree_selected.as_ref().and_then(|item| {
            selectable
                .iter()
                .position(|(_, row)| row.matches_item(item))
        });
        let next = match selected {
            Some(index) => {
                selectable.get(index.saturating_add_signed(delta).min(selectable.len() - 1))
            }
            None if delta < 0 => selectable.last(),
            None => selectable.first(),
        };
        if let Some((index, item)) = next.and_then(|(index, row)| Some((*index, row.item()?))) {
            self.cancel_path_request_after_tree_selection(&item);
            self.tree_selected = Some(item);
            self.tree_scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn select_tree_item(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        self.cancel_tree_preview(cx);
        let index = self
            .tree_rows(cx)
            .iter()
            .position(|row| row.matches_item(&item));
        if let Some(index) = index {
            self.cancel_path_request_after_tree_selection(&item);
            self.tree_selected = Some(item);
            self.tree_scroll.scroll_to_item(index);
            cx.notify();
        }
    }

    fn cancel_path_request_after_tree_selection(&mut self, item: &TreeItem) {
        if self.tree_selected.as_ref() != Some(item)
            && let Some(preview) = &mut self.preview
            && !matches!(item, TreeItem::Entry(path) if *path == preview.selection.path)
        {
            preview.path_request = None;
        }
    }

    fn activate_tree_item(&mut self, item: TreeItem, cx: &mut Context<Self>) {
        self.cancel_tree_preview(cx);
        match item {
            TreeItem::Entry(path) if self.is_directory(&path) => self.toggle_directory(path, cx),
            TreeItem::Entry(path) => self.select(Match { path, line: None }, cx),
            TreeItem::LoadMore(path) => self.load_directory(path, cx),
        }
    }

    fn open_tree_selection(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        if self.tree_selected.is_none() {
            self.move_tree_selection(1, cx);
        }
        if let Some(item) = self.tree_selected.clone() {
            if item
                .file_path(&self.directories)
                .is_some_and(|path| self.tree_file_is_previewed(path))
            {
                // Keep an already started read, but make it an explicit choice so
                // moving focus does not undo it.
                if matches!(self.tree_preview.take(), Some(TreePreview::Reading { .. })) {
                    self.remember_preview_selection(cx);
                }
                return;
            }
            self.activate_tree_item(item, cx);
        }
    }

    fn tree_file_is_previewed(&self, path: &str) -> bool {
        self.preview.as_ref().is_some_and(|preview| {
            preview.selection.path == path
                && preview.selection.line.is_none()
                && preview.error.is_none()
        })
    }

    fn cancel_tree_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(TreePreview::Reading { previous }) = self.tree_preview.take() {
            self.preview_task = None;
            self.preview_generation += 1;
            self.preview = previous;
            cx.notify();
        }
    }

    fn move_tree_cursor(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        self.move_tree_selection(delta, cx);
        self.queue_tree_preview(window, cx);
    }

    fn queue_tree_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.selected_tree_file().map(str::to_owned) else {
            self.cancel_tree_preview(cx);
            return;
        };
        if self.tree_file_is_previewed(&path)
            && matches!(self.tree_preview, Some(TreePreview::Reading { .. }))
        {
            return;
        }
        self.cancel_tree_preview(cx);
        if self.tree_file_is_previewed(&path) || self.reveal.is_some() {
            return;
        }
        let scope_generation = self.scope_generation;
        let task = cx.spawn_in(window, async move |this, cx| {
            smol::Timer::after(TREE_PREVIEW_DELAY).await;
            let _ = cx.update(|window, cx| {
                let _ = this.update(cx, |this, cx| {
                    this.begin_tree_preview(path, scope_generation, window, cx);
                });
            });
        });
        self.tree_preview = Some(TreePreview::Waiting { _task: task });
    }

    fn begin_tree_preview(
        &mut self,
        path: String,
        scope_generation: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        self.tree_preview = None;
        if self.scope_generation != scope_generation
            || !self.current(cx)
            || !window.is_window_active()
            || !self.tree_focus.contains_focused(window, cx)
            || self.reveal.is_some()
            || !self.query.read(cx).value().trim().is_empty()
            || self.selected_tree_file() != Some(path.as_str())
            || self.tree_file_is_previewed(&path)
            || (self.docked
                && !self
                    .app
                    .upgrade()
                    .is_some_and(|app| app.read(cx).can_dock_project_files(window)))
        {
            return;
        }
        let previous = self
            .preview
            .take()
            .filter(|preview| !preview.loading)
            .map(|mut preview| {
                preview.editor_task = None;
                preview.path_request = None;
                preview
            });
        self.tree_preview = Some(TreePreview::Reading { previous });
        self.read_preview(Match { path, line: None }, PreviewRead::Tree, cx);
    }

    fn selected_tree_file(&self) -> Option<&str> {
        self.tree_selected.as_ref()?.file_path(&self.directories)
    }

    fn is_directory(&self, path: &str) -> bool {
        tree_entry(&self.directories, path).is_some_and(|entry| entry.directory)
    }

    fn tree_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control
            || modifiers.platform
            || modifiers.alt
            || modifiers.shift
            || !self.current(cx)
        {
            return;
        }
        let Some(item) = self.tree_selected.clone() else {
            return;
        };
        match (event.keystroke.key.as_str(), item) {
            ("left", TreeItem::Entry(path)) => {
                if self.directory_expanded(&path, cx) {
                    self.toggle_directory(path, cx);
                } else if let Some((parent, _)) = path.rsplit_once('/') {
                    self.select_tree_item(TreeItem::Entry(parent.to_owned()), cx);
                }
            }
            ("left", TreeItem::LoadMore(path)) if !path.is_empty() => {
                self.select_tree_item(TreeItem::Entry(path), cx);
            }
            ("right", TreeItem::Entry(path)) if self.is_directory(&path) => {
                if self.directory_expanded(&path, cx) {
                    if let Some(child) = self
                        .directories
                        .get(&path)
                        .and_then(|folder| folder.entries.first())
                    {
                        self.select_tree_item(TreeItem::Entry(child.path.clone()), cx);
                        self.queue_tree_preview(window, cx);
                    } else if self
                        .directories
                        .get(&path)
                        .is_some_and(|folder| folder.error.is_some())
                    {
                        self.select_tree_item(TreeItem::LoadMore(path), cx);
                    }
                } else {
                    self.toggle_directory(path, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn select(&mut self, selection: Match, cx: &mut Context<Self>) {
        self.cancel_tree_preview(cx);
        self.read_preview(selection, PreviewRead::Open(None), cx);
    }

    fn open_markdown_link(
        &mut self,
        source: &PreviewIdentity,
        destination: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.current(cx) || self.preview_identity().as_ref() != Some(source) {
            return false;
        }
        let Some(execution_directory) = self.preview.as_ref().and_then(|preview| {
            preview.markdown.as_ref()?;
            preview.execution_directory.clone()
        }) else {
            return false;
        };
        let Some((path, fragment)) = relative_markdown_target(&source.selection.path, destination)
        else {
            return false;
        };
        self.cancel_tree_preview(cx);
        self.read_preview(
            Match { path, line: None },
            PreviewRead::Open(Some(MarkdownNavigation {
                execution_directory,
                fragment: fragment.map(str::to_owned),
            })),
            cx,
        );
        true
    }

    fn read_preview(&mut self, selection: Match, request: PreviewRead, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        self.reveal = None;
        self.reveal_missing = None;
        let scope = self.scope.as_ref().expect("current file scope");
        let allow_image = selection.line.is_none();
        let expected_directory = match &request {
            PreviewRead::Open(Some(link)) => Some(link.execution_directory.as_str()),
            PreviewRead::Restore(saved) => saved.directory.as_deref(),
            _ => None,
        };
        let params = scope.params(json!({
            "relativePath":selection.path,
            "allowImage":allow_image,
            "expectedDirectory":expected_directory,
        }));
        let runtime = self.runtime.clone().expect("current file runtime");
        self.preview_generation += 1;
        self.preview = Some(Preview {
            selection,
            execution_directory: None,
            loading: true,
            text: None,
            image: None,
            sha256: None,
            markdown: None,
            error: None,
            scroll: ScrollHandle::new(),
            editor_task: None,
            editor_error: None,
            path_request: None,
            path_status: None,
        });
        if matches!(request, PreviewRead::Open(_)) {
            self.remember_preview_selection(cx);
        }
        let generation = self.scope_generation;
        let preview_generation = self.preview_generation;
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let result: Result<_, FileError> = async {
                let mut value = runtime
                    .request("project.files.preview", params)
                    .await
                    .map_err(|error| match error.as_str() {
                        "Project image exceeds preview limit" => FileError::Owned(
                            "图片超过 4 MiB，暂不在窗口中预览；可在文件管理器中查看。",
                        ),
                        _ => FileError::preview(error),
                    })?;
                let sha256 = value["sha256"]
                    .as_str()
                    .filter(|digest| {
                        digest.len() == 64
                            && digest
                                .bytes()
                                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                    })
                    .map(str::to_owned);
                let execution_directory = value["executionDirectory"].as_str().map(str::to_owned);
                let preview = if allow_image
                    && let Some(image) = value
                        .as_object_mut()
                        .and_then(|object| object.remove("image"))
                {
                    PreparedPreview::Image(image_preview::prepare(image).await?)
                } else {
                    let text = value["text"].as_str().ok_or(FileError::Owned("读取失败"))?;
                    PreparedPreview::Text(TextPages::new(text.to_owned().into()))
                };
                Ok((preview, sha256, execution_directory))
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.scope_generation != generation
                    || this.preview_generation != preview_generation
                    || !this.current(cx)
                {
                    return;
                }
                let tree_read = matches!(this.tree_preview, Some(TreePreview::Reading { .. }));
                if tree_read {
                    if this.selected_tree_file()
                        != this
                            .preview
                            .as_ref()
                            .map(|preview| preview.selection.path.as_str())
                    {
                        this.cancel_tree_preview(cx);
                        return;
                    }
                    this.tree_preview = None;
                }
                let Some(preview) = this.preview.as_mut() else {
                    return;
                };
                let (reading, fragment) = match request {
                    PreviewRead::Restore(saved) => (saved.reading, None),
                    PreviewRead::Open(Some(link)) => (None, link.fragment),
                    _ => (None, None),
                };
                preview.loading = false;
                match result {
                    Ok((PreparedPreview::Text(mut text), sha256, execution_directory)) => {
                        if let Some(line) = preview.selection.line {
                            text.show_line(line);
                        }
                        if let Some(reading) = reading {
                            reading.restore_page(&mut text);
                            preview.scroll.set_offset(reading.source_scroll);
                        }
                        if is_markdown_file(&preview.selection.path)
                            && supports_markdown_preview(&text.full)
                        {
                            let saved_markdown = reading.and_then(|reading| reading.markdown);
                            let mode = saved_markdown.map_or_else(
                                || initial_markdown_mode(&preview.selection),
                                |(mode, _)| mode,
                            );
                            let markdown = MarkdownPreview::new(mode, &text, cx);
                            if let Some((_, offset)) = saved_markdown {
                                markdown.pending_scroll.set(Some(offset));
                            }
                            if let Some(fragment) = fragment {
                                markdown.reveal_initial_heading(fragment, cx);
                            }
                            preview.markdown = Some(markdown);
                        }
                        preview.text = Some(text);
                        preview.sha256 = sha256;
                        preview.execution_directory = execution_directory;
                    }
                    Ok((PreparedPreview::Image(image), _, execution_directory)) => {
                        preview.image = Some(ImageViewport::new(image, cx));
                        preview.execution_directory = execution_directory;
                    }
                    Err(error) => preview.error = Some(error),
                }
                if tree_read {
                    this.remember_preview_selection(cx);
                } else {
                    this.save_preview_position(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn preview_page(&mut self, step: isize, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let Some(preview) = &mut self.preview else {
            return;
        };
        if preview
            .markdown
            .as_ref()
            .is_some_and(MarkdownPreview::active)
        {
            return;
        }
        let Some(text) = &mut preview.text else {
            return;
        };
        if step_preview(text, &mut preview.scroll, step) {
            cx.notify();
        }
    }

    fn set_preview_mode(&mut self, mode: FilePreviewMode, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        if let Some(Preview {
            markdown: Some(markdown),
            text: Some(text),
            ..
        }) = &mut self.preview
        {
            markdown.switch(mode, text, cx);
            cx.notify();
        }
    }

    fn jump_to_preview_hit(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        if let Some(Preview {
            selection: Match {
                line: Some(line), ..
            },
            markdown: Some(markdown),
            text: Some(text),
            scroll,
            ..
        }) = &mut self.preview
        {
            markdown.switch(FilePreviewMode::Source, text, cx);
            text.show_line(*line);
            *scroll = ScrollHandle::new();
            cx.notify();
        }
    }

    fn preview_identity(&self) -> Option<PreviewIdentity> {
        let preview = self.preview.as_ref()?;
        if preview.loading || preview.error.is_some() || !preview.loaded() {
            return None;
        }
        self.selected_file_identity()
    }

    fn selected_file_identity(&self) -> Option<PreviewIdentity> {
        let preview = self.preview.as_ref()?;
        Some(PreviewIdentity {
            scope: self.scope.clone()?,
            preview_generation: self.preview_generation,
            selection: preview.selection.clone(),
        })
    }

    fn file_path_identity(&self) -> Option<PreviewIdentity> {
        if self.preview.as_ref()?.loading {
            return None;
        }
        self.selected_file_identity()
    }

    fn reveal_in_file_manager_button(
        &self,
        browser: WeakEntity<Self>,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
        cx: &App,
    ) -> Option<Button> {
        let identity = self.file_path_identity()?;
        let busy = self.preview.as_ref()?.path_request.is_some();
        let label = tr("在文件管理器中显示");
        Some(
            Button::new("reveal-project-file-in-manager")
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .size(px(24.))
                .icon(Glyph::FolderOpen)
                .accessibility_label(label)
                .tooltip(label)
                .disabled(busy)
                .on_click(move |_, _, cx| {
                    let _ = browser.update(cx, |browser, cx| {
                        browser.run_file_path_action(
                            FilePathAction::Reveal,
                            identity.clone(),
                            reader.clone(),
                            cx,
                        );
                    });
                }),
        )
    }

    fn copy_path_menu(
        &self,
        browser: WeakEntity<Self>,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
        cx: &App,
    ) -> Option<AnyElement> {
        let identity = self.file_path_identity()?;
        let busy = self.preview.as_ref()?.path_request.is_some();
        let label = tr("复制路径");
        Some(
            Button::new("copy-project-file-path")
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .size(px(24.))
                .icon(Glyph::Copy)
                .accessibility_label(label)
                .tooltip(label)
                .disabled(busy)
                .dropdown_menu(move |menu, _, _| {
                    [
                        ("复制相对路径", FilePathAction::CopyRelative),
                        ("复制绝对路径", FilePathAction::CopyAbsolute),
                    ]
                    .into_iter()
                    .fold(menu, |menu, (label, action)| {
                        let browser = browser.clone();
                        let reader = reader.clone();
                        let identity = identity.clone();
                        menu.item(PopupMenuItem::new(tr(label)).on_click(move |_, _, cx| {
                            let _ = browser.update(cx, |browser, cx| {
                                browser.run_file_path_action(
                                    action,
                                    identity.clone(),
                                    reader.clone(),
                                    cx,
                                );
                            });
                        }))
                    })
                })
                .into_any_element(),
        )
    }

    fn run_file_path_action(
        &mut self,
        action: FilePathAction,
        identity: PreviewIdentity,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
        cx: &mut Context<Self>,
    ) {
        if !self.current(cx)
            || self.file_path_identity().as_ref() != Some(&identity)
            || !ExpandedFilePreview::request_active(reader.as_ref(), cx)
        {
            return;
        }
        let preview = self.preview.as_mut().expect("current file selection");
        if preview.path_request.is_some() {
            return;
        }
        if matches!(action, FilePathAction::CopyRelative) {
            cx.write_to_clipboard(ClipboardItem::new_string(identity.selection.path));
            preview.path_status = Some(FilePathStatus::RelativeCopied);
            cx.notify();
            return;
        }

        preview.path_status = None;
        let runtime = self.runtime.clone().expect("current file runtime");
        let path = identity.selection.path.clone();
        let directory = identity.scope.tree.source.directory().map(PathBuf::from);
        let params = identity.file_params();
        let request_reader = reader.clone();
        let active_id = Rc::new(());
        let request_id = active_id.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = async {
                let response = runtime.request("project.files.resolveOpen", params).await?;
                let file = serde_json::from_value::<ResolvedEditorFile>(response)
                    .map_err(|error| error.to_string())?;
                smol::unblock(move || file.into_validated_path(directory.as_deref(), &path)).await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                if !this.current(cx) || this.file_path_identity().as_ref() != Some(&identity) {
                    return;
                }
                let preview = this.preview.as_mut().expect("current file selection");
                if !preview
                    .path_request
                    .as_ref()
                    .is_some_and(|pending| Rc::ptr_eq(&request_id, &pending.request_id))
                {
                    return;
                }
                preview.path_request = None;
                if ExpandedFilePreview::request_active(reader.as_ref(), cx) {
                    preview.path_status = match action {
                        FilePathAction::Reveal => match result {
                            Ok(path) => {
                                cx.reveal_path(&path);
                                None
                            }
                            Err(_) => Some(FilePathStatus::RevealFailed),
                        },
                        FilePathAction::CopyAbsolute => Some(
                            match result
                                .ok()
                                .and_then(|path| path.into_os_string().into_string().ok())
                            {
                                Some(path) => {
                                    cx.write_to_clipboard(ClipboardItem::new_string(path));
                                    FilePathStatus::AbsoluteCopied
                                }
                                None => FilePathStatus::CopyFailed,
                            },
                        ),
                        FilePathAction::CopyRelative => {
                            unreachable!("relative copy does not resolve on disk")
                        }
                    };
                }
                cx.notify();
            });
        });
        preview.path_request = Some(FilePathRequest {
            reader: request_reader,
            request_id: active_id,
            _task: task,
        });
        cx.notify();
    }

    fn attach_target(&self, cx: &App) -> Option<PreviewAttachment> {
        if !self.current(cx) {
            return None;
        }
        let identity = self.preview_identity()?;
        let sha256 = self.preview.as_ref()?.sha256.clone()?;
        let app = self.app.upgrade()?;
        let target = Target::capture(app.read(cx))?;
        (identity.scope.tree.workspace == target.workspace
            && identity.scope.session == target.session)
            .then_some(PreviewAttachment {
                identity,
                target,
                sha256,
            })
    }

    fn accepts_attachment(app: &HiveApp, target: &Target) -> bool {
        target.current(app)
            && app.viewing_conversation()
            && (app.session.is_none() || app.conversation_ready)
            && app.can_reference_project_file()
    }

    fn can_attach_preview(&self, target: &Target, cx: &App) -> bool {
        self.app
            .upgrade()
            .is_some_and(|app| Self::accepts_attachment(app.read(cx), target))
    }

    fn attach_preview(
        &mut self,
        attachment: &PreviewAttachment,
        expanded: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let PreviewAttachment {
            identity,
            target,
            sha256,
        } = attachment;
        if !self.current(cx) || self.preview_identity().as_ref() != Some(identity) {
            return false;
        }
        let accepted = self
            .app
            .update(cx, |app, cx| {
                if !Self::accepts_attachment(app, target)
                    || !app.reference_project_file(
                        target,
                        identity.selection.path.clone(),
                        Some(sha256.clone()),
                        cx,
                    )
                {
                    return false;
                }
                if window.viewport_size().width < px(820.) {
                    app.sidebar_open = false;
                    cx.notify();
                }
                true
            })
            .unwrap_or(false);
        if !accepted {
            return false;
        }

        if expanded {
            window.close_dialog(cx);
        }
        if !self.docked {
            window.close_dialog(cx);
            self.dismiss(cx);
        }
        let target = target.clone();
        let app = self.app.clone();
        window.on_next_frame(move |window, cx| {
            if !window.has_active_dialog(cx)
                && let Some(app) = app.upgrade()
            {
                let app = app.read(cx);
                if target.current(app) && app.viewing_conversation() {
                    app.input.read(cx).focus_handle(cx).focus(window, cx);
                }
            }
        });
        true
    }

    fn attach_button(&self, cx: &Context<Self>) -> Option<Button> {
        let attachment = self.attach_target(cx)?;
        Some(
            Button::new("browser-attach-to-draft")
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .size(px(24.))
                .icon(Glyph::Paperclip)
                .accessibility_label(tr("添加到草稿"))
                .tooltip(tr("添加到草稿"))
                .disabled(!self.can_attach_preview(&attachment.target, cx))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.attach_preview(&attachment, false, window, cx);
                })),
        )
    }

    fn editor_menu(
        &self,
        browser: WeakEntity<Self>,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
        cx: &App,
    ) -> Option<AnyElement> {
        let identity = self.preview_identity()?;
        let busy = self.preview.as_ref()?.editor_task.is_some();
        open_targets::file_editor_menu(
            busy,
            move |target, cx| {
                let _ = browser.update(cx, |browser, cx| {
                    browser.open_in_editor(identity.clone(), reader.clone(), target, cx);
                });
            },
            cx,
        )
    }

    fn editor_request_current(&self, identity: &PreviewIdentity, cx: &App) -> bool {
        self.current(cx) && self.preview_identity().as_ref() == Some(identity)
    }

    fn open_in_editor(
        &mut self,
        identity: PreviewIdentity,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
        target: OpenTarget,
        cx: &mut Context<Self>,
    ) {
        if !self.editor_request_current(&identity, cx)
            || !ExpandedFilePreview::request_active(reader.as_ref(), cx)
        {
            return;
        }
        let preview = self.preview.as_mut().expect("current preview");
        if preview.editor_task.is_some() {
            return;
        }
        preview.editor_error = None;
        let Some(application) = open_targets::editor_path(target, cx) else {
            preview.editor_error = Some(tr("目标应用未安装").to_owned());
            cx.notify();
            return;
        };
        let runtime = self.runtime.clone().expect("current file runtime");
        let path = identity.selection.path.clone();
        let directory = identity.scope.tree.source.directory().map(PathBuf::from);
        let params = identity.file_params();
        preview.editor_task = Some(cx.spawn(async move |this, cx| {
            let response = runtime
                .request("project.files.resolveOpen", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<ResolvedEditorFile>(value)
                        .map_err(|error| error.to_string())
                });
            let command = match response {
                Ok(file) => {
                    smol::unblock(move || {
                        open_targets::prepare_editor_file(
                            &application,
                            file,
                            directory.as_deref(),
                            &path,
                        )
                    })
                    .await
                }
                Err(error) => Err(error),
            };
            let launch = this
                .update(cx, |this, cx| {
                    if !this.editor_request_current(&identity, cx) {
                        return None;
                    }
                    if !ExpandedFilePreview::request_active(reader.as_ref(), cx) {
                        this.editor_finished(Ok(()), cx);
                        return None;
                    }
                    match command.and_then(open_targets::start_editor_file) {
                        Ok(Some(child)) => Some(child),
                        result => {
                            this.editor_finished(result.map(|_| ()), cx);
                            None
                        }
                    }
                })
                .ok()
                .flatten();
            if let Some(child) = launch {
                let result = smol::unblock(move || open_targets::finish_editor_file(child)).await;
                let _ = this.update(cx, |this, cx| {
                    if this.editor_request_current(&identity, cx) {
                        let result = if ExpandedFilePreview::request_active(reader.as_ref(), cx) {
                            result
                        } else {
                            Ok(())
                        };
                        this.editor_finished(result, cx);
                    }
                });
            }
        }));
        cx.notify();
    }

    fn editor_finished(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        let preview = self.preview.as_mut().expect("current preview");
        preview.editor_task = None;
        preview.editor_error = result.err();
        cx.notify();
    }

    fn handle_expand_preview_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.is_held
            || !is_expand_preview_chord(&event.keystroke, cx.platform_info().is_mac())
            || !self.browser_focus.contains_focused(window, cx)
            || self
                .query
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
        {
            return;
        }
        if self.tree_focus.contains_focused(window, cx)
            && self.preview.as_ref().is_some_and(|preview| {
                !matches!(&self.tree_selected, Some(TreeItem::Entry(path)) if *path == preview.selection.path)
            })
        {
            return;
        }
        if self.expand_preview(window, cx) {
            cx.stop_propagation();
        }
    }

    fn expand_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.current(cx)
            || self
                .expanded_preview
                .as_ref()
                .and_then(WeakEntity::upgrade)
                .is_some()
            || self.docked && window.has_active_dialog(cx)
        {
            return false;
        }
        let Some(preview) = &self.preview else {
            return false;
        };
        if !preview.loaded() {
            return false;
        }
        let Some(scope) = self.scope.clone() else {
            return false;
        };
        let browser = cx.entity();
        let app = self.app.clone();
        let selection = preview.selection.clone();
        let text = preview.text.clone();
        let image = preview.image.as_ref().map(ImageViewport::expanded);
        let scroll = ScrollHandle::new();
        scroll.set_offset(preview.scroll.offset());
        let scope_generation = self.scope_generation;
        let preview_generation = self.preview_generation;
        let markdown_start_offset = preview
            .markdown
            .as_ref()
            .map(|preview| preview.scroll.offset())
            .filter(|offset| offset.y != px(0.));
        let markdown = preview
            .markdown
            .as_ref()
            .zip(text.as_ref())
            .map(|(preview, text)| {
                let mut markdown = MarkdownPreview::new(preview.mode, text, cx);
                markdown.headings = preview.headings.clone();
                markdown
            });
        let return_focus = window
            .focused(cx)
            .filter(|_| self.browser_focus.contains_focused(window, cx));
        let reader = cx.new(|cx| {
            cx.observe(&browser, |_, _, cx| cx.notify()).detach();
            if let Some(app) = app.upgrade() {
                cx.observe(&app, |_, _, cx| cx.notify()).detach();
            }
            let reader = ExpandedFilePreview {
                browser: browser.downgrade(),
                app,
                scope,
                scope_generation,
                preview_generation,
                selection,
                text,
                image,
                markdown,
                markdown_start_offset,
                scroll,
                find: None,
                copied_text: None,
                focus: cx.focus_handle(),
                return_focus,
                dialog_focus: None,
                dismissed: false,
            };
            reader.observe_markdown_document(cx);
            reader
        });
        self.expanded_preview = Some(reader.downgrade());
        let focus = reader.read(cx).focus.clone();
        let reader_owner = reader.downgrade();
        let dialog_reader = reader.clone();
        // Keep the Files dialog mounted below the expanded reader in narrow windows.
        window.open_dialog(cx, move |dialog, window, _| {
            let reader_owner = reader_owner.clone();
            dialog
                .title(tr("文件预览"))
                .w((window.viewport_size().width - px(48.)).min(px(1120.)))
                .on_ok(|_, _, _| false)
                .on_close(move |_, window, cx| {
                    let _ = reader_owner.update(cx, |reader, cx| {
                        reader.restore_focus_after_close(window, cx)
                    });
                })
                .child(dialog_reader.clone())
        });
        let dialog_focus = window.focused(cx);
        reader.update(cx, |reader, _| reader.dialog_focus = dialog_focus.clone());
        window.on_next_frame(move |window, cx| {
            if dialog_focus.is_some_and(|dialog| dialog.contains_focused(window, cx)) {
                focus.focus(window, cx);
            }
        });
        true
    }

    fn file_row(&self, result: &SearchHit, active: usize, cx: &Context<Self>) -> Button {
        let hit = result.selection.clone();
        let label = hit
            .line
            .map_or_else(|| hit.path.clone(), |line| format!("{}:{line}", hit.path));
        let generation = self.search_generation;
        let selected = if self.contents {
            matches!(&self.content_results.cursor, Some(ContentPick::Hit(pick)) if *pick == hit)
        } else {
            active == self.search_selected
        };
        let row = Button::new(SharedString::from(format!(
            "browser-file-{}-{:?}",
            hit.path, hit.line
        )))
        .ghost()
        .compact()
        .w_full()
        .min_w_0()
        .justify_start()
        .selected(selected)
        .tab_stop(!self.contents)
        .icon(Glyph::FileText)
        .accessibility_label(label.clone())
        .on_click(cx.listener(move |this, _, window, cx| {
            if this.search_generation == generation
                && this
                    .search
                    .matches
                    .get(active)
                    .is_some_and(|row| row.selection == hit)
            {
                if this.contents {
                    if !this.content_results.hit_visible(&this.search.matches, &hit) {
                        return;
                    }
                    this.content_results.cursor = Some(ContentPick::Hit(hit.clone()));
                    this.content_results_focus.focus(window, cx);
                } else {
                    this.search_selected = active;
                }
                this.select(hit.clone(), cx);
            }
        }));
        let mark = HighlightStyle {
            color: Some(Color::Accent.get(cx).into()),
            font_weight: Some(FontWeight::SEMIBOLD),
            ..Default::default()
        };
        let Some((text, ranges)) = result.snippet.as_ref().and_then(SearchSnippet::display) else {
            let path = div().min_w_0().truncate();
            let path = if result.path_highlights.is_empty() {
                path.child(label.clone())
            } else {
                path.child(
                    StyledText::new(label.clone()).with_highlights(
                        result
                            .path_highlights
                            .iter()
                            .cloned()
                            .map(|range| (range, mark)),
                    ),
                )
            };
            return row.tooltip(label).child(path);
        };
        row.h(px(42.)).tooltip(format!("{label}\n{text}")).child(
            v_flex()
                .min_w_0()
                .flex_1()
                .child(div().min_w_0().truncate().child(label))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx))
                        .child(
                            StyledText::new(text)
                                .with_highlights(ranges.into_iter().map(|range| (range, mark))),
                        ),
                ),
        )
    }

    fn content_group_row(
        &self,
        path: &str,
        count: usize,
        expanded: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let selected =
            matches!(&self.content_results.cursor, Some(ContentPick::File(pick)) if pick == path);
        let generation = self.search_generation;
        let path = path.to_owned();
        div()
            .id(SharedString::from(format!("browser-content-group-{path}")))
            .role(Role::TreeItem)
            .aria_label(path.clone())
            .aria_level(1)
            .aria_expanded(expanded)
            .aria_selected(selected)
            .when(selected, |row| row.aria_active_descendant())
            .child(
                Button::new("toggle-content-group")
                    .ghost()
                    .compact()
                    .small()
                    .w_full()
                    .min_w_0()
                    .justify_start()
                    .tab_stop(false)
                    .selected(selected)
                    .icon(if expanded {
                        Glyph::ChevronDown
                    } else {
                        Glyph::ChevronRight
                    })
                    .accessibility_label(path.clone())
                    .tooltip(format!("{path}\n{}: {count}", tr("本页命中数")))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .child(path.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if this.search_generation == generation {
                            this.content_results_focus.focus(window, cx);
                            this.toggle_content_group(&path, None, cx);
                        }
                    })),
            )
            .into_any_element()
    }

    fn tree_rows(&self, cx: &App) -> Vec<TreeRow<'_>> {
        visible_tree(&self.directories, self.expanded_paths(cx))
    }

    fn git_mark(&self, path: &str, directory: bool) -> Option<ChangeMark> {
        match &self.git_status {
            TreeGitStatus::Ready(changes) => changes.mark(path, directory),
            _ => None,
        }
    }

    fn open_changed_file_diff(
        browser: WeakEntity<Self>,
        target: FileDiffTarget,
        generation: u64,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.defer(cx, move |window, cx| {
            let Some(browser) = browser.upgrade() else {
                return;
            };
            let path = target.path();
            let (scope, docked, app, dialog_focus) = {
                let browser = browser.read(cx);
                if browser.git_generation != generation
                    || !browser.current(cx)
                    || browser.git_busy(cx)
                    || !browser
                        .git_mark(path, false)
                        .is_some_and(mark_allows_file_diff)
                    || !target.matches_preview(browser.file_path_identity().as_ref())
                {
                    return;
                }
                (
                    browser.scope.clone(),
                    browser.docked,
                    browser.app.clone(),
                    browser.dialog_focus.clone(),
                )
            };

            if let FileDiffTarget::Preview {
                reader: Some(reader),
                ..
            } = &target
            {
                let Some(reader) = reader.upgrade() else {
                    return;
                };
                let state = reader.read(cx);
                if !state.current(cx)
                    || !state
                        .dialog_focus
                        .as_ref()
                        .is_some_and(|focus| focus.contains_focused(window, cx))
                {
                    return;
                }
                reader.update(cx, |reader, cx| {
                    reader.dismissed = true;
                    reader.cancel_path_request(cx);
                });
                window.close_dialog(cx);
            }
            if !docked {
                if !dialog_focus.is_some_and(|focus| focus.contains_focused(window, cx)) {
                    return;
                }
                window.close_dialog(cx);
                browser.update(cx, |browser, cx| browser.dismiss(cx));
            }
            let _ = app.update(cx, |app, cx| {
                if app.project_file_scope() == scope {
                    app.open_git_file_diff(path.to_owned(), docked, window, cx);
                }
            });
        });
    }

    fn preview_diff_button(
        &self,
        browser: WeakEntity<Self>,
        reader: Option<WeakEntity<ExpandedFilePreview>>,
        cx: &App,
    ) -> Option<Button> {
        let identity = self.file_path_identity()?;
        if self.git_busy(cx)
            || !self
                .git_mark(&identity.selection.path, false)
                .is_some_and(mark_allows_file_diff)
        {
            return None;
        }
        let id = if reader.is_some() {
            "file-reader-show-diff"
        } else {
            "browser-preview-show-diff"
        };
        let target = FileDiffTarget::Preview { identity, reader };
        let generation = self.git_generation;
        let label = tr("查看 Diff");
        Some(
            Button::new(id)
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .size(px(24.))
                .icon(Glyph::FileDiff)
                .tooltip(label)
                .accessibility_label(label)
                .on_click(move |_, window, cx| {
                    Self::open_changed_file_diff(
                        browser.clone(),
                        target.clone(),
                        generation,
                        window,
                        cx,
                    );
                }),
        )
    }

    fn tree_row(&self, row: &TreeRow<'_>, cx: &Context<Self>) -> AnyElement {
        let change = match row {
            TreeRow::Entry(entry, _) => self.git_mark(&entry.path, entry.directory),
            _ => None,
        };
        let action_path = match row {
            TreeRow::Entry(entry, _)
                if !entry.directory && change.is_some_and(mark_allows_file_diff) =>
            {
                Some(entry.path.clone())
            }
            _ => None,
        };
        let mark = change.map(|m| git_mark_appearance(m, cx));
        let (id, item, label, title, depth, enabled, icon, expanded) = match row {
            TreeRow::Entry(entry, depth) => {
                let path = entry.path.clone();
                let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
                let expanded = entry.directory.then(|| self.directory_expanded(&path, cx));
                let icon = match expanded {
                    None => Glyph::FileText,
                    Some(false) => Glyph::ChevronRight,
                    Some(true) => Glyph::ChevronDown,
                };
                (
                    format!("browser-entry-{path}"),
                    TreeItem::Entry(path.clone()),
                    name,
                    path,
                    *depth,
                    true,
                    Some(icon),
                    expanded,
                )
            }
            TreeRow::LoadMore(path, folder, depth) => {
                let label = tr(if folder.error.is_some() {
                    "重试"
                } else {
                    "加载更多"
                })
                .to_owned();
                (
                    format!("browser-more-{path}"),
                    TreeItem::LoadMore((*path).to_owned()),
                    label.clone(),
                    label,
                    *depth,
                    !folder.loading,
                    None,
                    None,
                )
            }
            TreeRow::Status(folder, depth) => {
                let (label, color) = if folder.loading {
                    (tr("正在读取…").to_owned(), Color::Muted)
                } else if let Some(error) = &folder.error {
                    (error.text(), Color::Error)
                } else {
                    (tr("此文件夹为空").to_owned(), Color::Muted)
                };
                return div()
                    .pl(px(8. + (*depth).min(12) as f32 * 12.))
                    .text_xs()
                    .text_color(color.get(cx))
                    .child(label)
                    .into_any_element();
            }
        };
        let selected = self.tree_selected.as_ref() == Some(&item);
        div()
            .id(SharedString::from(id))
            .role(Role::TreeItem)
            .aria_label(mark.map_or_else(
                || label.clone(),
                |(_, description, _)| format!("{label} · {}", tr(description)),
            ))
            .aria_level(depth + 1)
            .aria_selected(selected)
            .when(selected, |row| row.aria_active_descendant())
            .when_some(expanded, |row, expanded| row.aria_expanded(expanded))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .child(
                        Button::new("activate")
                            .role(None)
                            .ghost()
                            .compact()
                            .tab_stop(false)
                            .flex_1()
                            .min_w_0()
                            .justify_start()
                            .pl(px(8. + depth.min(12) as f32 * 12.))
                            .tooltip(mark.map_or(title.clone(), |(_, description, _)| {
                                format!("{title} · {}", tr(description))
                            }))
                            .selected(selected)
                            .disabled(!enabled)
                            .when_some(icon, |row, icon| row.icon(icon))
                            .child(div().min_w_0().truncate().child(label))
                            .when_some(
                                mark.filter(|_| action_path.is_none()),
                                |button, (marker, _, color)| {
                                    button.child(
                                        div()
                                            .ml_auto()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .text_color(color)
                                            .child(marker),
                                    )
                                },
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if this.current(cx) {
                                    this.tree_focus.focus(window, cx);
                                    this.select_tree_item(item.clone(), cx);
                                    this.activate_tree_item(item.clone(), cx);
                                }
                            })),
                    )
                    .when_some(action_path.zip(mark), |row, (path, (marker, _, color))| {
                        let title = format!("{}: {path}", tr("查看 Diff"));
                        let generation = self.git_generation;
                        row.child(
                            Button::new("show-file-diff")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .size(px(26.))
                                .flex_shrink_0()
                                .tooltip(title.clone())
                                .accessibility_label(title)
                                .child(div().text_xs().text_color(color).child(marker))
                                .on_click(cx.listener(move |_, _, window, cx| {
                                    Self::open_changed_file_diff(
                                        cx.weak_entity(),
                                        FileDiffTarget::Tree(path.clone()),
                                        generation,
                                        window,
                                        cx,
                                    );
                                })),
                        )
                    }),
            )
            .into_any_element()
    }
}

impl FileBrowser {
    fn search_rows(&self, cx: &Context<Self>) -> Vec<AnyElement> {
        if self.search_loading {
            return vec![
                div()
                    .id("browser-search-loading")
                    .p_2()
                    .text_sm()
                    .role(Role::Status)
                    .child(tr("正在搜索…"))
                    .into_any_element(),
            ];
        }
        if let Some(error) = &self.search_error {
            let notice = error.text();
            return vec![
                div()
                    .id("browser-search-error")
                    .p_2()
                    .text_sm()
                    .role(Role::Alert)
                    .aria_label(notice.clone())
                    .text_color(Color::Error.get(cx))
                    .child(notice)
                    .into_any_element(),
                Button::new("browser-retry-search")
                    .ghost()
                    .small()
                    .label(tr("重试"))
                    .on_click(cx.listener(|this, _, _, cx| this.load_search(false, cx)))
                    .into_any_element(),
            ];
        }
        if self.search.matches.is_empty() {
            return vec![
                div()
                    .id("browser-search-empty")
                    .p_2()
                    .text_sm()
                    .role(Role::Status)
                    .child(tr("没有匹配的项目文件"))
                    .into_any_element(),
            ];
        }
        if self.contents {
            return self.content_results.rows(&self.search.matches).into_iter().map(|row| {
                match row {
                    ContentRow::File { path, count, expanded } => self.content_group_row(path, count, expanded, cx),
                    ContentRow::Hit(index, hit) => {
                        let selected = matches!(&self.content_results.cursor, Some(ContentPick::Hit(pick)) if *pick == hit.selection);
                        div().id(SharedString::from(format!("browser-content-hit-{}-{:?}", hit.selection.path, hit.selection.line)))
                            .role(Role::TreeItem)
                            .aria_level(2)
                            .aria_selected(selected)
                            .when(selected, |row| row.aria_active_descendant())
                            .pl(px(12.))
                            .child(self.file_row(hit, index, cx))
                            .into_any_element()
                    }
                }
            }).collect();
        }
        self.search
            .matches
            .iter()
            .enumerate()
            .map(|(index, hit)| self.file_row(hit, index, cx).into_any_element())
            .collect()
    }

    fn set_file_split(&mut self, percent: Option<u8>, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let mut settings = crate::general::current(cx);
        if settings.file_preview_split != percent {
            settings.file_preview_split = percent;
            crate::general::set(settings, cx);
        }
        cx.notify();
    }

    fn finish_file_split(&mut self, cx: &mut Context<Self>) {
        if let Some(percent) = self.split_drag.take() {
            self.set_file_split(Some(percent), cx);
        }
    }

    fn file_split_handle(&self, split: FileSplit, cx: &Context<Self>) -> impl IntoElement {
        let focus = self.split_focus.clone();
        resize_handle("project-file-preview-resize", Axis::Vertical)
            .inside(HandleEdge::Leading)
            .role(Role::Splitter)
            .aria_label(tr(
                "调整文件列表与预览的高度，使用上下方向键，双击或按 Home 恢复默认",
            ))
            .aria_orientation(accesskit::Orientation::Horizontal)
            .aria_numeric_value(f64::from(split.list))
            .aria_min_numeric_value(f64::from(split.minimum))
            .aria_max_numeric_value(f64::from(split.maximum))
            .track_focus(&focus)
            .tab_stop(true)
            .with_appearance(Rc::new(move |_, window, cx| {
                focus.is_focused(window).then(|| {
                    div()
                        .w_full()
                        .h(px(2.))
                        .flex_none()
                        .bg(Color::Secondary.get(cx))
                        .into_any_element()
                })
            }))
            .on_drag(
                ResizeFilePreview {
                    list: split.list,
                    starting_y: Cell::default(),
                    generation: self.scope_generation,
                },
                |drag, _, window, cx| {
                    drag.starting_y.set(window.mouse_position().y);
                    cx.new(|_| drag.clone())
                },
            )
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<ResizeFilePreview>, _, cx| {
                    let drag = event.drag(cx);
                    if drag.generation != this.scope_generation || !this.current(cx) {
                        return;
                    }
                    let list =
                        drag.list + f32::from(event.event.position.y - drag.starting_y.get());
                    let percent = split.percent(list);
                    if this.split_drag != Some(percent) {
                        this.split_drag = Some(percent);
                        cx.notify();
                    }
                },
            ))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_file_split(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.finish_file_split(cx)),
            )
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                let modifiers = event.keystroke.modifiers;
                if modifiers.alt || modifiers.control || modifiers.platform || modifiers.shift {
                    return;
                }
                let percent = match event.keystroke.key.as_str() {
                    "up" => Some(split.percent(split.list - FILE_SPLIT_STEP)),
                    "down" => Some(split.percent(split.list + FILE_SPLIT_STEP)),
                    "home" => None,
                    _ => return,
                };
                this.set_file_split(percent, cx);
                cx.stop_propagation();
            }))
            .on_double_click(cx.listener(|this, _, _, cx| this.set_file_split(None, cx)))
    }

    fn toggle_image_size(&mut self, generation: u64, cx: &mut Context<Self>) {
        if self.preview_generation == generation
            && self.current(cx)
            && let Some(image) = self
                .preview
                .as_mut()
                .and_then(|preview| preview.image.as_mut())
        {
            image.toggle_size();
            cx.notify();
        }
    }

    fn preview_view(
        &self,
        height: Pixels,
        stretch: bool,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let mut panel = v_flex()
            .id("browser-compact-preview")
            .when(self.has_compact_preview(), |panel| {
                panel.key_context(COMPACT_PREVIEW_CONTEXT)
            })
            .track_focus(&self.preview_focus)
            .tab_stop(false)
            .flex_1()
            .min_w_0()
            .min_h_0()
            .gap_2();
        let Some(preview) = &self.preview else {
            return panel.child(
                div()
                    .text_sm()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("选择文件以预览")),
            );
        };
        let selection = preview.selection.clone();
        let path = preview.selection.path.clone();
        let preview_generation = self.preview_generation;
        panel = panel.child(
            h_flex()
                .justify_between()
                .gap_2()
                .child(div().min_w_0().truncate().text_sm().child(path.clone()))
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new("browser-reload-preview")
                                .ghost()
                                .small()
                                .label(tr("重新读取"))
                                .disabled(preview.loading)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select(selection.clone(), cx)
                                })),
                        )
                        .when(preview.loaded(), |actions| {
                            actions.child(
                                Button::new("browser-expand-preview")
                                    .custom(palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .size(px(24.))
                                    .icon(Glyph::Maximize2)
                                    .tooltip(expand_preview_hint(cx))
                                    .accessibility_label(tr("展开预览"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.expand_preview(window, cx);
                                    })),
                            )
                        })
                        .when_some(preview.image.as_ref(), |actions, image| {
                            actions.child(image.size_button("browser-image-size", cx).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    this.toggle_image_size(preview_generation, cx)
                                }),
                            ))
                        })
                        .children(self.preview_diff_button(cx.weak_entity(), None, cx))
                        .children(self.attach_button(cx))
                        .children(self.reveal_in_file_manager_button(cx.weak_entity(), None, cx))
                        .children(self.copy_path_menu(cx.weak_entity(), None, cx))
                        .children(self.editor_menu(cx.weak_entity(), None, cx))
                        .child(
                            Button::new("browser-close-preview")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .size(px(24.))
                                .icon(Glyph::X)
                                .accessibility_label(tr("关闭文件预览"))
                                .tooltip(format!("{} (Esc)", tr("关闭文件预览")))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    if this.preview_generation == preview_generation {
                                        this.close_compact_preview(window, cx);
                                    }
                                })),
                        ),
                ),
        );
        if let Some(error) = &preview.editor_error {
            panel = panel.child(editor_open_error(error, cx));
        }
        if let Some(status) = preview.path_status {
            panel = panel.child(file_path_status(status, cx));
        }
        if preview.loading {
            return panel.child(
                div()
                    .id("browser-preview-loading")
                    .text_sm()
                    .role(Role::Status)
                    .child(tr("正在读取…")),
            );
        }
        if let Some(error) = &preview.error {
            let message = error.text();
            return panel.child(
                div()
                    .id("browser-preview-error")
                    .text_sm()
                    .role(Role::Status)
                    .aria_label(message.clone())
                    .text_color(Color::Error.get(cx))
                    .child(message),
            );
        }
        if let Some(image) = &preview.image {
            return panel.child(
                image
                    .region("browser-preview-image", &path, height, window, cx)
                    .when(stretch, |region| region.flex_1())
                    .on_double_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_image_size(preview_generation, cx)
                    })),
            );
        }
        let Some(text) = &preview.text else {
            return panel;
        };
        let rendered = preview
            .markdown
            .as_ref()
            .filter(|markdown| markdown.active());
        let scroll = rendered.map_or(&preview.scroll, |markdown| &markdown.scroll);
        let show_jump = preview.markdown.is_some() && preview.selection.line.is_some();
        let show_pages = rendered.is_none() && text.count() > 1;
        let browser = cx.weak_entity();
        let source = self.preview_identity();
        panel
            .when_some(preview.markdown.as_ref(), |panel, markdown| {
                panel.child(markdown_mode_controls(
                    markdown.mode,
                    Self::set_preview_mode,
                    cx,
                ))
            })
            .when(
                preview.markdown.is_none() && is_markdown_file(&path) && !text.full.is_empty(),
                |panel| panel.child(markdown_limit_notice(cx)),
            )
            .child(
                text_reading::region("browser-preview".into(), path, scroll, height, cx)
                    .when(stretch, |region| region.flex_1().min_h_0())
                    .child(rendered.map_or_else(
                        || preview_text("browser-text-preview", text, true, None, cx),
                        |markdown| {
                            markdown.render(
                                "browser-markdown-preview",
                                move |destination, _, cx| {
                                    if let Some(source) = &source {
                                        let _ = browser.update(cx, |browser, cx| {
                                            if browser
                                                .preview
                                                .as_ref()
                                                .and_then(|preview| preview.markdown.as_ref())
                                                .is_some_and(MarkdownPreview::active)
                                            {
                                                browser.open_markdown_link(source, destination, cx);
                                            }
                                        });
                                    }
                                },
                                cx,
                            )
                        },
                    )),
            )
            .when(show_jump || show_pages, |panel| {
                panel.child(
                    h_flex()
                        .items_center()
                        .justify_end()
                        .flex_wrap()
                        .gap_2()
                        .when(show_jump, |actions| {
                            actions.child(
                                Button::new("browser-preview-jump-to-match")
                                    .ghost()
                                    .small()
                                    .label(tr("跳到搜索命中行"))
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.jump_to_preview_hit(cx)),
                                    ),
                            )
                        })
                        .when(show_pages, |actions| {
                            actions
                                .child(
                                    Button::new("browser-preview-before")
                                        .ghost()
                                        .small()
                                        .label(tr("上一页"))
                                        .disabled(text.page == 0)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.preview_page(-1, cx)),
                                        ),
                                )
                                .child(div().text_xs().child(format!(
                                    "{} / {}",
                                    text.page + 1,
                                    text.count()
                                )))
                                .child(
                                    Button::new("browser-preview-after")
                                        .ghost()
                                        .small()
                                        .label(tr("下一页"))
                                        .disabled(text.page + 1 >= text.count())
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.preview_page(1, cx)),
                                        ),
                                )
                        }),
                )
            })
    }
}

impl ExpandedFilePreview {
    fn current(&self, cx: &App) -> bool {
        !self.dismissed
            && self.browser.upgrade().is_some_and(|browser| {
                let browser = browser.read(cx);
                browser.scope_generation == self.scope_generation
                    && browser.preview_generation == self.preview_generation
                    && browser.scope.as_ref() == Some(&self.scope)
                    && browser.current(cx)
                    && browser.preview.as_ref().is_some_and(|preview| {
                        preview.selection == self.selection && preview.loaded()
                    })
            })
    }

    fn open_markdown_link(
        &mut self,
        destination: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.current(cx) || !self.markdown.as_ref().is_some_and(MarkdownPreview::active) {
            return;
        }
        let source = PreviewIdentity {
            scope: self.scope.clone(),
            preview_generation: self.preview_generation,
            selection: self.selection.clone(),
        };
        let target_focus = self
            .browser
            .update(cx, |browser, cx| {
                browser
                    .open_markdown_link(&source, destination, cx)
                    .then(|| browser.preview_focus.clone())
            })
            .ok()
            .flatten();
        if let Some(focus) = target_focus {
            self.dismissed = true;
            window.close_dialog(cx);
            focus.focus(window, cx);
        }
    }

    fn observe_markdown_document(&self, cx: &mut Context<Self>) {
        if let Some(document) = self.markdown.as_ref().and_then(|md| md.document.as_ref())
            && document.read(cx).rendered_text().source().is_empty()
        {
            // The outline lives in the toolbar, outside the Markdown TextView.
            let mut waiting = true;
            cx.observe(document, move |_, document, cx| {
                if waiting && !document.read(cx).rendered_text().source().is_empty() {
                    waiting = false;
                    cx.notify();
                }
            })
            .detach();
        }
    }

    fn outline_menu(&self, markdown: &MarkdownPreview, cx: &Context<Self>) -> Option<AnyElement> {
        let rendered = markdown.document.as_ref()?.read(cx).rendered_text();
        let source = rendered.source();
        if source.is_empty() {
            return None;
        }
        markdown
            .headings
            .get_or_init(|| MarkdownHeadings::parse(source))
            .outline()?;
        let headings = markdown.headings.clone();
        let reader = cx.weak_entity();
        Some(
            Button::new("file-reader-outline")
                .outline()
                .small()
                .label(tr("本页目录"))
                .dropdown_menu(move |menu, window, _| {
                    let outline = headings
                        .get()
                        .and_then(MarkdownHeadings::outline)
                        .expect("available outline");
                    let top_level = outline.iter().map(|heading| heading.level).min().unwrap();
                    let width = (window.viewport_size().width - px(32.)).min(px(360.));
                    outline.iter().enumerate().fold(
                        menu.min_w(width).max_w(width).scrollable(true),
                        |menu, (index, heading)| {
                            let title: SharedString = heading.title.clone().into();
                            let indent = px(12. * f32::from(heading.level - top_level));
                            let select = {
                                let reader = reader.clone();
                                move |cx: &mut App| {
                                    let _ = reader.update(cx, |reader, cx| {
                                        reader.select_outline_heading(index, cx);
                                    });
                                }
                            };
                            let activate = select.clone();
                            menu.item(
                                PopupMenuItem::element(move |_, _| {
                                    let activate = activate.clone();
                                    div()
                                        .id(("file-reader-outline-heading", index))
                                        .role(Role::Button)
                                        .aria_label(title.clone())
                                        .on_a11y_action(
                                            AccessibleAction::Click,
                                            move |_, window, cx| {
                                                cx.stop_propagation();
                                                window.dispatch_action(Box::new(Cancel), cx);
                                                activate(cx);
                                            },
                                        )
                                        .flex_1()
                                        .min_w_0()
                                        .pl(indent)
                                        .truncate()
                                        .child(title.clone())
                                })
                                .on_click(move |_, _, cx| select(cx)),
                            )
                        },
                    )
                })
                .into_any_element(),
        )
    }

    fn select_outline_heading(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        if let Some(markdown) = self.markdown.as_ref().filter(|md| md.active())
            && let Some(document) = &markdown.document
            && let Some(heading) = markdown
                .headings
                .get()
                .and_then(MarkdownHeadings::outline)
                .and_then(|outline| outline.get(index))
        {
            let range = heading.range.clone();
            document.update(cx, |document, cx| {
                MarkdownPreview::reveal_heading_range(document, range, cx);
            });
        }
    }

    fn copy_file_text(&mut self, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let Some(text) = &self.text else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text.full.to_string()));
        self.copied_text = Some(cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                this.copied_text = None;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn attach_button(&self, browser: &FileBrowser, cx: &Context<Self>) -> Option<Button> {
        let attachment = browser.attach_target(cx)?;
        Some(
            Button::new("file-reader-attach-to-draft")
                .outline()
                .small()
                .icon(Glyph::Paperclip)
                .label(tr("添加到草稿"))
                .disabled(!browser.can_attach_preview(&attachment.target, cx))
                .on_click(cx.listener(move |this, _, window, cx| {
                    if !this.current(cx) {
                        return;
                    }
                    let accepted = this
                        .browser
                        .update(cx, |browser, cx| {
                            browser.attach_preview(&attachment, true, window, cx)
                        })
                        .unwrap_or(false);
                    if accepted {
                        this.dismissed = true;
                        this.cancel_path_request(cx);
                    }
                })),
        )
    }

    fn request_active(reader: Option<&WeakEntity<Self>>, cx: &App) -> bool {
        reader.is_none_or(|reader| {
            reader
                .upgrade()
                .is_some_and(|reader| !reader.read(cx).dismissed)
        })
    }

    fn cancel_path_request(&self, cx: &mut Context<Self>) {
        let reader = cx.weak_entity();
        let browser = self.browser.clone();
        cx.defer(move |cx| {
            let _ = browser.update(cx, |browser, cx| {
                if let Some(preview) = &mut browser.preview
                    && preview
                        .path_request
                        .as_ref()
                        .and_then(|request| request.reader.as_ref())
                        == Some(&reader)
                {
                    preview.path_request = None;
                    cx.notify();
                }
            });
        });
    }

    fn restore_focus_after_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dismissed {
            return;
        }
        let current = self.current(cx);
        self.dismissed = true;
        self.find = None;
        self.cancel_path_request(cx);
        if let Some(browser) = self.browser.upgrade() {
            let browser = browser.read(cx);
            let docked = browser.docked;
            let fallback = browser.browser_focus.clone();
            let dock_visible = !docked
                || self
                    .app
                    .upgrade()
                    .is_some_and(|app| app.read(cx).can_dock_project_files(window));
            if current && dock_visible {
                let focus = self.return_focus.take().unwrap_or(fallback);
                focus.focus(window, cx);
                return;
            }
            if !docked && window.has_active_dialog(cx) {
                fallback.focus(window, cx);
                return;
            }
        }
        if !window.has_active_dialog(cx) {
            let composer = self.app.upgrade().and_then(|app| {
                let app = app.read(cx);
                app.viewing_conversation()
                    .then(|| app.input.read(cx).focus_handle(cx))
            });
            if let Some(focus) = composer {
                focus.focus(window, cx);
            } else {
                window.blur(cx);
            }
        }
    }

    fn page(&mut self, step: isize, cx: &mut Context<Self>) {
        if self.current(cx)
            && !self.markdown.as_ref().is_some_and(MarkdownPreview::active)
            && let Some(text) = &mut self.text
            && step_preview(text, &mut self.scroll, step)
        {
            self.clear_find_reveal();
            cx.notify();
        }
    }

    fn zoom_image(&mut self, action: ZoomAction, window: &Window, cx: &mut Context<Self>) {
        if self.current(cx)
            && let Some(image) = &mut self.image
            && image.zoom(action, window, cx)
        {
            cx.notify();
        }
    }

    fn toggle_source_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.current(cx)
            || self.markdown.as_ref().is_some_and(MarkdownPreview::active)
            || self.text.as_ref().is_none_or(|text| text.full.is_empty())
        {
            return;
        }
        let wrap = cx
            .global::<crate::appearance::Appearance>()
            .reader_word_wrap();
        crate::appearance::choose_reader_word_wrap(!wrap, window, cx);
        if wrap
            != cx
                .global::<crate::appearance::Appearance>()
                .reader_word_wrap()
        {
            let y = self.scroll.offset().y;
            self.scroll.set_offset(point(px(0.), y));
            self.refresh_find_reveal();
        }
        cx.notify();
    }

    fn set_mode(&mut self, mode: FilePreviewMode, cx: &mut Context<Self>) {
        if self.current(cx)
            && let Some(markdown) = &mut self.markdown
            && let Some(text) = &self.text
        {
            let starts_parser = mode == FilePreviewMode::Markdown && markdown.document.is_none();
            markdown.switch(mode, text, cx);
            if starts_parser {
                self.observe_markdown_document(cx);
            }
            if mode == FilePreviewMode::Markdown {
                self.find = None;
            }
            cx.notify();
        }
    }
}

impl Render for ExpandedFilePreview {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let back = Button::new("file-reader-back")
            .outline()
            .small()
            .label(tr("返回文件"))
            .tooltip(format!("{} (Esc)", tr("返回文件")))
            .on_click(cx.listener(|this, _, window, cx| {
                window.close_dialog(cx);
                this.restore_focus_after_close(window, cx);
            }));
        let view = v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .on_action(
                cx.listener(|this, _: &crate::OpenConversationFind, window, cx| {
                    this.open_find(window, cx);
                }),
            )
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                this.close_find(window, cx);
            }));
        if !self.current(cx) {
            return view
                .child(
                    div()
                        .id("file-reader-stale")
                        .role(Role::Status)
                        .child(tr("项目、会话或后台连接已变化，请关闭后重新选择。")),
                )
                .child(back);
        }
        let path = self.selection.line.map_or_else(
            || self.selection.path.clone(),
            |line| format!("{}:{line}", self.selection.path),
        );
        let markdown_limited = self.markdown.is_none()
            && is_markdown_file(&self.selection.path)
            && self.text.as_ref().is_some_and(|text| !text.full.is_empty());
        let rendered = self.markdown.as_ref().filter(|markdown| markdown.active());
        let reader = cx.weak_entity();
        let source_visible =
            rendered.is_none() && self.text.as_ref().is_some_and(|text| !text.full.is_empty());
        let wrap = !source_visible
            || cx
                .global::<crate::appearance::Appearance>()
                .reader_word_wrap();
        let extra_rows = u8::from(self.markdown.is_some() || source_visible)
            + u8::from(markdown_limited)
            + u8::from(self.find.is_some());
        let header = px(194. + 32. * f32::from(extra_rows));
        let height = (window.viewport_size().height - header).max(px(112.));
        // Wait for the background parser so the empty first layout cannot
        // clamp the inherited scroll position to zero.
        if self.markdown_start_offset.is_some()
            && let Some(markdown) = rendered
            && let Some(document) = &markdown.document
            && !document.read(cx).rendered_text().source().is_empty()
            && let Some(offset) = self.markdown_start_offset.take()
        {
            markdown.scroll.set_offset(offset);
        }
        let scroll = rendered.map_or(&self.scroll, |markdown| &markdown.scroll);
        let browser = self.browser.upgrade().expect("current file browser");
        let editor_menu =
            browser
                .read(cx)
                .editor_menu(self.browser.clone(), Some(cx.weak_entity()), cx);
        let reveal = browser.read(cx).reveal_in_file_manager_button(
            self.browser.clone(),
            Some(cx.weak_entity()),
            cx,
        );
        let attach = self.attach_button(browser.read(cx), cx);
        let diff =
            browser
                .read(cx)
                .preview_diff_button(self.browser.clone(), Some(cx.weak_entity()), cx);
        let copy_path =
            browser
                .read(cx)
                .copy_path_menu(self.browser.clone(), Some(cx.weak_entity()), cx);
        let editor_error = browser
            .read(cx)
            .preview
            .as_ref()
            .and_then(|preview| preview.editor_error.as_deref());
        let path_status = browser
            .read(cx)
            .preview
            .as_ref()
            .and_then(|preview| preview.path_status);
        let view = view
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        text_reading::selectable_text("file-reader-source", path.clone()).text_sm(),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .when_some(self.image.as_ref(), |actions, image| {
                                let reader = cx.weak_entity();
                                actions.child(image.zoom_controls(
                                    move |action, window, cx| {
                                        let _ = reader.update(cx, |this, cx| {
                                            this.zoom_image(action, window, cx)
                                        });
                                    },
                                    window,
                                    cx,
                                ))
                            })
                            .children(diff)
                            .children(attach)
                            .children(reveal)
                            .children(copy_path)
                            .when(self.text.is_some(), |actions| {
                                let copied = self.copied_text.is_some();
                                let label = tr(if copied {
                                    "已复制文件文本"
                                } else {
                                    "复制文件文本"
                                });
                                let icon = if copied {
                                    Glyph::Check
                                } else {
                                    Glyph::ClipboardCopy
                                };
                                actions.child(
                                    Button::new("file-reader-copy-text")
                                        .custom(palette::quiet_icon_variant(cx))
                                        .compact()
                                        .xsmall()
                                        .size(px(24.))
                                        .icon(icon)
                                        .accessibility_label(label)
                                        .tooltip(label)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.copy_file_text(cx)),
                                        ),
                                )
                            })
                            .children(editor_menu)
                            .child(back),
                    ),
            )
            .when_some(editor_error, |view, error| {
                view.child(editor_open_error(error, cx))
            })
            .when_some(path_status, |view, status| {
                view.child(file_path_status(status, cx))
            });
        if let Some(image) = &self.image {
            return view.child(
                image
                    .region("file-reader-image", &path, height, window, cx)
                    .track_focus(&self.focus)
                    .on_double_click(cx.listener(|this, _, window, cx| {
                        this.zoom_image(ZoomAction::ToggleFit, window, cx)
                    })),
            );
        }
        let Some(text) = &self.text else {
            return view;
        };
        view.when(self.markdown.is_some() || source_visible, |view| {
            view.child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .when_some(self.markdown.as_ref(), |row, markdown| {
                        row.child(markdown_mode_controls(markdown.mode, Self::set_mode, cx))
                    })
                    .children(rendered.and_then(|markdown| self.outline_menu(markdown, cx)))
                    .when(source_visible, |row| {
                        row.child(
                            Button::new("file-reader-wrap-source")
                                .outline()
                                .small()
                                .label(tr("自动换行"))
                                .tooltip(tr("切换文件源码自动换行"))
                                .accessibility_label(tr("切换文件源码自动换行"))
                                .selected(wrap)
                                .toggled(wrap)
                                .disabled(!crate::appearance::available(cx))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.toggle_source_wrap(window, cx)
                                })),
                        )
                        .child(
                            Button::new("file-reader-find-open")
                                .outline()
                                .compact()
                                .small()
                                .icon(Glyph::Search)
                                .tooltip(tr("在此文件中查找"))
                                .accessibility_label(tr("在此文件中查找"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.open_find(window, cx)),
                                ),
                        )
                    }),
            )
        })
        .when(markdown_limited, |view| {
            view.child(markdown_limit_notice(cx))
        })
        .children(self.find_bar(cx))
        .child(
            text_reading::source_region("file-reader".into(), path, scroll, height, wrap, cx)
                .track_focus(&self.focus)
                .child(rendered.map_or_else(
                    || {
                        preview_text(
                            "file-reader-text",
                            text,
                            wrap,
                            self.find_source_match(wrap),
                            cx,
                        )
                    },
                    |markdown| {
                        markdown.render(
                            "file-reader-markdown",
                            move |destination, window, cx| {
                                let _ = reader.update(cx, |reader, cx| {
                                    reader.open_markdown_link(destination, window, cx);
                                });
                            },
                            cx,
                        )
                    },
                )),
        )
        .child(
            h_flex()
                .items_center()
                .justify_end()
                .gap_2()
                .flex_wrap()
                .when_some(self.selection.line, |actions, line| {
                    actions.child(
                        Button::new("file-reader-jump-to-match")
                            .ghost()
                            .small()
                            .label(tr("跳到搜索命中行"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if this.current(cx)
                                    && let Some(text) = &mut this.text
                                {
                                    if let Some(markdown) = &mut this.markdown {
                                        markdown.switch(FilePreviewMode::Source, text, cx);
                                    }
                                    text.show_line(line);
                                    this.scroll = ScrollHandle::new();
                                    this.clear_find_reveal();
                                    cx.notify();
                                }
                            })),
                    )
                })
                .when(rendered.is_none() && text.count() > 1, |actions| {
                    actions
                        .child(
                            Button::new("file-reader-previous-page")
                                .ghost()
                                .small()
                                .label(tr("上一页"))
                                .disabled(text.page == 0)
                                .on_click(cx.listener(|this, _, _, cx| this.page(-1, cx))),
                        )
                        .child(div().text_xs().child(format!(
                            "{} / {}",
                            text.page + 1,
                            text.count()
                        )))
                        .child(
                            Button::new("file-reader-next-page")
                                .ghost()
                                .small()
                                .label(tr("下一页"))
                                .disabled(text.page + 1 >= text.count())
                                .on_click(cx.listener(|this, _, _, cx| this.page(1, cx))),
                        )
                }),
        )
    }
}

impl Render for FileBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut view = v_flex()
            .id("project-file-browser-body")
            .track_focus(&self.browser_focus)
            .tab_stop(false)
            .capture_key_down(cx.listener(Self::handle_expand_preview_key))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if !this.close_preview_from_key(window, cx) {
                    cx.propagate();
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if is_close_preview_key(event) && this.close_preview_from_key(window, cx) {
                    cx.stop_propagation();
                }
            }))
            .min_w_0()
            .gap_2()
            .when(self.docked, |view| view.size_full().min_h_0());
        if !self.current(cx) {
            let label = if self.docked {
                tr("项目文件暂不可用。")
            } else {
                tr("项目、会话或后台连接已变化，请关闭后重新选择。")
            };
            return view.child(
                div()
                    .id("browser-stale")
                    .role(Role::Status)
                    .text_sm()
                    .child(label),
            );
        }
        if self.git_busy(cx) {
            if !matches!(self.git_status, TreeGitStatus::Busy) {
                self.reset_git_status();
                self.git_status = TreeGitStatus::Busy;
            }
        } else if matches!(self.git_status, TreeGitStatus::Busy) {
            self.git_status = TreeGitStatus::Idle;
            let browser = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = browser.update(cx, |browser, cx| browser.load_git_status(cx));
            });
        }
        let searching = !self.query.read(cx).value().trim().is_empty();
        let rows = if searching {
            self.search_rows(cx)
        } else {
            let rows = visible_tree(&self.directories, self.expanded_paths(cx));
            if self
                .tree_selected
                .as_ref()
                .is_some_and(|item| !rows.iter().any(|row| row.item().as_ref() == Some(item)))
            {
                self.tree_selected = None;
            }
            rows.iter().map(|row| self.tree_row(row, cx)).collect()
        };
        let narrow = self.docked || window.viewport_size().width < px(690.);
        let stacked = narrow && self.preview.is_some();
        let body = window.viewport_size().height * 0.62;
        let name = if searching {
            tr("项目文件搜索结果")
        } else {
            tr("项目目录树")
        };
        let scroll = if searching {
            &self.search_scroll
        } else {
            &self.tree_scroll
        };
        let list = v_flex()
            .min_w_0()
            .when(self.docked, |list| list.flex_1().min_h_0())
            .gap_1()
            .when(!narrow, |list| list.w(px(320.)).flex_shrink_0())
            .child(
                v_flex()
                    .id("browser-list")
                    .role(if searching && !self.contents {
                        Role::List
                    } else {
                        Role::Tree
                    })
                    .aria_label(name)
                    .when(self.docked || stacked, |list| list.flex_1().min_h_0())
                    .when(!self.docked && !stacked, |list| list.h(body))
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .vertical_scrollbar(scroll)
                    .when(searching && self.contents, |list| {
                        list.track_focus(&self.content_results_focus)
                            .focus(|style| style.border_color(Color::Secondary.get(cx)))
                            .border_1()
                            .border_color(gpui::transparent_black())
                            .rounded_md()
                            .capture_key_down(cx.listener(Self::content_results_key))
                    })
                    .when(!searching, |list| {
                        list.key_context(tree_key_context(self.has_compact_preview()))
                            .track_focus(&self.tree_focus)
                            .tab_stop(true)
                            .focus(|style| style.border_color(Color::Secondary.get(cx)))
                            .border_1()
                            .border_color(gpui::transparent_black())
                            .rounded_md()
                            .on_action(cx.listener(
                                |this, _: &crate::NextSessionResult, window, cx| {
                                    this.move_tree_cursor(1, window, cx)
                                },
                            ))
                            .on_action(cx.listener(
                                |this, _: &crate::PreviousSessionResult, window, cx| {
                                    this.move_tree_cursor(-1, window, cx)
                                },
                            ))
                            .on_action(cx.listener(
                                |this, _: &crate::OpenProjectFileSelection, _, cx| {
                                    this.open_tree_selection(cx)
                                },
                            ))
                            .capture_key_down(cx.listener(Self::tree_key))
                    })
                    .children(rows),
            )
            .when(searching && self.search.limited, |list| {
                list.child(
                    div()
                        .id("browser-search-limit")
                        .role(Role::Status)
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("内容搜索仅扫描最多 64 MiB，跳过大于 1 MiB 的文件。")),
                )
            })
            .when(searching && self.search.total > PAGE_SIZE, |list| {
                list.child(
                    h_flex()
                        .justify_end()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("browser-search-before")
                                .ghost()
                                .small()
                                .label(tr("上一页"))
                                .disabled(self.search_offset == 0)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.search_offset =
                                        this.search_offset.saturating_sub(PAGE_SIZE);
                                    this.load_search(false, cx);
                                })),
                        )
                        .child(div().text_xs().child(format!(
                            "{} / {}",
                            self.search_offset + self.search.matches.len(),
                            self.search.total
                        )))
                        .child(
                            Button::new("browser-search-after")
                                .ghost()
                                .small()
                                .label(tr("下一页"))
                                .disabled(
                                    self.search_offset + self.search.matches.len()
                                        >= self.search.total,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.search_offset += PAGE_SIZE;
                                    this.load_search(false, cx);
                                })),
                        ),
                )
            });
        let panels = if stacked {
            let height = if self.split_height > px(0.) {
                self.split_height
            } else {
                body
            };
            let preferred = self
                .split_drag
                .or(crate::general::current(cx).file_preview_split);
            let split = FileSplit::new(height, preferred, self.docked);
            let share = split.list / split.available;
            let browser = cx.weak_entity();
            v_flex()
                .min_h_0()
                .when(self.docked, |panels| panels.flex_1())
                .when(!self.docked, |panels| panels.h(body).flex_shrink_0())
                .child(list.flex_1().flex_grow(share).min_h_0().overflow_hidden())
                .child(
                    div()
                        .relative()
                        .h(px(FILE_SPLIT_HANDLE))
                        .w_full()
                        .flex_none()
                        .child(self.file_split_handle(split, cx)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .flex_grow(1. - share)
                        .min_h_0()
                        .overflow_hidden()
                        .child(self.preview_view(window.viewport_size().height, true, window, cx)),
                )
                .on_prepaint(move |bounds, _, cx| {
                    let _ = browser.update(cx, |browser, cx| {
                        if browser.split_height != bounds.size.height {
                            browser.split_height = bounds.size.height;
                            cx.notify();
                        }
                    });
                })
                .into_any_element()
        } else if narrow {
            list.into_any_element()
        } else {
            h_flex()
                .items_start()
                .gap_3()
                .child(list)
                .child(self.preview_view(body - px(64.), false, window, cx))
                .into_any_element()
        };
        let source = self
            .scope
            .as_ref()
            .and_then(|scope| scope.tree.source.directory())
            .map(str::to_owned);
        view = view.when_some(source, |view, source| {
            view.child(text_reading::selectable_text("browser-directory", source).text_xs())
        });
        view = view.when_some(self.reveal_missing.as_ref(), |view, path| {
            view.child(
                div()
                    .id("browser-reveal-missing")
                    .role(Role::Status)
                    .text_xs()
                    .text_color(Color::Error.get(cx))
                    .child(format!("{} {}", tr("当前文件树中找不到此文件。"), path)),
            )
        });
        let git_notice = match &self.git_status {
            TreeGitStatus::Ready(changes) if changes.partial => {
                Some("仅显示此次读取到的部分 Git 改动。")
            }
            TreeGitStatus::Busy => Some("正在处理 Git 操作，完成后可刷新状态。"),
            TreeGitStatus::Failed(_) => Some("Git 改动暂不可用。"),
            _ => None,
        };
        view = view.when_some(git_notice, |view, notice| {
            view.child(
                div()
                    .id("file-tree-git-status")
                    .role(Role::Status)
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr(notice)),
            )
        });
        view.child(
            h_flex()
                .gap_1()
                .items_center()
                .flex_wrap()
                .child(
                    div()
                        .key_context("HiveProjectFileSearch")
                        .min_w(px(180.))
                        .flex_1()
                        .on_action(
                            cx.listener(|this, _: &crate::NextSessionResult, window, cx| {
                                this.move_search_selection(1, window, cx)
                            }),
                        )
                        .on_action(cx.listener(
                            |this, _: &crate::PreviousSessionResult, window, cx| {
                                this.move_search_selection(-1, window, cx)
                            },
                        ))
                        .on_action(cx.listener(
                            |this, _: &crate::OpenProjectFileSelection, window, cx| {
                                this.open_search_selection(window, cx)
                            },
                        ))
                        .child(
                            Input::new(&self.query)
                                .cleanable(true)
                                .aria_label(tr("搜索项目文件")),
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .flex_shrink_0()
                        .children([(false, "文件名"), (true, "文件内容")].map(
                            |(contents, label)| {
                                Button::new(("browser-search-mode", usize::from(contents)))
                                    .outline()
                                    .small()
                                    .label(tr(label))
                                    .selected(self.contents == contents)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.contents = contents;
                                        this.search_offset = 0;
                                        this.load_search(false, cx);
                                    }))
                            },
                        ))
                        .child(
                            Button::new("browser-match-case")
                                .outline()
                                .small()
                                .label("Aa")
                                .tooltip(tr("区分大小写"))
                                .accessibility_label(tr("区分大小写"))
                                .selected(self.search_flags.case_sensitive)
                                .toggled(self.search_flags.case_sensitive)
                                .on_click(cx.listener(Self::toggle_search_case)),
                        )
                        .child(
                            Button::new("browser-whole-word")
                                .outline()
                                .small()
                                .label("ab")
                                .tooltip(tr("整词匹配"))
                                .accessibility_label(tr("整词匹配"))
                                .selected(self.search_flags.whole_word)
                                .toggled(self.search_flags.whole_word)
                                .on_click(cx.listener(Self::toggle_search_whole_word)),
                        )
                        .child(
                            Button::new("browser-search-regex")
                                .outline()
                                .small()
                                .label(".*")
                                .tooltip(tr("正则表达式"))
                                .accessibility_label(tr("正则表达式"))
                                .selected(self.search_flags.regex)
                                .toggled(self.search_flags.regex)
                                .on_click(cx.listener(Self::toggle_search_regex)),
                        ),
                )
                .when(!searching, |view| {
                    view.child(
                        Button::new("file-tree-collapse-all")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .size(px(26.))
                            .icon(Glyph::ChevronUp)
                            .tooltip(tr("折叠所有文件夹"))
                            .accessibility_label(tr("折叠所有文件夹"))
                            .disabled(self.expanded_paths(cx).is_none_or(HashSet::is_empty))
                            .on_click(cx.listener(Self::collapse_all_folders)),
                    )
                })
                .when(
                    !matches!(self.git_status, TreeGitStatus::NotRepository),
                    |view| {
                        let tooltip = match &self.git_status {
                            TreeGitStatus::Failed(error) => error.as_str(),
                            _ => tr("刷新 Git 状态"),
                        };
                        view.child(
                            Button::new("file-tree-refresh-git-status")
                                .custom(palette::quiet_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .size(px(26.))
                                .icon(Glyph::RefreshCw)
                                .tooltip(tooltip.to_owned())
                                .accessibility_label(tr("刷新 Git 状态"))
                                .disabled(self.git_task.is_some())
                                .on_click(cx.listener(|this, _, _, cx| this.load_git_status(cx))),
                        )
                    },
                ),
        )
        .child(panels)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Directory, DirectoryEntry, FILE_TREE_STATUS_LIMIT, FileError, FilePreviewMode, FileReveal,
        FileTreeExpansions, MARKDOWN_MAX_BLOCK_BYTES, MARKDOWN_MAX_BYTES, MARKDOWN_MAX_LINES,
        Match, PreviewIdentity, RevealStep, Scope, TextPages, TreeItem, TreeRow, TreeScope,
        TreeSource, dock_width, external_markdown_url, initial_markdown_mode,
        is_expand_preview_chord, is_markdown_file, json, px, step_preview,
        supports_markdown_preview, unrequested_expansions, visible_tree,
    };
    use super::{FileSplit, SearchPage};
    use gpui_kit::{Keystroke, Modifiers, ScrollHandle, point};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn file_preview_split_uses_the_measured_container_and_keeps_both_panes_usable() {
        assert_eq!(FileSplit::new(px(608.), None, true).list, 300.);
        assert_eq!(FileSplit::new(px(608.), None, false).list, 240.);
        let large = FileSplit::new(px(608.), Some(62), true);
        assert_eq!(large.list, 372.);
        assert_eq!(large.percent(large.list + 24.), 66);
        assert_eq!(large.percent(large.list - 24.), 58);
        let reduced = FileSplit::new(px(308.), Some(80), false);
        assert_eq!(reduced.list, 200.);
        assert_eq!(reduced.available - reduced.list, 100.);
        assert_eq!(reduced.percent(0.), 33);
        assert_eq!(reduced.percent(300.), 67);
        let large_again = FileSplit::new(px(1008.), Some(80), false);
        assert_eq!(large_again.list, 800.);
    }

    #[test]
    fn file_name_ranges_are_optional_and_checked_against_the_exact_ordered_path() {
        let projection = |ranges| {
            let mut page = serde_json::from_value::<SearchPage>(json!({
                "paths": ["archive/readme.md", "docs/🙂-Cafe\u{301}.md", "plain.txt"],
                "ranges": ranges,
                "total": 103,
            }))
            .unwrap();
            page.append_name_paths();
            assert_eq!(page.total, 103);
            assert_eq!(page.matches[1].selection.path, "docs/🙂-Cafe\u{301}.md");
            assert_eq!(page.matches[1].selection.line, None);
            page.matches
                .into_iter()
                .map(|hit| hit.path_highlights)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            projection(json!([[[0, 7]], [[10, 16]], "malformed"])),
            [vec![0..7], vec![10..16], vec![]]
        );
        assert_eq!(
            projection(json!([[[0, 7]], [[6, 9]], [[2, 1]]])),
            [vec![0..7], vec![], vec![]]
        );
        for ranges in [json!(null), json!([[[0, 7]]]), json!({"wrong": "shape"})] {
            assert!(projection(ranges).iter().all(Vec::is_empty));
        }
    }

    #[test]
    fn content_results_accept_old_payloads_and_keep_snippets_out_of_selection_identity() {
        let page = serde_json::from_value::<SearchPage>(json!({
            "matches": [
                {"path": "docs/ReadMe.md", "line": 350},
                {"path": "docs/ReadMe.md", "line": 620, "snippet": {"text": "original", "ranges": [[0, 8]]}},
            ],
            "total": 2,
        })).unwrap();
        assert_eq!(page.matches[0].selection.path, "docs/ReadMe.md");
        assert_eq!(page.matches[0].selection.line, Some(350));
        assert!(page.matches[0].snippet.is_none());
        assert_eq!(page.matches[1].selection.line, Some(620));
        assert_eq!(
            page.matches[1]
                .snippet
                .as_ref()
                .unwrap()
                .display()
                .unwrap()
                .0,
            "original"
        );
    }

    #[test]
    fn file_dock_preserves_chat_space_and_the_existing_dialog_threshold() {
        assert_eq!(dock_width(None, px(919.)), None);
        assert_eq!(dock_width(None, px(920.)), Some(px(320.)));
        assert_eq!(dock_width(Some(px(250.)), px(1200.)), Some(px(320.)));
        assert_eq!(dock_width(Some(px(700.)), px(1200.)), Some(px(600.)));
    }

    #[test]
    fn file_dock_resolves_the_same_preference_as_available_space_changes() {
        let preferred = Some(px(480.));
        assert_eq!(dock_width(preferred, px(1200.)), preferred);
        assert_eq!(dock_width(preferred, px(1000.)), Some(px(400.)));
        assert_eq!(dock_width(preferred, px(900.)), None);
        assert_eq!(dock_width(preferred, px(1200.)), preferred);
    }

    #[test]
    fn preview_expand_accepts_only_the_platform_modified_enter() {
        let command = Modifiers {
            platform: true,
            ..Default::default()
        };
        let control = Modifiers {
            control: true,
            ..Default::default()
        };
        let key = |key: &str, modifiers| Keystroke {
            key: key.into(),
            key_char: None,
            modifiers,
        };
        assert!(is_expand_preview_chord(&key("enter", command), true));
        assert!(is_expand_preview_chord(&key("enter", control), false));
        assert!(!is_expand_preview_chord(&key("enter", control), true));
        assert!(!is_expand_preview_chord(&key("enter", command), false));
        assert!(!is_expand_preview_chord(
            &key("enter", Modifiers::none()),
            true
        ));
        assert!(!is_expand_preview_chord(
            &key(
                "enter",
                Modifiers {
                    shift: true,
                    ..command
                }
            ),
            true
        ));
        assert!(!is_expand_preview_chord(&key("escape", command), true));
    }

    #[test]
    fn close_preview_key_only_accepts_an_unmodified_escape_press() {
        let event = |key: &str, modifiers, is_held| gpui_kit::KeyDownEvent {
            keystroke: Keystroke {
                key: key.into(),
                key_char: None,
                modifiers,
            },
            is_held,
            prefer_character_input: false,
        };
        let plain = Modifiers::default();
        assert!(super::is_close_preview_key(&event("escape", plain, false)));
        assert!(!super::is_close_preview_key(&event("escape", plain, true)));
        assert!(!super::is_close_preview_key(&gpui_kit::KeyDownEvent {
            prefer_character_input: true,
            ..event("escape", plain, false)
        }));
        assert!(!super::is_close_preview_key(&event("enter", plain, false)));
        assert!(!super::is_close_preview_key(&event(
            "escape",
            Modifiers {
                shift: true,
                ..plain
            },
            false,
        )));
    }

    #[test]
    fn active_file_preview_escape_precedes_the_outer_conversation_binding() {
        use gpui_kit::{KeyBinding, KeyContext, Keymap, component::dialog::Cancel};

        let keymap = Keymap::new(vec![
            KeyBinding::new(
                "escape",
                crate::CloseConversationFind,
                Some("HiveConversation"),
            ),
            KeyBinding::new("escape", Cancel, Some(super::COMPACT_PREVIEW_CONTEXT)),
        ]);
        let first_binding = |context| {
            let (bindings, _) = keymap.bindings_for_input(
                &[Keystroke::parse("escape").unwrap()],
                &[
                    KeyContext::parse("HiveConversation").unwrap(),
                    KeyContext::parse(context).unwrap(),
                ],
            );
            bindings.into_iter().next().unwrap()
        };

        for context in [
            super::tree_key_context(true),
            super::COMPACT_PREVIEW_CONTEXT,
        ] {
            assert!(first_binding(context).action().as_any().is::<Cancel>());
        }
        for context in [super::tree_key_context(false), "HiveProjectFileSearch"] {
            assert!(
                first_binding(context)
                    .action()
                    .as_any()
                    .is::<crate::CloseConversationFind>()
            );
        }
        assert!(
            KeyContext::parse(super::tree_key_context(true))
                .unwrap()
                .contains("HiveProjectFileTree")
        );
    }

    fn session_with_execution_path(id: &str, path: Option<&str>) -> crate::protocol::Session {
        serde_json::from_value(json!({
            "id": id,
            "title": id,
            "workspaceId": "w",
            "executionPath": path,
        }))
        .unwrap()
    }

    #[test]
    fn expansions_follow_the_execution_directory_of_each_workspace() {
        let root = "/project";
        let key = |workspace: &str, session: Option<&crate::protocol::Session>| TreeScope {
            workspace: workspace.to_owned(),
            source: TreeSource::new(session, Some(root)),
        };
        let root_chat = session_with_execution_path("root-chat", Some(root));
        let imported = session_with_execution_path("imported", Some("/project/subfolder"));
        let worktree = session_with_execution_path("worktree", Some("/managed/worktree"));
        let unknown = session_with_execution_path("unknown", None);
        let other_unknown = session_with_execution_path("other-unknown", None);
        let mut expansions = FileTreeExpansions::default();
        assert!(expansions.toggle(key("w", Some(&root_chat)), "docs".into()));
        assert!(
            expansions
                .paths
                .get(&key("w", None))
                .unwrap()
                .contains("docs")
        );
        assert!(
            !expansions
                .paths
                .contains_key(&key("other-workspace", Some(&root_chat)))
        );
        assert!(!expansions.paths.contains_key(&key("w", Some(&imported))));
        assert!(!expansions.paths.contains_key(&key("w", Some(&worktree))));
        assert!(expansions.toggle(key("w", Some(&unknown)), "docs".into()));
        assert!(
            !expansions
                .paths
                .contains_key(&key("w", Some(&other_unknown)))
        );
    }

    #[test]
    fn compact_preview_restores_only_the_latest_file_in_its_exact_tree() {
        let tree = |workspace: &str, directory: &str| TreeScope {
            workspace: workspace.into(),
            source: TreeSource::Directory(directory.into()),
        };
        let a = tree("project", "/tree/a");
        let b = tree("project", "/tree/b");
        let different_project = tree("other-project", "/tree/a");
        let selection = Match {
            path: "docs/notes.md".into(),
            line: Some(350),
        };
        let owner = gpui_kit::EntityId::from(1);
        let next_owner = gpui_kit::EntityId::from(2);
        let mut state = FileTreeExpansions {
            last_preview: Some(super::RememberedPreview {
                owner,
                tree: a.clone(),
                selection: selection.clone(),
                directory: Some("/tree/a".into()),
                reading: None,
            }),
            ..Default::default()
        };
        assert!(state.restore_preview(&b, next_owner).is_none());
        assert!(
            state
                .restore_preview(&different_project, next_owner)
                .is_none()
        );
        state.forget_preview(&b);
        let saved = state.restore_preview(&a, next_owner).unwrap();
        assert_eq!(saved.owner, next_owner);
        assert_eq!(saved.directory.as_deref(), Some("/tree/a"));
        assert!(saved.selection == selection);

        state.last_preview = Some(super::RememberedPreview {
            tree: b.clone(),
            directory: Some("/tree/b".into()),
            ..saved
        });
        assert!(state.restore_preview(&a, owner).is_none());
        state.forget_preview(&b);
        assert!(state.restore_preview(&b, owner).is_none());
    }

    #[test]
    fn compact_preview_restores_paging_from_fresh_text_and_clamps_if_it_shrinks() {
        let text = |lines, marker| {
            let bytes = (1..=lines)
                .map(|line| format!("{marker} {line}\n"))
                .collect::<String>();
            TextPages::new(bytes.into())
        };
        let mut original = text(800, "old");
        original.show_line(350);
        assert_eq!(original.first_line, 350);
        let hit = super::PreviewReadingPosition {
            page: original.page,
            source_scroll: gpui_kit::point(px(0.), px(-12.)),
            markdown: None,
        };
        let mut refreshed = text(800, "new");
        refreshed.show_line(350);
        hit.restore_page(&mut refreshed);
        assert!(refreshed.visible.starts_with("new 350\n"));

        assert!(original.show(original.page + 1));
        let later_page = super::PreviewReadingPosition {
            page: original.page,
            ..hit
        };
        later_page.restore_page(&mut refreshed);
        assert_eq!(refreshed.first_line, original.first_line);
        assert!(refreshed.visible.starts_with("new "));

        let mut shorter = text(300, "short");
        shorter.show_line(350);
        later_page.restore_page(&mut shorter);
        assert_eq!(shorter.page, shorter.count() - 1);
        assert!(shorter.visible.ends_with("short 300\n"));
    }

    #[test]
    fn collapse_all_clears_only_the_current_scope_and_keeps_the_cursor_visible() {
        let scope = |directory: &str| TreeScope {
            workspace: "workspace".into(),
            source: TreeSource::Directory(directory.into()),
        };
        let alpha = scope("/worktree/alpha");
        let beta = scope("/worktree/beta");
        let mut expansions = FileTreeExpansions::default();
        for path in ["docs", "docs/specs", "src"] {
            expansions.expand(alpha.clone(), path.into());
        }
        expansions.expand(beta.clone(), "docs".into());
        assert!(expansions.collapse_all(&alpha));
        assert!(!expansions.collapse_all(&alpha));
        assert_eq!(
            expansions.paths.get(&beta),
            Some(&HashSet::from(["docs".into()]))
        );

        let entry = |path: &str, directory| DirectoryEntry {
            path: path.into(),
            directory,
        };
        let mut directories = HashMap::from([(
            String::new(),
            Directory {
                entries: vec![entry("docs", true), entry("README.md", false)],
                next_offset: 2,
                total: 3,
                ..Directory::default()
            },
        )]);
        // Pages that arrive after collapse can populate the cache without
        // making descendants visible or reselecting their old retry row.
        directories.insert(
            "docs".into(),
            Directory {
                entries: vec![entry("docs/specs", true)],
                ..Directory::default()
            },
        );
        assert_eq!(
            visible_tree(&directories, expansions.paths.get(&alpha))
                .iter()
                .filter_map(TreeRow::item)
                .collect::<Vec<_>>(),
            [
                TreeItem::Entry("docs".into()),
                TreeItem::Entry("README.md".into()),
                TreeItem::LoadMore(String::new())
            ]
        );
        let selection = |item| super::collapsed_tree_selection(&directories, &item);
        assert_eq!(
            selection(TreeItem::Entry("docs/specs/note.md".into())),
            Some((0, TreeItem::Entry("docs".into())))
        );
        assert_eq!(
            selection(TreeItem::LoadMore("docs/specs".into())),
            Some((0, TreeItem::Entry("docs".into())))
        );
        assert_eq!(
            selection(TreeItem::Entry("README.md".into())),
            Some((1, TreeItem::Entry("README.md".into())))
        );
        assert_eq!(
            selection(TreeItem::LoadMore(String::new())),
            Some((2, TreeItem::LoadMore(String::new())))
        );
        assert_eq!(selection(TreeItem::Entry("gone/notes.md".into())), None);
    }

    #[test]
    fn keyboard_tree_preview_requires_an_existing_file_entry() {
        let make_entry = |path: &str, directory| DirectoryEntry {
            path: path.into(),
            directory,
        };
        let directories = HashMap::from([
            (
                String::new(),
                Directory {
                    entries: vec![make_entry("docs", true), make_entry("README.md", false)],
                    ..Directory::default()
                },
            ),
            (
                "docs".into(),
                Directory {
                    entries: vec![make_entry("docs/README.md", false)],
                    ..Directory::default()
                },
            ),
        ]);
        let root_file = TreeItem::Entry("README.md".into());
        let nested_file = TreeItem::Entry("docs/README.md".into());
        assert_eq!(root_file.file_path(&directories), Some("README.md"));
        assert_eq!(nested_file.file_path(&directories), Some("docs/README.md"));
        assert_eq!(TreeItem::Entry("docs".into()).file_path(&directories), None);
        assert_eq!(
            TreeItem::Entry("gone/README.md".into()).file_path(&directories),
            None
        );
        assert_eq!(
            TreeItem::LoadMore("docs".into()).file_path(&directories),
            None
        );
    }

    #[test]
    fn resolved_file_path_uses_the_worktree_and_omits_the_search_line() {
        let preview = PreviewIdentity {
            scope: Scope {
                tree: TreeScope {
                    workspace: "project".into(),
                    source: TreeSource::Directory("/worktree".into()),
                },
                session: Some("worktree-session".into()),
                connection: 1,
                open_request: 2,
            },
            preview_generation: 3,
            selection: Match {
                path: "docs/with spaces.md".into(),
                line: Some(42),
            },
        };
        assert_eq!(
            preview.file_params(),
            json!({
                "workspaceId":"project",
                "sessionId":"worktree-session",
                "relativePath":"docs/with spaces.md"
            })
        );
    }

    #[test]
    fn git_status_uses_the_exact_worktree_session_and_scope() {
        let project = Scope {
            tree: TreeScope {
                workspace: "workspace".into(),
                source: TreeSource::Directory("/project".into()),
            },
            session: None,
            connection: 1,
            open_request: 2,
        };
        let worktree = Scope {
            tree: TreeScope {
                workspace: "workspace".into(),
                source: TreeSource::Directory("/worktree".into()),
            },
            session: Some("worktree-session".into()),
            ..project.clone()
        };
        let scan = || json!({"offset":0,"limit":FILE_TREE_STATUS_LIMIT});
        assert_eq!(
            worktree.params(scan()),
            json!({"offset":0,"limit":500,"workspaceId":"workspace","sessionId":"worktree-session"})
        );
        assert_eq!(
            project.params(scan()),
            json!({"offset":0,"limit":500,"workspaceId":"workspace"})
        );
        let reopened = Scope {
            open_request: 3,
            ..worktree.clone()
        };
        assert!(worktree != reopened);
        assert!(worktree != project);
    }

    #[test]
    fn markdown_preview_keeps_content_hits_in_source_and_limits_full_documents() {
        assert!(is_markdown_file("docs/README.MD"));
        assert!(is_markdown_file("docs/example.markdown"));
        assert!(!is_markdown_file("docs/README.md.txt"));

        let mut selection = Match {
            path: "docs/README.md".into(),
            line: None,
        };
        assert!(initial_markdown_mode(&selection) == FilePreviewMode::Markdown);
        selection.line = Some(350);
        assert!(initial_markdown_mode(&selection) == FilePreviewMode::Source);

        assert!(!supports_markdown_preview(""));
        let mut at_size = format!("{}\n\n", "a".repeat(4094)).repeat(MARKDOWN_MAX_BYTES / 4096);
        assert!(supports_markdown_preview(&at_size));
        at_size.push('a');
        assert!(!supports_markdown_preview(&at_size));
        assert!(supports_markdown_preview(&"a\n".repeat(MARKDOWN_MAX_LINES)));
        assert!(supports_markdown_preview(
            &"a\r\n".repeat(MARKDOWN_MAX_LINES)
        ));
        assert!(!supports_markdown_preview(
            &"a\r".repeat(MARKDOWN_MAX_LINES + 1)
        ));
        assert!(!supports_markdown_preview(&format!(
            "{}b",
            "a\n".repeat(MARKDOWN_MAX_LINES)
        )));
        assert!(!supports_markdown_preview(
            &"a\n".repeat(MARKDOWN_MAX_LINES + 1)
        ));
    }

    #[test]
    fn markdown_preview_bounds_the_size_of_each_run_of_nonblank_lines() {
        let long_line = "a".repeat(MARKDOWN_MAX_BLOCK_BYTES);
        assert!(supports_markdown_preview(&long_line));
        assert!(!supports_markdown_preview(&format!("{long_line}a")));
        let soft_wrapped = "one sentence followed by a soft break\n".repeat(300);
        assert!(!supports_markdown_preview(&soft_wrapped));
        for line_break in ["\n", "\r\n", "\r"] {
            let short_paragraphs = format!("a sentence{line_break} \t{line_break}").repeat(350);
            assert!(supports_markdown_preview(&short_paragraphs));
        }
    }

    #[test]
    fn markdown_links_allow_only_absolute_web_destinations() {
        assert_eq!(
            external_markdown_url("https://example.com/readme#details")
                .unwrap()
                .as_str(),
            "https://example.com/readme#details"
        );
        assert!(external_markdown_url("http://localhost:8080/").is_some());
        for destination in [
            "#details",
            "./other.md",
            "//example.com/readme",
            "file:///tmp/readme.md",
            "data:text/plain,hello",
            "mailto:example@example.com",
            "hive://open",
        ] {
            assert!(
                external_markdown_url(destination).is_none(),
                "{destination}"
            );
        }
    }

    #[test]
    fn expanded_preview_pages_independently_and_can_return_to_the_exact_hit() {
        let sample = (1..=640)
            .map(|line| format!("Fixture line {line:03}\n"))
            .collect::<String>();
        let mut preview = TextPages::new(sample.into());
        preview.show_line(350);
        let preview_page = preview.page;
        let preview_scroll = ScrollHandle::new();
        let previous_offset = point(px(0.), px(-42.));
        preview_scroll.set_offset(previous_offset);

        let mut expanded = preview.clone();
        let mut expanded_scroll = ScrollHandle::new();
        expanded_scroll.set_offset(preview_scroll.offset());
        assert_eq!(expanded.first_line, 350);
        assert!(expanded.visible.starts_with("Fixture line 350\n"));
        assert!(step_preview(&mut expanded, &mut expanded_scroll, 1));
        assert_eq!(expanded.first_line, 513);
        assert_eq!(expanded_scroll.offset(), point(px(0.), px(0.)));
        assert_eq!(preview.page, preview_page);
        assert_eq!(preview_scroll.offset(), previous_offset);
        expanded.show_line(350);
        assert_eq!(expanded.page, preview_page);
        assert_eq!(expanded.visible, preview.visible);
    }

    #[test]
    fn pending_file_action_cannot_follow_a_reopened_file_or_changed_scope() {
        let requested = PreviewIdentity {
            scope: Scope {
                tree: TreeScope {
                    workspace: "workspace".into(),
                    source: TreeSource::Directory("/worktree".into()),
                },
                session: Some("worktree-session".into()),
                connection: 1,
                open_request: 2,
            },
            preview_generation: 3,
            selection: Match {
                path: "docs/note.txt".into(),
                line: Some(350),
            },
        };
        assert!(requested == requested.clone());
        let diff = super::FileDiffTarget::Preview {
            identity: requested.clone(),
            reader: None,
        };
        assert_eq!(diff.path(), "docs/note.txt");
        assert!(diff.matches_preview(Some(&requested)));
        assert!(!diff.matches_preview(None));
        for changed in [
            PreviewIdentity {
                preview_generation: 4,
                ..requested.clone()
            },
            PreviewIdentity {
                scope: Scope {
                    session: Some("other-session".into()),
                    ..requested.scope.clone()
                },
                ..requested.clone()
            },
            PreviewIdentity {
                scope: Scope {
                    tree: TreeScope {
                        source: TreeSource::Directory("/other-worktree".into()),
                        ..requested.scope.tree.clone()
                    },
                    ..requested.scope.clone()
                },
                ..requested.clone()
            },
            PreviewIdentity {
                scope: Scope {
                    connection: 4,
                    ..requested.scope.clone()
                },
                ..requested.clone()
            },
            PreviewIdentity {
                scope: Scope {
                    open_request: 4,
                    ..requested.scope.clone()
                },
                ..requested.clone()
            },
            PreviewIdentity {
                selection: Match {
                    path: requested.selection.path.clone(),
                    line: None,
                },
                ..requested.clone()
            },
        ] {
            assert!(requested != changed);
            assert!(!diff.matches_preview(Some(&changed)));
        }
    }

    #[test]
    fn remembered_expansions_wait_for_fresh_parent_entries_and_skip_failures() {
        let dir = |path: &str| DirectoryEntry {
            path: path.into(),
            directory: true,
        };
        let mut expanded = HashSet::from(["docs".into(), "docs/nested".into(), "removed".into()]);
        let mut directories = HashMap::from([(
            "".into(),
            Directory {
                entries: vec![dir("docs")],
                loaded: true,
                ..Default::default()
            },
        )]);
        assert_eq!(unrequested_expansions(&directories, &expanded), ["docs"]);
        directories.insert(
            "docs".into(),
            Directory {
                loaded: true,
                total: 101,
                next_offset: 100,
                ..Default::default()
            },
        );
        assert!(unrequested_expansions(&directories, &expanded).is_empty());
        directories
            .get_mut("docs")
            .unwrap()
            .entries
            .push(dir("docs/nested"));
        assert_eq!(
            unrequested_expansions(&directories, &expanded),
            ["docs/nested"]
        );
        expanded.remove("docs");
        assert!(unrequested_expansions(&directories, &expanded).is_empty());
        expanded.insert("docs".into());
        directories.insert(
            "docs/nested".into(),
            Directory {
                error: Some(FileError::Request("Offline".into())),
                ..Default::default()
            },
        );
        assert!(unrequested_expansions(&directories, &expanded).is_empty());
        let rows = visible_tree(&directories, Some(&expanded));
        assert!(rows.iter().any(|row| matches!(row, TreeRow::LoadMore("docs/nested", folder, _) if folder.error.is_some())));
    }

    #[test]
    fn reveal_expands_cached_ancestors_and_pages_until_the_exact_file() {
        let reveal = FileReveal {
            path: "docs/nested/目标.txt".into(),
        };
        let entry = |path: &str, directory| DirectoryEntry {
            path: path.into(),
            directory,
        };
        let mut directories = HashMap::new();
        let mut expanded = HashSet::new();
        assert_eq!(reveal.step(&directories, None), RevealStep::Load(""));
        directories.insert(
            "".into(),
            Directory {
                entries: vec![entry("docs", true)],
                loaded: true,
                ..Default::default()
            },
        );
        directories.insert(
            "docs".into(),
            Directory {
                entries: vec![entry("docs/nested", true)],
                loaded: true,
                ..Default::default()
            },
        );
        assert_eq!(
            reveal.step(&directories, Some(&expanded)),
            RevealStep::Expand("docs")
        );
        expanded.insert("docs".into());
        assert_eq!(
            reveal.step(&directories, Some(&expanded)),
            RevealStep::Expand("docs/nested")
        );
        expanded.insert("docs/nested".into());
        assert_eq!(
            reveal.step(&directories, Some(&expanded)),
            RevealStep::Load("docs/nested")
        );
        directories.insert(
            "docs/nested".into(),
            Directory {
                entries: vec![entry("docs/nested/另一目标.txt", false)],
                total: 101,
                next_offset: 100,
                loaded: true,
                ..Default::default()
            },
        );
        assert_eq!(
            reveal.step(&directories, Some(&expanded)),
            RevealStep::Load("docs/nested")
        );
        let folder = directories.get_mut("docs/nested").unwrap();
        folder.next_offset = 101;
        folder.entries.push(entry("docs/nested/目标.txt", false));
        assert_eq!(
            reveal.step(&directories, Some(&expanded)),
            RevealStep::Select
        );
        directories.clear();
        assert_eq!(
            reveal.step(&directories, Some(&expanded)),
            RevealStep::Load("")
        );
    }

    #[test]
    fn reveal_waits_for_failed_reads_and_reports_missing_or_replaced_files() {
        let reveal = FileReveal {
            path: "removed.txt".into(),
        };
        let mut root = HashMap::from([(
            "".into(),
            Directory {
                error: Some(FileError::Request("Retry this read".into())),
                ..Default::default()
            },
        )]);
        assert_eq!(reveal.step(&root, None), RevealStep::Wait);
        let folder = root.get_mut("").unwrap();
        folder.error = None;
        folder.loaded = true;
        assert_eq!(reveal.step(&root, None), RevealStep::Missing);
        root.get_mut("").unwrap().entries.push(DirectoryEntry {
            path: "removed.txt".into(),
            directory: true,
        });
        assert_eq!(reveal.step(&root, None), RevealStep::Missing);
        let traversal = FileReveal {
            path: "docs/../outside.txt".into(),
        };
        assert_eq!(traversal.step(&root, None), RevealStep::Missing);
    }
}
