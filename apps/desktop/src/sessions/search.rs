use crate::focus_scroll::{Reveal, Viewport};
use crate::language::{english, tr};
use crate::{app::HiveApp, palette::Color, transport::Runtime};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Icon,
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
use std::{sync::Arc, time::Duration};

use crate::protocol::SessionHit as Hit;
#[derive(Default, Deserialize)]
pub(crate) struct Page {
    pub(crate) matches: Vec<Hit>,
    pub(crate) total: usize,
}

/// Shared wire contract for the global browser and the scoped sidebar. None
/// deliberately omits workspaceId to preserve the global browser contract.
pub(crate) async fn search_page(
    runtime: &Runtime,
    query: String,
    archived: bool,
    offset: usize,
    workspace: Option<String>,
) -> Result<Page, String> {
    let mut params = json!({"query":query,"archived":archived,"offset":offset});
    if let Some(workspace) = workspace {
        params["workspaceId"] = json!(workspace);
    }
    runtime
        .request("chat.search", params)
        .await
        .and_then(|value| serde_json::from_value::<Page>(value).map_err(|error| error.to_string()))
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

enum SearchError {
    Request(String),
    WorkspaceMissing,
}
impl SearchError {
    fn text(&self) -> String {
        match self {
            Self::Request(error) => {
                format!(
                    "{}: {}",
                    tr("搜索失败"),
                    crate::language::localize_owned_error(error)
                )
            }
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
            let result = search_page(&runtime, query, archived, offset, None).await;
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
            let label = if self.archived {
                tr("搜索已归档会话、项目路径或 Agent…")
            } else {
                tr("搜索会话名称、项目路径或 Agent…")
            };
            self.query
                .update(cx, |query, cx| query.set_placeholder(label, window, cx));
            self.placeholder_locale = Some(locale);
        }
        let results_height = px((self.page.matches.len().max(1) * 58) as f32)
            .max(px(140.))
            .min(px(380.))
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
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .child(
                        div().flex_1().min_w_0().child(
                            Input::new(&self.query)
                                .appearance(false)
                                .bordered(false)
                                .focus_bordered(false)
                                .cleanable(true)
                                .prefix(
                                    Icon::new(Glyph::Search)
                                        .size(px(14.))
                                        .text_color(Color::Muted.get(cx)),
                                )
                                .aria_label(tr("搜索会话名称、项目路径或 Agent")),
                        ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .h(results_height)
                    .min_h_0()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .overflow_hidden()
                    .flex()
                    .child(Viewport::new(
                        "session-search-viewport",
                        &self.scroll,
                        v_flex()
                            .id("session-search-results")
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .p_1p5()
                            .gap_0p5()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .vertical_scrollbar(&self.scroll)
                            .when(self.loading, |list| {
                                list.child(
                                    h_flex()
                                        .p_4()
                                        .items_center()
                                        .justify_center()
                                        .gap_2()
                                        .text_sm()
                                        .text_color(Color::Muted.get(cx))
                                        .child(tr("正在搜索…")),
                                )
                            })
                            .when_some(self.error.as_ref().map(|error| error.text()), |list, error| {
                                list.child(
                                    v_flex()
                                        .p_3()
                                        .m_1()
                                        .gap_2()
                                        .rounded_lg()
                                        .bg(Color::ErrorSurface.get(cx))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Error.get(cx))
                                                .child(error),
                                        )
                                        .child(Reveal::new(
                                            "retry-session-search-reveal",
                                            &self.scroll,
                                            Button::new("retry-session-search")
                                                .outline()
                                                .compact()
                                                .h(px(28.))
                                                .px_2p5()
                                                .rounded_lg()
                                                .label(tr("重新搜索"))
                                                .on_click(
                                                    cx.listener(|this, _, _, cx| this.load(false, cx)),
                                                ),
                                        )),
                                )
                            })
                            .when(
                                !self.loading && self.error.is_none() && self.page.matches.is_empty(),
                                |list| {
                                    list.child(
                                        v_flex()
                                            .flex_1()
                                            .py_8()
                                            .px_4()
                                            .items_center()
                                            .justify_center()
                                            .gap_2()
                                            .child(
                                                Icon::new(if self.archived {
                                                    Glyph::Archive
                                                } else {
                                                    Glyph::Search
                                                })
                                                .size(px(20.))
                                                .text_color(Color::Muted.get(cx).opacity(0.65)),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(tr("没有匹配的会话")),
                                            ),
                                    )
                                },
                            )
                            .children(self.page.matches.iter().enumerate().map(|(i, hit)| {
                                let active = i == self.selected;
                                let title = crate::view::untitled_session_label(&hit.session).unwrap_or(&hit.title);
                                let role = hit.session.agent_id.as_deref().map(|agent| {
                                    if agent.ends_with(":orchestrator") {
                                        tr("主控")
                                    } else if agent.ends_with(":__workflow__") {
                                        tr("工作流")
                                    } else {
                                        tr("成员")
                                    }
                                });
                                let suffix = std::path::Path::new(&hit.workspace_path)
                                    .components()
                                    .rev()
                                    .take(2)
                                    .collect::<Vec<_>>()
                                    .into_iter()
                                    .rev()
                                    .map(|part| part.as_os_str().to_string_lossy())
                                    .collect::<Vec<_>>()
                                    .join(std::path::MAIN_SEPARATOR_STR);
                                let project = if suffix.is_empty() {
                                    hit.workspace_name.clone()
                                } else {
                                    format!("{} · {suffix}", hit.workspace_name)
                                };
                                let identity = if hit.workspace_path.is_empty() {
                                    hit.workspace_name.clone()
                                } else {
                                    format!("{} · {}", hit.workspace_name, hit.workspace_path)
                                };
                                let role_suffix = role.map(|role| format!(" · {role}")).unwrap_or_default();
                                let time = crate::palette::compact_relative_time(
                                    hit.session.updated_at,
                                    english(),
                                );
                                Reveal::new(
                                    ("search-session-reveal", i),
                                    &self.scroll,
                                    Button::new(("search-session", i))
                                        .ghost()
                                        .compact()
                                        .w_full()
                                        .h_auto()
                                        .px_2p5()
                                        .py_2()
                                        .rounded_lg()
                                        .justify_start()
                                        .selected(active)
                                        .accessibility_label(if english() {
                                            format!(
                                                "Open {} · {}{} · {}",
                                                title,
                                                identity,
                                                role_suffix,
                                                tr(hit.session.backend.label())
                                            )
                                        } else {
                                            format!(
                                                "打开 {} · {}{} · {}",
                                                title,
                                                identity,
                                                role_suffix,
                                                tr(hit.session.backend.label())
                                            )
                                        })
                                        .tooltip(format!(
                                            "{title} · {identity}{role_suffix} · {}",
                                            tr(hit.session.backend.label())
                                        ))
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_center()
                                                .justify_between()
                                                .gap_2p5()
                                                .child(crate::palette::project_badge(
                                                    &hit.workspace_name,
                                                    22.,
                                                    cx,
                                                ))
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .items_start()
                                                        .gap_0p5()
                                                        .child(
                                                            h_flex()
                                                                .w_full()
                                                                .min_w_0()
                                                                .items_center()
                                                                .gap_1p5()
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w_0()
                                                                        .truncate()
                                                                        .text_size(px(13.))
                                                                        .font_weight(if active {
                                                                            FontWeight::SEMIBOLD
                                                                        } else {
                                                                            FontWeight::MEDIUM
                                                                        })
                                                                        .text_color(Color::Text.get(cx))
                                                                        .child(title.to_owned()),
                                                                )
                                                                .when_some(role, |row, role_label| {
                                                                    row.child(
                                                                        div()
                                                                            .flex_shrink_0()
                                                                            .px_1p5()
                                                                            .py(px(1.))
                                                                            .rounded_md()
                                                                            .bg(Color::Inset.get(cx))
                                                                            .text_size(px(10.5))
                                                                            .font_weight(FontWeight::MEDIUM)
                                                                            .text_color(Color::Muted.get(cx))
                                                                            .child(role_label),
                                                                    )
                                                                })
                                                                .child(
                                                                    div()
                                                                        .flex_shrink_0()
                                                                        .px_1p5()
                                                                        .py(px(1.))
                                                                        .rounded_md()
                                                                        .bg(Color::Inset.get(cx))
                                                                        .text_size(px(10.5))
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(Color::Muted.get(cx))
                                                                        .child(tr(hit.session.backend.label())),
                                                                ),
                                                        )
                                                        .child(
                                                            div()
                                                                .w_full()
                                                                .truncate()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(project),
                                                        ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .flex_shrink_0()
                                                        .items_center()
                                                        .gap_1p5()
                                                        .when_some(time, |row, time| {
                                                            row.child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .text_color(Color::Muted.get(cx))
                                                                    .child(time),
                                                            )
                                                        })
                                                        .when(active, |row| {
                                                            row.child(
                                                                Icon::new(Glyph::CornerDownLeft)
                                                                    .size(px(13.))
                                                                    .text_color(Color::Accent.get(cx)),
                                                            )
                                                        }),
                                                ),
                                        )
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            this.selected = i;
                                            this.open_selected(window, cx);
                                        })),
                                )
                            })),
                    )),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(Color::Divider.get(cx))
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(if self.page.total == 0 {
                                tr("0 个会话").into()
                            } else if english() {
                                format!(
                                    "{}–{} of {} conversations",
                                    self.offset + 1,
                                    self.offset + self.page.matches.len(),
                                    self.page.total
                                )
                            } else {
                                format!(
                                    "第 {}–{} 个，共 {} 个",
                                    self.offset + 1,
                                    self.offset + self.page.matches.len(),
                                    self.page.total
                                )
                            })
                            .child(
                                Button::new("refresh-session-search")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(26.))
                                    .px_2()
                                    .rounded_lg()
                                    .icon(Glyph::RefreshCw)
                                    .label(tr("刷新"))
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, _, cx| this.load(false, cx))),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(tr("↑↓ 选择 · Enter 打开 · Esc 返回"))
                            .when(
                                self.offset > 0
                                    || self.offset + self.page.matches.len() < self.page.total,
                                |row| {
                                    row.child(
                                        Button::new("session-search-previous")
                                            .outline()
                                            .compact()
                                            .xsmall()
                                            .h(px(26.))
                                            .px_2()
                                            .rounded_lg()
                                            .label(tr("上一页"))
                                            .disabled(self.loading || self.offset == 0)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.offset = this.offset.saturating_sub(100);
                                                this.load(false, cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("session-search-next")
                                            .outline()
                                            .compact()
                                            .xsmall()
                                            .h(px(26.))
                                            .px_2()
                                            .rounded_lg()
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
                                    )
                                },
                            ),
                    ),
            )
            .when(self.archived, |view| {
                view.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded_lg()
                        .bg(Color::Inset.get(cx))
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("历史和草稿均保留。打开会话后可恢复或继续发送。")),
                )
            })
    }
}
