use crate::app::HiveApp;
use crate::chrome;
use crate::conversation_list::CHAT_COLUMN_WIDTH;
use crate::language::{english, tr};
use crate::palette::Color;
use gpui_kit::assets::IconName as Glyph;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Input, Textarea},
        menu::{DropdownMenu, PopupMenuItem},
        *,
    },
    *,
};

pub(crate) fn untitled_session_label(session: &crate::protocol::Session) -> Option<&'static str> {
    // New records bind to an agent before their first user turn. Keep actual
    // conversation titles and explicit renames verbatim, even for this text.
    (!session.custom_title && session.agent_id.is_none() && session.title == "新会话")
        .then(|| tr("新会话"))
}

impl HiveApp {
    /// Shared connection recovery for the workspace and its modal settings.
    pub(crate) fn render_connection_notice(&self, inset: Pixels, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .flex_shrink_0()
            .min_h(px(if inset > px(0.) {
                chrome::TOPBAR_HEIGHT
            } else {
                40.
            }))
            .items_center()
            .gap_2p5()
            .px_3()
            .py_1p5()
            .when(inset > px(0.), |b| b.pl(inset + px(8.)))
            .bg(Color::Sidebar.get(cx))
            .border_b_1()
            .border_color(Color::Divider.get(cx))
            .child(
                div()
                    .size(px(6.))
                    .flex_shrink_0()
                    .rounded_full()
                    .bg(if self.restarting {
                        Color::Accent.get(cx)
                    } else {
                        Color::Error.get(cx)
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_xs()
                    .text_color(Color::Text.get(cx))
                    .child(if self.restarting {
                        tr("正在重新启动后台，当前草稿已保留。")
                    } else {
                        tr("后台未连接，当前草稿已保留。")
                    }),
            )
            .child(
                Button::new("restart-backend")
                    .outline()
                    .compact()
                    .small()
                    .h(px(26.))
                    .px_2p5()
                    .rounded_lg()
                    .label(tr("重新启动后台"))
                    .disabled(self.restarting)
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.restart_backend(window, cx)
                        }),
                    ),
            )
            .into_any_element()
    }

    fn render_chat_breadcrumb(
        &self,
        project: Option<crate::protocol::Workspace>,
        title: String,
        disabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let backend = match self
            .session
            .as_ref()
            .map(|session| session.backend)
            .unwrap_or_default()
        {
            crate::protocol::Backend::Pi => "pi",
            crate::protocol::Backend::Codex => "codex",
            crate::protocol::Backend::PiRpc => "pi-rpc",
            crate::protocol::Backend::Claude => "claude",
        };
        h_flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .items_center()
            .gap_2()
            .when_some(project, |crumb, project| {
                let project_id = project.id.clone();
                let label = if english() {
                    format!("New conversation in {}", project.name)
                } else {
                    format!("在 {} 中新建会话", project.name)
                };
                crumb
                    .child(
                        Button::new("chat-project-new-session")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .h(px(26.))
                            .px_1p5()
                            .rounded_md()
                            .min_w_0()
                            .max_w(px(160.))
                            .flex_shrink_1()
                            .tooltip(label.clone())
                            .accessibility_label(label)
                            .disabled(
                                !self.connected || self.loading || !self.workspace_writes_allowed(),
                            )
                            .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_center()
                                    .gap_1p5()
                                    .child(crate::palette::project_badge(&project.name, 14., cx))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(13.))
                                            .text_color(Color::Secondary.get(cx))
                                            .child(project.name),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.new_session_in_project(project_id.clone(), backend, cx)
                            })),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(12.))
                            .text_color(Color::Muted.get(cx).opacity(0.55))
                            .child("/"),
                    )
            })
            .when_some(self.session.clone(), |crumb, session| {
                let weak = cx.weak_entity();
                let rename = weak.clone();
                let rename_id = session.id.clone();
                let double_click_id = rename_id.clone();
                let archive_blocked = self.session_archive_blocked(&session.id);
                let pinning = self.session_pin_loading.contains(&session.id)
                    || self.session_archive_loading.contains(&session.id);
                crumb.child(
                    v_flex().flex_1().min_w_0().max_w(px(420.)).child(
                        Button::new("session-title-menu")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .h(px(26.))
                            .px_1p5()
                            .rounded_md()
                            .w_full()
                            .min_w_0()
                            .tooltip(title.clone())
                            .accessibility_label(if english() {
                                format!("Current conversation actions: {title}")
                            } else {
                                format!("当前会话操作：{title}")
                            })
                            .disabled(disabled || pinning)
                            .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id("session-title")
                                            .role(Role::Label)
                                            .aria_label(title.clone())
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(Color::Text.get(cx))
                                            .child(title),
                                    )
                                    .child(
                                        Icon::new(Glyph::ChevronDown)
                                            .size(px(12.))
                                            .flex_shrink_0()
                                            .text_color(Color::Muted.get(cx)),
                                    ),
                            )
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                if event.click_count() == 2
                                    && !event.modifiers().modified()
                                    && this
                                        .session
                                        .as_ref()
                                        .is_some_and(|session| session.id == double_click_id)
                                {
                                    // The in-window menu leaves the title under the pointer.
                                    // Removing its trigger closes it before the rename field appears.
                                    this.begin_session_rename(window, cx);
                                    let focus = this.session_name.read(cx).focus_handle(cx);
                                    window.on_next_frame(move |window, cx| focus.focus(window, cx));
                                }
                            }))
                            .dropdown_menu(move |menu, _, _| {
                                let rename = rename.clone();
                                let rename_id = rename_id.clone();
                                let menu = menu
                                    .item(PopupMenuItem::new(tr("重命名当前会话")).on_click(
                                        move |_, window, cx| {
                                            let _ =
                                                rename.update(cx, |this, cx| {
                                                    if this.session.as_ref().is_some_and(
                                                        |session| session.id == rename_id,
                                                    ) {
                                                        this.begin_session_rename(window, cx);
                                                    }
                                                });
                                        },
                                    ))
                                    .separator();
                                Self::session_menu_items(
                                    menu,
                                    session.clone(),
                                    archive_blocked,
                                    weak.clone(),
                                )
                            }),
                    ),
                )
            })
            .when(self.session.is_none(), |crumb| {
                crumb.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .text_color(Color::Text.get(cx))
                        .child(tr("新对话")),
                )
            })
            .into_any_element()
    }

    pub(crate) fn agent_name(&self, id: &str) -> String {
        self.members
            .iter()
            .find(|member| member.id == id)
            .map(|member| member.name.clone())
            .unwrap_or_else(|| {
                if id.ends_with(":orchestrator") {
                    tr("主控").into()
                } else if id.ends_with(":__workflow__") {
                    tr("工作流").into()
                } else if let Some(session) = self
                    .sessions
                    .iter()
                    .find(|session| session.agent_id.as_deref() == Some(id))
                {
                    // A stale/in-flight roster is not evidence that this member is historical.
                    if self.member_loading
                        || self.member_error.is_some()
                        || self.busy.contains(&session.id)
                    {
                        return self.session_title(session).to_owned();
                    }
                    if english() {
                        format!("{} · Historical conversation", self.session_title(session))
                    } else {
                        format!("{} · 历史会话", self.session_title(session))
                    }
                } else {
                    id.into()
                }
            })
    }

    pub(crate) fn session_title<'a>(&'a self, session: &'a crate::protocol::Session) -> &'a str {
        if session.custom_title {
            return &session.title;
        }
        if let Some(title) = untitled_session_label(session) {
            return title;
        }
        session
            .agent_id
            .as_ref()
            .and_then(|id| self.members.iter().find(|member| &member.id == id))
            .map(|member| member.name.as_str())
            .unwrap_or(&session.title)
    }
}

impl HiveApp {
    pub(crate) fn message_actions(
        &self,
        index: usize,
        message: &crate::app::Message,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let copy_id = message.id.clone();
        let reply_id = message.id.clone();
        let delivery = message.delivery.is_some();
        h_flex()
            .gap_0p5()
            .when(message.role == "user", |row| row.justify_end())
            .child(
                Button::new(("copy", index))
                    .custom(crate::palette::subtle_icon_variant(cx))
                    .compact()
                    .xsmall()
                    .h(px(24.))
                    .w(px(24.))
                    .rounded_md()
                    .icon(Glyph::Copy)
                    .disabled(!delivery && message.text.is_empty())
                    .tooltip(if delivery {
                        tr("复制完整原文")
                    } else {
                        tr("复制消息")
                    })
                    .accessibility_label(if delivery {
                        tr("复制完整消息原文")
                    } else {
                        tr("复制消息内容")
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(message) = this.messages.iter().find(|m| m.id == copy_id) {
                            let text = message.delivery.as_ref().map_or(&message.text, |d| &d.raw);
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        }
                    })),
            )
            .when(
                !message.text.is_empty()
                    && (message.role == "assistant" || message.explicit_user || delivery),
                |row| {
                    row.child(
                        Button::new(("quote", index))
                            .custom(crate::palette::subtle_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(24.))
                            .w(px(24.))
                            .rounded_md()
                            .icon(Glyph::Reply)
                            .tooltip(tr("引用回复"))
                            .accessibility_label(tr("引用消息到草稿"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if let Some(message) =
                                    this.messages.iter().find(|m| m.id == reply_id)
                                {
                                    let quote = message
                                        .text
                                        .lines()
                                        .map(|line| format!("> {line}"))
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    this.input.update(cx, |input, cx| {
                                        let value = format!("{quote}\n\n{}", input.value());
                                        input.replace_all(value, window, cx);
                                        input.focus(window, cx);
                                    });
                                }
                            })),
                    )
                },
            )
    }

    pub(crate) fn dispatch_link(
        &self,
        source: &str,
        dispatch: Option<&str>,
        cx: &Context<Self>,
    ) -> Option<Button> {
        let id = dispatch?.to_owned();
        if uuid::Uuid::try_parse(&id).is_err() {
            return None;
        }
        Some(
            Button::new(SharedString::from(format!("dispatch-link-{source}")))
                .custom(crate::palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(22.))
                .px_1p5()
                .rounded_md()
                .label(tr("派单详情"))
                .accessibility_label(tr("打开关联派单"))
                .disabled(!self.connected)
                .on_click(
                    cx.listener(move |this, _, _, cx| this.open_dispatch_from_chat(id.clone(), cx)),
                ),
        )
    }

    pub(crate) fn member_session_link(
        &self,
        source_id: &str,
        worker_id: Option<&str>,
        label: &'static str,
        cx: &Context<Self>,
    ) -> Option<impl IntoElement + use<>> {
        let worker = worker_id?;
        let member_chat = self
            .sessions
            .iter()
            .find(|s| s.agent_id.as_deref() == Some(worker))?;
        let id = member_chat.id.clone();
        Some(
            Button::new(SharedString::from(format!("member-link-{source_id}")))
                .custom(crate::palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(22.))
                .px_1p5()
                .rounded_md()
                .label(label)
                .accessibility_label(format!(
                    "{}: {}",
                    tr("打开成员会话"),
                    self.session_title(member_chat)
                ))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_member_session(id.clone(), window, cx)
                })),
        )
    }

    fn conversation_return(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        use crate::app::ConversationSource;
        let session = self.session.as_ref()?;
        let origin = self.conversation_sources.last().cloned();
        let (label, source) = match origin {
            Some(ConversationSource::Dispatch(id)) => {
                if !self
                    .collaboration
                    .selected
                    .as_ref()
                    .is_some_and(|d| d.id == id)
                {
                    return None;
                }
                (
                    tr("返回派单详情").to_owned(),
                    ConversationSource::Dispatch(id),
                )
            }
            Some(ConversationSource::Workflow) => {
                (tr("返回工作流").to_owned(), ConversationSource::Workflow)
            }
            Some(ConversationSource::Team) => {
                (tr("返回团队成员").to_owned(), ConversationSource::Team)
            }
            Some(ConversationSource::Session(id)) => {
                let previous = self.sessions.iter().find(|s| s.id == id)?;
                (
                    if english() {
                        format!("Back to {}", self.session_title(previous))
                    } else {
                        format!("返回 {}", self.session_title(previous))
                    },
                    ConversationSource::Session(id),
                )
            }
            None => {
                let lead = format!("{}:orchestrator", session.workspace_id);
                if session.agent_id.as_deref().is_none_or(|id| id == lead) {
                    return None;
                }
                let previous = self.sessions.iter().find(|s| {
                    s.workspace_id == session.workspace_id && s.agent_id.as_deref() == Some(&lead)
                })?;
                (
                    tr("打开主会话").to_owned(),
                    ConversationSource::Session(previous.id.clone()),
                )
            }
        };
        Some(
            Button::new("conversation-return")
                .custom(crate::palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .h(px(28.))
                .w(px(28.))
                .rounded_lg()
                .icon(Glyph::ArrowLeft)
                .accessibility_label(label.clone())
                .tooltip(label)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.conversation_sources.pop();
                    match &source {
                        ConversationSource::Dispatch(_) => {
                            this.collaboration.visible = true;
                            this.collaboration.focus_navigation = true;
                            this.load_dispatch_record(cx);
                            if this.collaboration.chat_origin.is_none() {
                                this.load_tasks(cx);
                            }
                            if this.collaboration.message_loading {
                                let after = this.collaboration.requested_after.clone();
                                this.load_dispatch_messages(after, cx);
                            }
                        }
                        ConversationSource::Workflow => this.show_workflows(window, cx),
                        ConversationSource::Team => this.show_team(window, cx),
                        ConversationSource::Session(id) => this.show_session(id.clone(), cx),
                    }
                })),
        )
    }

    pub(crate) fn render_error_banner(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        h_flex()
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .items_start()
            .px_3p5()
            .py_2p5()
            .gap_2p5()
            .bg(Color::ErrorSurface.get(cx))
            .border_1()
            .border_color(Color::Error.get(cx).opacity(0.24))
            .rounded_xl()
            .child(
                div()
                    .id("error-banner-text")
                    .role(Role::Label)
                    .aria_label(
                        self.error
                            .as_ref()
                            .map(|error| error.text())
                            .unwrap_or_default(),
                    )
                    .flex_1()
                    .min_w_0()
                    .max_h(px(160.))
                    .overflow_y_scroll()
                    .text_sm()
                    .whitespace_normal()
                    .child(
                        self.error
                            .as_ref()
                            .map(|error| error.text())
                            .unwrap_or_default(),
                    ),
            )
            .child(
                Button::new("dismiss-error")
                    .outline()
                    .compact()
                    .small()
                    .h(px(26.))
                    .rounded_lg()
                    .flex_shrink_0()
                    .label(tr("关闭"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.error = None;
                        cx.notify();
                    })),
            )
    }

    pub(crate) fn render_composer_card(
        &self,
        window: &mut Window,
        welcome: bool,
        running: bool,
        can_send: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let composer_focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let is_dark = Theme::global(cx).is_dark();
        let composer_border = if is_dark {
            if composer_focused {
                hsla(0., 0., 1., 0.13)
            } else {
                hsla(0., 0., 1., 0.08)
            }
        } else if composer_focused {
            hsla(0., 0., 0., 0.11)
        } else {
            hsla(0., 0., 0., 0.08)
        };
        v_flex()
            .relative()
            .w_full()
            .min_w_0()
            .min_h_0()
            .max_w(px(CHAT_COLUMN_WIDTH))
            .mx_auto()
            .flex_shrink_1()
            .gap_2()
            .px(px(14.))
            .pt(px(12.))
            .pb(px(8.))
            .when(welcome, |composer| {
                composer.min_h(px(118.)).justify_between()
            })
            .rounded_2xl()
            .border_1()
            .border_color(composer_border)
            .bg(Color::Card.get(cx))
            .shadow_xs()
            .child(crate::focus_scroll::Viewport::new(
                "composer-body-viewport",
                &self.composer_scroll,
                v_flex()
                    .id("composer-body-scroll")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.composer_scroll)
                    .vertical_scrollbar(&self.composer_scroll)
                    .child(
                        v_flex()
                            .flex_shrink_0()
                            .min_w_0()
                            .gap_2()
                            .when_some(
                                self.render_session_failure(window, running, cx),
                                |body, failure| {
                                    body.child(crate::focus_scroll::Reveal::new(
                                        "composer-failure-reveal",
                                        &self.composer_scroll,
                                        failure,
                                    ))
                                },
                            )
                            .when_some(self.render_pending_user_inputs(cx), |body, inputs| {
                                body.child(inputs)
                            })
                            .when_some(
                                self.session.as_ref().and_then(|session| {
                                    self.failed_drafts
                                        .get(&session.id)
                                        .map(|drafts| (session.id.clone(), drafts))
                                }),
                                |view, (session_id, drafts)| {
                                    view.child(v_flex().gap_2().children(
                                        drafts.iter().enumerate().map(|(i, draft)| {
                                            let session_id = session_id.clone();
                                            let text = draft.clone();
                                            crate::focus_scroll::Reveal::new(
                                                ("failed-draft-reveal", i),
                                                &self.composer_scroll,
                                                h_flex()
                                                    .items_center()
                                                    .gap_2()
                                                    .p_3()
                                                    .rounded_xl()
                                                    .border_1()
                                                    .border_color(
                                                        Color::Error.get(cx).opacity(0.28),
                                                    )
                                                    .bg(Color::ErrorSurface.get(cx))
                                                    .child(
                                                        div().flex_1().min_w_0().text_sm().child(
                                                            format!(
                                                                "{}: {draft}",
                                                                tr("发送失败，内容已保留")
                                                            ),
                                                        ),
                                                    )
                                                    .child(
                                                        Button::new(("copy-failed-draft", i))
                                                            .outline()
                                                            .compact()
                                                            .small()
                                                            .h(px(26.))
                                                            .rounded_lg()
                                                            .label(tr("复制内容"))
                                                            .accessibility_label(tr(
                                                                "复制发送失败的内容",
                                                            ))
                                                            .on_click(cx.listener(
                                                                move |_, _, _, cx| {
                                                                    cx.write_to_clipboard(
                                                                        ClipboardItem::new_string(
                                                                            text.clone(),
                                                                        ),
                                                                    )
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Button::new(("dismiss-failed-draft", i))
                                                            .custom(
                                                                crate::palette::quiet_icon_variant(
                                                                    cx,
                                                                ),
                                                            )
                                                            .compact()
                                                            .small()
                                                            .h(px(26.))
                                                            .rounded_lg()
                                                            .label(tr("移除"))
                                                            .accessibility_label(tr(
                                                                "移除发送失败的内容",
                                                            ))
                                                            .on_click(cx.listener(
                                                                move |this, _, _, cx| {
                                                                    if let Some(drafts) = this
                                                                        .failed_drafts
                                                                        .get_mut(&session_id)
                                                                    {
                                                                        if i < drafts.len() {
                                                                            drafts.remove(i);
                                                                        }
                                                                    }
                                                                    this.schedule_draft_save(cx);
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    ),
                                            )
                                        }),
                                    ))
                                },
                            )
                            .when(!self.connected && self.runtime.is_some(), |view| {
                                view.child(crate::focus_scroll::Reveal::new(
                                    "disconnected-draft-reveal",
                                    &self.composer_scroll,
                                    Button::new("copy-disconnected-draft")
                                        .label(tr("复制当前草稿"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            cx.write_to_clipboard(ClipboardItem::new_string(
                                                this.input.read(cx).value().to_string(),
                                            ));
                                        })),
                                ))
                            })
                            .when(self.command_picker.open, |view| {
                                view.child(
                                    deferred(
                                        anchored()
                                            .anchor(Anchor::BottomLeft)
                                            .snap_to_window()
                                            .offset(point(px(14.), px(-8.)))
                                            .child(self.render_commands(window, cx)),
                                    )
                                    .with_priority(1),
                                )
                            })
                            .when_some(self.render_input_attachments(cx), |body, attachments| {
                                body.child(crate::focus_scroll::Reveal::new(
                                    "composer-attachments-reveal",
                                    &self.composer_scroll,
                                    attachments,
                                ))
                            })
                            .child(crate::focus_scroll::Reveal::new(
                                "composer-input-reveal",
                                &self.composer_scroll,
                                div()
                                    .capture_action(cx.listener(Self::composer_enter))
                                    .capture_action(cx.listener(Self::command_up))
                                    .capture_action(cx.listener(Self::command_down))
                                    .capture_action(cx.listener(Self::command_escape))
                                    .child(
                                        Textarea::new(&self.input)
                                            .appearance(false)
                                            .bordered(false)
                                            .aria_label(tr("聊天消息"))
                                            .on_paste({
                                                let app = cx.weak_entity();
                                                move |item, _, cx| {
                                                    app.update(cx, |this, cx| {
                                                        if this.workspace.is_none() {
                                                            return false;
                                                        }
                                                        this.paste_images(item, cx)
                                                    })
                                                    .unwrap_or(false)
                                                }
                                            }),
                                    ),
                            )),
                    ),
            ))
            .child(div().flex_shrink_0().child(self.render_composer_controls(
                window, welcome, running, can_send, cx,
            )))
    }

    pub(crate) fn render_composer_status(
        &self,
        running: bool,
        pending: usize,
        needs_pi_model: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        v_flex()
            .w_full()
            .max_w(px(CHAT_COLUMN_WIDTH))
            .mx_auto()
            .gap_1p5()
            .when(
                !self.connected || running || pending > 0 || !self.status.is_background_connected(),
                |v| {
                    v.child(
                        div()
                            .px_1()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(if !self.connected {
                                tr("后台暂不可用 · 可在窗口顶部重新启动后台").to_string()
                            } else if !self.status.is_background_connected() {
                                self.status.text()
                            } else if running {
                                tr("Agent 正在处理 · 新消息会按顺序排队").to_string()
                            } else {
                                tr("有待处理消息，请先继续").to_string()
                            }),
                    )
                },
            )
            .when(needs_pi_model && self.connected, |surface| {
                surface.child(
                    h_flex()
                        .w_full()
                        .px_1()
                        .min_w_0()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(if let Some(model) = &self.unavailable_model {
                                    if english() {
                                        format!("{} / {} is unavailable", model.provider, model.id)
                                    } else {
                                        format!("{} / {} 暂不可用", model.provider, model.id)
                                    }
                                } else if self.models.iter().any(|m| m.available) {
                                    tr("选择模型后即可发送。").into()
                                } else {
                                    tr("请先连接 Pi 账号或配置模型提供商，然后刷新。").into()
                                }),
                        )
                        .when_some(self.pi_agent_dir.clone(), |row, dir| {
                            row.child(
                                Button::new("locate-pi-config")
                                    .ghost()
                                    .compact()
                                    .label(tr("定位配置目录"))
                                    .tooltip(dir.display().to_string())
                                    .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&dir))),
                            )
                        })
                        .child(
                            Button::new("open-pi-accounts")
                                .ghost()
                                .compact()
                                .label(tr("连接账号"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.settings_section = crate::app::SettingsSection::PiAccounts;
                                    this.open_settings(window, cx);
                                })),
                        )
                        .child(
                            Button::new("refresh-pi-models")
                                .ghost()
                                .compact()
                                .label(if self.refreshing_models {
                                    tr("刷新中…")
                                } else {
                                    tr("刷新模型")
                                })
                                .disabled(self.refreshing_models || running)
                                .on_click(cx.listener(|this, _, _, cx| this.refresh_pi_models(cx))),
                        ),
                )
            })
    }
}

impl Render for HiveApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(feature = "ui-preview")]
        crate::ui_preview::maybe_apply(self, window, cx);
        let locale = cx.global::<crate::language::Language>().locale;
        if self.interface_locale != Some(locale) {
            self.localize_placeholders(window, cx);
            self.interface_locale = Some(locale);
        }
        self.restore_task_editor(window, cx);
        if self.draft_target != self.draft_owner {
            if let Some(key) = self
                .draft_target
                .as_ref()
                .filter(|key| key.starts_with("workspace:"))
            {
                if self.draft_owner.is_none()
                    && self
                        .drafts
                        .get(key)
                        .is_none_or(|existing| existing.is_empty())
                {
                    let carried = self.input.read(cx).value().to_string();
                    if !carried.trim().is_empty() {
                        self.drafts.insert(key.clone(), carried);
                    }
                }
            }
            let text = self
                .draft_target
                .as_ref()
                .and_then(|key| self.drafts.get(key))
                .cloned()
                .unwrap_or_default();
            self.input.update(cx, |input, cx| {
                let end = text.len();
                input.set_value(text, window, cx);
                input.set_selected_range(end..end, cx);
            });
            self.draft_owner = self.draft_target.clone();
        }
        let placeholder: String = tr("输入任何问题，支持粘贴图片与 / 命令").into();
        if self.input_placeholder != placeholder {
            self.input_placeholder = placeholder.clone();
            self.input
                .update(cx, |input, cx| input.set_placeholder(placeholder, window, cx));
        }
        self.prepare_interactions(window, cx);
        self.prepare_terminal_view(cx);
        let has_session_failure = self.agent_diagnostic.is_some() || self.delivery_failure.is_some();
        let running = self
            .session
            .as_ref()
            .is_some_and(|s| self.busy.contains(&s.id));
        let has_interactions = self
            .session
            .as_ref()
            .and_then(|session| self.interactions.sessions.get(&session.id))
            .is_some_and(|cards| !cards.is_empty());
        let pending = self
            .session
            .as_ref()
            .and_then(|s| self.pending.get(&s.id))
            .copied()
            .unwrap_or(0);
        let open_failed = self.session.is_some()
            && !self.conversation_ready
            && !self.loading
            && self.connected
            && !self.closing;
        let needs_pi_model = self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Pi))
            && self.conversation_ready
            && self.model.is_none()
            && !self.loading;
        self.ensure_attachment_previews(cx);
        let can_submit = self.attachments.ready(self.draft_target.as_ref())
            && self.conversation_ready
            && self.session.is_some()
            && !needs_pi_model
            && self.workspace_writes_allowed()
            && self.connected
            && self
                .session
                .as_ref()
                .is_none_or(|session| !self.inflight_inputs.contains_key(&session.id))
            && !self.loading
            && (pending == 0 || running)
            && !self.closing;
        let has_text = !self.input.read(cx).value().trim().is_empty()
            || !self.attachments.refs(self.draft_target.as_ref()).is_empty();
        self.composer_has_text = has_text;
        let can_pick_project_to_send =
            self.workspace.is_none() && has_text && self.connected && !self.loading && !self.closing;
        self.composer_can_submit = can_submit || can_pick_project_to_send;
        let submit_on_enter = (can_submit || can_pick_project_to_send)
            && cx.global::<crate::composer::ComposerPreferences>().settings.mode
                == crate::composer::SendMode::Enter;
        if submit_on_enter != self.composer_submit_on_enter {
            self.composer_submit_on_enter = submit_on_enter;
            self.input
                .update(cx, |input, cx| input.set_submit_on_enter(submit_on_enter, cx));
        }
        self.sync_conversation_list(cx);
        let welcome = self.session.as_ref().is_none_or(|s| s.agent_id.is_none())
            && !open_failed
            && self.pending_reports.is_empty()
            && self.messages.is_empty()
            && !has_interactions
            && !running
            && pending == 0
            && !has_session_failure
            && !self
                .session
                .as_ref()
                .is_some_and(|s| self.failed_drafts.get(&s.id).is_some_and(|d| !d.is_empty()))
            && self.connected
            && !self.loading;
        let reasoning_key = self
            .session
            .as_ref()
            .filter(|_| {
                self.conversation_ready && self.connected && !self.loading && !self.closing
            })
            .map(|session| {
                (
                    session.id.clone(),
                    self.model
                        .as_ref()
                        .map(|model| format!("{}/{}", model.provider, model.id)),
                )
            });
        let loaded_reasoning_key =
            window.use_keyed_state("reasoning-autoload-key", cx, |_, _| {
                None::<(String, Option<String>)>
            });
        if let Some(key) = reasoning_key {
            if loaded_reasoning_key.read(cx).as_ref() != Some(&key)
                && !self.reasoning_picker.loading
            {
                loaded_reasoning_key.update(cx, |slot, _| *slot = Some(key));
                let app = cx.entity().downgrade();
                window.on_next_frame(move |_, cx| {
                    let _ = app.update(cx, |this, cx| this.load_reasoning(None, cx));
                });
            }
        }
        let narrow = window.viewport_size().width < px(820.);
        let preferences = self.settings_panel || self.roles.visible;
        self.ensure_preferences_modal(window, cx);
        if !self.loading
            && (!narrow || !self.sidebar_open)
            && self.viewing_conversation()
            && std::mem::take(&mut self.focus_project_input)
        {
            let focus = self.input.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        if !preferences
            && (!narrow || !self.sidebar_open)
            && self.collaboration.visible
            && std::mem::take(&mut self.collaboration.focus_navigation)
        {
            let focus = self.collaboration.navigation_focus.clone();
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        let inset = chrome::traffic_light_inset(window);
        let disconnected_banner =
            !self.connected && !self.closing && (!self.loading || self.restarting);
        let remote_banner = self.connected
            && self.remote.valid
            && self
                .remote
                .pending
                .iter()
                .any(|p| p.expires_at > crate::settings::remote::now())
            && !(self.settings_panel
                && self.settings_section == crate::app::SettingsSection::Remote);

        v_flex()
            .id("workspace-root")
            .aria_hidden(preferences)
            .size_full()
            .bg(Color::Surface.get(cx))
            .text_color(Color::Text.get(cx))
            .when(disconnected_banner, |view| {
                view.child(
                    self.render_connection_notice(inset, cx),
                )
            })
            .when(remote_banner, |view| {
                view.child(
                    h_flex()
                        .flex_shrink_0()
                        .min_h(px(if inset > px(0.) && !disconnected_banner {
                            chrome::TOPBAR_HEIGHT
                        } else {
                            40.
                        }))
                        .items_center()
                        .gap_2p5()
                        .px_3()
                        .py_1p5()
                        .when(inset > px(0.) && !disconnected_banner, |b| {
                            b.pl(inset + px(8.))
                        })
                        .bg(Color::Sidebar.get(cx))
                        .border_b_1()
                        .border_color(Color::Divider.get(cx))
                        .child(
                            div()
                                .size(px(6.))
                                .flex_shrink_0()
                                .rounded_full()
                                .bg(Color::Accent.get(cx)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_xs()
                                .text_color(Color::Text.get(cx))
                                .child(crate::language::tr(
                                    "有新设备等待配对，请核对两端安全短码后确认。",
                                )),
                        )
                        .child(
                            Button::new("remote-pending-banner")
                                .outline()
                                .compact()
                                .small()
                                .h(px(26.))
                                .px_2p5()
                                .rounded_lg()
                                .label(crate::language::tr("查看配对请求"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.select_settings_section(
                                        crate::app::SettingsSection::Remote,
                                        window,
                                        cx,
                                    )
                                })),
                        ),
                )
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .when(self.sidebar_open, |view| {
                        view.child(self.render_navigation(narrow, window, cx))
                    })
                    .when(!narrow || !self.sidebar_open, |view| {
                        view.child(if self.memory.visible || self.workflows.visible || self.collaboration.visible || self.member_panel || self.terminal.visible {
                            let body = if self.memory.visible {
                                self.render_memory(cx).into_any_element()
                            } else if self.workflows.visible {
                                self.render_workflows(window, cx)
                            } else if self.collaboration.visible {
                                self.render_collaboration(window, cx)
                            } else if self.member_panel {
                                self.render_members(window, narrow, cx)
                            } else {
                                self.render_terminal(cx).into_any_element()
                            };
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .child(div().flex_1().min_h_0().min_w_0().w_full().child(body))
                                .into_any_element()
                        } else {
                            let active_project = self
                                .workspaces
                                .iter()
                                .find(|ws| Some(&ws.id) == self.workspace.as_ref())
                                .cloned();
                            let renaming = self.session.is_some()
                                && self.session_name_target.as_deref()
                                    == self.session.as_ref().map(|s| s.id.as_str());
                            let session_title = self
                                .session
                                .as_ref()
                                .map(|s| {
                                    self.session_title(s)
                                        .split_whitespace()
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                                .unwrap_or_else(|| tr("新对话").to_string());
                            let topbar_inset = if !self.sidebar_open
                                && inset > px(0.)
                                && !disconnected_banner
                                && !remote_banner
                            {
                                inset + px(8.)
                            } else {
                                px(16.)
                            };

                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .id("conversation-surface")
                                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                                    this.import_image_paths(paths.paths().to_vec(), cx)
                                }))
                                .drag_over::<ExternalPaths>(|style, _, _, cx| {
                                    style.bg(Color::Attention.get(cx))
                                })
                                .key_context("HiveConversation")
                                .on_action(cx.listener(
                                    |this, _: &crate::CloseConversationFind, window, cx| {
                                        this.close_conversation_find(window, cx)
                                    },
                                ))
                                .child(
                                    chrome::drag_bar("chat-topbar", window, cx)
                                        .w_full()
                                        .pl(topbar_inset)
                                        .pr_3()
                                        .gap_1p5()
                                        .when(!welcome, |bar| {
                                            bar.border_b_1().border_color(Color::Divider.get(cx))
                                        })
                                        .when(!self.sidebar_open, |bar| {
                                            bar.child(
                                                Button::new("toggle-navigation")
                                                    .custom(crate::palette::quiet_icon_variant(cx))
                                                    .compact()
                                                    .xsmall()
                                                    .h(px(28.))
                                                    .w(px(28.))
                                                    .rounded_lg()
                                                    .icon(Glyph::PanelLeft)
                                                    .tooltip(tr("打开或收起导航"))
                                                    .accessibility_label(tr(
                                                        "打开或收起工作区与会话导航",
                                                    ))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.sidebar_open = !this.sidebar_open;
                                                        cx.notify();
                                                    })),
                                            )
                                        })
                                        .when(!welcome, |bar| {
                                            bar.when_some(
                                                self.conversation_return(cx),
                                                |header, back| header.child(back),
                                            )
                                        })
                                        .when(!renaming, |header| {
                                            let show_conv_tools = self.conversation_ready
                                                && self.session.is_some()
                                                && !self.loading;
                                            let title_disabled = !self.connected
                                                || self.session_name_loading
                                                || !self.workspace_writes_allowed();
                                            header
                                                .child(self.render_chat_breadcrumb(active_project.clone(), session_title.clone(), title_disabled, cx))
                                                .when(has_session_failure, |bar| {
                                                    bar.child(
                                                        Button::new("show-session-failure")
                                                            .custom(
                                                                crate::palette::quiet_icon_variant(
                                                                    cx,
                                                                ),
                                                            )
                                                            .small()
                                                            .h(px(26.))
                                                            .px_2()
                                                            .rounded_lg()
                                                            .label(tr("错误详情"))
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.composer_scroll.set_offset(
                                                                        point(px(0.), px(0.)),
                                                                    );
                                                                    this.failure_scroll.set_offset(
                                                                        point(px(0.), px(0.)),
                                                                    );
                                                                    window.refresh();
                                                                    cx.notify();
                                                                },
                                                            )),
                                                    )
                                                })
                                                .when(
                                                    !has_session_failure
                                                        && (!self.status.is_background_connected()
                                                            || running),
                                                    |bar| {
                                                        bar.child(
                                                            h_flex()
                                                                .flex_shrink_0()
                                                                .items_center()
                                                                .gap_1p5()
                                                                .px_2p5()
                                                                .py_0p5()
                                                                .rounded_full()
                                                                .bg(Color::AccentSoft.get(cx))
                                                                .child(
                                                                    div()
                                                                        .w(px(6.))
                                                                        .h(px(6.))
                                                                        .rounded_full()
                                                                        .bg(Color::Accent.get(cx)),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .font_weight(
                                                                            FontWeight::MEDIUM,
                                                                        )
                                                                        .text_color(
                                                                            Color::Accent.get(cx),
                                                                        )
                                                                        .child(
                                                                            if running
                                                                                && self
                                                                                    .status
                                                                                    .is_background_connected()
                                                                            {
                                                                                tr("正在回复…")
                                                                                    .to_string()
                                                                            } else {
                                                                                self.status.text()
                                                                            },
                                                                        ),
                                                                ),
                                                        )
                                                    },
                                                )
                                                .when(self.reading_above_latest(), |bar| {
                                                    bar.child(
                                                        Button::new("jump-to-latest")
                                                            .custom(
                                                                crate::palette::quiet_icon_variant(
                                                                    cx,
                                                                ),
                                                            )
                                                            .small()
                                                            .h(px(28.))
                                                            .px_2()
                                                            .rounded_lg()
                                                            .icon(Glyph::ArrowDown)
                                                            .label(tr("回到最新"))
                                                            .accessibility_label(tr("回到最新消息"))
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.jump_to_latest(cx);
                                                                },
                                                            )),
                                                    )
                                                })
                                                .when(
                                                    show_conv_tools || active_project.is_some(),
                                                    |bar| {
                                                        bar.child(
                                                            h_flex()
                                                                .flex_shrink_0()
                                                                .items_center()
                                                                .gap_1()
                                                                .when(show_conv_tools, |tools| {
                                                                    tools.child(
                                                                        Button::new(
                                                                            "find-in-conversation",
                                                                        )
                                                                        .custom(
                                                                            crate::palette::quiet_icon_variant(
                                                                                cx,
                                                                            ),
                                                                        )
                                                                        .compact()
                                                                        .xsmall()
                                                                        .h(px(28.))
                                                                        .w(px(28.))
                                                                        .rounded_lg()
                                                                        .icon(Glyph::Search)
                                                                        .accessibility_label(tr(
                                                                            "查找当前会话",
                                                                        ))
                                                                        .tooltip(
                                                                            if cfg!(
                                                                                target_os = "macos"
                                                                            ) {
                                                                                tr("查找会话内容 · ⌘F")
                                                                            } else {
                                                                                tr("查找会话内容 · Ctrl+F")
                                                                            },
                                                                        )
                                                                        .on_click(cx.listener(
                                                                            |this, _, window, cx| {
                                                                                this.open_conversation_find(
                                                                                    window, cx,
                                                                                )
                                                                            },
                                                                        )),
                                                                    )
                                                                })
                                                                .when(
                                                                    !show_conv_tools && welcome,
                                                                    |tools| {
                                                                        tools.when_some(
                                                                            active_project.clone(),
                                                                            |tools, ws| {
                                                                                let folder_path =
                                                                                    std::path::PathBuf::from(
                                                                                        &ws.path,
                                                                                    );
                                                                                tools.child(
                                                                                    Button::new("topbar-open-folder")
                                                                                        .custom(crate::palette::quiet_icon_variant(cx))
                                                                                        .compact()
                                                                                        .xsmall()
                                                                                        .h(px(28.))
                                                                                        .w(px(28.))
                                                                                        .rounded_lg()
                                                                                        .icon(Glyph::FolderOpen)
                                                                                        .tooltip(tr("在文件管理器中打开项目"))
                                                                                        .accessibility_label(tr(
                                                                                            "在文件管理器中打开当前项目",
                                                                                        ))
                                                                                        .on_click(cx.listener(
                                                                                            move |_, _, _, cx| {
                                                                                                cx.reveal_path(&folder_path)
                                                                                            },
                                                                                        )),
                                                                                )
                                                                            },
                                                                        )
                                                                    },
                                                                )
                                                                .when_some(
                                                                    active_project.clone(),
                                                                    |tools, _| {
                                                                        tools.child(
                                                                            Button::new(
                                                                                "topbar-quick-terminal",
                                                                            )
                                                                            .custom(
                                                                                crate::palette::quiet_icon_variant(
                                                                                    cx,
                                                                                ),
                                                                            )
                                                                            .compact()
                                                                            .xsmall()
                                                                            .h(px(28.))
                                                                            .w(px(28.))
                                                                            .rounded_lg()
                                                                            .icon(Glyph::SquareTerminal)
                                                                            .tooltip(tr("打开工作区终端"))
                                                                            .accessibility_label(tr(
                                                                                "打开工作区终端",
                                                                            ))
                                                                            .disabled(!self.connected)
                                                                            .on_click(cx.listener(
                                                                                |this, _, window, cx| {
                                                                                    this.show_terminal(
                                                                                        window, cx,
                                                                                    );
                                                                                    cx.notify();
                                                                                },
                                                                            )),
                                                                        )
                                                                    },
                                                                )
                                                                .when(show_conv_tools, |tools| {
                                                                    tools.child(
                                                                        Button::new(
                                                                            "export-conversation",
                                                                        )
                                                                        .custom(
                                                                            crate::palette::quiet_icon_variant(
                                                                                cx,
                                                                            ),
                                                                        )
                                                                        .compact()
                                                                        .xsmall()
                                                                        .h(px(28.))
                                                                        .w(px(28.))
                                                                        .rounded_lg()
                                                                        .icon(Glyph::FileDown)
                                                                        .accessibility_label(tr(
                                                                            "导出当前对话",
                                                                        ))
                                                                        .tooltip(tr(
                                                                            "导出对话为 Markdown",
                                                                        ))
                                                                        .disabled(
                                                                            self.messages.is_empty(),
                                                                        )
                                                                        .on_click(cx.listener(
                                                                            |this, _, window, cx| {
                                                                                this.open_conversation_export(
                                                                                    window, cx,
                                                                                )
                                                                            },
                                                                        )),
                                                                    )
                                                                }),
                                                        )
                                                    },
                                                )
                                        })
                                        .when(renaming, |header| {
                                            header
                                                .key_context("HiveSessionName")
                                                .on_action(cx.listener(
                                                    |this,
                                                     _: &crate::CancelSessionRename,
                                                     _,
                                                     cx| {
                                                        if !this.session_name_loading {
                                                            this.session_name_target = None;
                                                            cx.notify();
                                                        }
                                                    },
                                                ))
                                                .child(
                                                    div().flex_1().min_w_0().child(
                                                        Input::new(&self.session_name)
                                                            .small()
                                                            .aria_label(tr("会话名称"))
                                                            .disabled(self.session_name_loading),
                                                    ),
                                                )
                                                .child(
                                                    Button::new("save-session-name")
                                                        .primary()
                                                        .small()
                                                        .h(px(28.))
                                                        .rounded_lg()
                                                        .label(tr("保存名称"))
                                                        .disabled(
                                                            !self.connected
                                                                || self.session_name_loading,
                                                        )
                                                        .on_click(cx.listener(
                                                            |this, _, _, cx| {
                                                                this.save_session_name(cx)
                                                            },
                                                        )),
                                                )
                                                .child(
                                                    Button::new("cancel-session-name")
                                                        .outline()
                                                        .small()
                                                        .h(px(28.))
                                                        .rounded_lg()
                                                        .label(tr("取消"))
                                                        .disabled(self.session_name_loading)
                                                        .on_click(cx.listener(
                                                            |this, _, _, cx| {
                                                                this.session_name_target = None;
                                                                cx.notify();
                                                            },
                                                        )),
                                                )
                                        }),
                                )
                                .when(self.conversation_find.is_some(), |view| {
                                    view.child(self.render_conversation_find(cx))
                                })
                                .when(self.error.is_some(), |view| {
                                    view.child(
                                        div()
                                            .w_full()
                                            .max_w(px(CHAT_COLUMN_WIDTH + 64.))
                                            .mx_auto()
                                            .px_5()
                                            .pt_2()
                                            .child(self.render_error_banner(cx)),
                                    )
                                })
                                .when(
                                    self.session.as_ref().is_some_and(|session| {
                                        self.interaction_count(&session.id) > 0
                                    }),
                                    |view| {
                                        view.child(
                                            div().px_5().pt_2().child(
                                                Button::new("show-pending-interactions")
                                                    .outline()
                                                    .compact()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_3()
                                                    .rounded_full()
                                                    .label(if english() {
                                                        format!(
                                                            "Waiting for response · {}",
                                                            self.session.as_ref().map_or(
                                                                0,
                                                                |session| self
                                                                    .interaction_count(&session.id),
                                                            )
                                                        )
                                                    } else {
                                                        format!(
                                                            "等待回应 · {} 项",
                                                            self.session.as_ref().map_or(
                                                                0,
                                                                |session| self
                                                                    .interaction_count(&session.id),
                                                            )
                                                        )
                                                    })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.jump_to_latest(cx);
                                                    })),
                                            ),
                                        )
                                    },
                                )
                                .when(
                                    self.session.as_ref().is_some_and(|session| session.archived),
                                    |surface| {
                                        let session = self.session.as_ref().unwrap().clone();
                                        surface.child(
                                            h_flex()
                                                .flex_wrap()
                                                .gap_2()
                                                .px_5()
                                                .py_2()
                                                .border_b_1()
                                                .border_color(Color::Divider.get(cx))
                                                .items_center()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx))
                                                        .child(tr(
                                                            "已归档 · 发送或收到新消息后自动恢复",
                                                        )),
                                                )
                                                .child(
                                                    Button::new("restore-current-session")
                                                        .ghost()
                                                        .compact()
                                                        .label(tr("恢复会话"))
                                                        .disabled(
                                                            !self.connected
                                                                || self
                                                                    .session_archive_loading
                                                                    .contains(&session.id)
                                                                || self
                                                                    .session_pin_loading
                                                                    .contains(&session.id),
                                                        )
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.set_session_archived(
                                                                    session.clone(),
                                                                    false,
                                                                    cx,
                                                                )
                                                            },
                                                        )),
                                                ),
                                        )
                                    },
                                )
                                .when(welcome, |surface| {
                                    let hero_can_send = if self.workspace.is_none() {
                                        has_text && self.connected && !self.loading && !self.closing
                                    } else {
                                        can_submit && has_text
                                    };
                                    surface.child(self.render_hero_stage(
                                        window,
                                        narrow,
                                        running,
                                        hero_can_send,
                                        pending,
                                        needs_pi_model,
                                        cx,
                                    ))
                                })
                                .when(!welcome && open_failed, |surface| {
                                    surface.child(
                                        v_flex()
                                            .flex_1()
                                            .min_h_0()
                                            .items_center()
                                            .justify_center()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .child(tr("未能打开会话，草稿已保留。")),
                                            )
                                            .child(
                                                self.render_agent_repair_actions(
                                                    !self.connected || self.loading,
                                                    cx,
                                                ),
                                            )
                                            .child(
                                                Button::new("retry-open-session")
                                                    .label(tr("重新打开会话"))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Some(session) = &this.session {
                                                            this.error = None;
                                                            this.show_session(
                                                                session.id.clone(),
                                                                cx,
                                                            );
                                                        }
                                                    })),
                                            ),
                                    )
                                })
                                .when(
                                    !welcome && !open_failed && self.session.is_some(),
                                    |surface| {
                                        surface.child(self.render_conversation_list(narrow, cx))
                                    },
                                )
                                .when(!welcome, |surface| {
                                    surface.child(
                                        v_flex()
                                            .w_full()
                                            .min_w_0()
                                            .min_h_0()
                                            .max_h(window.viewport_size().height * 0.62)
                                            .flex_shrink_0()
                                            .gap_1p5()
                                            .px(if narrow { px(16.) } else { px(32.) })
                                            .pt_1()
                                            .pb_3()
                                            .child(self.render_composer_card(
                                                window,
                                                welcome,
                                                running,
                                                can_submit && has_text,
                                                cx,
                                            ))
                                            .child(self.render_composer_meta_bar(
                                                window, running, cx,
                                            ))
                                            .child(self.render_composer_status(
                                                running,
                                                pending,
                                                needs_pi_model,
                                                cx,
                                            )),
                                    )
                                })
                                .into_any_element()
                        })
                    }),
            )
    }
}
