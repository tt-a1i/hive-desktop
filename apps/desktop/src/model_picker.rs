use crate::{
    app::HiveApp,
    language::tr,
    palette::Color,
    protocol::{Backend, Model, ModelIdentity},
};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        Icon,
        button::{Button, ButtonVariants},
        input::{Escape, Input, MoveDown, MoveUp},
        popover::Popover,
        scroll::ScrollableElement,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::{Value, json};

#[derive(Default)]
pub(crate) enum ProjectModelDefaultState {
    #[default]
    Idle,
    Loading,
    Ready(Option<ModelIdentity>),
    Failed(String),
}

#[derive(serde::Deserialize)]
struct ProjectModelDefaultResponse {
    model: Option<ModelIdentity>,
}

#[derive(Default)]
pub(crate) struct ModelSearchNavigation {
    active: Option<(String, String)>,
    enter: Option<(String, String)>,
}

impl ModelSearchNavigation {
    pub(crate) fn cancel_enter(&mut self) {
        self.enter = None;
    }

    fn active_index(&self, models: &[&Model]) -> usize {
        self.active
            .as_ref()
            .and_then(|(provider, id)| {
                models
                    .iter()
                    .position(|model| model.provider == *provider && model.id == *id)
            })
            .unwrap_or(0)
    }
}

fn model_key(model: &Model) -> (String, String) {
    (model.provider.clone(), model.id.clone())
}

fn model_provider_glyph(
    provider: Option<&str>,
    model_id: Option<&str>,
    backend: Option<Backend>,
) -> Glyph {
    let p = provider.unwrap_or("").to_ascii_lowercase();
    let m = model_id.unwrap_or("").to_ascii_lowercase();
    if p.contains("openai")
        || p.contains("codex")
        || m.starts_with("gpt")
        || m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4")
    {
        Glyph::Aperture
    } else if p.contains("anthropic") || p.contains("claude") || m.contains("claude") {
        Glyph::Asterisk
    } else if p.contains("google") || p.contains("gemini") || m.contains("gemini") {
        Glyph::Sparkles
    } else if p.contains("deepseek")
        || p.contains("qwen")
        || p.contains("xai")
        || p.contains("grok")
        || p.contains("mistral")
        || p.contains("meta")
        || p.contains("llama")
        || m.contains("deepseek")
        || m.contains("qwen")
        || m.contains("grok")
    {
        Glyph::Orbit
    } else {
        backend
            .map(crate::composer_controls::backend_glyph)
            .unwrap_or(Glyph::Sparkles)
    }
}

impl HiveApp {
    fn project_model_scope(&self) -> Option<(&str, &str, &crate::agent_profiles::ProfileRef)> {
        let session = self.session.as_ref()?;
        let lead = format!("{}:orchestrator", session.workspace_id);
        if self.workspace.as_ref() != Some(&session.workspace_id)
            || session.backend == Backend::Acp
            || session.scratch || session.archived || session.worktree_released
            || !matches!(session.identity_state.as_deref(), Some("new" | "known"))
            || session.agent_id.as_ref().is_some_and(|id| id != &lead)
            || session.origin_agent_id.as_ref().is_some_and(|id| id != &lead)
        {
            return None;
        }
        Some((&session.workspace_id, &session.id, session.profile.as_ref()?))
    }

    fn confirmed_project_model(&self) -> Option<&ModelIdentity> {
        if self.current_history_access().is_some_and(|access| access.saved_history_passive)
            || self.unavailable_model.is_some()
        {
            return None;
        }
        let model = self.model.as_ref()?;
        self.models.iter().any(|candidate| {
            candidate.available && candidate.provider == model.provider && candidate.id == model.id
        }).then_some(model)
    }

    pub(crate) fn unavailable_project_draft_model(&self) -> Option<&ModelIdentity> {
        self.session.as_ref()?.project_default_draft_model.then_some(())?;
        self.unavailable_inherited_model()
    }

    pub(crate) fn load_model_picker_contents(&mut self, cx: &mut Context<Self>) {
        self.request_project_model_default(None, cx);
        if self.unavailable_project_draft_model().is_none() {
            self.load_reasoning(None, cx);
            self.load_mode(None, cx);
            self.load_permission_mode(None, cx);
        }
    }

    fn request_project_model_default(&mut self, change: Option<Value>, cx: &mut Context<Self>) {
        if !self.connected || self.closing || !self.conversation_ready
            || matches!(self.project_model_default, ProjectModelDefaultState::Loading)
            || change.is_some() && !self.workspace_writes_allowed()
        {
            return;
        }
        let (Some(runtime), Some((workspace, session, profile))) =
            (self.runtime.clone(), self.project_model_scope()) else { return; };
        let mut params = json!({"workspaceId": workspace, "sessionId": session, "profile": profile});
        let (method, error_label) = if let Some(model) = change {
            params["model"] = model;
            ("chat.setProjectModelDefault", "无法保存项目默认模型")
        } else {
            ("chat.projectModelDefault", "无法读取项目默认模型")
        };
        let opened = self.open_request;
        let connection = self.connection_generation;
        self.project_model_default = ProjectModelDefaultState::Loading;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await.and_then(|value| {
                serde_json::from_value::<ProjectModelDefaultResponse>(value).map_err(|error| error.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection || this.open_request != opened || this.closing {
                    return;
                }
                this.project_model_default = match result {
                    Ok(result) => ProjectModelDefaultState::Ready(result.model),
                    Err(error) => ProjectModelDefaultState::Failed(format!("{}\n{error}", tr(error_label))),
                };
                cx.notify();
            });
        }).detach();
    }

    fn set_current_project_model_default(&mut self, cx: &mut Context<Self>) {
        if let Some(model) = self.confirmed_project_model() {
            self.request_project_model_default(Some(json!({"provider": model.provider, "id": model.id})), cx);
        }
    }

    fn project_model_default_controls(&self, cx: &Context<Self>) -> impl IntoElement {
        use ProjectModelDefaultState::{Failed, Idle, Loading, Ready};
        let saved = match &self.project_model_default {
            Ready(model) => model.as_ref(),
            _ => None,
        };
        let current = self.confirmed_project_model();
        let blocked =
            matches!(self.project_model_default, Idle | Loading) || !self.workspace_writes_allowed();
        let (status_text, status_error) = match &self.project_model_default {
            Failed(error) => (error.clone(), true),
            Ready(Some(model)) => {
                let identity = format!("{} / {}", model.provider, model.id);
                let missing = self.unavailable_project_draft_model().is_some_and(|stale| {
                    stale.provider == model.provider && stale.id == model.id
                }) || (!self.models.is_empty()
                    && !self.models.iter().any(|candidate| {
                        candidate.available
                            && candidate.provider == model.provider
                            && candidate.id == model.id
                    }));
                if missing {
                    (
                        tr("项目默认模型 {model} 暂不可用。").replace("{model}", &identity),
                        true,
                    )
                } else {
                    (format!("{}：{}", tr("项目默认"), identity), false)
                }
            }
            Ready(None) => (tr("未设项目默认").to_owned(), false),
            Idle | Loading => (tr("项目默认").to_owned(), false),
        };
        h_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .justify_between()
            .gap_2()
            .px_2p5()
            .py_1p5()
            .border_t_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Inset.get(cx).opacity(0.3))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.5))
                    .text_color(if status_error {
                        Color::Error.get(cx)
                    } else {
                        Color::Muted.get(cx)
                    })
                    .child(status_text),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("set-project-default-model")
                            .custom(crate::palette::quiet_icon_variant(cx))
                            .compact()
                            .xsmall()
                            .h(px(22.))
                            .px_2()
                            .rounded_md()
                            .label(tr("设为项目默认"))
                            .tooltip(tr("使用当前模型作为项目默认"))
                            .accessibility_label(tr("使用当前模型作为项目默认"))
                            .disabled(
                                blocked
                                    || current.is_none_or(|model| {
                                        saved.is_some_and(|saved| {
                                            saved.provider == model.provider
                                                && saved.id == model.id
                                        })
                                    }),
                            )
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_current_project_model_default(cx)),
                            ),
                    )
                    .when(
                        saved.is_some() || matches!(self.project_model_default, Failed(_)),
                        |row| {
                            row.child(
                                Button::new("reset-project-default-model")
                                    .custom(crate::palette::quiet_icon_variant(cx))
                                    .compact()
                                    .xsmall()
                                    .h(px(22.))
                                    .px_2()
                                    .rounded_md()
                                    .label(tr("重置"))
                                    .tooltip(tr("重置项目默认模型"))
                                    .accessibility_label(tr("重置项目默认模型"))
                                    .disabled(blocked)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_project_model_default(Some(Value::Null), cx)
                                    })),
                            )
                        },
                    ),
            )
    }

    fn matching_picker_models(&self, cx: &App) -> Vec<&Model> {
        let query = self.model_search.read(cx).value().trim().to_lowercase();
        let mut matching = self
            .models
            .iter()
            .filter(|model| {
                model.available
                    && (query.is_empty()
                        || format!("{} {} {}", model.provider, model.id, model.name)
                            .to_lowercase()
                            .contains(&query))
            })
            .collect::<Vec<_>>();
        matching.sort_by_key(|model| {
            !self.model.as_ref().is_some_and(|selected| {
                selected.provider == model.provider && selected.id == model.id
            })
        });
        matching
    }

    pub(crate) fn reset_model_picker_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_picker_search_changed(cx);
        self.model_search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.reasoning_picker.scroll.set_offset(point(px(0.), px(0.)));
    }

    pub(crate) fn model_picker_search_changed(&mut self, cx: &mut Context<Self>) {
        self.reasoning_picker.model_search_navigation = Default::default();
        self.reasoning_picker.model_scroll.set_offset(point(px(0.), px(0.)));
        cx.notify();
    }

    fn model_search_is_composing(&self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.model_search.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        })
    }

    fn move_model_search(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.model_search_is_composing(window, cx) {
            return;
        }
        cx.stop_propagation();
        let matching = self.matching_picker_models(cx);
        if matching.is_empty() {
            return;
        }
        let current = self.reasoning_picker.model_search_navigation.active_index(&matching);
        let index = current.saturating_add_signed(delta).min(matching.len() - 1);
        let key = model_key(matching[index]);
        self.reasoning_picker.model_search_navigation.active = Some(key);
        self.reasoning_picker.model_search_navigation.cancel_enter();
        self.reasoning_picker.model_scroll.scroll_to_item(index);
        cx.notify();
    }

    fn model_search_enter_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "enter"
            || event.keystroke.modifiers.modified()
            || self.model_search_is_composing(window, cx)
        {
            return;
        }
        cx.stop_propagation();
        if event.is_held
            || self.reasoning_picker.loading
            || self.reasoning_picker.model_search_navigation.enter.is_some()
        {
            return;
        }
        // Keep focus in the search until Enter is released. Otherwise a held key
        // can land in the composer as soon as the popover closes and send its draft.
        let matching = self.matching_picker_models(cx);
        let index = self.reasoning_picker.model_search_navigation.active_index(&matching);
        self.reasoning_picker.model_search_navigation.enter =
            matching.get(index).map(|model| model_key(model));
    }

    fn model_search_enter_up(
        &mut self,
        event: &KeyUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key != "enter" {
            return;
        }
        let Some((provider, id)) = self.reasoning_picker.model_search_navigation.enter.take() else {
            return;
        };
        cx.stop_propagation();
        if event.keystroke.modifiers.modified()
            || !self.choosing_model
            || self.reasoning_picker.loading
            || self.model_search_is_composing(window, cx)
        {
            return;
        }
        if let Some(model) = self
            .matching_picker_models(cx)
            .into_iter()
            .find(|model| model.provider == provider && model.id == id)
        {
            self.set_model(model.clone(), cx);
        }
    }

    fn model_search_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        self.reasoning_picker.model_search_navigation.cancel_enter();
        if self.model_search_is_composing(window, cx) {
            self.model_search
                .update(cx, |input, cx| input.unmark_text(window, cx));
            return;
        }
        self.choosing_model = false;
        cx.notify();
    }

    pub(crate) fn model_picker(
        &self,
        window: &Window,
        running: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let current = self.model.as_ref();
        let selected = current.and_then(|current| {
            self.models
                .iter()
                .find(|model| model.provider == current.provider && model.id == current.id)
        });
        let raw_name = selected
            .filter(|model| !model.name.trim().is_empty())
            .map(|model| model.name.clone())
            .or_else(|| current.and_then(|model| model.name.as_ref().filter(|name| !name.trim().is_empty()).cloned()))
            .or_else(|| current.map(|model| model.id.clone()))
            .unwrap_or_else(|| tr("选择模型").into());
        let name = ["Codex", "Claude", "Pi", "外部 Pi", tr("外部 Pi")]
            .into_iter()
            .find_map(|tag| raw_name.strip_suffix(&format!(" · {tag}")))
            .map(str::to_owned)
            .unwrap_or(raw_name);
        let effort = self.reasoning_label().filter(|level| !level.trim().is_empty());
        let label = effort
            .as_ref()
            .map(|level| format!("{name} · {level}"))
            .unwrap_or_else(|| name.clone());
        let identity = current
            .map(|model| format!("{} / {}", model.provider, model.id))
            .unwrap_or_else(|| tr("选择会话模型").into());
        let unavailable =
            !self.connected || !self.conversation_ready || running || self.loading || self.closing;
        let backend_icon = Some(model_provider_glyph(
            current.map(|m| m.provider.as_str()),
            current.map(|m| m.id.as_str()),
            self.session.as_ref().map(|session| session.backend),
        ));
        let profile_note = self
            .session
            .as_ref()
            .and_then(|s| s.profile.as_ref())
            .map(|profile| format!("\n{}", self.profile_label(profile)))
            .unwrap_or_default();
        let picker_label = tr("选择会话模型与推理强度");
        let accessibility = if self.session.as_ref().is_some_and(|session| {
            session.inherited_draft_model || session.inherited_draft_reasoning.is_some()
        }) {
            format!("{picker_label} · {label}")
        } else {
            picker_label.to_owned()
        };
        let trigger = Button::new("choose-model")
            .accessibility_label(accessibility)
            .custom(crate::palette::quiet_icon_variant(cx))
            .compact()
            .h(px(28.))
            .px_2()
            .rounded_lg()
            .selected(self.choosing_model)
            .min_w_0()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_1p5()
                    .when_some(backend_icon, |row, glyph| {
                        row.child(
                            Icon::new(glyph)
                                .size(px(14.5))
                                .text_color(Color::Secondary.get(cx))
                                .flex_shrink_0(),
                        )
                    })
                    .child(
                        div()
                            .min_w_0()
                            .max_w(px(180.))
                            .truncate()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Text.get(cx))
                            .child(name.clone()),
                    )
                    .when_some(effort, |row, level| {
                        row.child(
                            div()
                                .flex_shrink_0()
                                .pl_0p5()
                                .text_size(px(12.5))
                                .text_color(Color::Muted.get(cx))
                                .child(level),
                        )
                    }),
            )
            .tooltip(format!(
                "{label}\n{identity}{profile_note}\n{}\n{}",
                tr("只影响当前会话，不修改 Agent 全局默认"),
                crate::keybindings::hint(crate::keybindings::Target::ChooseModel, "快捷键", cx),
            ))
            .disabled(unavailable);
        if unavailable {
            return trigger.into_any_element();
        }
        let mut trigger_layout = div().min_w_0();
        let trigger_style = trigger_layout.style().clone();
        Popover::new("session-model-picker")
            .anchor(Anchor::BottomRight)
            .trigger_style(trigger_style)
            .p_0()
            .rounded_xl()
            .border_1()
            .border_color(Color::Divider.get(cx))
            .bg(Color::Card.get(cx))
            .shadow_lg()
            .overflow_hidden()
            .w(px(340.).min(window.viewport_size().width - px(32.)))
            .open(self.choosing_model)
            .track_focus(&self.model_search.read(cx).focus_handle(cx))
            .trigger(trigger)
            .on_open_change(cx.listener(|this, open, window, cx| {
                if *open
                    && this.unavailable_project_draft_model().is_none()
                    && this.connect_saved_session(crate::sessions::NativeSessionPanel::Models, cx)
                {
                    return;
                }
                this.choosing_model = *open;
                if *open {
                    this.reset_model_picker_search(window, cx);
                    this.load_model_picker_contents(cx);
                }
                cx.notify();
            }))
            .when(self.choosing_model, |popover| {
                let matching = self.matching_picker_models(cx);
                let active = self
                    .reasoning_picker
                    .model_search_navigation
                    .active_index(&matching);
                let search_focused = self
                    .model_search
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window);
                let empty = matching.is_empty();
                let multi_provider = matching
                    .first()
                    .is_some_and(|first| matching.iter().any(|m| m.provider != first.provider));
                let model_scroll = &self.reasoning_picker.model_scroll;
                let session_backend = self.session.as_ref().map(|s| s.backend);
                let list_max_h = (window.viewport_size().height - px(260.))
                    .min(px(208.))
                    .max(px(120.));
                popover.child(
                    v_flex()
                        .w_full()
                        .min_w_0()
                        .child(
                            h_flex()
                                .items_center()
                                .h(px(36.))
                                .px_2p5()
                                .border_b_1()
                                .border_color(Color::Divider.get(cx))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .capture_action(cx.listener(
                                            |this, _: &MoveUp, window, cx| {
                                                this.move_model_search(-1, window, cx)
                                            },
                                        ))
                                        .capture_action(cx.listener(
                                            |this, _: &MoveDown, window, cx| {
                                                this.move_model_search(1, window, cx)
                                            },
                                        ))
                                        .capture_action(cx.listener(Self::model_search_escape))
                                        .capture_key_down(
                                            cx.listener(Self::model_search_enter_down),
                                        )
                                        .capture_key_up(cx.listener(Self::model_search_enter_up))
                                        .child(
                                            Input::new(&self.model_search)
                                                .appearance(false)
                                                .bordered(false)
                                                .focus_bordered(false)
                                                .cleanable(true)
                                                .prefix(
                                                    Icon::new(Glyph::Search)
                                                        .size(px(13.5))
                                                        .text_color(Color::Muted.get(cx)),
                                                )
                                                .aria_label(tr("搜索会话模型")),
                                        ),
                                ),
                        )
                        .child(
                            v_flex()
                                .w_full()
                                .min_w_0()
                                .max_h(list_max_h)
                                .child(crate::focus_scroll::Viewport::new(
                                    "model-picker-viewport",
                                    model_scroll,
                                    v_flex()
                                        .id("model-list")
                                        .flex_1()
                                        .min_w_0()
                                        .min_h_0()
                                        .p_1p5()
                                        .gap_0p5()
                                        .overflow_y_scroll()
                                        .track_scroll(model_scroll)
                                        .vertical_scrollbar(model_scroll)
                                        .when(empty, |list| {
                                            list.child(
                                                div()
                                                    .py_5()
                                                    .px_3()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        crate::text_reading::selectable_text(
                                                            "model-picker-empty",
                                                            tr("没有匹配的可用模型"),
                                                        )
                                                        .text_xs()
                                                        .text_color(Color::Muted.get(cx)),
                                                    ),
                                            )
                                        })
                                        .children(matching.into_iter().enumerate().map(
                                            |(i, model)| {
                                                let selected = self.model.as_ref().is_some_and(
                                                    |m| {
                                                        m.id == model.id
                                                            && m.provider == model.provider
                                                    },
                                                );
                                                let model = model.clone();
                                                let provider_icon = model_provider_glyph(
                                                    Some(&model.provider),
                                                    Some(&model.id),
                                                    session_backend,
                                                );
                                                let separator = if crate::language::english() {
                                                    ", "
                                                } else {
                                                    "，"
                                                };
                                                let mut label = format!(
                                                    "{}{separator}{} / {}",
                                                    model.name, model.provider, model.id
                                                );
                                                if selected {
                                                    label.push_str(separator);
                                                    label.push_str(tr("当前生效"));
                                                }
                                                if i == active
                                                    && search_focused
                                                    && !self.reasoning_picker.loading
                                                {
                                                    label.push_str(separator);
                                                    label.push_str(tr("按 Enter 选择"));
                                                }
                                                crate::focus_scroll::Reveal::new(
                                                    ("model-option-reveal", i),
                                                    model_scroll,
                                                    Button::new(("model", i))
                                                        .custom(
                                                            crate::palette::sidebar_item_variant(
                                                                i == active,
                                                                cx,
                                                            ),
                                                        )
                                                        .compact()
                                                        .justify_start()
                                                        .flex_shrink_0()
                                                        .w_full()
                                                        .min_w_0()
                                                        .h(px(32.))
                                                        .px_2()
                                                        .rounded_lg()
                                                        .disabled(self.reasoning_picker.loading)
                                                        .accessibility_label(label)
                                                        .tooltip(format!(
                                                            "{}\n{} / {}",
                                                            model.name, model.provider, model.id
                                                        ))
                                                        .child(
                                                            h_flex()
                                                                .w_full()
                                                                .min_w_0()
                                                                .items_center()
                                                                .gap_2()
                                                                .child(
                                                                    Icon::new(provider_icon)
                                                                        .size(px(14.))
                                                                        .text_color(if selected {
                                                                            Color::Secondary.get(cx)
                                                                        } else {
                                                                            Color::Muted.get(cx)
                                                                        })
                                                                        .flex_shrink_0(),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .flex_1()
                                                                        .min_w_0()
                                                                        .truncate()
                                                                        .text_size(px(13.))
                                                                        .font_weight(if selected {
                                                                            FontWeight::SEMIBOLD
                                                                        } else {
                                                                            FontWeight::NORMAL
                                                                        })
                                                                        .text_color(
                                                                            Color::Text.get(cx),
                                                                        )
                                                                        .child(model.name.clone()),
                                                                )
                                                                .when(multi_provider, |row| {
                                                                    row.child(
                                                                        div()
                                                                            .flex_shrink_0()
                                                                            .text_size(px(11.))
                                                                            .text_color(
                                                                                Color::Muted
                                                                                    .get(cx),
                                                                            )
                                                                            .child(
                                                                                model
                                                                                    .provider
                                                                                    .clone(),
                                                                            ),
                                                                    )
                                                                })
                                                                .when(selected, |row| {
                                                                    row.child(
                                                                        Icon::new(Glyph::Check)
                                                                            .size(px(13.5))
                                                                            .text_color(
                                                                                Color::Accent
                                                                                    .get(cx),
                                                                            )
                                                                            .flex_shrink_0(),
                                                                    )
                                                                }),
                                                        )
                                                        .selected(i == active)
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.set_model(model.clone(), cx)
                                                            },
                                                        )),
                                                )
                                            },
                                        )),
                                )),
                        )
                        .children(self.render_reasoning_settings(running, cx))
                        .children(self.render_mode_settings(running, cx))
                        .when(self.project_model_scope().is_some(), |col| {
                            col.child(self.project_model_default_controls(cx))
                        }),
                )
            })
            .into_any_element()
    }
}
