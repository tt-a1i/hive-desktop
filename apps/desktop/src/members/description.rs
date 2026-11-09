use crate::language::{english, tr};
use crate::{app::HiveApp, palette::Color, protocol::Member};
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
    pub(crate) fn render_member_description(
        &self,
        member: &Member,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = member.id.clone();
        let text = member.description.clone();
        let scroll = self.member_details_open.get(&id);
        let expanded = scroll.is_some();
        let empty = text.trim().is_empty();
        let summary = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or(tr("未设置职责说明"))
            .to_string();
        v_flex()
            .min_w_0()
            .gap_2()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!(
                            "toggle-member-description-{id}"
                        )))
                        .ghost()
                        .compact()
                        .icon(if expanded {
                            Glyph::ChevronDown
                        } else {
                            Glyph::ChevronRight
                        })
                        .label(tr("职责说明"))
                        .toggled(expanded)
                        .accessibility_label(if english() { format!("{} {}’s responsibilities", if expanded { tr("收起") } else { tr("展开") }, member.name) } else { format!("{} {} 的职责说明", if expanded { tr("收起") } else { tr("展开") }, member.name) })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.member_details_open.remove(&id).is_none() {
                                this.member_details_open
                                    .insert(id.clone(), ScrollHandle::new());
                            }
                            cx.notify();
                        })),
                    )
                    .when(!expanded, |row| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(Color::Secondary.get(cx))
                                .child(summary),
                        )
                    })
                    .when(expanded && !empty, |row| {
                        row.child(
                            Button::new(SharedString::from(format!(
                                "copy-member-description-{}",
                                member.id
                            )))
                            .ghost()
                            .compact()
                            .icon(Glyph::Copy)
                            .label(tr("复制"))
                            .accessibility_label(if english() { format!("Copy {}’s full responsibilities", member.name) } else { format!("复制 {} 的完整职责说明", member.name) })
                            .on_click({
                                let text = text.clone();
                                cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()))
                                })
                            }),
                        )
                    }),
            )
            .when_some(scroll, |view, scroll| {
                view.child(
                    crate::text_reading::region(
                        format!("member-description-body-{}", member.id),
                        if english() {
                            format!(
                                "{}’s responsibilities; scroll with arrow or page keys",
                                member.name
                            )
                        } else {
                            format!("{} 的职责说明，可用方向键或翻页键滚动", member.name)
                        },
                        scroll,
                        px(220.),
                        cx,
                    )
                    .w_full()
                    .p_3()
                    .rounded_lg()
                    .bg(Color::Inset.get(cx))
                    .text_sm()
                    .child(gpui_kit::base::SelectableText::new(
                        SharedString::from(format!("member-description-text-{}", member.id)),
                        if empty {
                            tr("未设置职责说明").to_owned()
                        } else {
                            text
                        },
                    )),
                )
            })
            .into_any_element()
    }
}
