use crate::{
    app::HiveApp,
    language::{english, tr},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        menu::{ContextMenuExt, PopupMenuItem},
        text::{TextView, TextViewState},
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub(crate) fn is_collapsible_user_message(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }
    // ~552px body width inside 580px bubble at 15px font (100 units = 1px).
    const LINE_WIDTH_UNITS: usize = 55_200;
    let mut line_count = 0usize;
    for line in trimmed.split('\n') {
        let mut units = 0usize;
        for ch in line.chars() {
            let width = match ch {
                'i' | 'l' | 'j' | 't' | 'f' | 'r' | 'I' | '!' | '|' | '.' | ',' | ':' | ';'
                | '\'' | '`' | ' ' => 450,
                '1' | '-' | '(' | ')' | '[' | ']' | '{' | '}' | '/' | '\\' => 675,
                'W' | 'M' | '@' => 1_200,
                _ if ch.is_ascii() => 870,
                _ => 1_500,
            };
            units += width;
            if line_count + units.max(1).div_ceil(LINE_WIDTH_UNITS) > 8 {
                return true;
            }
        }
        line_count += units.max(1).div_ceil(LINE_WIDTH_UNITS);
        if line_count > 8 {
            return true;
        }
    }
    false
}

impl HiveApp {
    pub(crate) fn render_conversation_message(
        &self,
        i: usize,
        markdown: Option<&Entity<TextViewState>>,
        merge_prev: bool,
        merge_next: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let message = &self.messages[i];
        if let Some(tool) = &message.tool {
            return self
                .render_tool_message(i, message, tool, cx)
                .into_any_element();
        }
        let is_delivery = message.delivery.is_some();
        let is_plain_user = message.role == "user" && !is_delivery;
        let is_worker_session = self
            .session
            .as_ref()
            .and_then(|s| s.display_agent_id())
            .is_some_and(|id| !id.ends_with(":orchestrator") && !id.ends_with(":__workflow__"));
        let is_team_context = self.has_active_team_context() || is_worker_session || self.messages.iter().any(|m| m.delivery.is_some());
        let heading: SharedString = message
            .delivery
            .as_ref()
            .map(|delivery| delivery.heading_text().into())
            .unwrap_or_else(|| {
                match message.role.as_str() {
                    "user" if message.explicit_user => tr("你").into(),
                    "user" => tr("对话输入").into(),
                    "assistant" => self
                        .session
                        .as_ref()
                        .and_then(|s| s.display_agent_id())
                        .filter(|_| is_team_context)
                        .map(|id| self.agent_name(id).into())
                        .unwrap_or_else(|| {
                            self.session
                                .as_ref()
                                .map(|s| tr(s.backend.label()))
                                .unwrap_or(tr("助手"))
                                .into()
                        }),
                    "notice" => tr("会话通知").into(),
                    _ => tr("工具").into(),
                }
            });
        let find_matched = self.conversation_find_matches(&message.id);
        let hide_speaker_header = message.explicit_user
            || (message.role == "assistant"
                && !is_delivery
                && (!is_team_context
                    || !(i > 0 && self.messages[i - 1].delivery.is_some()
                        || (is_worker_session
                            && (i == 0
                                || self.messages[i - 1].role == "user"
                                || self.messages[i - 1].delivery.is_some())))));
        let mut delivery_actions = is_delivery.then(|| {
            let expanded = message.delivery.as_ref().is_some_and(|d| d.expanded);
            let dispatch_id = message
                .delivery
                .as_ref()
                .and_then(|d| d.dispatch_id.as_deref());
            let worker_id = message
                .delivery
                .as_ref()
                .and_then(|card| card.worker_id.as_deref());
            let message_id = message.id.clone();
            h_flex()
                .min_w_0()
                .flex_wrap()
                .items_center()
                .gap_1()
                .when_some(
                    self.dispatch_link(&message.id, dispatch_id, cx),
                    |links, link| links.child(link),
                )
                .when_some(
                    self.history_member_session_link(&message.id, dispatch_id, worker_id, tr("成员会话"), cx),
                    |links, link| links.child(link),
                )
                .child(
                    Button::new(("expand-delivery", i))
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .xsmall()
                        .h(px(22.))
                        .px_1p5()
                        .rounded_md()
                        .accessibility_label(format!(
                            "{} {}",
                            if expanded { tr("收起") } else { tr("查看") },
                            tr("消息原文")
                        ))
                        .label(if expanded {
                            tr("收起原文")
                        } else {
                            tr("查看原文")
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.prepare_conversation_expansion(&message_id);
                            if let Some(delivery) = this
                                .messages
                                .iter_mut()
                                .find(|m| m.id == message_id)
                                .and_then(|m| m.delivery.as_mut())
                            {
                                delivery.expanded = !delivery.expanded;
                            }
                            cx.notify();
                        })),
                )
        });
        let message_group = SharedString::from(format!("message-row-{i}"));
        v_flex()
            .id(("message", i))
            .group(message_group.clone())
            .when_some(message.markdown_focus.as_ref(), |view, focus| {
                view.track_focus(focus)
            })
            .role(Role::Group)
            .aria_label(heading.clone())
            .min_w_0()
            .when(is_plain_user, |view| view.gap_1())
            .when(!is_plain_user, |view| view.gap_2())
            .when(!is_plain_user && !is_delivery, |view| view.py_0p5())
            .when(!find_matched && is_delivery, |view| {
                view.px_4()
                    .py_3()
                    .rounded_xl()
                    .bg(Color::Card.get(cx))
                    .border_1()
                    .border_color(Color::Divider.get(cx))
            })
            .when(
                !find_matched && !is_delivery && message.role == "notice",
                |view| {
                    view.px_3p5()
                        .py_2p5()
                        .rounded_xl()
                        .bg(Color::Inset.get(cx))
                        .border_1()
                        .border_color(Color::Divider.get(cx))
                },
            )
            .when(find_matched, |view| {
                view.px_3p5()
                    .py_2p5()
                    .rounded_xl()
                    .bg(Color::AccentSoft.get(cx))
                    .border_1()
                    .border_color(Color::Accent.get(cx).opacity(0.32))
            })
            .when(is_plain_user, |bubble| {
                let bubble = bubble.ml_auto().items_end();
                if cx
                    .global::<crate::appearance::Appearance>()
                    .chat_width_max_px()
                    .is_some()
                {
                    bubble.max_w(px(580.))
                } else {
                    bubble.max_w(relative(0.8))
                }
            })
            .when(is_plain_user && message.steered && !merge_prev, |view| {
                view.child(
                    h_flex()
                        .id(("steer-badge", i))
                        .items_center()
                        .gap_1()
                        .px_1p5()
                        .py(px(1.))
                        .rounded_full()
                        .bg(Color::AccentSoft.get(cx))
                        .text_size(px(10.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Color::Accent.get(cx))
                        .tooltip(move |window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(tr("插话引导当前回合"))
                                .build(window, cx)
                        })
                        .child(Icon::new(Glyph::CornerDownRight).size(px(10.)))
                        .child(tr("插话")),
                )
            })
            .when(!hide_speaker_header, |view| {
                view.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .justify_between()
                        .when(is_delivery, |row| {
                            row.pb_1p5()
                                .border_b_1()
                                .border_color(Color::Divider.get(cx).opacity(0.75))
                        })
                        .child(
                            h_flex()
                                .min_w(px(120.))
                                .flex_1()
                                .flex_wrap()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .whitespace_normal()
                                        .when(!is_delivery && message.role == "assistant", |tag| {
                                            tag.px_2()
                                                .py(px(2.))
                                                .rounded_md()
                                                .bg(Color::Inset.get(cx))
                                                .text_size(px(11.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(Color::Secondary.get(cx))
                                        })
                                        .when(is_delivery || message.role != "assistant", |tag| {
                                            tag.text_xs()
                                                .font_weight(if is_delivery {
                                                    FontWeight::SEMIBOLD
                                                } else {
                                                    FontWeight::MEDIUM
                                                })
                                                .text_color(if is_delivery {
                                                    Color::Secondary.get(cx)
                                                } else {
                                                    Color::Muted.get(cx)
                                                })
                                        })
                                        .child(heading),
                                )
                                .when(
                                    message.delivery.as_ref().is_some_and(|card| card.report),
                                    |row| {
                                        let state = if self.completed_deliveries.contains(&message.id)
                                        {
                                            tr("处理回合已完成")
                                        } else {
                                            self.pending_reports
                                                .iter()
                                                .find(|report| report.delivery_id == message.id)
                                                .map(|report| tr(report.status()))
                                                .unwrap_or(tr("已进入会话"))
                                        };
                                        row.child(
                                            div()
                                                .id(("report-state", i))
                                                .role(Role::Label)
                                                .aria_value(state.to_owned())
                                                .rounded_full()
                                                .bg(Color::Inset.get(cx))
                                                .px_2()
                                                .py_0p5()
                                                .text_xs()
                                                .text_color(Color::Secondary.get(cx))
                                                .child(state),
                                        )
                                    },
                                ),
                        )
                        .when_some(delivery_actions.take(), |row, actions| row.child(actions)),
                )
            })
            .when_some(
                if hide_speaker_header {
                    delivery_actions.take()
                } else {
                    None
                },
                |view, actions| view.child(h_flex().w_full().justify_end().child(actions)),
            )
            .when(
                message.failure.is_some() && !self.hidden_current_failure(i),
                |view| view.child(self.render_reply_failure(i, cx)),
            )
            .children(
                message
                    .thinking
                    .iter()
                    .map(|block| self.render_thinking(i, &message.id, block, cx)),
            )
            .when_some(
                markdown,
                |view, markdown| {
                    let body = div()
                        .id(("message-body", i))
                        .role(Role::Label)
                        .aria_value(markdown.read(cx).rendered_text().as_str().to_string())
                        .child(TextView::new(markdown).selectable(true).stream_fade(true));
                    if message.role != "assistant" || self.current_worktree_released() {
                        return view.child(body);
                    }
                    let app = cx.weak_entity();
                    let markdown = markdown.clone();
                    let id = message.id.clone();
                    view.child(body.context_menu(move |menu, _, cx| {
                        // Build the action from this exact selection before the
                        // menu takes focus away from the response text.
                        let selection = app
                            .read_with(cx, |app, cx| {
                                app.capture_assistant_quote(&id, &markdown, cx)
                                    .map(|quote| (quote, app.open_request))
                            })
                            .ok()
                            .flatten();
                        let app = app.clone();
                        menu.item(
                            PopupMenuItem::new(tr("引用选中的文字"))
                                .disabled(selection.is_none())
                                .on_click(move |_, window, cx| {
                                    let Some((quote, revision)) = selection.clone() else {
                                        return;
                                    };
                                    let app = app.clone();
                                    window.defer(cx, move |window, cx| {
                                        let _ = app.update(cx, |app, cx| {
                                            app.insert_assistant_quote(quote, revision, window, cx);
                                        });
                                    });
                                }),
                        )
                    }))
                },
            )
            .when(
                markdown.is_none() && !message.display_body().is_empty(),
                |view| {
                    let body = if is_plain_user && message.explicit_user {
                        crate::quote_source::body_without_quote_source(message.display_body())
                            .to_string()
                    } else {
                        message.display_body().to_owned()
                    };
                    let collapsible = is_plain_user && is_collapsible_user_message(&body);
                    let user_expanded = message.user_expanded || find_matched;
                    let collapsed = collapsible && !user_expanded;
                    let bubble_bg = Color::Bubble.get(cx);
                    let toggle_id = message.id.clone();
                    view.child(
                        div()
                            .id(("message-body", i))
                            .role(Role::Label)
                            .aria_value(body.clone())
                            .min_w_0()
                            .when(is_delivery, |text| {
                                text.text_sm().line_height(rems(1.5))
                            })
                            .when(!is_delivery, |text| {
                                text.text_size(rems(15. / 16.)).line_height(rems(1.55))
                            })
                            .when(is_plain_user, |bubble| {
                                bubble
                                    .px_3p5()
                                    .py_2()
                                    .rounded_tl(px(16.))
                                    .rounded_bl(px(16.))
                                    .rounded_tr(px(if merge_prev { 6. } else { 16. }))
                                    .rounded_br(px(if merge_next { 6. } else { 16. }))
                                    .bg(bubble_bg)
                                    .border_1()
                                    .border_color(Color::Divider.get(cx).opacity(0.65))
                                    .text_color(Color::Text.get(cx))
                            })
                            .when(collapsed, |bubble| {
                                bubble
                                    .relative()
                                    .max_h(px(176.))
                                    .overflow_hidden()
                                    .child(body.clone())
                                    .child(
                                        div()
                                            .absolute()
                                            .left_0()
                                            .right_0()
                                            .bottom_0()
                                            .h(px(28.))
                                            .bg(linear_gradient(
                                                180.,
                                                linear_color_stop(bubble_bg.opacity(0.), 0.),
                                                linear_color_stop(bubble_bg, 1.),
                                            )),
                                    )
                            })
                            .when(!collapsed, |bubble| bubble.child(body)),
                    )
                    .when(collapsible && !find_matched, |view| {
                        view.child(
                            h_flex().w_full().justify_end().child(
                                Button::new(("toggle-user-expand", i))
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_full()
                                    .icon(if user_expanded {
                                        Glyph::ChevronUp
                                    } else {
                                        Glyph::ChevronDown
                                    })
                                    .label(if user_expanded {
                                        tr("收起")
                                    } else {
                                        tr("展开全部")
                                    })
                                    .accessibility_label(if user_expanded {
                                        tr("收起长消息")
                                    } else {
                                        tr("展开全部消息")
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.prepare_conversation_expansion(&toggle_id);
                                        if let Some(msg) = this.message_mut(&toggle_id) {
                                            msg.user_expanded = !msg.user_expanded;
                                            msg.layout_revision += 1;
                                        }
                                        cx.notify();
                                    })),
                            ),
                        )
                    })
                },
            )
            .when_some(
                (is_plain_user && message.explicit_user)
                    .then(|| self.render_assistant_quote_sources(message.display_body(), cx))
                    .flatten(),
                |view, sources| view.child(sources),
            )
            .when(message.images_loading
                || (message.history_images.is_some() && message.images.is_empty() && message.images_error.is_none()), |view| {
                view.child(
                    div()
                        .when(message.history_images.is_some(), |body| body.flex().items_center().h(px(112.)))
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("读取图片…")),
                )
            })
            .when_some(message.images_error.as_ref(), |view, error| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(Color::Error.get(cx))
                        .child(format!("{}: {error}", tr("图片预览失败"))),
                )
            })
            .when(!message.images.is_empty(), |view| {
                view.child(
                    h_flex()
                        .min_w_0()
                        .flex_wrap()
                        .gap_2()
                        .when(is_plain_user, |row| row.justify_end())
                        .children(
                            message
                                .images
                                .iter()
                                .cloned()
                                .enumerate()
                                .map(|(index, image)| {
                                    let original = message.image_sources[index].clone();
                                    let name = format!(
                                        "{}-{}.{}",
                                        if english() { "Image" } else { "图片" },
                                        index + 1,
                                        original.format.extension()
                                    );
                                    Button::new(SharedString::from(format!(
                                        "view-image-{i}-{index}"
                                    )))
                                    .ghost()
                                    .rounded_xl()
                                    .overflow_hidden()
                                    .w(px(144.))
                                    .h(px(112.))
                                    .accessibility_label(format!(
                                        "{} {}",
                                        tr("查看图片"),
                                        index + 1
                                    ))
                                    .child(
                                        img(image)
                                            .w(px(144.))
                                            .h(px(112.))
                                            .object_fit(ObjectFit::Contain),
                                    )
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.open_image(original.clone(), name.clone(), window, cx)
                                    }))
                                }),
                        ),
                )
            })
            .when(!message.files.is_empty(), |view| {
                view.child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .flex_wrap()
                        .gap_2()
                        .when(is_plain_user, |row| row.justify_end())
                        .children(
                            message
                                .files
                                .iter()
                                .cloned()
                                .map(|item| self.file_attachment_link(&message.id, item, cx)),
                        ),
                )
            })
            .when_some(message.delivery.as_ref(), |view, delivery| {
                view.children(
                    delivery
                        .artifacts
                        .iter()
                        .enumerate()
                        .map(|(i, path)| self.artifact_link(&message.id, i, path, delivery.dispatch_id.as_deref(), delivery.worker_id.as_deref(), cx)),
                )
            })
            .when(
                message.delivery.as_ref().is_some_and(|d| d.expanded),
                |view| {
                    view.child(
                        div()
                            .id(("delivery-raw", i))
                            .role(Role::Label)
                            .aria_value(
                                message
                                    .delivery
                                    .as_ref()
                                    .map(|d| d.raw.clone())
                                    .unwrap_or_default(),
                            )
                            .min_w_0()
                            .p_3()
                            .rounded_xl()
                            .bg(Color::Inset.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .text_sm()
                            .child(
                                message
                                    .delivery
                                    .as_ref()
                                    .map(|d| d.raw.clone())
                                    .unwrap_or_default(),
                            ),
                    )
                },
            )
            .when(
                !message.text.is_empty()
                    || !message.images.is_empty()
                    || !message.files.is_empty()
                    || message.delivery.is_some()
                    || (message.role == "user"
                        && (message.native_user_item_id.is_some()
                            || message.handoff_item_id.is_some())),
                |view| {
                    view.child(
                        div()
                            .opacity(0.)
                            .group_hover(message_group.clone(), |s| s.opacity(0.95))
                            .child(self.message_actions(i, message, cx)),
                    )
                },
            )
            .into_any_element()
    }

    pub(crate) fn render_conversation_tail(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let running = self
            .session
            .as_ref()
            .is_some_and(|session| self.busy.contains(&session.id));
        let needs_pi_model = self
            .session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Pi))
            && self.model.is_none()
            && !self.loading;
        v_flex()
            .id("conversation-tail")
            .track_focus(&self.conversation_list.tail_focus)
            .w_full()
            .min_w_0()
            .gap_4()
            .when(
                self.messages.is_empty()
                    && self.pending_reports.is_empty()
                    && (self.interaction_count(
                        self.session.as_ref().map(|s| s.id.as_str()).unwrap_or(""),
                    ) == 0
                        || !self.connected),
                |view| {
                    view.child(
                        div()
                            .py_6()
                            .text_sm()
                            .text_color(Color::Muted.get(cx))
                            .child(if self.loading {
                                tr("正在读取…")
                            } else if !self.connected {
                                tr("后台连接已断开，当前草稿仍保留。可在窗口顶部重新启动后台。")
                            } else if running {
                                tr("正在等待 Agent 回复…")
                            } else if needs_pi_model {
                                tr("还没有消息。")
                            } else {
                                tr("会话已就绪。发送第一条消息开始工作。")
                            }),
                    )
                },
            )
            .children(
                self.pending_reports
                    .iter()
                    .filter(|report| {
                        !self
                            .messages
                            .iter()
                            .any(|message| message.id == report.delivery_id)
                    })
                    .map(|report| {
                        let heading = format!(
                            "{} · {}",
                            report.worker_name.as_deref().unwrap_or("Worker"),
                            tr("成员汇报")
                        );
                        v_flex()
                            .id(SharedString::from(format!(
                                "pending-report-{}",
                                report.delivery_id
                            )))
                            .role(Role::Group)
                            .aria_label(heading.clone())
                            .min_w_0()
                            .gap_2()
                            .px_4()
                            .py_3()
                            .rounded_xl()
                            .bg(Color::Card.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(
                                h_flex()
                                    .w_full()
                                    .min_w_0()
                                    .flex_wrap()
                                    .gap_2()
                                    .items_center()
                                    .justify_between()
                                    .pb_1p5()
                                    .border_b_1()
                                    .border_color(Color::Divider.get(cx).opacity(0.75))
                                    .child(
                                        h_flex()
                                            .min_w_0()
                                            .flex_1()
                                            .flex_wrap()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(Color::Secondary.get(cx))
                                                    .child(heading),
                                            )
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "pending-report-state-{}",
                                                        report.delivery_id
                                                    )))
                                                    .role(Role::Label)
                                                    .aria_value(tr(report.status()).to_owned())
                                                    .rounded_full()
                                                    .bg(Color::Inset.get(cx))
                                                    .px_2()
                                                    .py_0p5()
                                                    .text_xs()
                                                    .text_color(Color::Secondary.get(cx))
                                                    .child(tr(report.status())),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .items_center()
                                            .gap_1()
                                            .when_some(
                                                self.dispatch_link(
                                                    &report.delivery_id,
                                                    report.dispatch_id.as_deref(),
                                                    cx,
                                                ),
                                                |row, link| row.child(link),
                                            )
                                            .when_some(
                                                self.history_member_session_link(
                                                    &report.delivery_id,
                                                    report.dispatch_id.as_deref(),
                                                    report.worker_id.as_deref(),
                                                    tr("成员会话"),
                                                    cx,
                                                ),
                                                |row, link| row.child(link),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id(SharedString::from(format!(
                                        "pending-report-body-{}",
                                        report.delivery_id
                                    )))
                                    .role(Role::Label)
                                    .aria_value(report.text.clone())
                                    .text_sm()
                                    .line_height(rems(1.5))
                                    .child(report.text.clone()),
                            )
                            .children(
                                report
                                    .artifacts
                                    .iter()
                                    .enumerate()
                                    .map(|(i, path)| self.artifact_link(&report.delivery_id, i, path, report.dispatch_id.as_deref(), report.worker_id.as_deref(), cx)),
                            )
                    }),
            )
            .child(self.render_interactions(cx))
    }

    pub(crate) fn render_turn_diff_card(
        &self,
        anchor_id: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(summary) = self.conversation_list.active_turn_diffs.get(anchor_id) else {
            return div().into_any_element();
        };
        let file_count = summary.files.len();
        let can_browse = self.can_browse_project_files();
        let first_path = summary
            .files
            .first()
            .map(|f| f.git_path.clone())
            .unwrap_or_default();
        let patch_changes = summary.tool_changes.clone();
        let fallback_changes = summary.tool_changes.clone();
        let extra_count = file_count.saturating_sub(4);
        v_flex()
            .id(SharedString::from(format!("turn-diff-{anchor_id}")))
            .min_w_0()
            .w_full()
            .gap_1p5()
            .px_3p5()
            .py_2p5()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        h_flex()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(Glyph::FileDiff)
                                    .flex_shrink_0()
                                    .size(px(13.))
                                    .text_color(Color::Muted.get(cx)),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("本轮改动")),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(Color::Muted.get(cx))
                                    .child(if english() {
                                        format!(
                                            "· {file_count} {}",
                                            if file_count == 1 { "file" } else { "files" }
                                        )
                                    } else {
                                        format!("· {file_count} 个文件")
                                    }),
                            )
                            .child({
                                let diff_palette = crate::diffs_settings::current_palette(cx);
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_1()
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .font_features(crate::palette::tabular_nums())
                                    .text_xs()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(diff_palette.added)
                                            .child(format!("+{}", summary.total_additions)),
                                    )
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(diff_palette.removed)
                                            .child(format!("-{}", summary.total_deletions)),
                                    )
                            }),
                    )
                    .child(
                        h_flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .when(can_browse && !first_path.is_empty(), |row| {
                                let open_path = first_path.clone();
                                let open_fallback = fallback_changes.clone();
                                row.child(
                                    Button::new(SharedString::from(format!(
                                        "turn-diff-git-{anchor_id}"
                                    )))
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_md()
                                    .icon(Glyph::FileDiff)
                                    .label(tr("在 Git 中查看"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if this.can_browse_project_files()
                                            && !window.has_active_dialog(cx)
                                        {
                                            this.open_git_file_diff(
                                                open_path.clone(),
                                                true,
                                                window,
                                                cx,
                                            );
                                        } else {
                                            this.open_turn_diff(open_fallback.clone(), window, cx);
                                        }
                                    })),
                                )
                            })
                            .child(
                                Button::new(SharedString::from(format!(
                                    "turn-diff-patch-{anchor_id}"
                                )))
                                .custom(crate::palette::subtle_icon_variant(cx))
                                .compact()
                                .xsmall()
                                .h(px(22.))
                                .px_2()
                                .rounded_md()
                                .label(tr("查看本轮补丁"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.open_turn_diff(patch_changes.clone(), window, cx);
                                })),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .min_w_0()
                    .gap_0p5()
                    .children(summary.files.iter().take(4).enumerate().map(|(idx, file)| {
                        let git_path = file.git_path.clone();
                        let file_fallback = summary.tool_changes.clone();
                        Button::new(SharedString::from(format!(
                            "turn-diff-file-{anchor_id}-{idx}"
                        )))
                        .custom(crate::palette::quiet_icon_variant(cx))
                        .compact()
                        .h(px(24.))
                        .px_1p5()
                        .rounded_md()
                        .w_full()
                        .min_w_0()
                        .child(
                            h_flex()
                                .w_full()
                                .min_w_0()
                                .items_center()
                                .gap_1p5()
                                .child(
                                    Icon::new(Glyph::FileText)
                                        .flex_shrink_0()
                                        .size(px(12.))
                                        .text_color(Color::Muted.get(cx)),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_left()
                                        .truncate()
                                        .text_xs()
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .text_color(Color::Secondary.get(cx))
                                        .child(file.display_path.clone()),
                                )
                                .child({
                                    let diff_palette = crate::diffs_settings::current_palette(cx);
                                    h_flex()
                                        .flex_shrink_0()
                                        .items_center()
                                        .gap_1()
                                        .font_family(Theme::global(cx).mono_font_family.clone())
                                        .font_features(crate::palette::tabular_nums())
                                        .text_size(px(11.))
                                        .child(
                                            div()
                                                .text_color(diff_palette.added)
                                                .child(format!("+{}", file.additions)),
                                        )
                                        .child(
                                            div()
                                                .text_color(diff_palette.removed)
                                                .child(format!("-{}", file.deletions)),
                                        )
                                }),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if this.can_browse_project_files() && !window.has_active_dialog(cx) {
                                this.open_git_file_diff(git_path.clone(), true, window, cx);
                            } else {
                                this.open_turn_diff(file_fallback.clone(), window, cx);
                            }
                        }))
                    }))
                    .when(extra_count > 0, |col| {
                        col.child(
                            div()
                                .px_1p5()
                                .pt_0p5()
                                .text_size(px(11.))
                                .text_color(Color::Muted.get(cx))
                                .child(if english() {
                                    format!("+{extra_count} more")
                                } else {
                                    format!("还有 {extra_count} 个文件")
                                }),
                        )
                    }),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::is_collapsible_user_message;

    #[test]
    fn collapsible_user_message_thresholds_handle_chars_and_lines() {
        assert!(!is_collapsible_user_message(""));
        assert!(!is_collapsible_user_message("short prompt"));
        assert!(!is_collapsible_user_message("a\nb\nc\nd\ne\nf\ng\nh"));
        assert!(is_collapsible_user_message("a\nb\nc\nd\ne\nf\ng\nh\ni"));
        assert!(!is_collapsible_user_message(&"x".repeat(400)));
        assert!(is_collapsible_user_message(&"x".repeat(601)));
        assert!(is_collapsible_user_message(&"中".repeat(320)));
        assert!(!is_collapsible_user_message(&"i".repeat(601)));
        assert!(!is_collapsible_user_message(&"l".repeat(601)));
        assert!(!is_collapsible_user_message(&".".repeat(601)));
        assert!(is_collapsible_user_message(&"i".repeat(1200)));
    }
}
