use super::*;
use crate::{
    sessions::search::{Page, search_page},
    transport::Runtime,
};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::scroll::ScrollableElement;
use std::{sync::Arc, time::Duration};

enum SearchFailure {
    Offline,
    TooLong,
    Request(String),
    WorkspaceMissing,
}
impl SearchFailure {
    fn text(&self) -> String {
        match self {
            Self::Offline => tr("后台尚未连接").to_owned(),
            Self::TooLong => tr("搜索内容不能超过 200 个字符").to_owned(),
            Self::WorkspaceMissing => tr("工作区正在移除或已不存在，请重新搜索。").to_owned(),
            Self::Request(error) => format!(
                "{}: {}",
                tr("搜索失败"),
                crate::language::localize_owned_error(error)
            ),
        }
    }
}
pub(super) struct SidebarSearch {
    pub query: Entity<InputState>,
    locale: Option<crate::language::Locale>,
    app: WeakEntity<HiveApp>,
    runtime: Option<Arc<Runtime>>,
    connection: u64,
    workspace: Option<String>,
    generation: u64,
    page: Page,
    offset: usize,
    selected: usize,
    loading: bool,
    error: Option<SearchFailure>,
    scroll: ScrollHandle,
}
impl SidebarSearch {
    pub fn new(app: WeakEntity<HiveApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索")));
        cx.subscribe(&query, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.offset = 0;
                this.load(true, cx);
            }
        })
        .detach();
        Self {
            query,
            locale: None,
            app,
            runtime: None,
            connection: 0,
            workspace: None,
            generation: 0,
            page: Page::default(),
            offset: 0,
            selected: 0,
            loading: false,
            error: None,
            scroll: ScrollHandle::new(),
        }
    }
    pub fn active(&self, cx: &App) -> bool {
        !self.query.read(cx).value().trim().is_empty()
    }
    pub fn sync(
        &mut self,
        runtime: Option<Arc<Runtime>>,
        connection: u64,
        workspace: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let locale = cx.global::<crate::language::Language>().locale;
        if self.locale != Some(locale) {
            self.locale = Some(locale);
            self.query.update(cx, |input, cx| {
                input.set_placeholder(tr("搜索"), window, cx)
            });
        }
        if self.connection != connection
            || self.workspace != workspace
            || self.runtime.is_some() != runtime.is_some()
        {
            self.runtime = runtime;
            self.connection = connection;
            self.workspace = workspace;
            self.offset = 0;
            self.load(false, cx);
        }
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.notify();
        let app = self.app.clone();
        cx.defer(move |cx| {
            let _ = app.update(cx, |_, cx| cx.notify());
        });
    }
    fn load(&mut self, debounce: bool, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let connection = self.connection;
        let query = self.query.read(cx).value().to_string();
        self.page = Page::default();
        self.selected = 0;
        self.scroll = ScrollHandle::new();
        self.error = None;
        self.loading = false;
        if query.trim().is_empty() {
            self.changed(cx);
            return;
        }
        if query.chars().count() > 200 {
            self.error = Some(SearchFailure::TooLong);
            self.changed(cx);
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            self.error = Some(SearchFailure::Offline);
            self.changed(cx);
            return;
        };
        let workspace = self.workspace.clone();
        let offset = self.offset;
        self.loading = true;
        self.changed(cx);
        cx.spawn(async move |this, cx| {
            if debounce {
                smol::Timer::after(Duration::from_millis(150)).await;
            }
            if this
                .update(cx, |this, cx| this.current(generation, connection, cx))
                .ok()
                != Some(true)
            {
                return;
            }
            let result = search_page(&runtime, query, false, offset, workspace).await;
            let _ = this.update(cx, |this, cx| {
                if !this.current(generation, connection, cx) {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(page) => {
                        if page.total > 0 && this.offset >= page.total && page.matches.is_empty() {
                            this.offset = ((page.total - 1) / 100) * 100;
                            this.load(false, cx);
                            return;
                        }
                        if page.total == 0 {
                            this.offset = 0;
                        }
                        this.page = page;
                    }
                    Err(error) => this.error = Some(SearchFailure::Request(error)),
                }
                this.changed(cx);
            });
        })
        .detach();
    }
    fn current(&self, generation: u64, connection: u64, cx: &App) -> bool {
        self.generation == generation
            && self.app.upgrade().is_some_and(|app| {
                let app = app.read(cx);
                app.connected && app.connection_generation == connection
            })
    }
    fn open(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.page.matches.get(index).cloned() else {
            return;
        };
        if self
            .app
            .update(cx, |app, cx| app.navigate_to_search_hit(hit, window, cx))
            .unwrap_or(false)
        {
            self.query
                .update(cx, |input, cx| input.set_value("", window, cx));
            self.offset = 0;
            self.load(false, cx);
        } else {
            self.error = Some(SearchFailure::WorkspaceMissing);
            self.changed(cx);
        }
    }
    pub fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform || modifiers.alt || modifiers.shift {
            return;
        }
        // Leave candidate navigation/commit/cancel to the input method.
        if self.query.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        match event.keystroke.key.as_str() {
            "down" | "up" if !self.page.matches.is_empty() => {
                let len = self.page.matches.len();
                self.selected = if event.keystroke.key == "down" {
                    (self.selected + 1) % len
                } else {
                    (self.selected + len - 1) % len
                };
                self.scroll.scroll_to_item(self.selected);
                cx.notify();
            }
            "enter" if self.active(cx) => self.open(self.selected, window, cx),
            "escape" if self.active(cx) => {
                self.query
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.offset = 0;
                self.load(false, cx);
            }
            _ => return,
        }
        cx.stop_propagation();
    }
}
impl Render for SidebarSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error = self.error.as_ref().map(SearchFailure::text);
        v_flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .gap_1()
            .child(Viewport::new(
                "sidebar-search-viewport",
                &self.scroll,
                v_flex()
                    .id("sidebar-search-results")
                    .role(Role::List)
                    .aria_label(tr("搜索会话"))
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .vertical_scrollbar(&self.scroll)
                    .when(self.loading, |list| {
                        list.child(
                            div()
                                .p_2()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("正在搜索…")),
                        )
                    })
                    .when_some(error, |list, error| {
                        list.child(
                            v_flex()
                                .p_2()
                                .gap_1()
                                .child(
                                    div()
                                        .id("sidebar-search-error")
                                        .role(Role::Status)
                                        .text_xs()
                                        .text_color(Color::Error.get(cx))
                                        .child(error),
                                )
                                .child(
                                    Button::new("retry-sidebar-search")
                                        .ghost()
                                        .small()
                                        .label(tr("重新搜索"))
                                        .disabled(self.runtime.is_none())
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.load(false, cx)),
                                        ),
                                ),
                        )
                    })
                    .when_some(self.page.warning(), |list, warning| {
                        list.child(div().id("sidebar-search-incomplete").role(Role::Status).p_2().text_xs().text_color(Color::Error.get(cx)).child(warning))
                    })
                    .when(
                        !self.loading && self.error.is_none() && self.page.matches.is_empty(),
                        |list| {
                            list.child(
                                div()
                                    .p_2()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("没有匹配的会话")),
                            )
                        },
                    )
                    .children(self.page.matches.iter().enumerate().map(|(index, hit)| {
                        let title = crate::view::untitled_session_label(&hit.session)
                            .unwrap_or(&hit.title)
                            .to_owned();
                        let role = hit.session.agent_id.as_deref().map(|agent| {
                            if agent.ends_with(":orchestrator") {
                                tr("主控")
                            } else if agent.ends_with(":__workflow__") {
                                tr("工作流")
                            } else {
                                tr("成员")
                            }
                        });
                        let identity = format!(
                            "{} · {}{} · {}",
                            hit.workspace_name,
                            hit.workspace_path,
                            role.map(|role| format!(" · {role}")).unwrap_or_default(),
                            tr(hit.session.backend.label())
                        );
                        Reveal::new(
                            ("sidebar-search-reveal", index),
                            &self.scroll,
                            Button::new(("sidebar-search-hit", index))
                                .ghost()
                                .compact()
                                .h_auto()
                                .w_full()
                                .min_w_0()
                                .px_2()
                                .py_1p5()
                                .rounded_md()
                                .justify_start()
                                .selected(index == self.selected)
                                .accessibility_label(format!("{} · {identity}", title))
                                .tooltip(format!("{title} · {identity}"))
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .flex_1()
                                        .items_start()
                                        .gap_0p5()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .w_full()
                                                .truncate()
                                                .text_sm()
                                                .child(title),
                                        )
                                        .child(
                                            div()
                                                .min_w_0()
                                                .w_full()
                                                .truncate()
                                                .text_xs()
                                                .text_color(Color::Muted.get(cx))
                                                .child(identity),
                                        )
                                        .when_some(hit.body_match.as_ref(), |row, body| {
                                            row.child(div().min_w_0().w_full().truncate().text_xs().text_color(Color::Muted.get(cx)).child(body.snippet.clone()))
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open(index, window, cx)
                                })),
                        )
                    })),
            ))
            .child(
                h_flex()
                    .flex_shrink_0()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(if crate::language::english() {
                                format!(
                                    "{} of {}",
                                    self.offset + self.page.matches.len(),
                                    self.page.total
                                )
                            } else {
                                format!(
                                    "{} / {}",
                                    self.offset + self.page.matches.len(),
                                    self.page.total
                                )
                            }),
                    )
                    .when(
                        self.offset > 0 || self.offset + self.page.matches.len() < self.page.total,
                        |row| {
                            row.child(
                                Button::new("sidebar-search-prev")
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .icon(Glyph::ChevronLeft)
                                    .accessibility_label(tr("上一页"))
                                    .disabled(self.loading || self.offset == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.offset = this.offset.saturating_sub(100);
                                        this.load(false, cx);
                                    })),
                            )
                            .child(
                                Button::new("sidebar-search-next")
                                    .ghost()
                                    .compact()
                                    .xsmall()
                                    .icon(Glyph::ChevronRight)
                                    .accessibility_label(tr("下一页"))
                                    .disabled(
                                        self.loading
                                            || self.offset + self.page.matches.len()
                                                >= self.page.total,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.offset += 100;
                                        this.load(false, cx);
                                    })),
                            )
                        },
                    ),
            )
    }
}
