use crate::{
    app::HiveApp,
    focus_scroll::Reveal,
    language::{english, tr},
    palette::{format_session_time, Color},
    protocol::SessionHit,
    sessions::search::{
        recorded_worktree_branch, recorded_worktree_branch_label, recorded_worktree_branch_row,
    },
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub struct ArchiveState {
    pub search: Entity<InputState>,
    pub workspace_filter: Option<String>,
    pub items: Vec<SessionHit>,
    pub total: usize,
    next_offset: Option<usize>,
    pub loading: bool,
    pub error: Option<String>,
    pub warning: Option<String>,
    pub loaded: bool,
    pub generation: u64,
    task: Option<Task<()>>,
}

impl ArchiveState {
    pub fn new(window: &mut Window, cx: &mut Context<HiveApp>) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder(tr("搜索已归档会话…")));
        cx.subscribe_in(
            &search,
            window,
            |this: &mut HiveApp, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.start_archive_search(false, true, cx);
                }
            },
        )
        .detach();
        Self {
            search,
            workspace_filter: None,
            items: Vec::new(),
            total: 0,
            next_offset: None,
            loading: false,
            error: None,
            warning: None,
            loaded: false,
            generation: 0,
            task: None,
        }
    }

    fn cancel_search(&mut self) {
        self.task = None;
        self.generation += 1;
        self.loading = false;
    }

    pub(crate) fn release_search(&mut self) {
        self.cancel_search();
        self.items = Vec::new();
        self.total = 0;
        self.next_offset = None;
        self.error = None;
        self.warning = None;
        self.loaded = false;
    }
}

impl HiveApp {
    pub(crate) fn load_archived_settings(&mut self, append: bool, cx: &mut Context<Self>) {
        self.start_archive_search(append, false, cx);
    }

    fn start_archive_search(&mut self, append: bool, debounce: bool, cx: &mut Context<Self>) {
        if !self.settings_panel || self.settings_section != crate::app::SettingsSection::Archive {
            return;
        }
        let offset = if append {
            let Some(offset) = self.archive_settings.next_offset else { return };
            self.archive_settings.cancel_search();
            offset
        } else {
            self.archive_settings.release_search();
            self.archive_scroll.set_offset(point(px(0.), px(0.)));
            0
        };
        if !self.connected || self.closing {
            cx.notify();
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            cx.notify();
            return;
        };
        let connection = self.connection_generation;
        let generation = self.archive_settings.generation;
        self.archive_settings.loading = true;
        self.archive_settings.error = None;
        let query = self.archive_settings.search.read(cx).value().to_string();
        let workspace = self.archive_settings.workspace_filter.clone();
        cx.notify();

        self.archive_settings.task = Some(cx.spawn(async move |this, cx| {
            if debounce {
                smol::Timer::after(std::time::Duration::from_millis(150)).await;
            }
            let result =
                crate::sessions::search::search_page(&runtime, query, true, offset, workspace)
                    .await;
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.archive_settings.generation != generation
                {
                    return;
                }
                this.archive_settings.loading = false;
                this.archive_settings.task = None;
                match result {
                    Ok(page) => {
                        this.archive_settings.warning = page.warning();
                        this.archive_settings.total = page.total;
                        let next = offset.saturating_add(page.matches.len());
                        this.archive_settings.next_offset =
                            (!page.matches.is_empty() && next < page.total).then_some(next);
                        if append {
                            for hit in page.matches {
                                if !this
                                    .archive_settings
                                    .items
                                    .iter()
                                    .any(|existing| existing.session.id == hit.session.id)
                                {
                                    this.archive_settings.items.push(hit);
                                }
                            }
                        } else {
                            this.archive_settings.items = page.matches;
                        }
                        this.archive_settings.loaded = true;
                        this.archive_settings.error = None;
                    }
                    Err(error) => {
                        this.archive_settings.error = Some(error);
                    }
                }
                cx.notify();
            });
        }));
    }

    pub(crate) fn render_archive_settings(&self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let state = &self.archive_settings;
        let has_query = !state.search.read(cx).value().trim().is_empty();
        let selected_ws = state.workspace_filter.clone();
        let ws_label = selected_ws
            .as_ref()
            .and_then(|id| self.workspaces.iter().find(|ws| &ws.id == id))
            .map(|ws| ws.name.clone())
            .unwrap_or_else(|| tr("全部项目").to_owned());
        let workspace_options: Vec<(String, String)> = self
            .sorted_projects(cx)
            .into_iter()
            .map(|ws| (ws.id.clone(), ws.name.clone()))
            .collect();
        let time_format = crate::general::current(cx).session_time_format;
        let is_english = english();
        let weak = cx.weak_entity();
        let loaded_count = state.items.len();
        let total = state.total;
        let has_more = state.next_offset.is_some();
        let loading = state.loading;

        v_flex()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(200.))
                            .child(
                                Input::new(&state.search)
                                    .small()
                                    .cleanable(true)
                                    .prefix(
                                        Icon::new(Glyph::Search)
                                            .size(px(14.))
                                            .text_color(Color::Muted.get(cx)),
                                    )
                                    .aria_label(tr("搜索已归档会话…")),
                            ),
                    )
                    .child(
                        Button::new("archive-workspace-filter")
                            .outline()
                            .small()
                            .h(px(28.))
                            .px_2p5()
                            .rounded_lg()
                            .icon(Glyph::Folder)
                            .label(ws_label)
                            .dropdown_menu(move |mut menu, _, _| {
                                let all_active = selected_ws.is_none();
                                let weak_all = weak.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(tr("全部项目"))
                                        .icon(if all_active {
                                            Glyph::Check
                                        } else {
                                            Glyph::Layers
                                        })
                                        .checked(all_active)
                                        .on_click(move |_, _, cx| {
                                            let _ = weak_all.update(cx, |this, cx| {
                                                this.archive_settings.workspace_filter = None;
                                                this.load_archived_settings(false, cx);
                                            });
                                        }),
                                );
                                for (ws_id, ws_name) in workspace_options.clone() {
                                    let active = selected_ws.as_ref() == Some(&ws_id);
                                    let weak_ws = weak.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(ws_name)
                                            .icon(if active {
                                                Glyph::Check
                                            } else {
                                                Glyph::Folder
                                            })
                                            .checked(active)
                                            .on_click(move |_, _, cx| {
                                                let _ = weak_ws.update(cx, |this, cx| {
                                                    this.archive_settings.workspace_filter =
                                                        Some(ws_id.clone());
                                                    this.load_archived_settings(false, cx);
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(Reveal::new(
                        "archive-refresh-reveal",
                        &self.archive_scroll,
                        Button::new("archive-refresh")
                            .outline()
                            .small()
                            .h(px(28.))
                            .px_2p5()
                            .rounded_lg()
                            .icon(Glyph::RotateCcw)
                            .label(if loading { tr("加载中…") } else { tr("刷新") })
                            .disabled(!self.connected || loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.load_archived_settings(false, cx);
                            })),
                    )),
            )
            .when_some(state.error.clone(), |panel, error| {
                panel.child(super::settings_notice_box(
                    "archive-settings-error",
                    error,
                    true,
                    cx,
                ))
            })
            .when_some(state.warning.clone(), |panel, warning| {
                panel.child(super::settings_notice_box(
                    "archive-settings-warning",
                    warning,
                    false,
                    cx,
                ))
            })
            .when(!self.connected, |panel| {
                panel.child(super::settings_card(cx).child(super::settings_row(false, cx).child(
                    div().text_sm().text_color(Color::Muted.get(cx)).child(tr("后台尚未连接，连接后将显示已归档会话。")),
                )))
            })
            .when(self.connected && state.items.is_empty() && state.error.is_none(), |panel| {
                let empty_text = if loading {
                    tr("正在加载已归档会话…")
                } else if has_query || state.workspace_filter.is_some() {
                    tr("没有匹配的归档会话。")
                } else {
                    tr("暂无归档会话。")
                };
                panel.child(
                    super::settings_card(cx).child(
                        super::settings_row(false, cx).child(
                            div()
                                .py_2()
                                .text_sm()
                                .text_color(Color::Muted.get(cx))
                                .child(empty_text),
                        ),
                    ),
                )
            })
            .when(!state.items.is_empty(), |panel| {
                panel.child(
                    super::settings_card(cx).children(state.items.iter().enumerate().map(
                        |(index, hit)| {
                            let hit_open = hit.clone();
                            let session_restore = hit.session.clone();
                            let restoring =
                                self.session_archive_loading.contains(&hit.session.id)
                                    || self.session_pin_loading.contains(&hit.session.id);
                            let formatted_time =
                                format_session_time(hit.session.updated_at, time_format, is_english);
                            let snippet = hit.body_match.as_ref().map(|m| m.snippet.clone());
                            let title = if hit.title.trim().is_empty() {
                                tr("未命名会话").to_owned()
                            } else {
                                hit.title.clone()
                            };
                            let mut accessible_result = format!(
                                "{title} · {} · {}",
                                hit.workspace_name, hit.workspace_path
                            );
                            let branch = recorded_worktree_branch(&hit.session);
                            if let Some(branch) = branch {
                                accessible_result.push('\n');
                                accessible_result.push_str(&recorded_worktree_branch_label(branch));
                            }
                            if let Some(snippet) = &snippet {
                                accessible_result.push('\n');
                                accessible_result.push_str(snippet);
                            }
                            let restore_label = format!("{}: {accessible_result}", tr("恢复会话"));
                            let open_label = format!("{}: {accessible_result}", tr("打开会话"));
                            super::settings_row(index > 0, cx)
                                .id(("archive-result-group", index))
                                .role(Role::Group)
                                .aria_label(accessible_result)
                                .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_start()
                                    .justify_between()
                                    .gap_3()
                                    .flex_wrap()
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w(px(200.))
                                            .gap_1()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .truncate()
                                                    .child(title),
                                            )
                                            .child(
                                                h_flex()
                                                    .flex_wrap()
                                                    .items_center()
                                                    .gap_2()
                                                    .child(
                                                        div()
                                                            .px_1p5()
                                                            .py_0p5()
                                                            .rounded_sm()
                                                            .bg(Color::Surface.get(cx))
                                                            .text_xs()
                                                            .text_color(Color::Secondary.get(cx))
                                                            .child(hit.workspace_name.clone()),
                                                    )
                                                    .when_some(formatted_time, |meta, label| {
                                                        meta.child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(label),
                                                        )
                                                    }),
                                            )
                                            .when_some(branch, |col, branch| {
                                                col.child(recorded_worktree_branch_row(branch, cx))
                                            })
                                            .when_some(snippet, |col, snippet| {
                                                col.child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .line_clamp(2)
                                                        .child(snippet),
                                                )
                                            }),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1p5()
                                            .child(Reveal::new(
                                                SharedString::from(format!(
                                                    "archive-restore-{index}-reveal"
                                                )),
                                                &self.archive_scroll,
                                                Button::new(("archive-restore", index))
                                                    .outline()
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_md()
                                                    .label(tr("恢复会话"))
                                                    .accessibility_label(restore_label)
                                                    .disabled(!self.connected || restoring)
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.set_session_archived(
                                                            session_restore.clone(),
                                                            false,
                                                            cx,
                                                        );
                                                    })),
                                            ))
                                            .child(Reveal::new(
                                                SharedString::from(format!(
                                                    "archive-open-{index}-reveal"
                                                )),
                                                &self.archive_scroll,
                                                Button::new(("archive-open", index))
                                                    .ghost()
                                                    .small()
                                                    .h(px(26.))
                                                    .px_2p5()
                                                    .rounded_md()
                                                    .label(tr("打开会话"))
                                                    .accessibility_label(open_label)
                                                    .disabled(!self.connected)
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            if this.navigate_to_search_hit(
                                                                hit_open.clone(),
                                                                window,
                                                                cx,
                                                            ) {
                                                                this.leave_preferences(window, cx);
                                                                window.close_dialog(cx);
                                                            }
                                                        },
                                                    )),
                                            )),
                                    ),
                            )
                        },
                    )),
                )
            })
            .when(state.loaded && (!state.items.is_empty() || has_more), |panel| {
                panel.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(if is_english {
                                    format!("Showing {loaded_count} of {total}")
                                } else {
                                    format!("已显示 {loaded_count} / 共 {total} 条")
                                }),
                        )
                        .when(has_more, |row| {
                            row.child(Reveal::new(
                                "archive-load-more-reveal",
                                &self.archive_scroll,
                                Button::new("archive-load-more")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(if loading {
                                        tr("加载中…")
                                    } else {
                                        tr("加载更多")
                                    })
                                    .disabled(!self.connected || loading)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.load_archived_settings(true, cx);
                                    })),
                            ))
                        }),
                )
            })
    }
}
