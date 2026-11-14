use super::Tasks;
use crate::{app::HiveApp, language::tr};
use crate::palette::Color;
use crate::focus_scroll::Reveal;
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

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct SavedTaskDraft {
    pub input: String,
    pub base: String,
    pub conflicting_content: Option<String>,
}

pub(crate) struct TaskDraft {
    input: Entity<TextareaState>,
    base: String,
    conflicting_content: Option<SharedString>,
    pub saving: bool,
    pub(super) operation: u64,
    pub(super) error: Option<(&'static str, String)>,
}

impl HiveApp {
    pub(crate) fn localize_task_placeholders(&self, window: &mut Window, cx: &mut App) {
        for draft in self.collaboration.task_drafts.values() {
            draft.input.update(cx, |input, cx| input.set_placeholder(tr("使用 Markdown 编写任务文档…"), window, cx));
        }
    }

    pub fn tasks_saving(&self) -> bool {
        self.workspace
            .as_ref()
            .and_then(|id| self.collaboration.task_drafts.get(id))
            .is_some_and(|draft| draft.saving)
    }

    pub(crate) fn task_draft_snapshots(
        &self,
        cx: &App,
    ) -> std::collections::HashMap<String, SavedTaskDraft> {
        let mut saved = self.collaboration.recovered_task_drafts.clone();
        for (id, draft) in &self.collaboration.task_drafts {
            saved.insert(
                id.clone(),
                SavedTaskDraft {
                    input: draft.input.read(cx).value().to_string(),
                    base: draft.base.clone(),
                    conflicting_content: draft.conflicting_content.as_ref().map(|s| s.to_string()),
                },
            );
        }
        saved
    }

    pub(crate) fn restore_task_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.collaboration.visible {
            return;
        }
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        if self.collaboration.task_drafts.contains_key(&workspace) {
            return;
        }
        if let Some(saved) = self.collaboration.recovered_task_drafts.remove(&workspace) {
            self.create_task_editor(workspace, saved, window, cx);
        }
    }

    fn create_task_editor(
        &mut self,
        workspace: String,
        saved: SavedTaskDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.collaboration.task_expanded = true;
        let input = cx.new(|cx| {
            let mut input = TextareaState::new(window, cx)
                .auto_grow(8, 22)
                .placeholder(tr("使用 Markdown 编写任务文档…"));
            input.set_value(saved.input, window, cx);
            input
        });
        cx.subscribe(
            &input,
            |this, _, event: &gpui_kit::component::input::InputEvent, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    this.schedule_draft_save(cx);
                }
            },
        )
        .detach();
        self.collaboration.task_generation += 1;
        self.collaboration.task_loading = false;
        self.collaboration.task_refreshing = false;
        self.collaboration.task_drafts.insert(
            workspace,
            TaskDraft {
                input,
                base: saved.base,
                conflicting_content: saved.conflicting_content.map(Into::into),
                saving: false,
                operation: 0,
                error: None,
            },
        );
    }

    fn edit_tasks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let base = self.collaboration.task_text.clone();
        self.create_task_editor(
            workspace.clone(),
            SavedTaskDraft {
                input: base.clone(),
                base,
                conflicting_content: None,
            },
            window,
            cx,
        );
        if let Some(draft) = self.collaboration.task_drafts.get(&workspace) {
            let focus = draft.input.read(cx).focus_handle(cx);
            window.on_next_frame(move |window, cx| focus.focus(window, cx));
        }
        self.schedule_draft_save(cx);
        cx.notify();
    }

    fn discard_task_draft(&mut self, cx: &mut Context<Self>) {
        if self.tasks_saving() {
            return;
        }
        let Some(workspace) = self.workspace.clone() else {
            return;
        };
        let removed = self.collaboration.task_drafts.remove(&workspace);
        if let Err(error) = self.flush_drafts(cx) {
            if let Some(mut draft) = removed {
                draft.error = Some(("无法清理本地草稿，改动仍保留", error.to_string()));
                self.collaboration.task_drafts.insert(workspace, draft);
            }
            cx.notify();
            return;
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
        if let Err(error) = self.flush_drafts(cx) {
            if let Some(draft) = self.collaboration.task_drafts.get_mut(&workspace) {
                draft.error = Some(("无法保存本地草稿，任务文档尚未更新", error.to_string()));
            }
            cx.notify();
            return;
        }
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
                            draft.error = Some(("保存任务失败", error));
                        }
                    }
                }
                if let Err(error) = this.flush_drafts(cx) {
                    this.error = Some(crate::app::ErrorNotice::Detail("任务保存结果已返回，但本地草稿未能同步；请重试退出或复制内容",error.to_string()));
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn render_tasks(&self, scroll: &ScrollHandle, cx: &mut Context<Self>) -> impl IntoElement {
        let draft = self
            .workspace
            .as_ref()
            .and_then(|workspace| self.collaboration.task_drafts.get(workspace));
        v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .p_4()
            .rounded_xl()
            .bg(Color::Card.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(Reveal::new(
                "task-document-toolbar-reveal",
                scroll,
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(Color::Text.get(cx))
                            .child(if draft.is_some() {
                                tr("任务 · 编辑 Markdown")
                            } else {
                                tr("任务")
                            }),
                    )
                    .when(draft.is_none() && !self.collaboration.task_text.is_empty(), |row| {
                        row.child(
                            Button::new("toggle-task-document")
                                .outline()
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .rounded_md()
                                .label(if self.collaboration.task_expanded {
                                    tr("收起任务文档")
                                } else {
                                    tr("展开任务文档")
                                })
                                .selected(self.collaboration.task_expanded)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.collaboration.task_expanded =
                                        !this.collaboration.task_expanded;
                                    cx.notify();
                                })),
                        )
                    })
                    .when(draft.is_none(), |view| {
                        view.child(
                            Button::new("edit-tasks")
                                .primary()
                                .compact()
                                .xsmall()
                                .h(px(26.))
                                .rounded_md()
                                .label(tr("编辑任务"))
                                .disabled(
                                    self.workspace.is_none()
                                        || self.collaboration.task_loading
                                        || self.collaboration.task_error.is_some(),
                                )
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.edit_tasks(window, cx)
                                })),
                        )
                    }),
            ))
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
                            .bg(Color::ErrorSurface.get(cx))
                            .border_1()
                            .border_color(Color::Divider.get(cx))
                            .child(Reveal::new(
                                "task-conflict-copy-reveal",
                                scroll,
                                h_flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(Color::Error.get(cx))
                                            .child(tr("其他人已更新任务文档")),
                                    )
                                    .child(
                                        Button::new("copy-latest-tasks")
                                            .outline()
                                            .compact()
                                            .xsmall()
                                            .h(px(24.))
                                            .rounded_md()
                                            .label(tr("复制最新版"))
                                            .on_click(cx.listener(move |_, _, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    copy.to_string(),
                                                ))
                                            })),
                                    ),
                            ))
                            .child(
                                div()
                                    .id("latest-task-content")
                                    .min_w_0()
                                    .max_h(px(200.))
                                    .overflow_y_scroll()
                                    .p_2()
                                    .rounded_md()
                                    .bg(Color::Inset.get(cx))
                                    .text_xs()
                                    .text_color(Color::Secondary.get(cx))
                                    .child(if latest.is_empty() {
                                        SharedString::from(tr("（文档为空）"))
                                    } else {
                                        latest
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(Color::Secondary.get(cx))
                            .child(tr("你的草稿 · 对照上方版本编辑后保存")),
                    )
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(Color::Muted.get(cx))
                        .child(tr("草稿保留在本机；点击“保存任务”后才会更新团队文档。")),
                )
                .child(Reveal::new(
                    "task-document-editor-reveal",
                    scroll,
                    Textarea::new(&draft.input)
                        .aria_label(tr("任务文档 Markdown"))
                        .disabled(draft.saving),
                ))
                .when_some(
                    draft.error.as_ref().map(|(key, error)| {
                        format!(
                            "{}: {}",
                            tr(key),
                            crate::language::localize_owned_error(error)
                        )
                    }),
                    |view, error| {
                        view.child(
                            div()
                                .id("task-draft-error")
                                .role(Role::Alert)
                                .aria_label(error.clone())
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(error),
                        )
                    },
                )
                .child(Reveal::new(
                    "task-draft-actions-reveal",
                    scroll,
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(
                            Button::new("save-tasks")
                                .primary()
                                .small()
                                .rounded_lg()
                                .label(if draft.saving {
                                    tr("正在保存…")
                                } else if draft.conflicting_content.is_some() {
                                    tr("用当前草稿更新文档")
                                } else {
                                    tr("保存任务")
                                })
                                .disabled(!self.connected || draft.saving)
                                .on_click(cx.listener(|this, _, _, cx| this.save_tasks(cx))),
                        )
                        .child(
                            Button::new("copy-task-draft")
                                .outline()
                                .small()
                                .rounded_lg()
                                .label(tr("复制草稿"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        input.read(cx).value().to_string(),
                                    ))
                                })),
                        )
                        .child(
                            Button::new("cancel-task-edit")
                                .ghost()
                                .small()
                                .rounded_lg()
                                .label(tr("取消编辑（丢弃改动）"))
                                .disabled(draft.saving)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.discard_task_draft(cx)
                                })),
                        ),
                ))
            })
            .when(draft.is_none(), |view| {
                view.when(self.collaboration.task_loading, |view| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(tr("正在读取任务…")),
                    )
                })
                .when_some(
                    self.collaboration.task_error.as_ref().map(|(key, error)| {
                        format!(
                            "{}: {}",
                            tr(key),
                            crate::language::localize_owned_error(error)
                        )
                    }),
                    |view, error| {
                        view.child(
                            div()
                                .px_3()
                                .py_2()
                                .rounded_lg()
                                .bg(Color::ErrorSurface.get(cx))
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(error),
                        )
                    },
                )
                .when(
                    !self.collaboration.task_loading
                        && self.collaboration.task_error.is_none()
                        && self.collaboration.task_text.is_empty(),
                    |view| {
                        view.child(
                            div()
                                .py_4()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(tr("任务文档为空")),
                        )
                    },
                )
                .when(
                    self.collaboration.task_expanded && !self.collaboration.task_text.is_empty(),
                    |view| {
                        view.child(
                            div()
                                .id("task-document-body")
                                .role(Role::Label)
                                .aria_label(
                                    self.collaboration
                                        .tasks
                                        .read(cx)
                                        .rendered_text()
                                        .as_str()
                                        .to_string(),
                                )
                                .p_3p5()
                                .rounded_lg()
                                .bg(Color::Inset.get(cx))
                                .border_1()
                                .border_color(Color::Divider.get(cx))
                                .child(TextView::new(&self.collaboration.tasks).selectable(true)),
                        )
                    },
                )
            })
    }
}
