use super::*;
use crate::diagnostics;

fn value(text: Option<&str>) -> String {
    text.unwrap_or(tr("无法获取")).to_owned()
}

fn info_row(label: &'static str, text: String, first: bool, cx: &App) -> Div {
    super::settings_row(!first, cx).child(
        h_flex()
            .w_full()
            .min_w_0()
            .gap_2()
            .flex_wrap()
            .child(
                div()
                    .min_w(px(150.))
                    .text_sm()
                    .text_color(Color::Secondary.get(cx))
                    .child(tr(label)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(150.))
                    .text_sm()
                    .text_color(Color::Text.get(cx))
                    .child(text),
            ),
    )
}

impl HiveApp {
    fn diagnostics_connection(&self) -> &'static str {
        if self.restarting {
            "restarting"
        } else if self.connected {
            "connected"
        } else if self.loading {
            "starting"
        } else {
            "disconnected"
        }
    }

    pub(super) fn render_about_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let connection = self.diagnostics_connection();
        let connected = self
            .connected
            .then_some(self.backend_info.as_ref())
            .flatten();
        let revision = diagnostics::revision().map(|revision| {
            let short = &revision[..12.min(revision.len())];
            if diagnostics::dirty() == Some(true) {
                format!("{short} · {}", tr("含已跟踪文件修改"))
            } else {
                short.to_owned()
            }
        });
        let backend_value = |get: fn(&diagnostics::BackendInfo) -> &Option<String>| {
            value(connected.and_then(|info| get(info).as_deref()))
        };
        let status = match connection {
            "connected" => tr("后台已连接"),
            "restarting" => tr("正在重新启动后台"),
            "starting" => tr("正在启动后台"),
            _ => tr("后台未连接"),
        };
        v_flex().flex_shrink_0().w_full().min_w_0().gap_4()
            .child(h_flex().w_full().min_w_0().items_center().justify_between().gap_3().flex_wrap()
                .child(v_flex().min_w_0().gap_1()
                    .child(div().text_size(px(22.)).font_weight(FontWeight::SEMIBOLD).text_color(Color::Text.get(cx)).child("Hive Desktop"))
                    .child(div().text_sm().text_color(Color::Secondary.get(cx)).child(tr("为你的 Agent 提供统一的协作工作台。"))))
                .child(crate::focus_scroll::Reveal::new("copy-diagnostics-reveal", &self.about_scroll,
                    Button::new("copy-desktop-diagnostics").outline().small().icon(gpui_kit::assets::IconName::Copy)
                        .label(tr("复制诊断信息"))
                        .accessibility_label(tr("复制桌面与后台版本诊断"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let backend = this.connected.then_some(this.backend_info.as_ref()).flatten();
                            cx.write_to_clipboard(ClipboardItem::new_string(diagnostics::copy_text(this.diagnostics_connection(), backend)));
                            this.diagnostics_copied = true;
                            cx.notify();
                        })))))
            .when(self.diagnostics_copied, |view| view.child(super::settings_notice_box("desktop-diagnostics-copied", tr("诊断信息已复制到本机剪贴板。"), false, cx)))
            .child(super::settings_card(cx)
                .child(info_row("桌面版本", env!("CARGO_PKG_VERSION").to_owned(), true, cx))
                .child(info_row("构建标识", value(revision.as_deref()), false, cx))
                .child(info_row("构建类型", match env!("HIVE_DESKTOP_BUILD_PROFILE") {
                    "release" => tr("优化版"), "debug" => tr("开发版"), _ => tr("无法获取"),
                }.to_owned(), false, cx))
                .child(info_row("桌面平台", format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH), false, cx)))
            .child(v_flex().gap_2()
                .child(h_flex().gap_2().items_center()
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(Color::Text.get(cx)).child(tr("运行中的后台")))
                    .child(super::status_pill(if self.connected { Color::Success.get(cx) } else { Color::Muted.get(cx) }, status, cx)))
                .child(super::settings_card(cx)
                    .child(info_row("桌面运行时", backend_value(|info| &info.version), true, cx))
                    .child(info_row("Node.js", backend_value(|info| &info.node_version), false, cx))
                    .child(info_row("Hive 包", backend_value(|info| &info.hive_version), false, cx))
                    .child(info_row("后台平台", connected.map(|info| format!("{} · {}", value(info.platform.as_deref()), value(info.arch.as_deref()))).unwrap_or_else(|| tr("无法获取").to_owned()), false, cx))))
            .child(div().text_xs().text_color(Color::Muted.get(cx))
                .child(tr("诊断信息包含版本、构建和连接状态，可用于反馈问题。版本来自当前程序与实际运行的后台。")))
    }
}
