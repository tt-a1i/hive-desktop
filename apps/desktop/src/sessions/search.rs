use crate::{app::HiveApp, palette::Color, protocol::Session, transport::Runtime};
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hit {
    session: Session,
    workspace_name: String,
    title: String,
}
#[derive(Default, Deserialize)]
struct Page {
    matches: Vec<Hit>,
    total: usize,
}
struct SessionSearch {
    app: WeakEntity<HiveApp>,
    runtime: Arc<Runtime>,
    query: Entity<InputState>,
    page: Page,
    loading: bool,
    error: Option<String>,
    generation: u64,
    selected: usize,
    scroll: ScrollHandle,
}

impl HiveApp {
    pub fn open_session_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected || window.has_active_dialog(cx) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let parent = cx.weak_entity();
        let search = cx.new(|cx| {
            let query =
                cx.new(|cx| InputState::new(window, cx).placeholder("搜索会话名称、项目或 Agent…"));
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
                app: parent,
                runtime,
                query,
                page: Page::default(),
                loading: false,
                error: None,
                generation: 0,
                selected: 0,
                scroll: ScrollHandle::new(),
            };
            search.load(false, cx);
            search
        });
        let focus = search.read(cx).query.read(cx).focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title("搜索会话").w(px(660.)).child(search.clone())
        });
        window.on_next_frame(move |window, cx| focus.focus(window, cx));
    }
}

impl SessionSearch {
    fn load(&mut self, debounce: bool, cx: &mut Context<Self>) {
        self.generation += 1;
        let generation = self.generation;
        let query = self.query.read(cx).value().to_string();
        let runtime = self.runtime.clone();
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
                .request("chat.search", json!({"query":query}))
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
                    Ok(page) => this.page = page,
                    Err(error) => this.error = Some(format!("搜索失败：{error}")),
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
                if !app
                    .workspaces
                    .iter()
                    .any(|workspace| workspace.id == hit.session.workspace_id)
                    || app.workspace_removing.as_ref() == Some(&hit.session.workspace_id)
                {
                    return false;
                }
                app.hide_terminal(cx);
                app.close_collaboration();
                if app.workspace.as_ref() != Some(&hit.session.workspace_id) {
                    let cached = app
                        .project_sessions
                        .entry(hit.session.workspace_id.clone())
                        .or_default();
                    if !cached.iter().any(|session| session.id == hit.session.id) {
                        cached.push(hit.session.clone());
                    }
                    app.select_workspace(hit.session.workspace_id.clone(), cx);
                } else if !app
                    .sessions
                    .iter()
                    .any(|session| session.id == hit.session.id)
                {
                    app.sessions.push(hit.session.clone());
                }
                app.open_session(hit.session.id, cx);
                true
            })
            .unwrap_or(false);
        if opened {
            window.close_dialog(cx);
        } else {
            self.error = Some("工作区正在移除或已不存在，请重新搜索。".into());
            cx.notify();
        }
    }
}

impl Render for SessionSearch {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            .child(Input::new(&self.query).aria_label("搜索会话名称、项目或 Agent"))
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
                                .child("正在搜索…"),
                        )
                    })
                    .when_some(self.error.clone(), |list, error| {
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
                                        .label("重新搜索")
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
                                    .child("没有匹配的会话"),
                            )
                        },
                    )
                    .children(self.page.matches.iter().enumerate().map(|(i, hit)| {
                        Button::new(("search-session", i))
                            .ghost()
                            .w_full()
                            .h_auto()
                            .py_2()
                            .justify_start()
                            .selected(i == self.selected)
                            .accessibility_label(format!(
                                "打开 {} · {} · {}",
                                hit.title,
                                hit.workspace_name,
                                hit.session.backend.label()
                            ))
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
                                            .child(hit.title.clone()),
                                    )
                                    .child(
                                        div()
                                            .truncate()
                                            .text_xs()
                                            .text_color(Color::Muted.get(cx))
                                            .child(format!(
                                                "{} · {}",
                                                hit.workspace_name,
                                                hit.session.backend.label()
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
                    .justify_between()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(if self.page.total > self.page.matches.len() {
                        format!(
                            "显示前 {} 个结果，共 {} 个，请继续输入缩小范围。",
                            self.page.matches.len(),
                            self.page.total
                        )
                    } else {
                        format!("{} 个会话", self.page.total)
                    })
                    .child("↑↓ 选择 · Enter 打开 · Esc 返回"),
            )
    }
}
