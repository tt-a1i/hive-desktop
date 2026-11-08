use crate::app::HiveApp;
mod view;
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
    pub open: bool,
    pub loading: bool,
    state: Option<ReasoningState>,
    error: Option<String>,
}
impl HiveApp {
    fn load_reasoning(&mut self, level: Option<String>, cx: &mut Context<Self>) {
        if self.reasoning_picker.loading
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
        let connection = self.connection_generation;
        let open = self.open_request;
        self.reasoning_picker.loading = true;
        self.reasoning_picker.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mutation = level.is_some();
            let result = if let Some(level) = level {
                runtime
                    .request("chat.setReasoning", json!({"sessionId":id,"level":level}))
                    .await
            } else {
                runtime
                    .request("chat.reasoning", json!({"sessionId":id}))
                    .await
            };
            let failure = result.as_ref().err().cloned();
            // Native setters can reject after a storage error. Read the actual
            // native state instead of leaving an optimistic selection visible.
            let result = if failure.is_some() && mutation {
                runtime
                    .request("chat.reasoning", json!({"sessionId":id}))
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
                    || !this.session.as_ref().is_some_and(|s| s.id == id)
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
                        }
                    }
                    Err(error) => {
                        this.reasoning_picker.state = None;
                        this.reasoning_picker.error = Some(error);
                    }
                }
                if let Some(error) = failure {
                    this.reasoning_picker.error = Some(error);
                }
                cx.notify();
            });
        })
        .detach();
    }
}
