use super::*;
use crate::{diagnostics, focus_scroll::Reveal, maintenance};
use gpui_kit::{
    assets::IconName as Glyph,
    component::scroll::ScrollableElement,
};

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

fn open_licenses_dialog(window: &mut Window, cx: &mut App) {
    const PACKAGES: &[(&str, &str, &str)] = &[
        ("GPUI & gpui-component", "Apache-2.0", "Zed Industries / Longbridge"),
        ("Tokio", "MIT", "Tokio Contributors"),
        ("Serde & serde_json", "MIT OR Apache-2.0", "Erick Tryzelaar & David Tolnay"),
        ("Rusqlite & SQLite", "MIT / Public Domain", "Rusqlite Contributors"),
        ("alacritty_terminal", "Apache-2.0", "Alacritty Authors"),
        ("portable-pty", "MIT", "Wez Furlong"),
        ("smol & futures", "Apache-2.0 OR MIT", "Stjepan Glavina & Rust Contributors"),
        ("Node.js", "MIT", "OpenJS Foundation"),
        ("TypeScript", "Apache-2.0", "Microsoft Corporation"),
        ("Pi Coding Agent", "MIT", "Pi Contributors"),
        ("Lucide Icons", "ISC", "Lucide Contributors"),
    ];
    let scroll = ScrollHandle::new();
    window.open_dialog(cx, move |dialog, window, cx| {
        let max_h = (window.viewport_size().height - px(240.)).max(px(220.));
        dialog
            .title(tr("开源软件许可"))
            .w(px(560.))
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("Hive Desktop 基于以下核心开源组件构建，各组件版权与许可协议均归其原作者所有。")),
                    )
                    .child(
                        v_flex()
                            .id("open-source-licenses-list")
                            .max_h(max_h)
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .vertical_scrollbar(&scroll)
                            .rounded_lg()
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .bg(Color::Inset.get(cx))
                            .children(PACKAGES.iter().enumerate().map(|(ix, (name, license, author))| {
                                h_flex()
                                    .when(ix > 0, |row| {
                                        row.border_t_1().border_color(Color::Divider.get(cx))
                                    })
                                    .px_3()
                                    .py_2()
                                    .justify_between()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        v_flex()
                                            .min_w_0()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(*name),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Color::Muted.get(cx))
                                                    .child(*author),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded_sm()
                                            .bg(Color::Card.get(cx))
                                            .border_1()
                                            .border_color(Color::Divider.get(cx))
                                            .text_xs()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(*license),
                                    )
                            })),
                    ),
            )
    });
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

    pub(crate) fn scan_disposable_cache(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.data_dir.clone() else {
            self.maintenance_state.notice =
                Some((true, tr("后台尚未连接，无法定位本地数据目录。").to_owned()));
            cx.notify();
            return;
        };
        if self.maintenance_state.scanning || self.maintenance_state.cleaning {
            return;
        }
        self.maintenance_state.scanning = true;
        self.maintenance_state.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || maintenance::scan_disposable(&root))
                .await
                .map_err(|error| error.to_string());
            let _ = this.update(cx, |this, cx| {
                this.maintenance_state.scanning = false;
                match result {
                    Ok(report) => {
                        let msg = if report.disposable_files == 0 {
                            tr("未发现可清理的临时文件或日志。").to_owned()
                        } else if english() {
                            format!(
                                "Found {} disposable file(s) ({}).",
                                report.disposable_files,
                                maintenance::format_bytes(report.disposable_bytes)
                            )
                        } else {
                            format!(
                                "已发现 {} 个可清理文件，共 {}。",
                                report.disposable_files,
                                maintenance::format_bytes(report.disposable_bytes)
                            )
                        };
                        this.maintenance_state.report = Some(report);
                        this.maintenance_state.notice = Some((false, msg));
                    }
                    Err(error) => {
                        this.maintenance_state.notice = Some((
                            true,
                            if english() {
                                format!("Failed to scan disposable files: {error}")
                            } else {
                                format!("扫描临时文件失败：{error}")
                            },
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn clean_disposable_cache_confirmed(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.data_dir.clone() else {
            return;
        };
        if self.maintenance_state.cleaning || self.maintenance_state.scanning {
            return;
        }
        self.maintenance_state.cleaning = true;
        self.maintenance_state.notice = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = smol::unblock(move || {
                let outcome = maintenance::clean_disposable(&root)?;
                let refreshed = maintenance::scan_disposable(&root)?;
                Ok::<_, std::io::Error>((outcome, refreshed))
            })
            .await
            .map_err(|error| error.to_string());
            let _ = this.update(cx, |this, cx| {
                this.maintenance_state.cleaning = false;
                match result {
                    Ok((outcome, refreshed)) => {
                        this.maintenance_state.report = Some(refreshed);
                        let msg = if outcome.failed_files == 0 {
                            if english() {
                                format!(
                                    "Removed {} file(s) and freed {}.",
                                    outcome.removed_files,
                                    maintenance::format_bytes(outcome.freed_bytes)
                                )
                            } else {
                                format!(
                                    "已清理 {} 个临时文件，释放 {}。",
                                    outcome.removed_files,
                                    maintenance::format_bytes(outcome.freed_bytes)
                                )
                            }
                        } else if english() {
                            format!(
                                "Removed {} file(s) ({}), {} could not be removed.",
                                outcome.removed_files,
                                maintenance::format_bytes(outcome.freed_bytes),
                                outcome.failed_files
                            )
                        } else {
                            format!(
                                "已清理 {} 个文件（释放 {}），另有 {} 个文件无法移除。",
                                outcome.removed_files,
                                maintenance::format_bytes(outcome.freed_bytes),
                                outcome.failed_files
                            )
                        };
                        this.maintenance_state.notice = Some((outcome.failed_files > 0, msg));
                    }
                    Err(error) => {
                        this.maintenance_state.notice = Some((
                            true,
                            if english() {
                                format!("Failed to clean disposable files: {error}")
                            } else {
                                format!("清理临时文件失败：{error}")
                            },
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
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

        let highlight_gen = |anchor: &'static str| {
            self.settings_highlight_anchor
                .filter(|(active, _)| *active == anchor)
                .map(|(_, generation)| generation)
        };
        let hl_maint = highlight_gen("about-maintenance");
        let hl_guide = highlight_gen("about-guide-licenses");

        let data_dir_text = self
            .data_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| tr("尚未连接后台").to_owned());
        let maint = &self.maintenance_state;
        let has_disposable = maint
            .report
            .as_ref()
            .is_some_and(|r| r.disposable_files > 0);
        let maint_status_text = if maint.scanning {
            tr("正在扫描临时文件与日志…").to_owned()
        } else if maint.cleaning {
            tr("正在清理临时文件与日志…").to_owned()
        } else if let Some(report) = &maint.report {
            if report.disposable_files == 0 {
                tr("上次扫描结果：暂无可清理项（0 B）。").to_owned()
            } else if english() {
                format!(
                    "Last scan: {} file(s) ({}).",
                    report.disposable_files,
                    maintenance::format_bytes(report.disposable_bytes)
                )
            } else {
                format!(
                    "上次扫描结果：{} 个可清理文件，共 {}。",
                    report.disposable_files,
                    maintenance::format_bytes(report.disposable_bytes)
                )
            }
        } else {
            tr("仅扫描 logs / tmp / cache 等临时目录，绝不触碰数据库、会话记录、草稿、工作树或配置文件。").to_owned()
        };

        v_flex()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .gap_4()
            .child(
                h_flex()
                    .w_full()
                    .min_w_0()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .flex_wrap()
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(22.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(Color::Text.get(cx))
                                    .child("Hive Desktop"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(tr("为你的 Agent 提供统一的协作工作台。")),
                            ),
                    )
                    .child(Reveal::new(
                        "copy-diagnostics-reveal",
                        &self.about_scroll,
                        Button::new("copy-desktop-diagnostics")
                            .outline()
                            .small()
                            .icon(gpui_kit::assets::IconName::Copy)
                            .label(tr("复制诊断信息"))
                            .accessibility_label(tr("复制桌面与后台版本诊断"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let backend =
                                    this.connected.then_some(this.backend_info.as_ref()).flatten();
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    diagnostics::copy_text(this.diagnostics_connection(), backend),
                                ));
                                this.diagnostics_copied = true;
                                cx.notify();
                            })),
                    )),
            )
            .when(self.diagnostics_copied, |view| {
                view.child(super::settings_notice_box(
                    "desktop-diagnostics-copied",
                    tr("诊断信息已复制到本机剪贴板。"),
                    false,
                    cx,
                ))
            })
            .child(
                super::settings_card(cx)
                    .child(info_row(
                        "桌面版本",
                        env!("CARGO_PKG_VERSION").to_owned(),
                        true,
                        cx,
                    ))
                    .child(info_row("构建标识", value(revision.as_deref()), false, cx))
                    .child(info_row(
                        "构建类型",
                        match env!("HIVE_DESKTOP_BUILD_PROFILE") {
                            "release" => tr("优化版"),
                            "debug" => tr("开发版"),
                            _ => tr("无法获取"),
                        }
                        .to_owned(),
                        false,
                        cx,
                    ))
                    .child(info_row(
                        "桌面平台",
                        format!("{} · {}", std::env::consts::OS, std::env::consts::ARCH),
                        false,
                        cx,
                    )),
            )
            .child(self.render_desktop_updates(cx))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Color::Text.get(cx))
                                    .child(tr("运行中的后台")),
                            )
                            .child(super::status_pill(
                                if self.connected {
                                    Color::Success.get(cx)
                                } else {
                                    Color::Muted.get(cx)
                                },
                                status,
                                cx,
                            )),
                    )
                    .child(
                        super::settings_card(cx)
                            .child(info_row(
                                "桌面运行时",
                                backend_value(|info| &info.version),
                                true,
                                cx,
                            ))
                            .child(info_row(
                                "Node.js",
                                backend_value(|info| &info.node_version),
                                false,
                                cx,
                            ))
                            .child(info_row(
                                "Hive 包",
                                backend_value(|info| &info.hive_version),
                                false,
                                cx,
                            ))
                            .child(info_row(
                                "后台平台",
                                connected
                                    .map(|info| {
                                        format!(
                                            "{} · {}",
                                            value(info.platform.as_deref()),
                                            value(info.arch.as_deref())
                                        )
                                    })
                                    .unwrap_or_else(|| tr("无法获取").to_owned()),
                                false,
                                cx,
                            )),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Text.get(cx))
                            .child(tr("本地数据与维护")),
                    )
                    .child(
                        super::settings_card(cx)
                            .child(
                                super::settings_anchor_row(
                                    "about-maintenance",
                                    false,
                                    hl_maint.is_some(),
                                    hl_maint.unwrap_or(0),
                                    cx,
                                )
                                .child(
                                    v_flex()
                                        .w_full()
                                        .min_w_0()
                                        .gap_3()
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .min_w_0()
                                                .items_center()
                                                .justify_between()
                                                .gap_3()
                                                .flex_wrap()
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w(px(200.))
                                                        .gap_0p5()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(Color::Text.get(cx))
                                                                .child(tr("本地数据目录")),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(Color::Muted.get(cx))
                                                                .child(data_dir_text),
                                                        ),
                                                )
                                                .child(Reveal::new(
                                                    "about-open-data-dir-reveal",
                                                    &self.about_scroll,
                                                    Button::new("about-open-data-dir")
                                                        .outline()
                                                        .small()
                                                        .h(px(28.))
                                                        .px_2p5()
                                                        .rounded_lg()
                                                        .icon(Glyph::FolderOpen)
                                                        .label(tr("打开数据目录"))
                                                        .disabled(self.data_dir.is_none())
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            if let Some(dir) = &this.data_dir {
                                                                cx.reveal_path(dir);
                                                            }
                                                        })),
                                                )),
                                        )
                                        .child(
                                            v_flex()
                                                .pt_2p5()
                                                .border_t_1()
                                                .border_color(Color::Divider.get(cx))
                                                .gap_2()
                                                .child(
                                                    h_flex()
                                                        .w_full()
                                                        .min_w_0()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_3()
                                                        .flex_wrap()
                                                        .child(
                                                            v_flex()
                                                                .flex_1()
                                                                .min_w(px(200.))
                                                                .gap_0p5()
                                                                .child(
                                                                    div()
                                                                        .text_sm()
                                                                        .font_weight(FontWeight::MEDIUM)
                                                                        .text_color(Color::Text.get(cx))
                                                                        .child(tr("可清理缓存与日志")),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .text_xs()
                                                                        .text_color(Color::Muted.get(cx))
                                                                        .child(maint_status_text),
                                                                ),
                                                        )
                                                        .child(
                                                            h_flex()
                                                                .gap_1p5()
                                                                .flex_wrap()
                                                                .child(Reveal::new(
                                                                    "about-scan-disposable-reveal",
                                                                    &self.about_scroll,
                                                                    Button::new("about-scan-disposable")
                                                                        .outline()
                                                                        .small()
                                                                        .h(px(28.))
                                                                        .px_2p5()
                                                                        .rounded_lg()
                                                                        .label(if maint.scanning {
                                                                            tr("扫描中…")
                                                                        } else {
                                                                            tr("扫描可清理项")
                                                                        })
                                                                        .disabled(
                                                                            self.data_dir.is_none()
                                                                                || maint.scanning
                                                                                || maint.cleaning,
                                                                        )
                                                                        .on_click(cx.listener(
                                                                            |this, _, _, cx| {
                                                                                this.scan_disposable_cache(cx);
                                                                            },
                                                                        )),
                                                                ))
                                                                .child(Reveal::new(
                                                                    "about-clean-disposable-reveal",
                                                                    &self.about_scroll,
                                                                    Button::new("about-clean-disposable")
                                                                        .outline()
                                                                        .small()
                                                                        .h(px(28.))
                                                                        .px_2p5()
                                                                        .rounded_lg()
                                                                        .label(if maint.cleaning {
                                                                            tr("清理中…")
                                                                        } else {
                                                                            tr("清理缓存与日志")
                                                                        })
                                                                        .disabled(
                                                                            self.data_dir.is_none()
                                                                                || !has_disposable
                                                                                || maint.scanning
                                                                                || maint.cleaning,
                                                                        )
                                                                        .on_click(cx.listener(
                                                                            |this, _, window, cx| {
                                                                                let Some(report) = &this.maintenance_state.report else {
                                                                                    return;
                                                                                };
                                                                                let files = report.disposable_files;
                                                                                let size = maintenance::format_bytes(report.disposable_bytes);
                                                                                let weak = cx.weak_entity();
                                                                                window.open_alert_dialog(cx, move |dialog, _, _| {
                                                                                    let weak = weak.clone();
                                                                                    dialog
                                                                                        .title(tr("清理临时缓存与日志？"))
                                                                                        .description(if english() {
                                                                                            format!("Delete {files} disposable file(s) ({size}) under cache/log/tmp folders? Databases, conversations, drafts, and configs are never touched.")
                                                                                        } else {
                                                                                            format!("确定清理 {files} 个临时缓存与日志文件（共 {size}）吗？此操作仅删除日志、临时文件与缓存，绝不会修改数据库、会话、草稿与配置文件。")
                                                                                        })
                                                                                        .show_cancel(true)
                                                                                        .ok_text(tr("确认清理"))
                                                                                        .cancel_text(tr("取消"))
                                                                                        .on_ok(move |_, _, cx| {
                                                                                            let _ = weak.update(cx, |this, cx| {
                                                                                                this.clean_disposable_cache_confirmed(cx);
                                                                                            });
                                                                                            true
                                                                                        })
                                                                                });
                                                                            },
                                                                        )),
                                                                )),
                                                        ),
                                                ),
                                        ),
                                ),
                            ),
                    ),
            )
            .when_some(maint.notice.as_ref(), |view, (is_error, msg)| {
                view.child(super::settings_notice_box(
                    "about-maintenance-notice",
                    msg.clone(),
                    *is_error,
                    cx,
                ))
            })
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Text.get(cx))
                            .child(tr("引导与许可")),
                    )
                    .child(
                        super::settings_card(cx).child(
                            super::settings_anchor_row(
                                "about-guide-licenses",
                                false,
                                hl_guide.is_some(),
                                hl_guide.unwrap_or(0),
                                cx,
                            )
                            .child(
                                v_flex()
                                    .w_full()
                                    .min_w_0()
                                    .gap_3()
                                    .child(
                                        h_flex()
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .flex_wrap()
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w(px(200.))
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(Color::Text.get(cx))
                                                            .child(tr("欢迎引导")),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(tr("查看本机 Agent 状态并添加项目文件夹。")),
                                                    ),
                                            )
                                            .child(Reveal::new(
                                                "about-reopen-welcome-guide-reveal",
                                                &self.about_scroll,
                                                Button::new("about-reopen-welcome-guide")
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_2p5()
                                                    .rounded_lg()
                                                    .label(tr("重新查看欢迎引导"))
                                                    .on_click(cx.listener(|this, _, window, cx| {
                                                        this.leave_preferences(window, cx);
                                                        window.close_dialog(cx);
                                                        this.open_welcome_setup(cx);
                                                    })),
                                            )),
                                    )
                                    .child(
                                        h_flex()
                                            .pt_2p5()
                                            .border_t_1()
                                            .border_color(Color::Divider.get(cx))
                                            .w_full()
                                            .min_w_0()
                                            .items_center()
                                            .justify_between()
                                            .gap_3()
                                            .flex_wrap()
                                            .child(
                                                v_flex()
                                                    .flex_1()
                                                    .min_w(px(200.))
                                                    .gap_0p5()
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(Color::Text.get(cx))
                                                            .child(tr("开源软件许可")),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Color::Muted.get(cx))
                                                            .child(tr("查看 Hive Desktop 所依赖的核心开源软件及其许可证声明。")),
                                                    ),
                                            )
                                            .child(Reveal::new(
                                                "about-open-licenses-reveal",
                                                &self.about_scroll,
                                                Button::new("about-open-licenses")
                                                    .outline()
                                                    .small()
                                                    .h(px(28.))
                                                    .px_2p5()
                                                    .rounded_lg()
                                                    .label(tr("查看开源许可"))
                                                    .on_click(cx.listener(|_, _, window, cx| {
                                                        open_licenses_dialog(window, cx);
                                                    })),
                                            )),
                                    ),
                            ),
                        ),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(tr("诊断信息包含版本、构建和连接状态，可用于反馈问题。版本来自当前程序与实际运行的后台。")),
            )
    }
}
