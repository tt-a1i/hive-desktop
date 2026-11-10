use crate::language::{english, tr};
use crate::{app::HiveApp, palette::Color, transport::Runtime};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::{sync::Arc, time::Duration};

use crate::protocol::SessionHit as Hit;
#[derive(Default, Deserialize)]
struct Page {
    matches: Vec<Hit>,
    total: usize,
}
struct SessionSearch {
    archived: bool,
    offset: usize,
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    query: Entity<InputState>,
    page: Page,
    loading: bool,
    error: Option<SearchError>,
    placeholder_locale: Option<crate::language::Locale>,
    generation: u64,
    selected: usize,
    scroll: ScrollHandle,
}

enum SearchError { Request(String), WorkspaceMissing }
impl SearchError {
    fn text(&self) -> String {
        match self {
            Self::Request(error) => format!("{}: {error}", tr("搜索失败")),
            Self::WorkspaceMissing => tr("工作区正在移除或已不存在，请重新搜索。").to_owned(),
        }
    }
}

impl HiveApp {
    pub fn open_session_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_session_browser(false, window, cx);
    }
    pub(crate) fn open_archived_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_session_browser(true, window, cx);
    }
    fn open_session_browser(
        &mut self,
        archived: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || window.has_active_dialog(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let parent = cx.weak_entity();
        let search = cx.new(|cx| {
            let query = cx.new(|cx| {
                InputState::new(window, cx).placeholder(if archived {
                    tr("搜索已归档会话、项目路径或 Agent…")
                } else {
                    tr("搜索会话名称、项目路径或 Agent…")
                })
            });
            cx.subscribe_in(
                &query,
                window,
                |this: &mut SessionSearch, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => this.load(true, cx),
                    InputEvent::PressEnter {
                        secondary: false,
                        shift: false,
                    } => this.open_selected(window, cx),
                    _ => (),
                },
            )
            .detach();
            let mut search = SessionSearch {
                archived,
                offset: 0,
                app: parent,
                runtime,
                query,
                page: Page::default(),
                loading: false,
                error: None,
                placeholder_locale: None,
                generation: 0,
                selected: 0,
                scroll: ScrollHandle::new(),
            };
            search.load(false, cx);
            search
        });
        let focus = search.read(cx).query.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(if archived {
                    tr("已归档会话")
                } else {
                    tr("搜索会话")
                })
                .w(px(660.))
                // Input's Enter propagates to the dialog before PressEnter is
                // delivered. Keep it alive for open_selected to navigate/close.
                .on_ok(|_, _, _| false)
                .child(search.clone())
        });
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
    }
}

impl SessionSearch {
    fn load(&mut self, debounce: bool, cx: &mut Context<Self>) {
        if debounce {
            self.offset = 0;
        }
        self.generation += 1;
        let generation = self.generation;
        let query = self.query.read(cx).value().to_string();
        let runtime = self.runtime.clone();
        let archived = self.archived;
        let offset = self.offset;
        self.loading = true;
        self.error = None;
        self.page = Page::default();
        self.selected = 0;
        self.scroll = ScrollHandle::new();
        cx.notify();
        cx.spawn(async move |this, cx| {
            if debounce {
                smol::Timer::after(Duration::from_millis(150)).await;
            }
            if this
                .update(cx, |this, _| this.generation == generation)
                .ok()
                != Some(true)
            {
                return;
            }
            let result = runtime
                .request(
                    "chat.search",
                    json!({"query":query,"archived":archived,"offset":offset}),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<Page>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
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
                    Err(error) => this.error = Some(SearchError::Request(error)),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn move_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.page.matches.is_empty() {
            return;
        }
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.page.matches.len() - 1);
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.page.matches.get(self.selected).cloned() else {
            return;
        };
        let opened = self
            .app
            .update(cx, |app, cx| {
                app.navigate_to_session(hit.session, window, cx)
            })
            .unwrap_or(false);
        if opened {
            window.close_dialog(cx);
        } else {
            self.error = Some(SearchError::WorkspaceMissing);
            cx.notify();
        }
    }
}

impl Render for SessionSearch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = cx.global::<crate::language::Language>().locale;
        if self.placeholder_locale != Some(locale) {
            let label = if self.archived { tr("搜索已归档会话、项目路径或 Agent…") } else { tr("搜索会话名称、项目路径或 Agent…") };
            self.query.update(cx, |query, cx| query.set_placeholder(label, window, cx));
            self.placeholder_locale = Some(locale);
        }
        let results_height = px((self.page.matches.len().max(1) * 64) as f32)
            .min(px(360.))
            .min(window.viewport_size().height * 0.55);
        v_flex()
            .key_context("HiveSessionSearch")
            .gap_3()
            .on_action(
                cx.listener(|this, _: &crate::NextSessionResult, _, cx| this.move_selection(1, cx)),
            )
            .on_action(
                cx.listener(|this, _: &crate::PreviousSessionResult, _, cx| {
                    this.move_selection(-1, cx)
                }),
            )
            .child(Input::new(&self.query).aria_label(tr("搜索会话名称、项目路径或 Agent")))
            .child(
                div()
                    .id("session-search-results")
                    .h(results_height)
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .when(self.loading, |list| {
                        list.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("正在搜索…")),
                        )
                    })
                    .when_some(self.error.as_ref().map(|error| error.text()), |list, error| {
                        list.child(
                            v_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(Color::Error.get(cx))
                                        .child(error),
                                )
                                .child(
                                    Button::new("retry-session-search")
                                        .label(tr("重新搜索"))
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.load(false, cx)),
                                        ),
                                ),
                        )
                    })
                    .when(
                        !self.loading && self.error.is_none() && self.page.matches.is_empty(),
                        |list| {
                            list.child(
                                div()
                                    .p_3()
                                    .text_sm()
                                    .text_color(Color::Muted.get(cx))
                                    .child(tr("没有匹配的会话")),
                            )
                        },
                    )
                    .children(self.page.matches.iter().enumerate().map(|(i, hit)| {
                        let title = crate::view::untitled_session_label(&hit.session).unwrap_or(&hit.title);
                        let suffix = std::path::Path::new(&hit.workspace_path).components().rev().take(2).collect::<Vec<_>>()
                            .into_iter().rev().map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join(std::path::MAIN_SEPARATOR_STR);
                        let project = if suffix.is_empty() { hit.workspace_name.clone() } else { format!("{} · {suffix}", hit.workspace_name) };
                        let identity = if hit.workspace_path.is_empty() { hit.workspace_name.clone() } else { format!("{} · {}", hit.workspace_name, hit.workspace_path) };
                        Button::new(("search-session", i))
                            .ghost()
                            .w_full()
                            .h_auto()
                            .py_2()
                            .justify_start()
                            .selected(i == self.selected)
                            .accessibility_label(if english() { format!("Open {} · {} · {}", title, identity, tr(hit.session.backend.label())) } else { format!("打开 {} · {} · {}", title, identity, tr(hit.session.backend.label())) })
                            .tooltip(format!("{title} · {identity} · {}", tr(hit.session.backend.label())))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .items_start()
                                    .gap_1()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(title.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(format!(
                                                "{} · {}",
                                                project,
                                                tr(hit.session.backend.label())
                                            )),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.selected = i;
                                this.open_selected(window, cx);
                            }))
                    })),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .justify_between()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(if self.page.total == 0 {
                        tr("0 个会话").into()
                    } else {
                        if english() { format!("{}–{} of {} conversations", self.offset + 1, self.offset + self.page.matches.len(), self.page.total) } else { format!("第 {}–{} 个，共 {} 个", self.offset + 1, self.offset + self.page.matches.len(), self.page.total) }
                    })
                    .child(
                        Button::new("refresh-session-search")
                            .ghost()
                            .compact()
                            .label(tr("刷新"))
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load(false, cx))),
                    )
                    .child(tr("↑↓ 选择 · Enter 打开 · Esc 返回")),
            )
            .when(
                self.offset > 0 || self.offset + self.page.matches.len() < self.page.total,
                |view| {
                    view.child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("session-search-previous")
                                    .ghost()
                                    .label(tr("上一页"))
                                    .disabled(self.loading || self.offset == 0)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.offset = this.offset.saturating_sub(100);
                                        this.load(false, cx);
                                    })),
                            )
                            .child(
                                Button::new("session-search-next")
                                    .ghost()
                                    .label(tr("下一页"))
                                    .disabled(
                                        self.loading
                                            || self.offset + self.page.matches.len()
                                                >= self.page.total,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.offset += 100;
                                        this.load(false, cx);
                                    })),
                            ),
                    )
                },
            )
            .when(self.archived, |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("历史和草稿均保留。打开会话后可恢复或继续发送。")),
                )
            })
    }
}
