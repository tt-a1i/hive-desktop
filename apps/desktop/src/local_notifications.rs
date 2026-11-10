use crate::{app::HiveApp, language::tr, protocol::{PiEvent, Session}};
use crate::focus_scroll::Reveal;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{component::{button::Button, switch::Switch, *}, *};
use std::{collections::HashSet, fs, io, path::{Path, PathBuf}};

const PREFIX: &str = "hive-session:";

#[derive(Default)]
pub struct Preferences {
    enabled: bool,
    root: Option<PathBuf>,
    notice: Option<String>,
}
impl Global for Preferences {}

#[derive(Default)]
pub struct Alerts {
    failed: HashSet<String>,
    posted: HashSet<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Finished, Failed, Interaction }
impl Kind {
    fn body(self) -> &'static str {
        match self {
            Self::Finished => tr("新的回复已结束，点击查看会话。"),
            Self::Failed => tr("会话遇到问题，点击查看并处理。"),
            Self::Interaction => tr("Agent 正在等待你的操作，点击返回会话。"),
        }
    }
}
impl Alerts {
    fn classify(&mut self, event: &PiEvent, was_busy: bool) -> Option<Kind> {
        let id = event.session_id();
        match event {
            PiEvent::State { busy: true, .. } if !was_busy => { self.failed.remove(id); None }
            PiEvent::Error { .. } => self.failed.insert(id.into()).then_some(Kind::Failed),
            // Restoring an already failed queue is not a new failure.
            PiEvent::State { delivery_failure: Some(_), busy: false, .. } if was_busy => {
                self.failed.insert(id.into()).then_some(Kind::Failed)
            }
            PiEvent::Settled { aborted, .. } => {
                let failed = self.failed.remove(id);
                (!aborted && !failed).then_some(Kind::Finished)
            }
            PiEvent::Interaction { interaction, .. } if interaction.state == "pending" => Some(Kind::Interaction),
            _ => None,
        }
    }
}
fn load(root: &Path) -> io::Result<bool> {
    match fs::File::open(root.join("desktop-notifications.json")) {
        Ok(file) => Ok(serde_json::from_reader(file)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}
fn save(root: &Path, enabled: bool) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let mut file = tempfile::NamedTempFile::new_in(root)?;
    serde_json::to_writer(file.as_file_mut(), &enabled)?;
    file.persist(root.join("desktop-notifications.json"))?;
    Ok(())
}
pub fn restore(root: &Path, cx: &mut App) {
    if cx.global::<Preferences>().root.is_some() { return; }
    let result = load(root);
    let state = cx.global_mut::<Preferences>();
    state.root = Some(root.into());
    match result {
        Ok(enabled) => state.enabled = enabled,
        Err(error) => state.notice = Some(format!("{}: {error}", tr("读取本机提醒设置失败，提醒保持关闭"))),
    }
}

impl HiveApp {
    pub(crate) fn register_local_notifications(window: &mut Window, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        cx.on_system_notification_response(move |response, cx| {
            let id = response.tag.strip_prefix(PREFIX).map(str::to_owned);
            if id.is_none() && response.tag != "hive-notification-test" { return; }
            let weak = weak.clone();
            cx.defer(move |cx| {
                cx.activate(true);
                let _ = handle.update(cx, |_, window, cx| {
                    window.activate_window();
                    if let Some(id) = id {
                        let _ = weak.update(cx, |this, cx| this.open_notification_session(id, handle, cx));
                    }
                });
            });
        });
    }
    fn open_notification_session(&mut self, id: String, handle: AnyWindowHandle, cx: &mut Context<Self>) {
        cx.dismiss_system_notification(&format!("{PREFIX}{id}"));
        self.local_alerts.posted.remove(&id);
        let Some(runtime) = self.runtime.clone().filter(|_| self.connected && !self.closing) else { return; };
        let workspaces: Vec<_> = self.workspaces.iter().map(|workspace| workspace.id.clone()).collect();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let found = async {
                for workspace in workspaces {
                    let value = runtime.request("chat.list", serde_json::json!({"workspaceId":workspace})).await?;
                    let sessions: Vec<Session> = serde_json::from_value(value).map_err(|error| error.to_string())?;
                    if let Some(session) = sessions.into_iter().find(|session| session.id == id) { return Ok(session); }
                }
                Err(tr("提醒对应的会话已不存在，请核对工作区。").to_owned())
            }.await;
            let _ = handle.update(cx, |_, window, cx| {
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection || this.closing { return; }
                    match found {
                        Ok(session) => {
                            if window.has_active_dialog(cx) { window.close_all_dialogs(cx); }
                            this.navigate_to_session(session, window, cx);
                        }
                        Err(error) => this.error = Some(error.into()),
                    }
                    cx.notify();
                });
            });
        }).detach();
    }
    pub(crate) fn local_notification_event(&mut self, event: &PiEvent, cx: &mut Context<Self>) {
        let Some(kind) = self.local_alerts.classify(event, self.busy.contains(event.session_id())) else { return; };
        let reading = self.session.as_ref().is_some_and(|session| session.id == event.session_id())
            && self.viewing_conversation();
        let sidebar_open = self.sidebar_open;
        // On macOS active_window can still identify the app's main window in the background.
        let foreground_chat = cx.active_window().and_then(|handle| handle.update(cx, |_, window, _|
            window.is_window_active() && (window.viewport_size().width >= px(820.) || !sidebar_open)).ok()).unwrap_or(false);
        if !cx.global::<Preferences>().enabled || self.closing || (reading && foreground_chat) { return; }
        let session = self.sessions.iter().chain(self.project_sessions.values().flatten())
            .find(|session| session.id == event.session_id());
        let title = session.map(|session| session.title.as_str()).unwrap_or("Hive").to_owned();
        self.local_alerts.posted.insert(event.session_id().into());
        cx.show_system_notification(SystemNotification {
            tag: format!("{PREFIX}{}", event.session_id()).into(),
            title: format!("Hive · {title}").into(), body: kind.body().into(), actions: vec![],
        });
    }
    pub(crate) fn render_local_notification_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<Preferences>();
        v_flex().gap_2().pb_4().border_b_1().border_color(crate::palette::Color::Divider.get(cx))
            .child(crate::text_reading::selectable_text("local-notification-description", tr("在其他应用或会话中工作时，提醒新的回复、错误和待处理请求。系统提醒只显示会话名称与通用提示，不包含正文或工具结果。")))
            .child(Reveal::new("local-notification-switch-reveal", &self.notification_scroll, Switch::new("local-notification-enabled").label(tr("本机系统提醒")).checked(state.enabled).disabled(state.root.is_none())
                .on_change(cx.listener(|this, enabled, _, cx| {
                    let Some(root) = cx.global::<Preferences>().root.clone() else { return; };
                    match save(&root, *enabled) {
                        Ok(()) => {
                            let state = cx.global_mut::<Preferences>(); state.enabled = *enabled;
                            state.notice = Some(tr("本机提醒偏好已保存；系统是否允许通知由系统设置决定。").into());
                            if !*enabled { for id in this.local_alerts.posted.drain() { cx.dismiss_system_notification(&format!("{PREFIX}{id}")); } }
                        }
                        Err(error) => cx.global_mut::<Preferences>().notice = Some(format!("{}: {error}", tr("保存本机提醒失败，原设置仍生效"))),
                    }
                    cx.notify();
                }))))
            .child(h_flex().child(Reveal::new("local-notification-test-reveal", &self.notification_scroll, Button::new("test-local-notification").label(tr("发送测试提醒")).disabled(!state.enabled)
                .on_click(cx.listener(|_, _, _, cx| {
                    cx.show_system_notification(SystemNotification { tag:"hive-notification-test".into(),title:"Hive".into(),body:tr("这是 Hive 的本机测试提醒。").into(),actions:vec![] });
                    cx.global_mut::<Preferences>().notice = Some(tr("测试提醒已提交给系统；若未显示，请检查系统通知权限与专注模式。").into()); cx.notify();
                })))))
            .child(crate::text_reading::selectable_text("local-notification-permission", tr("默认关闭。首次发送可能出现系统授权提示；允许、拒绝和专注模式均由系统控制。仅在应用运行时提醒。")))
            .when_some(state.notice.clone(), |view, notice| view.child(div().id("local-notification-notice").role(Role::Status).aria_label(notice.clone()).child(notice)))
    }
}

#[cfg(test)]
mod tests {
    use super::{Alerts, Kind, load, save};
    use crate::protocol::PiEvent;
    use std::fs;
    #[test]
    fn local_notifications_persist_and_preserve_failed_writes() {
        let root = tempfile::tempdir().unwrap();
        assert!(!load(root.path()).unwrap()); save(root.path(), true).unwrap(); assert!(load(root.path()).unwrap());
        save(root.path(), false).unwrap(); assert!(!load(root.path()).unwrap());
        let file = root.path().join("desktop-notifications.json"); fs::write(&file, "invalid").unwrap();
        assert!(load(root.path()).is_err()); assert_eq!(fs::read_to_string(&file).unwrap(), "invalid");
        fs::remove_file(&file).unwrap(); fs::create_dir(&file).unwrap();
        assert!(save(root.path(), true).is_err()); assert!(file.is_dir()); assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
    #[test]
    fn local_notifications_cover_pending_requests_and_queue_failures() {
        let mut alerts = Alerts::default();
        let event = |value| serde_json::from_value::<PiEvent>(value).unwrap();
        let pending = event(serde_json::json!({"type":"interaction","sessionId":"s","sequence":1,"interaction":{"id":"i","backend":"pi-rpc","method":"confirm","state":"pending","params":{},"nativeRequestId":null}}));
        assert_eq!(alerts.classify(&pending, true), Some(Kind::Interaction));
        let failure = event(serde_json::json!({"type":"state","sessionId":"s","sequence":2,"busy":false,"pending":1,"deliveryFailure":{"deliveryId":"d","message":"failed"}}));
        assert_eq!(alerts.classify(&failure, false), None);
        assert_eq!(alerts.classify(&failure, true), Some(Kind::Failed));
        let done = event(serde_json::json!({"type":"settled","sessionId":"s","sequence":3,"aborted":false}));
        assert_eq!(alerts.classify(&done, false), None);
    }
    #[test]
    fn local_notifications_distinguish_failure_cancel_and_completion() {
        let mut alerts = Alerts::default();
        let event = |value| serde_json::from_value::<PiEvent>(value).unwrap();
        let failed = event(serde_json::json!({"type":"error","sessionId":"s","sequence":1,"message":"failure"}));
        let done = event(serde_json::json!({"type":"settled","sessionId":"s","sequence":2,"aborted":false}));
        assert_eq!(alerts.classify(&failed, true), Some(Kind::Failed));
        assert_eq!(alerts.classify(&failed, true), None);
        assert_eq!(alerts.classify(&done, true), None);
        let started = event(serde_json::json!({"type":"state","sessionId":"s","sequence":3,"busy":true,"pending":0}));
        assert_eq!(alerts.classify(&started, false), None);
        let next_done = event(serde_json::json!({"type":"settled","sessionId":"s","sequence":4,"aborted":false}));
        assert_eq!(alerts.classify(&next_done, true), Some(Kind::Finished));
        let cancelled = event(serde_json::json!({"type":"settled","sessionId":"s","sequence":3,"aborted":true}));
        assert_eq!(alerts.classify(&cancelled, true), None);
    }
}
