use super::PAGE;
use crate::{
    app::HiveApp,
    focus_scroll::{Reveal, Viewport},
    language::{english, tr},
    palette::Color,
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(crate) fn render_collaboration(&self, cx: &mut Context<Self>) -> AnyElement {
        let scroll = if self.collaboration.selected.is_some() {
            &self.collaboration.detail_scroll
        } else {
            &self.collaboration_scroll
        };
        v_flex().flex_1().min_w_0().h_full().gap_3().p_5()
                .child(h_flex().gap_2().items_center()
                    .when(self.collaboration.selected.is_some(), |row| row.child(Button::new("back-dispatch-list").track_focus(&self.collaboration.navigation_focus).ghost().icon(Glyph::ArrowLeft).label(if self.collaboration.workflow_origin { tr("返回工作流") } else if self.collaboration.chat_origin.is_some() { tr("返回来源会话") } else { tr("返回派单列表") }).on_click(cx.listener(|this, _, window, cx| {
                        if this.collaboration.workflow_origin { this.close_collaboration(); this.show_workflows(window, cx); return; }
                        let c = &mut this.collaboration;
                        if let Some(id) = c.chat_origin.take() {
                            c.selected = None;
                            this.close_collaboration();
                            if this.session.as_ref().is_none_or(|s| s.id != id) { this.show_session(id, cx); }
                            cx.notify();
                            return;
                        }
                        let changed_filter = c.selected.take().is_some_and(|d| c.filter != "all" && c.filter != d.state);
                        if changed_filter { this.load_dispatches(cx); }
                        this.collaboration.navigation_focus.focus(window, cx);
                        cx.notify();
                    }))))
                    .child(div().flex_1().text_xl().font_weight(FontWeight::SEMIBOLD).child(if self.collaboration.selected.is_some() { tr("派单详情") } else { tr("协作概览") }))
                    .when(self.collaboration.selected.is_some(), |row| row.child(Button::new("refresh-dispatch").ghost().label(tr("刷新详情")).disabled(!self.connected || self.collaboration.message_loading).on_click(cx.listener(|this, _, _, cx| this.refresh_dispatch(cx)))))
                    .when(self.collaboration.selected.is_none(), |row| row.child(Button::new("refresh-collaboration").track_focus(&self.collaboration.navigation_focus).label(tr("刷新")).disabled(!self.connected || self.tasks_saving()).on_click(cx.listener(|this, _, _, cx| this.reload_collaboration(cx))))))
                .when(self.error.is_some(), |view| view.child(self.render_error_banner(cx)))
                .child(Viewport::new("collaboration-viewport", scroll, div().id("collaboration-content").flex_1().min_h_0().min_w_0().overflow_y_scroll().track_scroll(scroll).p_1()
                  .child(v_flex().w_full().min_w_0().flex_shrink_0().gap_3()
                    .when(self.collaboration.selected.is_none(), |list| list
                    .child(self.render_tasks(cx))
                    .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("派单")))
                    .child(Reveal::new("dispatch-filters-reveal", scroll, h_flex().flex_wrap().gap_1().children([("all", tr("全部")), ("queued", tr("排队中")), ("submitted", tr("已提交")), ("reported", tr("已汇报")), ("cancelled", tr("已取消"))].into_iter().enumerate().map(|(i, (filter, label))| Button::new(("dispatch-filter", i)).label(label).selected(self.collaboration.filter == filter).on_click(cx.listener(move |this, _, _, cx| { this.collaboration.filter = filter; this.collaboration.offset = 0; this.load_dispatches(cx); }))))))
                    .when(self.collaboration.list_loading, |view| view.child(div().text_sm().child(tr("正在读取派单…"))))
                    .when_some(self.collaboration.list_error.as_ref().map(|(key,error)| format!("{}: {error}", tr(key))), |view, error| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
                    .when(!self.collaboration.list_loading && self.collaboration.list_error.is_none() && self.collaboration.dispatches.is_empty(), |view| view.child(div().text_sm().child(tr("此页没有派单"))))
                    .children(self.collaboration.dispatches.iter().take(PAGE).enumerate().map(|(i, dispatch)| self.render_dispatch_card(i, dispatch, cx)))
                    .child(Reveal::new("dispatch-pagination-reveal", scroll, h_flex().gap_2()
                        .child(Button::new("dispatch-prev").label(tr("上一页")).disabled(!self.connected || self.collaboration.offset == 0 || self.collaboration.list_loading).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset = this.collaboration.offset.saturating_sub(PAGE); this.load_dispatches(cx); })))
                        .child(div().text_sm().child(if english() { format!("Page {}", self.collaboration.offset / PAGE + 1) } else { format!("第 {} 页", self.collaboration.offset / PAGE + 1) }))
                        .child(Button::new("dispatch-next").label(tr("下一页")).disabled(!self.connected || self.collaboration.dispatches.len() <= PAGE || self.collaboration.list_loading || self.collaboration.offset >= 100_000).on_click(cx.listener(|this, _, _, cx| { this.collaboration.offset += PAGE; this.load_dispatches(cx); })))))
                    )
                    .when_some(self.collaboration.selected.as_ref(), |view, item| view.child(v_flex().gap_2().p_3().rounded_lg().bg(Color::Card.get(cx))
                        .when_some(self.collaboration.detail_error.as_ref().map(|(key,error)| format!("{}: {error}", tr(key))), |view, error| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)))
                        .child(Reveal::new("dispatch-participants-reveal", scroll, h_flex().items_center().flex_wrap().gap_2()
                            .child(div().id("dispatch-state").role(Role::Label).aria_label(crate::collaboration::dispatch_state(&item.state).to_owned()).rounded_full().bg(Color::Inset.get(cx)).px_2().py_1().text_xs().child(crate::collaboration::dispatch_state(&item.state).to_owned()))
                            .child(div().id("dispatch-participants").role(Role::Label).aria_label(format!("{} → {}", item.from_agent_id.as_ref().map(|id| self.agent_name(id)).unwrap_or_else(|| tr("未记录发送者").into()), self.agent_name(&item.to_agent_id))).text_sm().text_color(Color::Secondary.get(cx)).child(format!("{} → {}", item.from_agent_id.as_ref().map(|id| self.agent_name(id)).unwrap_or_else(|| tr("未记录发送者").into()), self.agent_name(&item.to_agent_id))))
                            .when_some(self.member_session_link(&item.id, Some(&item.to_agent_id), tr("负责人的会话"), cx), |row, link| row.child(link))
                            .child(div().flex_1())
                            .child(Button::new("copy-dispatch-id").ghost().compact().icon(Glyph::Copy).accessibility_label(tr("复制派单编号")).tooltip(format!("{} · {}", tr("复制派单编号"), item.id)).on_click({
                                let id = item.id.clone(); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(id.clone())))
                            }))))
                        .child(Reveal::new("dispatch-task-actions-reveal", scroll, h_flex().items_center().gap_2()
                            .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("任务")))
                            .child(Button::new("copy-dispatch-task").ghost().compact().icon(Glyph::Copy).tooltip(tr("复制完整任务")).accessibility_label(tr("复制派单内容")).on_click({
                                let text = item.text.clone(); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
                            }))))
                        .child(div().id("dispatch-task-text").role(Role::Label).aria_label(format!("{}: {}", tr("任务"), item.text)).text_sm().child(item.text.clone()))
                        .when_some(item.report_text.clone(), |view, text| view.child(Reveal::new("dispatch-outcome-actions-reveal", scroll, h_flex().items_center().gap_2()
                            .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(if item.state == "cancelled" { tr("取消原因") } else { tr("汇报原文") }))
                            .child(Button::new("copy-dispatch-outcome").ghost().compact().icon(Glyph::Copy).tooltip(tr("复制完整原文")).accessibility_label(if item.state == "cancelled" { tr("复制取消原因") } else { tr("复制汇报原文") }).on_click({
                                let text = text.clone(); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
                            })))).child(div().id("dispatch-outcome-text").role(Role::Label).aria_label(format!("{}: {text}", if item.state == "cancelled" { tr("取消原因") } else { tr("汇报原文") })).text_sm().child(text)))
                        .when(!item.artifacts.is_empty(), |view| view.child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("产物"))).children(item.artifacts.iter().enumerate().map(|(i, path)| Reveal::new(format!("dispatch-{}-artifact-{i}-reveal", item.id), scroll, h_flex().child(self.artifact_link(&item.id, i, path, cx))))))
                        .when(self.collaboration.responsibilities.len() > 1, |view| view.child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("成员分工")))
                            .children(self.collaboration.responsibilities.iter().map(|task| v_flex().gap_1().py_1()
                                .child(Reveal::new(format!("responsibility-{}-actions-reveal", task.id), scroll, h_flex().items_center().gap_2().child(div().text_xs().text_color(Color::Secondary.get(cx)).child(format!("{} · {}", task.owner_name, crate::collaboration::dispatch_state(&task.state))))
                                    .when(task.id != item.id, |row| row.when_some(self.member_session_link(&task.id, Some(&task.to_agent_id), tr("成员会话"), cx), |row, link| row.child(link)))))
                                .child(div().id(SharedString::from(format!("responsibility-{}", task.id))).role(Role::Label).aria_label(format!("{} · {}: {}", task.owner_name, crate::collaboration::dispatch_state(&task.state), task.text)).text_sm().child(task.text.clone())))))
                        .child(div().pt_3().text_sm().font_weight(FontWeight::SEMIBOLD).child(tr("协作消息")))
                        .when(self.collaboration.message_loading, |view| view.child(div().text_sm().child(tr("正在读取消息…"))))
                        .when_some(self.collaboration.message_error.as_ref().map(|(key,error)| format!("{}: {error}", tr(key))), |view, error| view.child(div().text_sm().text_color(Color::Error.get(cx)).child(error)).child(Reveal::new("dispatch-messages-retry-reveal", scroll, Button::new("retry-dispatch-messages").label(tr("重试读取消息")).disabled(!self.connected || self.collaboration.message_loading || self.collaboration.message_refreshing).on_click(cx.listener(|this, _, _, cx| { if this.collaboration.message_refresh_failed {this.refresh_dispatch(cx);} else {let after = this.collaboration.requested_after.clone(); this.load_dispatch_messages(after, cx);} })))))
                        .when(!self.collaboration.message_loading && self.collaboration.message_error.is_none() && self.collaboration.messages.is_empty(), |view| view.child(div().id("dispatch-empty-messages").role(Role::Label).aria_label(tr("没有消息")).text_sm().child(tr("没有消息"))))
                        .children(self.collaboration.messages.iter().map(|message| v_flex().id(SharedString::from(format!("collaboration-message-{}", message.id))).role(Role::Group).aria_label(format!("{} → {} · {} · {}", self.agent_name(&message.from_agent_id), self.agent_name(&message.recipient_agent_id), crate::collaboration::message_kind(&message.kind), crate::collaboration::delivery_state(&message.delivery_state))).gap_1().p_3().rounded_lg().bg(Color::Inset.get(cx))
                            .child(Reveal::new(format!("collaboration-message-{}-actions-reveal", message.id), scroll, h_flex().items_center().gap_2()
                                .child(div().flex_1().text_xs().child(format!("{} → {} · {} · {}", self.agent_name(&message.from_agent_id), self.agent_name(&message.recipient_agent_id), crate::collaboration::message_kind(&message.kind), crate::collaboration::delivery_state(&message.delivery_state))))
                                .child(Button::new(SharedString::from(format!("copy-collaboration-message-{}", message.id))).ghost().compact().icon(Glyph::Copy).tooltip(tr("复制完整原文")).accessibility_label(tr("复制协作消息")).on_click({
                                    let text = message.text.clone(); cx.listener(move |_, _, _, cx| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
                                }))))
                            .when(self.collaboration.responsibilities.len() > 1, |row| row.when_some(self.collaboration.responsibilities.iter().find(|task| task.id == message.dispatch_id), |row, task|
                                row.child(div().text_xs().truncate().text_color(Color::Muted.get(cx)).child(format!("{} · {}", tr("派单"), task.text)))))
                            .when_some(message.reply_to.as_ref(), |row, id| {
                                let original = self.collaboration.messages.iter().find(|m| &m.id == id);
                                let quote = original.map(|m| if english() { format!("Reply to {}: {}", self.agent_name(&m.from_agent_id), m.text) } else { format!("回复 {}：{}", self.agent_name(&m.from_agent_id), m.text) }).unwrap_or_else(|| if english() { format!("Reply to message {id}") } else { format!("回复消息 {id}") });
                                row.child(div().id(SharedString::from(format!("collaboration-quote-{}", message.id))).role(Role::Label).aria_label(quote.clone()).border_l_2().border_color(Color::Divider.get(cx)).pl_2().text_sm().text_color(Color::Secondary.get(cx)).child(quote))
                            })
                            .child(div().id(SharedString::from(format!("collaboration-body-{}", message.id))).role(Role::Label).aria_label(message.text.clone()).text_sm().child(message.text.clone()))
                            .when_some(message.delivery_error.clone(), |view, error| view.child(div().id(SharedString::from(format!("collaboration-error-{}", message.id))).role(Role::Label).aria_label(format!("{}: {error}", tr("投递错误"))).text_xs().text_color(Color::Error.get(cx)).child(format!("{}: {error}", tr("投递错误")))))))
                        .when_some(self.collaboration.next_message.clone(), |view, cursor| view.child(Reveal::new("dispatch-messages-more-reveal", scroll, Button::new("more-dispatch-messages").label(tr("加载更多消息")).disabled(!self.connected || self.collaboration.message_loading).on_click(cx.listener(move |this, _, _, cx| this.load_dispatch_messages(Some(cursor.clone()), cx))))))))
                ))).into_any_element()
    }
}
