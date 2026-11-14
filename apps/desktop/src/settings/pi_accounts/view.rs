use super::*;
use crate::palette::{Color, quiet_icon_variant};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        input::Input,
        *,
    },
    prelude::FluentBuilder,
};

fn http_url(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

impl HiveApp {
    pub fn render_pi_accounts(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let accounts = &self.pi_accounts;
        let disabled = !self.connected || accounts.loading;
        let query = accounts.search.read(cx).value().trim().to_lowercase();
        let matching = accounts
            .providers
            .iter()
            .flatten()
            .filter(|provider| provider.matches(&query))
            .collect::<Vec<_>>();
        let config_dir = accounts
            .agent_dir
            .clone()
            .or_else(|| self.pi_agent_dir.as_ref().map(|path| path.to_string_lossy().into_owned()));
        v_flex()
            .min_w_0()
            .gap_4()
            .child(
                v_flex()
                    .gap_2p5()
                    .child(
                        crate::text_reading::selectable_text(
                            "pi-accounts-description",
                            tr("连接内置 Pi 使用的账号，与 Pi CLI 共用登录信息。外部 Agent 使用各自的账号配置。"),
                        )
                        .px_1()
                        .text_xs()
                        .text_color(Color::Secondary.get(cx)),
                    )
                    .child(crate::focus_scroll::Reveal::new(
                        "pi-accounts-actions-reveal",
                        &accounts.scroll,
                        h_flex()
                            .items_center()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                Button::new("reload-pi-accounts")
                                    .outline()
                                    .small()
                                    .h(px(28.))
                                    .px_3()
                                    .rounded_lg()
                                    .label(if accounts.loading {
                                        tr("处理中…")
                                    } else {
                                        tr("刷新账号状态")
                                    })
                                    .disabled(disabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.pi_auth_request(
                                            "pi.auth.providers",
                                            json!({}),
                                            window.window_handle(),
                                            cx,
                                        )
                                    })),
                            )
                            .when_some(config_dir, |row, dir| {
                                row.child(
                                    Button::new("open-pi-config-folder")
                                        .outline()
                                        .small()
                                        .h(px(28.))
                                        .px_3()
                                        .rounded_lg()
                                        .label(tr("打开 Pi 配置目录"))
                                        .tooltip(dir.clone())
                                        .on_click(move |_, _, cx| {
                                            cx.reveal_path(std::path::Path::new(&dir))
                                        }),
                                )
                            }),
                    )),
            )
            .when_some(accounts.message.as_ref(), |view, notice| {
                view.child(super::super::settings_notice_box(
                    "pi-accounts-notice",
                    notice.text(),
                    notice.is_error(),
                    cx,
                ))
            })
            .when_some(accounts.login.clone(), |view, login| {
                let running = login.state == "running";
                let api = login.auth_type == "api_key";
                let provider = accounts
                    .providers
                    .as_ref()
                    .and_then(|providers| {
                        providers
                            .iter()
                            .find(|provider| provider.id == login.provider_id)
                    })
                    .map(|provider| provider.name.clone())
                    .unwrap_or(login.provider_id.clone());
                let login_id = login.id.clone();
                let state_dot = match login.state.as_str() {
                    "running" => Color::Accent.get(cx),
                    "succeeded" => Color::Success.get(cx),
                    "cancelled" => Color::Muted.get(cx),
                    _ => Color::Error.get(cx),
                };
                let state_text = match login.state.as_str() {
                    "running" => {
                        if api {
                            tr("正在配置")
                        } else {
                            tr("正在登录")
                        }
                    }
                    "succeeded" => tr("已保存"),
                    "cancelled" => tr("已取消"),
                    _ => tr("登录失败"),
                };
                let replacing_existing = running
                    && accounts
                        .providers
                        .as_ref()
                        .and_then(|providers| {
                            providers
                                .iter()
                                .find(|provider| provider.id == login.provider_id)
                        })
                        .is_some_and(|provider| {
                            provider
                                .stored_type
                                .as_ref()
                                .is_some_and(|kind| kind != &login.auth_type)
                        });
                view.child(crate::focus_scroll::Reveal::new(
                    "pi-accounts-login-reveal",
                    &accounts.scroll,
                    super::super::settings_card(cx)
                        .when(running, |card| {
                            card.border_color(Color::Accent.get(cx).opacity(0.35))
                        })
                        .child(
                            h_flex()
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .flex_wrap()
                                .px_4()
                                .py_2p5()
                                .bg(Color::Inset.get(cx))
                                .border_b_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    h_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .items_center()
                                        .gap_2()
                                        .child(div().size(px(6.)).rounded_full().bg(state_dot))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(Color::Text.get(cx))
                                                .child(format!("{provider} · {state_text}")),
                                        ),
                                )
                                .when(running, |row| {
                                    row.child(
                                        Button::new("cancel-pi-login")
                                            .outline()
                                            .small()
                                            .h(px(26.))
                                            .px_2p5()
                                            .rounded_lg()
                                            .label(if api {
                                                tr("取消配置")
                                            } else {
                                                tr("取消登录")
                                            })
                                            .disabled(disabled)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.pi_auth_request(
                                                    "pi.auth.cancel",
                                                    json!({"loginId":login_id}),
                                                    window.window_handle(),
                                                    cx,
                                                )
                                            })),
                                    )
                                }),
                        )
                        .child(
                            v_flex()
                                .p_4()
                                .gap_3()
                                .when(
                                    login.state == "succeeded"
                                        && self.session.as_ref().is_some_and(|session| {
                                            matches!(session.backend, crate::protocol::Backend::Pi)
                                        }),
                                    |body| {
                                        body.child(h_flex().child(
                                            Button::new("return-after-pi-login")
                                                .primary()
                                                .small()
                                                .h(px(30.))
                                                .px_3p5()
                                                .rounded_lg()
                                                .label(tr("返回会话并刷新模型"))
                                                .disabled(
                                                    !self.connected
                                                        || self.session.as_ref().is_some_and(
                                                            |session| self.busy.contains(&session.id),
                                                        ),
                                                )
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.leave_preferences(window, cx);
                                                    window.close_dialog(cx);
                                                    this.refresh_pi_models(cx);
                                                })),
                                        ))
                                    },
                                )
                                .when(replacing_existing, |body| {
                                    body.child(
                                        div()
                                            .px_3()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(Color::Inset.get(cx))
                                            .text_xs()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(tr(
                                                "完成后将替换此提供商已保存的登录方式；中途取消会保留旧配置。",
                                            )),
                                    )
                                })
                                .when_some(login.message, |body, message| {
                                    body.child(
                                        div()
                                            .text_sm()
                                            .text_color(Color::Text.get(cx))
                                            .child(runtime_message(&message)),
                                    )
                                })
                                .when_some(
                                    login.authorization.as_ref().or(login.notice.as_ref()),
                                    |body, notice| {
                                        let kind =
                                            notice.get("type").and_then(Value::as_str).unwrap_or("");
                                        let uri = notice
                                            .get(if kind == "device_code" {
                                                "verificationUri"
                                            } else {
                                                "url"
                                            })
                                            .and_then(Value::as_str)
                                            .unwrap_or("")
                                            .to_string();
                                        let message = notice
                                            .get("instructions")
                                            .or_else(|| notice.get("message"))
                                            .and_then(Value::as_str)
                                            .unwrap_or("")
                                            .to_string();
                                        body.child(
                                            v_flex()
                                                .gap_2p5()
                                                .when(!message.is_empty(), |box_el| {
                                                    box_el.child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(Color::Secondary.get(cx))
                                                            .child(message),
                                                    )
                                                })
                                                .when_some(
                                                    notice
                                                        .get("userCode")
                                                        .and_then(Value::as_str)
                                                        .map(String::from),
                                                    |box_el, code| {
                                                        box_el.child(
                                                            h_flex()
                                                                .items_center()
                                                                .justify_between()
                                                                .gap_3()
                                                                .px_3()
                                                                .py_2()
                                                                .rounded_lg()
                                                                .bg(Color::Inset.get(cx))
                                                                .border_1()
                                                                .border_color(Color::Divider.get(cx))
                                                                .child(
                                                                    div()
                                                                        .font_family(
                                                                            Theme::global(cx)
                                                                                .mono_font_family
                                                                                .clone(),
                                                                        )
                                                                        .text_lg()
                                                                        .font_weight(
                                                                            FontWeight::SEMIBOLD,
                                                                        )
                                                                        .text_color(Color::Text.get(cx))
                                                                        .child(code.clone()),
                                                                )
                                                                .child(
                                                                    Button::new(
                                                                        "copy-pi-device-code",
                                                                    )
                                                                    .outline()
                                                                    .small()
                                                                    .h(px(28.))
                                                                    .px_2p5()
                                                                    .rounded_lg()
                                                                    .label(tr("复制设备码"))
                                                                    .on_click(move |_, _, cx| {
                                                                        cx.write_to_clipboard(
                                                                            ClipboardItem::new_string(
                                                                                code.clone(),
                                                                            ),
                                                                        )
                                                                    }),
                                                                ),
                                                        )
                                                    },
                                                )
                                                .when(http_url(&uri), |box_el| {
                                                    box_el.child(
                                                        h_flex()
                                                            .flex_wrap()
                                                            .gap_2()
                                                            .child(
                                                                Button::new("open-pi-login-url")
                                                                    .primary()
                                                                    .small()
                                                                    .h(px(30.))
                                                                    .px_3p5()
                                                                    .rounded_lg()
                                                                    .label(tr("在浏览器中授权"))
                                                                    .on_click({
                                                                        let uri = uri.clone();
                                                                        move |_, _, cx| {
                                                                            cx.open_url(&uri)
                                                                        }
                                                                    }),
                                                            )
                                                            .child(
                                                                Button::new("copy-pi-login-url")
                                                                    .outline()
                                                                    .small()
                                                                    .h(px(30.))
                                                                    .px_3()
                                                                    .rounded_lg()
                                                                    .label(tr("复制授权链接"))
                                                                    .on_click(move |_, _, cx| {
                                                                        cx.write_to_clipboard(
                                                                            ClipboardItem::new_string(
                                                                                uri.clone(),
                                                                            ),
                                                                        )
                                                                    }),
                                                            ),
                                                    )
                                                }),
                                        )
                                    },
                                )
                                .when_some(
                                    login
                                        .notice
                                        .as_ref()
                                        .filter(|_| login.authorization.is_some())
                                        .and_then(|notice| notice.get("message"))
                                        .and_then(Value::as_str)
                                        .map(String::from),
                                    |body, message| {
                                        body.child(
                                            div()
                                                .text_xs()
                                                .text_color(Color::Secondary.get(cx))
                                                .child(message),
                                        )
                                    },
                                )
                                .when_some(login.prompt.filter(|_| running), |body, prompt| {
                                    body.child(crate::focus_scroll::Reveal::new(
                                        "pi-accounts-prompt-reveal",
                                        &accounts.scroll,
                                        v_flex()
                                            .gap_2p5()
                                            .pt_1()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_weight(FontWeight::MEDIUM)
                                                    .text_color(Color::Text.get(cx))
                                                    .child(prompt.message.clone()),
                                            )
                                            .when(prompt.kind != "select", |box_el| {
                                                box_el
                                                    .child(
                                                        Input::new(&accounts.input)
                                                            .aria_label(prompt.message)
                                                            .disabled(disabled)
                                                            .when(
                                                                matches!(
                                                                    prompt.kind.as_str(),
                                                                    "secret" | "manual_code"
                                                                ),
                                                                |input| input.mask_toggle(),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex().child(
                                                            Button::new("submit-pi-login")
                                                                .primary()
                                                                .small()
                                                                .h(px(30.))
                                                                .px_3p5()
                                                                .rounded_lg()
                                                                .label(if api {
                                                                    tr("继续")
                                                                } else {
                                                                    tr("继续登录")
                                                                })
                                                                .disabled(
                                                                    disabled
                                                                        || (prompt.kind != "text"
                                                                            && accounts
                                                                                .input
                                                                                .read(cx)
                                                                                .value()
                                                                                .trim()
                                                                                .is_empty()),
                                                                )
                                                                .on_click(cx.listener(
                                                                    |this, _, window, cx| {
                                                                        this.submit_pi_login(
                                                                            None, window, cx,
                                                                        )
                                                                    },
                                                                )),
                                                        ),
                                                    )
                                            })
                                            .when(prompt.kind == "select", |box_el| {
                                                box_el.children(prompt.options.into_iter().map(
                                                    |choice| {
                                                        let id = choice.id;
                                                        v_flex()
                                                            .gap_1()
                                                            .child(
                                                                Button::new(SharedString::from(
                                                                    format!(
                                                                        "pi-login-choice-{id}"
                                                                    ),
                                                                ))
                                                                .outline()
                                                                .small()
                                                                .h(px(30.))
                                                                .px_3()
                                                                .rounded_lg()
                                                                .label(choice.label)
                                                                .disabled(disabled)
                                                                .on_click(cx.listener(
                                                                    move |this, _, window, cx| {
                                                                        this.submit_pi_login(
                                                                            Some(id.clone()),
                                                                            window,
                                                                            cx,
                                                                        )
                                                                    },
                                                                )),
                                                            )
                                                            .when_some(
                                                                choice.description,
                                                                |opt, description| {
                                                                    opt.child(
                                                                        div()
                                                                            .text_xs()
                                                                            .text_color(
                                                                                Color::Secondary
                                                                                    .get(cx),
                                                                            )
                                                                            .child(description),
                                                                    )
                                                                },
                                                            )
                                                    },
                                                ))
                                            }),
                                    ))
                                }),
                        ),
                ))
            })
            .child(
                v_flex()
                    .gap_2()
                    .child(crate::focus_scroll::Reveal::new(
                        "pi-accounts-search-reveal",
                        &accounts.scroll,
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(Glyph::Search)
                                    .size(px(14.))
                                    .text_color(Color::Muted.get(cx)),
                            )
                            .child(
                                div().flex_1().min_w_0().child(
                                    Input::new(&accounts.search)
                                        .aria_label(tr("搜索 Pi 提供商"))
                                        .disabled(accounts.providers.is_none()),
                                ),
                            ),
                    ))
                    .when(accounts.providers.is_some(), |view| {
                        view.child(crate::focus_scroll::Reveal::new(
                            "pi-accounts-count-reveal",
                            &accounts.scroll,
                            h_flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_1()
                                .child(
                                    crate::text_reading::selectable_text(
                                        "pi-provider-count",
                                        if english() {
                                            format!(
                                                "{} {}",
                                                matching.len(),
                                                if matching.len() == 1 {
                                                    "provider"
                                                } else {
                                                    "providers"
                                                }
                                            )
                                        } else {
                                            format!("{} 个提供商", matching.len())
                                        },
                                    )
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx)),
                                )
                                .when(!query.is_empty(), |row| {
                                    row.child(
                                        Button::new("clear-pi-provider-search")
                                            .custom(quiet_icon_variant(cx))
                                            .compact()
                                            .small()
                                            .h(px(26.))
                                            .px_2p5()
                                            .rounded_lg()
                                            .label(tr("清除搜索"))
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.pi_accounts.search.update(cx, |input, cx| {
                                                    input.set_value("", window, cx);
                                                    input.focus_handle(cx).focus(window, cx);
                                                });
                                            })),
                                    )
                                }),
                        ))
                    }),
            )
            .when(accounts.providers.is_none(), |view| {
                view.child(
                    super::super::settings_card(cx)
                        .p_5()
                        .items_center()
                        .child(
                            crate::text_reading::selectable_text(
                                "pi-accounts-loading",
                                if accounts.loading {
                                    tr("正在读取 Pi 账号…")
                                } else {
                                    tr("点击刷新读取可连接的账号。")
                                },
                            )
                            .text_sm()
                            .text_color(Color::Secondary.get(cx)),
                        ),
                )
            })
            .when(
                accounts.providers.is_some() && matching.is_empty(),
                |view| {
                    view.child(
                        super::super::settings_card(cx)
                            .p_6()
                            .items_center()
                            .child(
                                crate::text_reading::selectable_text(
                                    "pi-accounts-empty",
                                    tr("没有匹配的提供商。可以换个名称搜索，或清除搜索查看全部。"),
                                )
                                .text_sm()
                                .text_color(Color::Secondary.get(cx)),
                            ),
                    )
                },
            )
            .when(!matching.is_empty(), |view| {
                view.child(
                    super::super::settings_card(cx).children(
                        matching.into_iter().enumerate().map(|(index, provider)| {
                            div()
                                .w_full()
                                .min_w_0()
                                .when(index > 0, |row| {
                                    row.border_t_1().border_color(Color::Divider.get(cx))
                                })
                                .child(self.render_pi_provider(provider, cx))
                        }),
                    ),
                )
            })
            .child(
                crate::text_reading::selectable_text(
                    "pi-accounts-storage-note",
                    tr("本机保存的配置与 Pi 共用；环境变量等配置仍可生效。保存状态不代表已连接服务端。自定义模型可在“模型与接口”中编辑。"),
                )
                .px_1()
                .text_xs()
                .text_color(Color::Muted.get(cx)),
            )
    }
}

impl Provider {
    fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || format!(
                "{} {} {}",
                self.id,
                self.name,
                self.methods
                    .iter()
                    .map(|method| method.label.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
            .to_lowercase()
            .contains(query)
    }
}
impl HiveApp {
    fn render_pi_provider(
        &self,
        provider: &Provider,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let id = provider.id.clone();
        let disabled = !self.connected || self.pi_accounts.loading || self.pi_accounts.running();
        let (dot_color, status) = match provider.stored_type.as_deref() {
            Some("oauth") => (Color::Success.get(cx), tr("已保存订阅账号")),
            Some("api_key") => (Color::Success.get(cx), tr("已保存 API 配置")),
            _ => (Color::Muted.get(cx), tr("未保存本机凭据")),
        };
        let field = h_flex()
            .id(SharedString::from(format!("pi-provider-{id}")))
            .role(Role::Group)
            .aria_label(format!("{} · {status}", provider.name))
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .gap_3()
            .flex_wrap()
            .px_4()
            .py_3p5()
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(180.))
                    .gap_1()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .flex_wrap()
                            .child(
                                crate::text_reading::selectable_text(
                                    SharedString::from(format!("pi-provider-{id}-name")),
                                    provider.name.clone(),
                                )
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(Color::Text.get(cx)),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_1p5()
                                    .px_2()
                                    .py(px(2.))
                                    .rounded_full()
                                    .bg(Color::Inset.get(cx))
                                    .child(div().size(px(6.)).rounded_full().bg(dot_color))
                                    .child(
                                        crate::text_reading::selectable_text(
                                            SharedString::from(format!("pi-provider-{id}-status")),
                                            status.to_owned(),
                                        )
                                        .text_xs()
                                        .text_color(Color::Secondary.get(cx)),
                                    ),
                            ),
                    )
                    .when(provider.methods.is_empty(), |col| {
                        col.child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(if provider.registered {
                                    tr("沿用 Pi 的现有配置")
                                } else {
                                    tr("未安装此提供商，可移除保存的登录信息")
                                }),
                        )
                    }),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .children(provider.methods.iter().map(|method| {
                        let kind = method.kind.clone();
                        let id = provider.id.clone();
                        let label = match (kind.as_str(), provider.stored_type.as_deref()) {
                            ("oauth", Some("oauth")) => tr("重新登录"),
                            ("oauth", _) => tr("订阅登录"),
                            (_, Some("api_key")) => tr("更新 API 配置"),
                            _ => tr("API 配置"),
                        };
                        Button::new(SharedString::from(format!("connect-pi-{id}-{kind}")))
                            .outline()
                            .small()
                            .h(px(28.))
                            .px_3()
                            .rounded_lg()
                            .label(label)
                            .tooltip(method.label.clone())
                            .accessibility_label(format!("{} · {}", label, provider.name))
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.pi_auth_request(
                                    "pi.auth.start",
                                    json!({"providerId":id,"authType":kind}),
                                    window.window_handle(),
                                    cx,
                                )
                            }))
                    }))
                    .when(provider.stored_type.is_some(), |row| {
                        row.child(
                            Button::new(SharedString::from(format!("disconnect-pi-{id}")))
                                .custom(quiet_icon_variant(cx))
                                .compact()
                                .small()
                                .h(px(28.))
                                .px_2p5()
                                .rounded_lg()
                                .label(tr("移除登录信息"))
                                .accessibility_label(if english() {
                                    format!("Remove {} login information", provider.name)
                                } else {
                                    format!("移除 {} 登录信息", provider.name)
                                })
                                .disabled(disabled)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_pi_disconnect(id.clone(), window, cx)
                                })),
                        )
                    }),
            );
        crate::focus_scroll::Reveal::new(
            SharedString::from(format!("pi-provider-{}-reveal", provider.id)),
            &self.pi_accounts.scroll,
            field,
        )
    }
}
