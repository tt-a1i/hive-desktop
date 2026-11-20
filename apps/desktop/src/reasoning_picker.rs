use crate::app::{ErrorNotice, HiveApp};
mod view;
pub(crate) use view::reasoning_level_label;
use gpui_kit::*;
use serde_json::json;
#[derive(serde::Deserialize)]
pub struct ReasoningState {
    pub level: String,
    pub levels: Vec<String>,
    pub description: Option<String>,
    pub model: Option<crate::protocol::ModelIdentity>,
}
#[derive(Default)]
pub struct ReasoningPicker {
    pub model_scroll: ScrollHandle,
    pub(crate) model_search_navigation: crate::model_picker::ModelSearchNavigation,
    pub scroll: ScrollHandle,
    pub loading: bool,
    state: Option<ReasoningState>,
    error: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) enum EffortDirection {
    Increase,
    Decrease,
}

impl EffortDirection {
    fn wire_name(self) -> &'static str {
        match self {
            Self::Increase => "increase",
            Self::Decrease => "decrease",
        }
    }
}

enum ReasoningRequest {
    Read,
    Set(String),
    Step(EffortDirection),
}

const STEP_FAILED: &str = "无法切换推理强度";

impl HiveApp {
    pub(crate) fn load_reasoning(&mut self, level: Option<String>, cx: &mut Context<Self>) {
        self.request_reasoning(level.map_or(ReasoningRequest::Read, ReasoningRequest::Set), cx);
    }

    pub(crate) fn step_reasoning_shortcut(
        &mut self,
        direction: EffortDirection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.composer_key_is_focused(window, cx)
            || self.command_picker.open
            || !self.viewing_conversation()
            || !self.workspace_writes_allowed()
            || self.current_history_access().is_some_and(|access| access.saved_history_passive)
            || self.input.update(cx, |input, cx| input.marked_text_range(window, cx).is_some())
        {
            return;
        }
        let Some(session) = &self.session else {
            return;
        };
        if self.workspace.as_deref() != Some(session.workspace_id.as_str()) {
            return;
        }
        cx.stop_propagation();
        if self.busy.contains(&session.id) || self.inflight_inputs.contains_key(&session.id) {
            self.error = Some(ErrorNotice::Detail(
                STEP_FAILED,
                "请在当前回复结束后调整推理强度。".into(),
            ));
            cx.notify();
            return;
        }
        self.request_reasoning(ReasoningRequest::Step(direction), cx);
    }

    fn request_reasoning(&mut self, request: ReasoningRequest, cx: &mut Context<Self>) {
        if self.session.as_ref().is_some_and(|s| matches!(s.backend, crate::protocol::Backend::Acp)) {
            return;
        }
        if self.current_history_access().is_some_and(|a| a.handoff_passive())
            || self.conversation_is_read_only()
            || self.reasoning_picker.loading
            || !self.conversation_ready
            || self.loading
            || !self.connected
            || self.closing
        {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let id = session.id.clone();
        let workspace = session.workspace_id.clone();
        let connection = self.connection_generation;
        let open = self.open_request;
        let stepping = matches!(request, ReasoningRequest::Step(_));
        if stepping && matches!(self.error, Some(ErrorNotice::Detail(STEP_FAILED, _))) {
            self.error = None;
        }
        self.reasoning_picker.loading = true;
        self.reasoning_picker.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let setting = matches!(request, ReasoningRequest::Set(_));
            let (method, params) = match request {
                ReasoningRequest::Read => ("chat.reasoning", json!({"sessionId":id,"viewRevision":open})),
                ReasoningRequest::Set(level) => ("chat.setReasoning", json!({"sessionId":id,"level":level})),
                ReasoningRequest::Step(direction) => ("chat.stepReasoning", json!({
                    "sessionId": id, "viewRevision": open, "direction": direction.wire_name()
                })),
            };
            let result = runtime.request(method, params).await;
            let failure = result.as_ref().err().cloned();
            // The explicit picker can reconcile after a rejected setter. A
            // shortcut retains the last confirmed effort and never reopens an
            // agent after a failed step.
            let result = if failure.is_some() && setting {
                runtime
                    .request("chat.reasoning", json!({"sessionId":id,"viewRevision":open}))
                    .await
            } else {
                result
            };
            let state = result.and_then(|value| {
                serde_json::from_value::<ReasoningState>(value).map_err(|error| error.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != open
                    || !this.session.as_ref().is_some_and(|s| s.id == id && s.workspace_id == workspace)
                {
                    return;
                }
                this.reasoning_picker.loading = false;
                match state {
                    Ok(state) => {
                        if let Some(model) = &state.model {
                            this.model = Some(model.clone());
                        }
                        this.reasoning_picker.state = Some(state);
                        if failure.is_none() {
                            this.reasoning_picker.error = None;
                            if (setting || stepping) && let Some(session) = &mut this.session {
                                session.inherited_draft_reasoning = None;
                                session.unavailable_reasoning = None;
                            }
                        }
                    }
                    Err(error) => {
                        if !stepping {
                            this.reasoning_picker.state = None;
                        }
                        this.reasoning_picker.error = Some(error);
                    }
                }
                if let Some(error) = failure {
                    this.reasoning_picker.error = Some(error);
                }
                if stepping && let Some(error) = &this.reasoning_picker.error {
                    this.error = Some(ErrorNotice::Detail(STEP_FAILED, error.clone()));
                }
                cx.notify();
            });
        })
        .detach();
    }
}
