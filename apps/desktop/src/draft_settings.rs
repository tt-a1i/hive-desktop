use crate::{
    app::{ErrorNotice, HiveApp},
    protocol::{DraftWorkMode, ModelIdentity, Session},
};
use gpui_kit::*;
use serde_json::json;

const UNAVAILABLE_INHERITED_MODEL: &str =
    "继承的模型无法使用。请选择其他模型，或修复 Agent 配置后重试。";
const UNAVAILABLE_INHERITED_MODE: &str =
    "继承的工作模式无法使用。请选择其他模式，或修复 Agent 配置后重试。";

fn is_unavailable_error(error: &str, prefix: &str, message: &str) -> bool {
    error.strip_prefix(prefix).unwrap_or(error) == message
}

enum DraftSetting {
    Model(ModelIdentity),
    WorkMode(DraftWorkMode),
}

impl DraftSetting {
    fn request(&self, source: &Session) -> (&'static str, serde_json::Value) {
        match self {
            Self::Model(model) => (
                "chat.resetFreshDraftModel",
                json!({
                    "workspaceId": source.workspace_id,
                    "sessionId": source.id,
                    "expectedProvider": model.provider,
                    "expectedModelId": model.id,
                }),
            ),
            Self::WorkMode(mode) => (
                "chat.resetFreshDraftWorkMode",
                json!({
                    "workspaceId": source.workspace_id,
                    "sessionId": source.id,
                    "expectedMode": mode,
                }),
            ),
        }
    }

    fn matches_reset(&self, source: &Session, updated: &Session) -> bool {
        match self {
            Self::Model(_) => {
                !updated.inherited_draft_model
                    && updated.inherited_draft_work_mode == source.inherited_draft_work_mode
            }
            Self::WorkMode(_) => {
                updated.inherited_draft_work_mode.is_none()
                    && updated.unavailable_work_mode.is_none()
                    && updated.inherited_draft_model == source.inherited_draft_model
            }
        }
    }

    fn error_heading(&self) -> &'static str {
        match self {
            Self::Model(_) => "无法使用配置默认模型",
            Self::WorkMode(_) => "无法使用配置默认工作模式",
        }
    }
}

impl HiveApp {
    pub(crate) fn unavailable_inherited_model(&self) -> Option<&ModelIdentity> {
        let session = self.session.as_ref()?;
        (session.inherited_draft_model && self.workspace.as_ref() == Some(&session.workspace_id))
            .then_some(self.unavailable_model.as_ref())
            .flatten()
    }

    pub(crate) fn unavailable_inherited_work_mode(&self) -> Option<DraftWorkMode> {
        let session = self.session.as_ref()?;
        session.unavailable_work_mode.filter(|mode| {
            self.workspace.as_ref() == Some(&session.workspace_id)
                && session.inherited_draft_work_mode == Some(*mode)
        })
    }

    pub(crate) fn record_unavailable_draft_setting(&mut self, error: &ErrorNotice) -> bool {
        let ErrorNotice::Raw(message) = error else {
            return false;
        };
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        if session.inherited_draft_model
            && self.model.is_some()
            && is_unavailable_error(
                message,
                "[FRESH_DRAFT_MODEL_UNAVAILABLE] ",
                UNAVAILABLE_INHERITED_MODEL,
            )
        {
            self.unavailable_model.clone_from(&self.model);
            return true;
        }
        if let Some(mode) = session.inherited_draft_work_mode
            && is_unavailable_error(
                message,
                "[FRESH_DRAFT_WORK_MODE_UNAVAILABLE] ",
                UNAVAILABLE_INHERITED_MODE,
            )
        {
            session.unavailable_work_mode = Some(mode);
            return true;
        }
        false
    }

    pub(crate) fn can_reset_draft_setting(&self) -> bool {
        self.connected
            && !self.closing
            && !self.loading
            && self.conversation_ready
            && self.viewing_conversation()
            && self.workspace_writes_allowed()
            && !self.conversation_is_read_only()
            && self.resetting_draft_setting != Some(self.open_request)
            && self.session.as_ref().is_some_and(|session| {
                !self.inflight_inputs.contains_key(&session.id) && !self.busy.contains(&session.id)
            })
    }

    pub(crate) fn reset_inherited_model(&mut self, cx: &mut Context<Self>) {
        if let Some(model) = self.unavailable_inherited_model().cloned() {
            self.reset_draft_setting(DraftSetting::Model(model), cx);
        }
    }

    pub(crate) fn reset_inherited_work_mode(&mut self, cx: &mut Context<Self>) {
        if let Some(mode) = self.unavailable_inherited_work_mode() {
            self.reset_draft_setting(DraftSetting::WorkMode(mode), cx);
        }
    }

    fn reset_draft_setting(&mut self, setting: DraftSetting, cx: &mut Context<Self>) {
        if !self.can_reset_draft_setting() {
            return;
        }
        let (Some(runtime), Some(source)) = (self.runtime.clone(), self.session.clone()) else {
            return;
        };
        let (method, params) = setting.request(&source);
        let connection = self.connection_generation;
        let opened = self.open_request;
        self.resetting_draft_setting = Some(opened);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime.request(method, params).await.and_then(|value| {
                serde_json::from_value::<Session>(value).map_err(|error| error.to_string())
            });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.resetting_draft_setting != Some(opened)
                {
                    return;
                }
                this.resetting_draft_setting = None;
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
                            && setting.matches_reset(&source, &session) =>
                    {
                        this.error = None;
                        this.show_session(session.id, cx);
                    }
                    Ok(_) => this.error = Some("恢复草稿设置返回的会话不匹配。".into()),
                    Err(error) => {
                        this.error = Some(ErrorNotice::Detail(setting.error_heading(), error))
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
    use super::{
        DraftSetting, DraftWorkMode, Session, UNAVAILABLE_INHERITED_MODE,
        UNAVAILABLE_INHERITED_MODEL, is_unavailable_error,
    };
    use serde_json::json;

    #[test]
    fn inherited_setting_failures_accept_only_their_runtime_error_prefix() {
        for (prefix, message) in [
            (
                "[FRESH_DRAFT_MODEL_UNAVAILABLE] ",
                UNAVAILABLE_INHERITED_MODEL,
            ),
            (
                "[FRESH_DRAFT_WORK_MODE_UNAVAILABLE] ",
                UNAVAILABLE_INHERITED_MODE,
            ),
        ] {
            assert!(is_unavailable_error(message, prefix, message));
            assert!(is_unavailable_error(
                &format!("{prefix}{message}"),
                prefix,
                message
            ));
            assert!(!is_unavailable_error(
                &format!("[SESSION_ORIGIN_BUSY] {message}"),
                prefix,
                message
            ));
        }
    }

    #[test]
    fn restoring_default_work_mode_keeps_the_inherited_model() {
        let wire = json!({
            "id": "draft", "title": "Draft", "workspaceId": "project", "backend": "codex",
            "inheritedDraftModel": true, "inheritedDraftWorkMode": "native-plan",
            "unavailableWorkMode": "native-plan",
        });
        let source: Session = serde_json::from_value(wire).unwrap();
        let mode = DraftSetting::WorkMode(DraftWorkMode::NativePlan);
        let (method, params) = mode.request(&source);
        assert_eq!(method, "chat.resetFreshDraftWorkMode");
        assert_eq!(
            params,
            json!({"workspaceId":"project","sessionId":"draft","expectedMode":"native-plan"})
        );
        let mut updated = source.clone();
        updated.inherited_draft_work_mode = None;
        assert!(!mode.matches_reset(&source, &updated));
        updated.unavailable_work_mode = None;
        assert!(mode.matches_reset(&source, &updated));
        updated.inherited_draft_model = false;
        assert!(!mode.matches_reset(&source, &updated));
    }
}
