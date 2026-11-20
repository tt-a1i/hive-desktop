use crate::{
    app::{ErrorNotice, HiveApp},
    protocol::{ModelIdentity, Session},
};
use gpui_kit::*;
use serde_json::json;

const UNAVAILABLE_INHERITED_MODEL: &str =
    "继承的模型无法使用。请选择其他模型，或修复 Agent 配置后重试。";

pub(crate) fn is_unavailable_inherited_model_error(error: &str) -> bool {
    error
        .strip_prefix("[FRESH_DRAFT_MODEL_UNAVAILABLE] ")
        .unwrap_or(error)
        == UNAVAILABLE_INHERITED_MODEL
}

impl HiveApp {
    pub(crate) fn unavailable_inherited_model(&self) -> Option<&ModelIdentity> {
        let session = self.session.as_ref()?;
        (session.inherited_draft_model && self.workspace.as_ref() == Some(&session.workspace_id))
            .then_some(self.unavailable_model.as_ref())
            .flatten()
    }

    pub(crate) fn can_reset_inherited_model(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && self.conversation_ready
            && self.viewing_conversation()
            && self.workspace_writes_allowed()
            && !self.conversation_is_read_only()
            && self.resetting_inherited_model != Some(self.open_request)
            && self.session.as_ref().is_some_and(|session| {
                !self.inflight_inputs.contains_key(&session.id) && !self.busy.contains(&session.id)
            })
            && self.unavailable_inherited_model().is_some()
    }

    pub(crate) fn reset_inherited_model(&mut self, cx: &mut Context<Self>) {
        if !self.can_reset_inherited_model() {
            return;
        }
        let (Some(runtime), Some(source), Some(model)) = (
            self.runtime.clone(),
            self.session.clone(),
            self.unavailable_inherited_model(),
        ) else {
            return;
        };
        let params = json!({
            "workspaceId": source.workspace_id,
            "sessionId": source.id,
            "expectedProvider": model.provider,
            "expectedModelId": model.id,
        });
        let connection = self.connection_generation;
        let opened = self.open_request;
        self.resetting_inherited_model = Some(opened);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.resetFreshDraftModel", params)
                .await
                .and_then(|value| {
                    serde_json::from_value::<Session>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.resetting_inherited_model != Some(opened)
                {
                    return;
                }
                this.resetting_inherited_model = None;
                if this.closing
                    || !this.viewing_conversation()
                    || this.open_request != opened
                    || this.workspace.as_ref() != Some(&source.workspace_id)
                    || this
                        .session
                        .as_ref()
                        .is_none_or(|session| session.id != source.id)
                {
                    return;
                }
                match result {
                    Ok(session)
                        if session.id == source.id
                            && session.workspace_id == source.workspace_id
                            && session.execution == source.execution
                            && session.profile == source.profile
                            && session.backend == source.backend
                            && !session.inherited_draft_model =>
                    {
                        this.error = None;
                        this.show_session(session.id, cx);
                    }
                    Ok(_) => this.error = Some("重置模型返回的会话不匹配。".into()),
                    Err(error) => {
                        this.error = Some(ErrorNotice::Detail("无法使用配置默认模型", error))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::{UNAVAILABLE_INHERITED_MODEL, is_unavailable_inherited_model_error};

    #[test]
    fn inherited_model_failure_accepts_the_runtime_error_prefix() {
        assert!(is_unavailable_inherited_model_error(
            UNAVAILABLE_INHERITED_MODEL
        ));
        assert!(is_unavailable_inherited_model_error(&format!(
            "[FRESH_DRAFT_MODEL_UNAVAILABLE] {UNAVAILABLE_INHERITED_MODEL}"
        )));
        assert!(!is_unavailable_inherited_model_error(&format!(
            "[SESSION_ORIGIN_BUSY] {UNAVAILABLE_INHERITED_MODEL}"
        )));
    }
}
