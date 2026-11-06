use super::Tasks;
use crate::app::HiveApp;
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        input::{Textarea, TextareaState},
        text::TextView,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

pub(crate) struct TaskDraft {
    input: Entity<TextareaState>,
    base: String,
    pub saving: bool,
    operation: u64,
    error: Option<String>,
}

impl HiveApp {
    pub fn tasks_saving(&self) -> bool {
        self.workspace
            .as_ref()
            .and_then(|id| self.collaboration.task_drafts.get(id))
            .is_some_and(|draft| draft.saving)
    }

    fn edit_tasks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let base = self.collaboration.task_text.clone();
        let input = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .auto_grow(8, 22)
                .placeholder("使用 Markdown 编写任务文档…");
            input.set_value(base.clone(), window, cx);
            input
        });
        self.collaboration.task_drafts.insert(
            workspace,
            TaskDraft {
                input,
                base,
                saving: false,
                operation: 0,
                error: None,
            },
        );
        cx.notify();
    }

    fn discard_task_draft(&mut self, cx: &mut Context<Self>) {
        if self.tasks_saving() {
            return;
        }
        if let Some(workspace) = &self.workspace {
            self.collaboration.task_drafts.remove(workspace);
        }
        self.load_tasks(cx);
        cx.notify();
    }

    fn save_tasks(&mut self, cx: &mut Context<Self>) {
        let (Some(workspace), Some(runtime)) = (self.workspace.clone(), self.runtime.clone())
        else {
            return;
        };
        let Some(draft) = self.collaboration.task_drafts.get_mut(&workspace) else {
            return;
        };
        if draft.saving {
            return;
        }
        let content = draft.input.read(cx).value().to_string();
        let base = draft.base.clone();
        draft.saving = true;
        draft.error = None;
        self.collaboration.task_generation += 1;
        let operation = self.collaboration.task_generation;
        draft.operation = operation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let path = format!("/api/workspaces/{workspace}/tasks");
            let current = runtime.request("hive.request", json!({"method":"GET","path":path})).await
                .and_then(|value| serde_json::from_value::<Tasks>(value).map_err(|error| error.to_string()));
            let result = match current {
                Ok(current) if current.content != base => Err("任务文档已被其他窗口或 Agent 修改。你的改动已保留；请先复制草稿，再取消编辑读取最新文档。".to_string()),
                Ok(_) => runtime.request("hive.request", json!({"method":"PUT","path":path,"body":{"content":content}})).await
                    .and_then(|value| serde_json::from_value::<Tasks>(value).map_err(|error| error.to_string())),
                Err(error) => Err(format!("保存前读取任务失败：{error}")),
            };
            let _ = this.update(cx, |this, cx| {
                let c = &mut this.collaboration;
                if !c.task_drafts.get(&workspace).is_some_and(|draft| draft.saving && draft.operation == operation) { return; }
                match result {
                    Ok(tasks) => {
                        c.task_drafts.remove(&workspace);
                        if c.visible && this.workspace.as_ref() == Some(&workspace) {
                            c.task_text = tasks.content;
                            c.tasks.update(cx, |state, cx| state.set_text(&c.task_text, cx));
                            c.task_error = None;
                        }
                    }
                    Err(error) => if let Some(draft) = c.task_drafts.get_mut(&workspace) {
                        draft.saving = false;
                        draft.error = Some(error);
                    }
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn render_tasks(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self
            .workspace
            .as_ref()
            .and_then(|workspace| self.collaboration.task_drafts.get(workspace));
        v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .justify_between()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(
                        if draft.is_some() {
                            "任务 · 编辑 Markdown"
                        } else {
                            "任务"
                        },
                    ))
                    .when(draft.is_none(), |view| {
                        view.child(
                            Button::new("edit-tasks")
                                .label("编辑任务")
                                .disabled(
                                    self.workspace.is_none()
                                        || self.collaboration.task_loading
                                        || self.collaboration.task_error.is_some(),
                                )
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.edit_tasks(window, cx)),
                                ),
                        )
                    }),
            )
            .when_some(draft, |view, draft| {
                let input = draft.input.clone();
                view.child(
                    Textarea::new(&draft.input)
                        .aria_label("任务文档 Markdown")
                        .disabled(draft.saving),
                )
                .when_some(draft.error.clone(), |view, error| {
                    view.child(div().text_sm().text_color(rgb(0xb54131)).child(error))
                })
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("save-tasks")
                                .primary()
                                .label(if draft.saving {
                                    "正在保存…"
                                } else {
                                    "保存任务"
                                })
                                .disabled(draft.saving)
                                .on_click(cx.listener(|this, _, _, cx| this.save_tasks(cx))),
                        )
                        .child(Button::new("copy-task-draft").label("复制草稿").on_click(
                            cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    input.read(cx).value().to_string(),
                                ))
                            }),
                        ))
                        .child(
                            Button::new("cancel-task-edit")
                                .label("取消编辑（丢弃改动）")
                                .disabled(draft.saving)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.discard_task_draft(cx)),
                                ),
                        ),
                )
            })
            .when(draft.is_none(), |view| {
                view.when(self.collaboration.task_loading, |view| {
                    view.child(div().text_sm().child("正在读取任务…"))
                })
                .when_some(self.collaboration.task_error.clone(), |view, error| {
                    view.child(div().text_sm().text_color(rgb(0xb54131)).child(error))
                })
                .when(
                    !self.collaboration.task_loading
                        && self.collaboration.task_error.is_none()
                        && self.collaboration.task_text.is_empty(),
                    |view| view.child(div().text_sm().child("任务文档为空")),
                )
                .when(!self.collaboration.task_text.is_empty(), |view| {
                    view.child(
                        div()
                            .p_3()
                            .rounded_lg()
                            .bg(rgb(0xffffff))
                            .child(TextView::new(&self.collaboration.tasks).selectable(true)),
                    )
                })
            })
    }
}
