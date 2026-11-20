use super::{Change, GitWorkspace, Status, git_params};
use crate::{
    language::tr,
    palette::Color,
    project_files::{FileError, FilePreviewMode, ProjectFilePreviewText},
    text_reading::TextPages,
};
use gpui_kit::{
    component::{button::Button, *},
    prelude::FluentBuilder,
    *,
};
use serde_json::json;

pub(super) enum Read {
    Loading(Option<ProjectFilePreviewText>),
    Ready(ProjectFilePreviewText),
    Failed(FileError),
}

pub(super) fn live_path<'a>(status: &Status, change: &'a Change) -> Option<&'a str> {
    change.reveal_path().or_else(|| {
        // A staged removal and a recreated worktree file can have separate
        // status rows for the same exact path.
        (change.index_status == "D"
            && change.path_role.as_deref() != Some("rename-source")
            && status.entries.iter().any(|entry| {
                entry.kind == "untracked" && entry.reveal_path() == Some(change.path.as_str())
            }))
        .then_some(change.path.as_str())
    })
}

impl GitWorkspace {
    pub(super) fn current_file_path(&self) -> Option<&str> {
        live_path(self.status.as_ref()?, self.selected_change()?)
    }

    pub(super) fn clear_current_file(&mut self) {
        self.current_file_generation += 1;
        self.current_file_task = None;
        self.current_file = None;
    }

    pub(super) fn set_current_file_view(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_current_file == show || !self.current(cx) || self.status_loading {
            return;
        }
        if show && (self.preview_mock || self.current_file_path().is_none()) {
            return;
        }
        self.show_current_file = show;
        if show {
            self.diff_find = None;
            self.load_current_file(cx);
        } else if !show && self.diff.is_none() && !self.diff_loading && self.diff_error.is_none() {
            self.load_diff(cx);
        }
        cx.notify();
    }

    pub(super) fn load_selected_change(&mut self, cx: &mut Context<Self>) {
        if self.show_current_file {
            if !matches!(self.current_file, Some(Read::Loading(_))) {
                self.load_current_file(cx);
            }
        } else {
            self.load_diff(cx);
        }
    }

    pub(super) fn load_current_file(&mut self, cx: &mut Context<Self>) {
        if !self.show_current_file || self.preview_mock || !self.current(cx) || self.status_loading
        {
            return;
        }
        let Some(path) = self.current_file_path().map(str::to_owned) else {
            return;
        };
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let root = self
            .status
            .as_ref()
            .expect("current Git status")
            .identity
            .project_root
            .clone();
        let params = git_params(
            json!({"workspaceId": self.workspace, "relativePath": path}),
            &self.session,
        );
        let previous = match self.current_file.take() {
            Some(Read::Ready(reader)) => Some(reader),
            Some(Read::Loading(previous)) => previous,
            _ => None,
        };
        self.clear_current_file();
        let generation = self.current_file_generation;
        self.current_file = Some(Read::Loading(previous));
        self.current_file_task = Some(cx.spawn(async move |this, cx| {
            let result = runtime
                .request("project.files.preview", params)
                .await
                .and_then(|value| {
                    value["text"]
                        .as_str()
                        .map(|text| TextPages::new(text.to_owned().into()))
                        .ok_or_else(|| tr("读取失败").to_owned())
                });
            let _ = this.update(cx, |this, cx| {
                if this.current_file_generation != generation
                    || !this.current(cx)
                    || this.current_file_path() != Some(path.as_str())
                    || this
                        .status
                        .as_ref()
                        .is_none_or(|status| status.identity.project_root != root)
                {
                    return;
                }
                this.current_file_task = None;
                let previous = match this.current_file.take() {
                    Some(Read::Loading(previous)) => previous,
                    _ => None,
                };
                this.current_file = Some(match result {
                    Ok(text) => Read::Ready(
                        previous
                            .filter(|reader| reader.has_text(&text))
                            .unwrap_or_else(|| ProjectFilePreviewText::new(&path, text, cx)),
                    ),
                    Err(error) => Read::Failed(FileError::preview(error)),
                });
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn set_current_file_format(&mut self, mode: FilePreviewMode, cx: &mut Context<Self>) {
        if let Some(Read::Ready(reader)) = &mut self.current_file {
            reader.set_mode(mode, cx);
            cx.notify();
        }
    }

    fn step_current_file(&mut self, step: isize, cx: &mut Context<Self>) {
        if let Some(Read::Ready(reader)) = &mut self.current_file
            && reader.step(step)
        {
            cx.notify();
        }
    }

    pub(super) fn render_current_file(&self, height: Pixels, cx: &Context<Self>) -> AnyElement {
        let Some(path) = self.current_file_path() else {
            return div().into_any_element();
        };
        match &self.current_file {
            Some(Read::Ready(reader)) => reader
                .view(
                    path,
                    height,
                    Self::set_current_file_format,
                    Self::step_current_file,
                    cx,
                )
                .into_any_element(),
            Some(Read::Loading(_)) => div()
                .id("git-current-file-loading")
                .role(Role::Status)
                .text_xs()
                .text_color(Color::Secondary.get(cx))
                .p_3()
                .child(tr("正在读取…"))
                .into_any_element(),
            result => h_flex()
                .p_3()
                .gap_2()
                .items_center()
                .justify_between()
                .when_some(
                    result.as_ref().and_then(|state| match state {
                        Read::Failed(error) => Some(error.text()),
                        _ => None,
                    }),
                    |row, error| {
                        row.child(
                            div()
                                .id("git-current-file-error")
                                .role(Role::Status)
                                .text_xs()
                                .text_color(Color::Error.get(cx))
                                .child(error),
                        )
                    },
                )
                .child(
                    Button::new("git-read-current-file")
                        .outline()
                        .xsmall()
                        .label(if result.is_some() {
                            tr("重试")
                        } else {
                            tr("读取当前文件")
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.load_current_file(cx))),
                )
                .into_any_element(),
        }
    }
}
