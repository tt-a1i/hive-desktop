use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};
const LIMIT: u64 = 1024 * 1024;
#[cfg(test)]
fn read_file(root: &Path, reported: &str) -> Result<String, String> {
    read_text(&crate::artifacts::artifact_path(root, reported)?)
}
fn read_text(path: &Path) -> Result<String, String> {
    if !std::fs::metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("只能预览普通文本文件。".into());
    }
    let file = File::open(&path).map_err(|e| format!("无法读取文件：{e}"))?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("读取失败：{e}"))?;
    if bytes.len() as u64 > LIMIT {
        return Err("文件超过 1 MiB，暂不在窗口中预览；可在文件管理器中查看。".into());
    }
    if bytes.contains(&0) {
        return Err("文件包含二进制内容，无法作为文本预览。".into());
    }
    String::from_utf8(bytes).map_err(|_| "文件不是 UTF-8 文本，暂不支持此编码的预览。".into())
}
#[derive(Clone)]
enum FileSource {
    Workspace {
        root: PathBuf,
        reported: String,
    },
    Attachment {
        runtime: Arc<crate::transport::Runtime>,
        item: crate::attachments::AttachmentRef,
    },
}
impl FileSource {
    async fn path(self) -> Result<PathBuf, String> {
        match self {
            Self::Workspace { root, reported } => {
                smol::unblock(move || crate::artifacts::artifact_path(&root, &reported)).await
            }
            Self::Attachment { runtime, item } => {
                let value = runtime
                    .request(
                        "chat.attachments.path",
                        serde_json::json!({"workspaceId":item.workspace_id,"id":item.id}),
                    )
                    .await?;
                value["path"]
                    .as_str()
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .ok_or_else(|| "附件没有返回有效的本地路径。".into())
            }
        }
    }
    fn label(&self) -> String {
        match self {
            Self::Workspace { root, reported } => root.join(reported).display().to_string(),
            Self::Attachment { item, .. } => {
                format!("{} · {} · 上传副本", item.name, item.size_label())
            }
        }
    }
    fn description(&self) -> &'static str {
        match self {
            Self::Workspace { .. } => "只读预览 · 当前磁盘内容，不代表工具调用时的版本",
            Self::Attachment { .. } => "只读预览 · Hive 保存的附件副本，独立于原始文件",
        }
    }
}
struct FileViewer {
    source: FileSource,
    text: Option<String>,
    error: Option<String>,
    loading: bool,
    scroll: ScrollHandle,
}
impl HiveApp {
    pub(crate) fn open_file_preview(
        &mut self,
        reported: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(root) = self
            .workspaces
            .iter()
            .find(|w| Some(&w.id) == self.workspace.as_ref())
            .map(|w| PathBuf::from(&w.path))
        else {
            return;
        };
        let title = Path::new(&reported)
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or(&reported)
            .to_owned();
        open_viewer(title, FileSource::Workspace { root, reported }, window, cx);
    }
    pub(crate) fn open_attachment_preview(
        &mut self,
        item: crate::attachments::AttachmentRef,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.connected || self.workspace.as_ref() != Some(&item.workspace_id) {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        open_viewer(
            item.name.clone(),
            FileSource::Attachment { runtime, item },
            window,
            cx,
        );
    }
}
fn open_viewer(title: String, source: FileSource, window: &mut Window, cx: &mut Context<HiveApp>) {
    let viewer = cx.new(|cx| {
        let mut viewer = FileViewer {
            source,
            text: None,
            error: None,
            loading: false,
            scroll: ScrollHandle::new(),
        };
        viewer.load(cx);
        viewer
    });
    let width = (window.viewport_size().width - px(48.)).min(px(960.));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title(title.clone()).w(width).child(viewer.clone())
    });
}
impl FileViewer {
    fn load(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        let source = self.source.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match source.path().await {
                Ok(path) => smol::unblock(move || read_text(&path)).await,
                Err(error) => Err(error),
            };
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(text) => {
                        this.text = Some(text);
                        this.scroll.set_offset(point(px(0.), px(0.)));
                    }
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
impl Render for FileViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .min_w_0()
            .gap_3()
            .tab_group()
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Secondary.get(cx))
                    .child(self.source.description()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Color::Muted.get(cx))
                    .child(self.source.label()),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("reload-file-preview")
                            .ghost()
                            .compact()
                            .label(if self.loading {
                                "读取中…"
                            } else {
                                "重新读取"
                            })
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                    )
                    .child(
                        Button::new("copy-file-preview")
                            .ghost()
                            .compact()
                            .label("复制全文")
                            .disabled(self.text.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(text) = &this.text {
                                    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                                }
                            })),
                    )
                    .child(
                        Button::new("reveal-file-preview")
                            .ghost()
                            .compact()
                            .label("在文件管理器中显示")
                            .on_click(cx.listener(|this, _, _, cx| {
                                let source = this.source.clone();
                                cx.spawn(async move |this, cx| {
                                    let result = source.path().await;
                                    let _ = this.update(cx, |this, cx| match result {
                                        Ok(path) => cx.reveal_path(&path),
                                        Err(error) => {
                                            this.error = Some(error);
                                            cx.notify();
                                        }
                                    });
                                })
                                .detach();
                            })),
                    ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(Color::Error.get(cx))
                        .child(error),
                )
            })
            .when(
                self.text.is_some() && (self.loading || self.error.is_some()),
                |view| {
                    view.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child("仍显示上次成功读取的内容。"),
                    )
                },
            )
            .when_some(self.text.as_ref(), |view, text| {
                view.child(
                    crate::text_reading::region(
                        "file-preview-text".into(),
                        "文件内容，可用方向键或翻页键滚动".into(),
                        &self.scroll,
                        (window.viewport_size().height - px(300.))
                            .max(px(120.))
                            .min(px(520.)),
                        cx,
                    )
                    .font_family(Theme::global(cx).mono_font_family.clone())
                    .text_size(px(12.))
                    .line_height(px(19.))
                    .when(text.is_empty(), |view| {
                        view.child(
                            div()
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child("文件为空"),
                        )
                    })
                    .child(gpui_kit::base::SelectableText::new(
                        "file-preview-source",
                        text.clone(),
                    )),
                )
            })
    }
}

pub(crate) fn tool_paths(name: &str, args: &serde_json::Value) -> Vec<String> {
    let mut paths = Vec::new();
    if name == "fileChange" {
        for change in args["changes"].as_array().into_iter().flatten() {
            if let Some(path) = change["kind"]["move_path"]
                .as_str()
                .or_else(|| change["path"].as_str())
            {
                paths.push(path.to_owned());
            }
        }
    } else if matches!(
        name.to_ascii_lowercase().as_str(),
        "read" | "write" | "edit" | "multiedit" | "read_file" | "write_file"
    ) {
        if let Some(path) = args["file_path"].as_str().or_else(|| args["path"].as_str()) {
            paths.push(path.to_owned());
        }
    }
    let mut unique = Vec::new();
    for path in paths {
        if !path.trim().is_empty() && !unique.contains(&path) {
            unique.push(path);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::{LIMIT, read_file};
    #[test]
    fn reads_exact_utf8_and_refuses_missing_binary_directories_and_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let text = "第一行\r\n  第二行\n末尾 🐝\n";
        std::fs::write(root.join("报告 1.txt"), text).unwrap();
        assert_eq!(read_file(root, "报告 1.txt").unwrap(), text);
        assert_eq!(
            read_file(root, root.join("报告 1.txt").to_str().unwrap()).unwrap(),
            text
        );
        let outside = tempfile::tempdir().unwrap();
        let external = outside.path().join("outside.txt");
        std::fs::write(&external, "outside project").unwrap();
        assert!(read_file(root, external.to_str().unwrap()).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&external, root.join("outside-link")).unwrap();
            assert!(read_file(root, "outside-link").is_err());
            std::os::unix::fs::symlink(root.join("报告 1.txt"), root.join("inside-link")).unwrap();
            assert_eq!(read_file(root, "inside-link").unwrap(), text);
        }
        assert!(read_file(root, "missing.txt").is_err());
        assert!(read_file(root, ".").is_err());
        std::fs::write(root.join("binary"), [0, 1, 2]).unwrap();
        assert!(read_file(root, "binary").is_err());
        std::fs::write(root.join("encoding"), [255, 254, 255]).unwrap();
        assert!(read_file(root, "encoding").is_err());
        std::fs::write(root.join("large"), vec![b'x'; LIMIT as usize + 1]).unwrap();
        assert!(read_file(root, "large").is_err());
        std::fs::write(root.join("limit"), vec![b'x'; LIMIT as usize]).unwrap();
        assert_eq!(
            read_file(root, "limit").unwrap(),
            "x".repeat(LIMIT as usize)
        );
        std::fs::write(root.join("empty"), b"").unwrap();
        assert_eq!(read_file(root, "empty").unwrap(), "");
    }
}
