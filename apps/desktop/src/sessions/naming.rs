use super::*;
use gpui_kit::component::input::InputState;

impl HiveApp {
    pub fn begin_session_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = &self.session else { return };
        if self.session_name_loading {
            return;
        }
        self.session_name_target = Some(session.id.clone());
        let name = self.session_title(session).to_owned();
        self.session_name.update(cx, |input: &mut InputState, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        cx.notify();
    }

    pub fn save_session_name(&mut self, cx: &mut Context<Self>) {
        if self.session_name_loading || !self.connected || !self.workspace_writes_allowed() {
            return;
        }
        let (Some(runtime), Some(id)) = (self.runtime.clone(), self.session_name_target.clone())
        else {
            return;
        };
        let title = self.session_name.read(cx).value().trim().to_owned();
        if title.is_empty() || title.chars().count() > 120 || title.chars().any(char::is_control) {
            self.error = Some("会话名称需为 1–120 个字符，且不能包含换行或控制字符。".into());
            cx.notify();
            return;
        }
        self.error = None;
        self.session_name_loading = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.rename", json!({"sessionId":id,"title":title}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Session>(value).map_err(|error| error.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                this.session_name_loading = false;
                match result {
                    Ok(renamed) => {
                        for item in this
                            .sessions
                            .iter_mut()
                            .chain(this.project_sessions.values_mut().flatten())
                        {
                            if item.id == id {
                                *item = renamed.clone();
                            }
                        }
                        if this.session.as_ref().is_some_and(|item| item.id == id) {
                            this.session = Some(renamed);
                        }
                        if this.session_name_target.as_ref() == Some(&id) {
                            this.session_name_target = None;
                        }
                    }
                    Err(error) => this.error = Some(format!("保存会话名称失败：{error}")),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
