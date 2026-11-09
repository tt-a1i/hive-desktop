mod snapshot;
mod view;
use crate::{
    app::HiveApp,
    language::{english, tr},
};
use gpui_kit::{component::*, *};
use snapshot::Snapshot;
use std::path::PathBuf;

struct ConversationExport {
    snapshot: Snapshot,
    directory: PathBuf,
    thinking: bool,
    tools: bool,
    text: String,
    scroll: ScrollHandle,
    saving: bool,
    notice: Option<ExportNotice>,
    error: Option<(&'static str, String)>,
}
struct ExportFooter {
    export: Entity<ConversationExport>,
}
enum ExportNotice {
    Saved(PathBuf),
    Cancelled,
    Copied,
}
impl ExportNotice {
    fn text(&self) -> String {
        match self {
            Self::Saved(path) => format!("{}: {}", tr("已保存"), path.display()),
            Self::Cancelled => tr("已取消保存，快照仍保留。").to_owned(),
            Self::Copied => tr("已复制完整快照").to_owned(),
        }
    }
}
impl HiveApp {
    pub(crate) fn open_conversation_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.viewing_conversation()
            || !self.conversation_ready
            || self.loading
            || self.messages.is_empty()
            || window.has_active_dialog(cx)
        {
            return;
        }
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let workspace = self
            .workspaces
            .iter()
            .find(|item| item.id == session.workspace_id);
        let snapshot = Snapshot::capture(self, session, workspace);
        let directory = workspace
            .map(|item| PathBuf::from(&item.path))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let text = snapshot.markdown(false, false);
        let export = cx.new(|_| ConversationExport {
            snapshot,
            directory,
            text,
            thinking: false,
            tools: false,
            scroll: ScrollHandle::new(),
            saving: false,
            notice: None,
            error: None,
        });
        let footer = cx.new(|cx| {
            cx.observe(&export, |_, _, cx| cx.notify()).detach();
            ExportFooter {
                export: export.clone(),
            }
        });
        let width = (window.viewport_size().width - px(48.)).min(px(900.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(tr("导出对话"))
                .w(width)
                .child(export.clone())
                .footer(footer.clone())
        });
    }
}
impl ConversationExport {
    fn refresh(&mut self, cx: &mut Context<Self>) {
        self.text = self.snapshot.markdown(self.thinking, self.tools);
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.notice = None;
        self.error = None;
        cx.notify();
    }
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.saving = true;
        self.notice = None;
        self.error = None;
        let receiver = cx.prompt_for_new_path(&self.directory, Some(&self.snapshot.filename()));
        let text = self.text.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match receiver.await {
                Ok(Ok(Some(path))) => smol::unblock(move || {
                    crate::file_save::save_file(&path, text.as_bytes()).map(|_| path)
                })
                .await
                .map(Some)
                .map_err(|error| ("保存失败，快照仍保留", error)),
                Ok(Ok(None)) => Ok(None),
                error => Err(("无法选择保存位置", format!("{error:?}"))),
            };
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(Some(path)) => this.notice = Some(ExportNotice::Saved(path)),
                    Ok(None) => this.notice = Some(ExportNotice::Cancelled),
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
