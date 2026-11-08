use crate::language::tr;
mod view;
mod resources;
use crate::{app::HiveApp, protocol::Backend};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Escape, MoveDown, MoveUp},
        *,
    },
    *,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeCommand {
    pub name: String,
    pub description: Option<String>,
    pub source: String,
    pub source_info: Value,
}
#[derive(Deserialize)]
struct Catalog {
    supported: bool,
    commands: Vec<NativeCommand>,
}
#[derive(Default)]
pub struct CommandPicker {
    pub open: bool,
    pub loading: bool,
    loaded: bool,
    commands: Vec<NativeCommand>,
    error: Option<CatalogNotice>,
    selected: usize,
    scroll: ScrollHandle,
}
enum CatalogNotice { Message(&'static str), Error(&'static str,String) }
impl From<&'static str> for CatalogNotice {
    fn from(key: &'static str) -> Self { Self::Message(key) }
}
impl CatalogNotice {
    fn text(&self) -> String {
        match self {
            Self::Message(key) => tr(key).to_owned(),
            Self::Error(key,error) => format!("{}: {error}",tr(key)),
        }
    }
}
impl HiveApp {
    pub(crate) fn pi_commands_available(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| matches!(s.backend, Backend::Pi | Backend::PiRpc))
    }
    pub(crate) fn composer_commands_changed(&mut self, cx: &mut Context<Self>) {
        let value = self.input.read(cx).value().to_string();
        let open = self.pi_commands_available()
            && self.connected
            && self.conversation_ready
            && value.starts_with('/')
            && !value.chars().any(char::is_whitespace);
        if !open && !self.command_picker.open {
            return;
        }
        self.command_picker.open = open;
        self.command_picker.selected = 0;
        self.command_picker.scroll.set_offset(point(px(0.), px(0.)));
        if self.command_picker.open && !self.command_picker.loaded {
            self.load_commands(cx);
        }
        cx.notify();
    }
    fn matching_commands(&self, cx: &App) -> Vec<NativeCommand> {
        let value = self.input.read(cx).value().to_string();
        let query = value
            .strip_prefix('/')
            .unwrap_or("")
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();
        self.command_picker
            .commands
            .iter()
            .filter(|command| {
                query.is_empty()
                    || command.name.to_lowercase().contains(&query)
                    || command
                        .description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect()
    }
    pub(crate) fn load_commands(&mut self, cx: &mut Context<Self>) {
        if self.command_picker.loading
            || !self.connected
            || !self.conversation_ready
            || !self.pi_commands_available()
        {
            return;
        }
        let (Some(runtime), Some(session)) = (self.runtime.clone(), self.session.as_ref()) else {
            return;
        };
        let id = session.id.clone();
        let connection = self.connection_generation;
        let open = self.open_request;
        self.command_picker.loading = true;
        self.command_picker.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request("chat.commands", json!({"sessionId":id}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Catalog>(value).map_err(|e| e.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection
                    || this.open_request != open
                    || !this.session.as_ref().is_some_and(|s| s.id == id)
                {
                    return;
                }
                this.command_picker.loading = false;
                this.command_picker.loaded = true;
                this.command_picker.selected = 0;
                match result {
                    Ok(catalog) if catalog.supported => {
                        this.command_picker.commands = catalog.commands;
                        this.command_picker.error = None;
                    }
                    Ok(_) => {
                        this.command_picker.commands.clear();
                        this.command_picker.error = Some("此后端尚未提供原生命令目录。".into());
                    }
                    Err(error) => {
                        this.command_picker.commands.clear();
                        this.command_picker.error = Some(CatalogNotice::Error("读取命令失败",error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn accept_command(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pi_commands_available() || !self.conversation_ready {
            return;
        }
        let text = self.input.read(cx).value().to_string();
        let end = if text.starts_with('/') {
            text.find(char::is_whitespace).unwrap_or(text.len())
        } else {
            0
        };
        let suffix = if text.get(end..).is_some_and(|s| s.starts_with(' ')) {
            ""
        } else {
            " "
        };
        self.input.update(cx, |input, cx| {
            input.set_selected_range(0..end, cx);
            input.replace(format!("/{name}{suffix}"), window, cx);
            input.focus(window, cx);
        });
        self.command_picker.open = false;
        self.schedule_draft_save(cx);
        cx.notify();
    }
    pub(crate) fn accept_selected_command(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return false;
        }
        if self.command_picker.loading {
            return true;
        }
        if let Some(command) = self.matching_commands(cx).get(self.command_picker.selected) {
            self.accept_command(command.name.clone(), window, cx);
            true
        } else {
            false
        }
    }
    fn move_command(&mut self, delta: isize, cx: &mut Context<Self>) {
        let count = self.matching_commands(cx).len();
        if count > 0 {
            self.command_picker.selected = self
                .command_picker
                .selected
                .saturating_add_signed(delta)
                .min(count - 1);
            self.command_picker
                .scroll
                .scroll_to_item(self.command_picker.selected);
        }
        cx.notify();
    }
    pub(crate) fn command_up(&mut self, _: &MoveUp, _: &mut Window, cx: &mut Context<Self>) {
        if self.command_picker.open {
            cx.stop_propagation();
            self.move_command(-1, cx);
        }
    }
    pub(crate) fn command_down(&mut self, _: &MoveDown, _: &mut Window, cx: &mut Context<Self>) {
        if self.command_picker.open {
            cx.stop_propagation();
            self.move_command(1, cx);
        }
    }
    pub(crate) fn command_escape(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        if self.command_picker.open {
            cx.stop_propagation();
            self.command_picker.open = false;
            cx.notify();
        }
    }
    pub(crate) fn command_button(&self, cx: &Context<Self>) -> AnyElement {
        Button::new("native-commands")
            .ghost()
            .compact()
            .label("/")
            .tooltip(tr("技能、提示模板与扩展命令"))
            .accessibility_label(tr("选择原生命令"))
            .selected(self.command_picker.open)
            .disabled(!self.connected || !self.conversation_ready || self.loading)
            .on_click(cx.listener(|this, _, _, cx| {
                this.command_picker.open = !this.command_picker.open;
                if this.command_picker.open {
                    this.load_commands(cx);
                }
                cx.notify();
            }))
            .into_any_element()
    }
}
