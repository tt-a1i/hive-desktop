use crate::{app::HiveApp, focus_scroll::Reveal, language::tr, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{button::Button, *},
    prelude::FluentBuilder,
    *,
};

impl HiveApp {
    pub(super) fn render_dispatch_empty_state(&self, cx: &Context<Self>) -> impl IntoElement {
        let first_page = self.collaboration.filter == "all" && self.collaboration.offset == 0;
        let orchestrator = self.workspace.as_ref().and_then(|workspace| {
            let agent = format!("{workspace}:orchestrator");
            self.sessions
                .iter()
                .find(|session| !session.archived && session.agent_id.as_ref() == Some(&agent))
                .map(|session| session.id.clone())
        });
        Reveal::new(
            "collaboration-empty-reveal",
            &self.collaboration_scroll,
            v_flex()
                .id("collaboration-empty")
                .items_center()
                .gap_2()
                .p_3()
                .rounded_xl()
                .bg(Color::Card.get(cx))
                .border_1()
                .border_color(Color::Divider.get(cx))
                .child(h_flex().w_full().gap_2().items_center()
                    .child(Icon::new(if first_page { Glyph::Network } else { Glyph::Search })
                        .size(px(18.)).text_color(Color::Muted.get(cx)))
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD)
                        .text_color(Color::Text.get(cx))
                        .child(if first_page { tr("开始团队协作") } else { tr("此页没有派单") })))
                .when(first_page, |card| card
                    .child(v_flex().w_full().gap_2()
                        .child(Button::new("collaboration-manage-members")
                            .outline().small().label(tr("管理成员"))
                            .disabled(!self.connected || !self.workspace_writes_allowed())
                            .on_click(cx.listener(|this, _, window, cx| this.show_team(window, cx))))
                        .when_some(orchestrator, |actions, id| actions.child(
                            Button::new("collaboration-open-orchestrator")
                                .outline().small().label(tr("继续主控会话"))
                                .disabled(!self.connected || !self.workspace_writes_allowed())
                                .on_click(cx.listener(move |this, _, _, cx| this.open_session(id.clone(), cx)))))
                        .child(self.new_chat_menu("collaboration-new-orchestrator", tr("新建主控会话"), None, cx)))
                    .child(div().text_xs().text_color(Color::Secondary.get(cx))
                        .child(tr("添加成员后，向主控描述任务。主控负责派单，成员显式汇报后，结果会回到来源会话。"))))
                .when(!first_page, |card| card.child(
                    Button::new("collaboration-reset-filters")
                        .outline().small().label(tr("查看全部派单"))
                        .disabled(!self.connected)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.collaboration.filter = "all";
                            this.collaboration.offset = 0;
                            this.load_dispatches(cx);
                        })))),
        )
    }
}
