use super::FileError;
use crate::{
    app::HiveApp,
    language::tr,
    palette::{self, Color},
    text_reading::{self, TextPages},
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
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};

const PAGE_SIZE: usize = 100;

#[derive(Deserialize)]
struct DirectoryEntry {
    path: String,
    directory: bool,
}

#[derive(Default)]
struct Directory {
    entries: Vec<DirectoryEntry>,
    expanded: bool,
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

#[derive(Clone, Deserialize)]
struct Match {
    path: String,
    line: Option<usize>,
}

#[derive(Default, Deserialize)]
struct SearchPage {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    matches: Vec<Match>,
    total: usize,
    #[serde(default)]
    limited: bool,
}

struct Preview {
    selection: Match,
    loading: bool,
    text: Option<TextPages>,
    error: Option<FileError>,
    scroll: ScrollHandle,
}

struct FileBrowser {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    workspace: String,
    session: Option<String>,
    connection: u64,
    open_request: u64,
    source_directory: Option<String>,
    dismissed: bool,
    query: Entity<InputState>,
    contents: bool,
    directories: HashMap<String, Directory>,
    scroll: ScrollHandle,
    search: SearchPage,
    search_offset: usize,
    search_loading: bool,
    search_error: Option<FileError>,
    search_task: Option<Task<()>>,
    preview: Option<Preview>,
    preview_task: Option<Task<()>>,
}

impl HiveApp {
    pub(crate) fn can_browse_project_files(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && self.runtime.is_some()
            && self.workspace.is_some()
    }

    pub(crate) fn project_file_browser_button(
        &self,
        id: &'static str,
        cx: &Context<Self>,
    ) -> Button {
        Button::new(id)
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .h(px(26.))
            .w(px(26.))
            .rounded_md()
            .icon(Glyph::FolderOpen)
            .tooltip(tr("浏览项目文件"))
            .accessibility_label(tr("浏览项目文件"))
            .disabled(!self.can_browse_project_files())
            .on_click(cx.listener(|this, _, window, cx| this.open_project_file_browser(window, cx)))
    }

    pub(crate) fn open_project_file_browser(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_browse_project_files() || window.has_active_dialog(cx) {
            return;
        }
        let (Some(runtime), Some(workspace)) = (self.runtime.clone(), self.workspace.clone())
        else {
            return;
        };
        let source_directory = self
            .session
            .as_ref()
            .and_then(|session| session.execution.as_ref())
            .filter(|execution| execution.kind == "worktree")
            .map(|execution| execution.path.clone())
            .or_else(|| {
                self.workspaces
                    .iter()
                    .find(|project| project.id == workspace)
                    .map(|project| project.path.clone())
            });
        let session = self.session.as_ref().map(|session| session.id.clone());
        let connection = self.connection_generation;
        let open_request = self.open_request;
        let app = cx.weak_entity();
        let browser = cx.new(|cx| {
            let query = cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索项目文件…")));
            cx.subscribe(
                &query,
                |this: &mut FileBrowser, _, event: &InputEvent, cx| match event {
                    InputEvent::Change => {
                        this.search_offset = 0;
                        this.load_search(true, cx);
                    }
                    InputEvent::PressEnter {
                        secondary: false,
                        shift: false,
                    } => {
                        if let Some(hit) = this.search.matches.first().cloned() {
                            this.select(hit, cx);
                        }
                    }
                    _ => {}
                },
            )
            .detach();
            if let Some(parent) = app.upgrade() {
                cx.observe(&parent, |this: &mut FileBrowser, _, cx| {
                    if !this.current(cx) {
                        this.dismiss();
                    }
                    cx.notify();
                })
                .detach();
            }
            let pending = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = pending.update(cx, |this, cx| this.load_directory(String::new(), cx));
            });
            FileBrowser {
                app,
                runtime,
                workspace,
                session,
                connection,
                open_request,
                source_directory,
                dismissed: false,
                query,
                contents: false,
                directories: HashMap::new(),
                scroll: ScrollHandle::new(),
                search: SearchPage::default(),
                search_offset: 0,
                search_loading: false,
                search_error: None,
                search_task: None,
                preview: None,
                preview_task: None,
            }
        });
        let focus = browser.read(cx).query.read(cx).focus_handle(cx);
        let width = (window.viewport_size().width - px(44.)).min(px(1040.));
        let dismiss = browser.downgrade();
        window.open_dialog(cx, move |dialog, _, _| {
            let dismiss = dismiss.clone();
            dialog
                .title(tr("浏览项目文件"))
                .w(width)
                .on_ok(|_, _, _| false)
                .on_close(move |_, _, cx| {
                    let _ = dismiss.update(cx, |this, _| this.dismiss());
                })
                .child(browser.clone())
        });
        let dialog_focus = window.focused(cx);
        window.on_next_frame(move |window, cx| {
            if dialog_focus.is_some_and(|root| root.contains_focused(window, cx)) {
                focus.focus(window, cx);
            }
        });
    }
}

impl FileBrowser {
    fn current(&self, cx: &App) -> bool {
        !self.dismissed
            && self.app.upgrade().is_some_and(|app| {
                let app = app.read(cx);
                app.can_browse_project_files()
                    && app.connection_generation == self.connection
                    && app.open_request == self.open_request
                    && app.workspace.as_ref() == Some(&self.workspace)
                    && app.session.as_ref().map(|session| &session.id) == self.session.as_ref()
            })
    }

    fn dismiss(&mut self) {
        self.dismissed = true;
        self.search_task = None;
        self.preview_task = None;
        self.preview = None;
    }

    fn params(&self, mut params: Value) -> Value {
        params["workspaceId"] = json!(self.workspace);
        if let Some(session) = &self.session {
            params["sessionId"] = json!(session);
        }
        params
    }

    fn load_directory(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let folder = self.directories.entry(path.clone()).or_default();
        if folder.loading {
            return;
        }
        folder.expanded = true;
        folder.loading = true;
        folder.error = None;
        let offset = folder.next_offset;
        let params = self.params(json!({"directory":path, "offset":offset}));
        let runtime = self.runtime.clone();
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
                if !this.current(cx) {
                    return;
                }
                let Some(folder) = this.directories.get_mut(&path) else {
                    return;
                };
                folder.loading = false;
                match result {
                    Ok(page) => {
                        folder.loaded = true;
                        folder.total = page.total;
                        folder.next_offset += page.entries.len();
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
                cx.notify();
            });
        })
        .detach();
    }

    fn toggle_directory(&mut self, path: String, cx: &mut Context<Self>) {
        let folder = self.directories.entry(path.clone()).or_default();
        folder.expanded = !folder.expanded;
        if folder.expanded && !folder.loaded {
            self.load_directory(path, cx);
        } else {
            cx.notify();
        }
    }

    fn load_search(&mut self, debounce: bool, cx: &mut Context<Self>) {
        self.search_task = None;
        self.search = SearchPage::default();
        self.search_error = None;
        self.search_loading = false;
        self.scroll = ScrollHandle::new();
        cx.notify();
        let query = self.query.read(cx).value().trim().to_owned();
        if query.is_empty() || !self.current(cx) {
            return;
        }
        self.search_loading = true;
        let runtime = self.runtime.clone();
        let contents = self.contents;
        let params = self.params(json!({"query":query,"offset":self.search_offset}));
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
                if !this.current(cx) {
                    return;
                }
                this.search_loading = false;
                match result {
                    Ok(mut page) => {
                        page.matches
                            .extend(page.paths.drain(..).map(|path| Match { path, line: None }));
                        if this.search_offset >= page.total && this.search_offset > 0 {
                            this.search_offset =
                                page.total.saturating_sub(1) / PAGE_SIZE * PAGE_SIZE;
                            this.load_search(false, cx);
                            return;
                        }
                        this.search = page;
                    }
                    Err(error) => this.search_error = Some(FileError::Request(error)),
                }
                cx.notify();
            });
        }));
    }

    fn select(&mut self, selection: Match, cx: &mut Context<Self>) {
        if !self.current(cx) {
            return;
        }
        let params = self.params(json!({"relativePath":selection.path}));
        let runtime = self.runtime.clone();
        self.preview = Some(Preview {
            selection,
            loading: true,
            text: None,
            error: None,
            scroll: ScrollHandle::new(),
        });
        self.preview_task = Some(cx.spawn(async move |this, cx| {
            let result = runtime
                .request("project.files.preview", params)
                .await
                .and_then(|value| {
                    value["text"]
                        .as_str()
                        .map(|text| TextPages::new(text.to_owned().into()))
                        .ok_or_else(|| tr("读取失败").to_owned())
                });
            let _ = this.update(cx, |this, cx| {
                if !this.current(cx) {
                    return;
                }
                let Some(preview) = this.preview.as_mut() else {
                    return;
                };
                preview.loading = false;
                match result {
                    Ok(mut text) => {
                        if let Some(line) = preview.selection.line {
                            text.show_line(line);
                        }
                        preview.text = Some(text);
                    }
                    Err(error) => {
                        preview.error = Some(match error.as_str() {
                            "Project file exceeds preview limit" => FileError::Owned(
                                "文件超过 1 MiB，暂不在窗口中预览；可在文件管理器中查看。",
                            ),
                            "Project file is not text" => {
                                FileError::Owned("文件包含二进制内容，无法作为文本预览。")
                            }
                            "Project file is not UTF-8 text" => {
                                FileError::Owned("文件不是 UTF-8 文本，暂不支持此编码的预览。")
                            }
                            _ => FileError::Request(error),
                        });
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn preview_page(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(preview) = &mut self.preview else {
            return;
        };
        let Some(text) = &mut preview.text else {
            return;
        };
        if text
            .page
            .checked_add_signed(step)
            .is_some_and(|page| text.show(page))
        {
            preview.scroll = ScrollHandle::new();
            cx.notify();
        }
    }

    fn file_row(&self, hit: Match, label: String, depth: usize, cx: &Context<Self>) -> Button {
        let path = hit.path.clone();
        let selected = self
            .preview
            .as_ref()
            .is_some_and(|preview| preview.selection.path == path);
        Button::new(SharedString::from(format!("browser-file-{}", hit.path)))
            .ghost()
            .compact()
            .w_full()
            .min_w_0()
            .justify_start()
            .pl(px(8. + depth.min(12) as f32 * 12.))
            .selected(selected)
            .icon(Glyph::FileText)
            .tooltip(label.clone())
            .accessibility_label(label.clone())
            .child(div().min_w_0().truncate().child(label))
            .on_click(cx.listener(move |this, _, _, cx| this.select(hit.clone(), cx)))
    }

    fn directory_rows(
        &self,
        path: &str,
        depth: usize,
        rows: &mut Vec<AnyElement>,
        cx: &Context<Self>,
    ) {
        let Some(folder) = self.directories.get(path).filter(|folder| folder.expanded) else {
            return;
        };
        for entry in &folder.entries {
            let name = entry
                .path
                .rsplit('/')
                .next()
                .unwrap_or(&entry.path)
                .to_owned();
            if !entry.directory {
                rows.push(
                    self.file_row(
                        Match {
                            path: entry.path.clone(),
                            line: None,
                        },
                        name,
                        depth,
                        cx,
                    )
                    .into_any_element(),
                );
                continue;
            }
            let path = entry.path.clone();
            let expanded = self
                .directories
                .get(&path)
                .is_some_and(|child| child.expanded);
            let toggle = path.clone();
            rows.push(
                Button::new(SharedString::from(format!("browser-folder-{path}")))
                    .ghost()
                    .compact()
                    .w_full()
                    .min_w_0()
                    .justify_start()
                    .pl(px(8. + depth.min(12) as f32 * 12.))
                    .tooltip(path.clone())
                    .accessibility_label(path.clone())
                    .icon(if expanded {
                        Glyph::ChevronDown
                    } else {
                        Glyph::ChevronRight
                    })
                    .child(div().min_w_0().truncate().child(name))
                    .on_click(
                        cx.listener(move |this, _, _, cx| {
                            this.toggle_directory(toggle.clone(), cx)
                        }),
                    )
                    .into_any_element(),
            );
            if expanded {
                self.directory_rows(&path, depth + 1, rows, cx);
            }
        }
        let detail = if folder.loading {
            Some(tr("正在读取…").to_owned())
        } else if let Some(error) = &folder.error {
            Some(error.text())
        } else if folder.entries.is_empty() {
            Some(tr("此文件夹为空").to_owned())
        } else {
            None
        };
        if let Some(detail) = detail {
            rows.push(
                div()
                    .pl(px(8. + depth.min(12) as f32 * 12.))
                    .text_xs()
                    .text_color(
                        if folder.error.is_some() {
                            Color::Error
                        } else {
                            Color::Muted
                        }
                        .get(cx),
                    )
                    .child(detail)
                    .into_any_element(),
            );
        }
        if !folder.loading && (folder.error.is_some() || folder.next_offset < folder.total) {
            let path = path.to_owned();
            rows.push(
                Button::new(SharedString::from(format!("browser-more-{path}")))
                    .ghost()
                    .compact()
                    .justify_start()
                    .pl(px(8. + depth.min(12) as f32 * 12.))
                    .label(tr(if folder.error.is_some() {
                        "重试"
                    } else {
                        "加载更多"
                    }))
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.load_directory(path.clone(), cx)),
                    )
                    .into_any_element(),
            );
        }
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
            return vec![
                div()
                    .id("browser-search-error")
                    .p_2()
                    .text_sm()
                    .role(Role::Status)
                    .text_color(Color::Error.get(cx))
                    .child(error.text())
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
        self.search
            .matches
            .iter()
            .map(|hit| {
                let label = hit
                    .line
                    .map_or_else(|| hit.path.clone(), |line| format!("{}:{line}", hit.path));
                self.file_row(hit.clone(), label, 0, cx).into_any_element()
            })
            .collect()
    }

    fn preview_view(&self, height: Pixels, cx: &Context<Self>) -> impl IntoElement {
        let mut panel = v_flex().flex_1().min_w_0().min_h_0().gap_2();
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
        panel = panel.child(
            h_flex()
                .justify_between()
                .gap_2()
                .child(div().min_w_0().truncate().text_sm().child(path.clone()))
                .child(
                    Button::new("browser-reload-preview")
                        .ghost()
                        .small()
                        .label(tr("重新读取"))
                        .disabled(preview.loading)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.select(selection.clone(), cx)),
                        ),
                ),
        );
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
            return panel.child(
                div()
                    .id("browser-preview-error")
                    .text_sm()
                    .role(Role::Status)
                    .text_color(Color::Error.get(cx))
                    .child(error.text()),
            );
        }
        let Some(text) = &preview.text else {
            return panel;
        };
        let appearance = cx.global::<crate::appearance::Appearance>();
        let font = appearance
            .code_font_family()
            .font_family_name()
            .map(SharedString::from)
            .unwrap_or_else(|| Theme::global(cx).mono_font_family.clone());
        let content = if text.full.is_empty() {
            tr("此文件为空").into()
        } else {
            text.visible.clone()
        };
        panel
            .child(
                text_reading::region("browser-preview".into(), path, &preview.scroll, height, cx)
                    .child(
                        text_reading::selectable_text("browser-text-preview", content)
                            .text_sm()
                            .font_family(font),
                    ),
            )
            .when(text.count() > 1, |panel| {
                panel.child(
                    h_flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("browser-preview-before")
                                .ghost()
                                .small()
                                .label(tr("上一页"))
                                .disabled(text.page == 0)
                                .on_click(cx.listener(|this, _, _, cx| this.preview_page(-1, cx))),
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
                                .on_click(cx.listener(|this, _, _, cx| this.preview_page(1, cx))),
                        ),
                )
            })
    }
}

impl Render for FileBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut view = v_flex().min_w_0().gap_2();
        if !self.current(cx) {
            return view.child(
                div()
                    .id("browser-stale")
                    .role(Role::Status)
                    .text_sm()
                    .child(tr("项目、会话或后台连接已变化，请关闭后重新选择。")),
            );
        }
        let searching = !self.query.read(cx).value().trim().is_empty();
        let mut rows = Vec::new();
        if searching {
            rows = self.search_rows(cx);
        } else {
            self.directory_rows("", 0, &mut rows, cx);
        }
        let narrow = window.viewport_size().width < px(690.);
        let body = window.viewport_size().height * 0.62;
        let list_height = if narrow { body * 0.40 } else { body };
        let preview_height = if narrow {
            (body * 0.60 - px(62.)).max(px(60.))
        } else {
            body - px(64.)
        };
        let name = if searching {
            tr("项目文件搜索结果")
        } else {
            tr("项目目录树")
        };
        let list = v_flex()
            .min_w_0()
            .gap_1()
            .when(!narrow, |list| list.w(px(320.)).flex_shrink_0())
            .child(
                v_flex()
                    .id("browser-list")
                    .role(Role::List)
                    .aria_label(name)
                    .h(list_height)
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .vertical_scrollbar(&self.scroll)
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
        let preview = self.preview_view(preview_height, cx);
        let panels = if narrow {
            v_flex()
                .gap_2()
                .child(list)
                .child(preview)
                .into_any_element()
        } else {
            h_flex()
                .items_start()
                .gap_3()
                .child(list)
                .child(preview)
                .into_any_element()
        };
        view = view.when_some(self.source_directory.clone(), |view, source| {
            view.child(text_reading::selectable_text("browser-directory", source).text_xs())
        });
        view.child(
            h_flex()
                .gap_1()
                .items_center()
                .flex_wrap()
                .child(
                    div().min_w(px(180.)).flex_1().child(
                        Input::new(&self.query)
                            .cleanable(true)
                            .aria_label(tr("搜索项目文件")),
                    ),
                )
                .children(
                    [(false, "文件名"), (true, "文件内容")].map(|(contents, label)| {
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
                    }),
                ),
        )
        .child(panels)
    }
}
