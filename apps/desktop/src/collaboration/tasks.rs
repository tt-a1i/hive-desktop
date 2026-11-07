use super::Tasks;
use crate::app::HiveApp;
use crate::palette::Color;
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

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum SaveOutcome {
    Saved(Tasks),
    Conflict {
        #[serde(rename = "currentContent")]
        current_content: String,
    },
}

pub(crate) struct TaskDraft {
    input: Entity<TextareaState>,
    base: String,
    conflicting_content: Option<SharedString>,
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
                conflicting_content: None,
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
        if !self.connected || !self.workspace_writes_allowed() {
            return;
        }
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
        let base = draft
            .conflicting_content
            .as_deref()
            .unwrap_or(&draft.base)
            .to_owned();
        draft.saving = true;
        draft.error = None;
        self.collaboration.task_generation += 1;
        let operation = self.collaboration.task_generation;
        draft.operation = operation;
        cx.notify();
        let connection = self.connection_generation;
        cx.spawn(async move |this, cx| {
            let result = runtime
                .request(
                    "workspace.saveTasks",
                    json!({
                        "workspaceId": workspace, "base": base, "content": content
                    }),
                )
                .await
                .and_then(|value| {
                    serde_json::from_value::<SaveOutcome>(value).map_err(|e| e.to_string())
                });
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                let c = &mut this.collaboration;
                if !c
                    .task_drafts
                    .get(&workspace)
                    .is_some_and(|draft| draft.saving && draft.operation == operation)
                {
                    return;
                }
                match result {
                    Ok(SaveOutcome::Saved(tasks)) => {
                        c.task_drafts.remove(&workspace);
                        if c.visible && this.workspace.as_ref() == Some(&workspace) {
                            c.task_text = tasks.content;
                            c.tasks
                                .update(cx, |state, cx| state.set_text(&c.task_text, cx));
                            c.task_error = None;
                        }
                    }
                    Ok(SaveOutcome::Conflict { current_content }) => {
                        if let Some(draft) = c.task_drafts.get_mut(&workspace) {
                            draft.saving = false;
                            draft.conflicting_content = Some(current_content.into());
                        }
                    }
                    Err(error) => {
                        if let Some(draft) = c.task_drafts.get_mut(&workspace) {
                            draft.saving = false;
                            draft.error = Some(error);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
                view.when_some(draft.conflicting_content.clone(), |view, latest| {
                    let copy = latest.clone();
                    view.child(
                        v_flex()
                            .min_w_0()
                            .gap_2()
                            .p_3()
                            .rounded_lg()
                            .bg(Color::Inset.get(cx))
                            .child(
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("其他人已更新任务文档"),
                                    )
                                    .child(
                                        Button::new("copy-latest-tasks")
                                            .ghost()
                                            .compact()
                                            .label("复制最新版")
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy.to_string(),
                                                ))
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .id("latest-task-content")
                                    .min_w_0()
                                    .max_h(px(200.))
                                    .overflow_y_scroll()
                                    .text_sm()
                                    .child(if latest.is_empty() {
                                        SharedString::from("（文档为空）")
                                    } else {
                                        latest
                                    }),
                            ),
                    )
                    .child(div().text_sm().child("你的草稿 · 对照上方版本编辑后保存"))
                })
                .child(
                    Textarea::new(&draft.input)
                        .aria_label("任务文档 Markdown")
                        .disabled(draft.saving),
                )
                .when_some(draft.error.clone(), |view, error| {
                    view.child(
                        div()
                            .text_sm()
                            .text_color(Color::Error.get(cx))
                            .child(error),
                    )
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
                                } else if draft.conflicting_content.is_some() {
                                    "用当前草稿更新文档"
                                } else {
                                    "保存任务"
                                })
                                .disabled(!self.connected || draft.saving)
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
                    view.child(
                        div()
                            .text_sm()
                            .text_color(Color::Error.get(cx))
                            .child(error),
                    )
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
                            .bg(Color::Card.get(cx))
                            .child(TextView::new(&self.collaboration.tasks).selectable(true)),
                    )
                })
            })
    }
}
