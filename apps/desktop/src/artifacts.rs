use crate::language::tr;
use crate::{app::HiveApp, palette::Color};
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    *,
};
use std::path::{Path, PathBuf};

// Reports may contain absolute paths as well as paths relative to the workspace.
// The shell receives the original spelling (including the user's symlink names).
pub(crate) fn artifact_path(root: &Path, reported: &str) -> Result<PathBuf, String> {
    if reported.trim().is_empty() || reported.chars().any(char::is_control) {
        return Err("文件路径无效".into());
    }
    let path = Path::new(reported);
    // Avoid resolving remote or device paths supplied by a report.
    if reported.starts_with("//") || reported.starts_with(r"\\") {
        return Err("不支持远程或设备路径".into());
    }
    let path = root.join(path);
    let canonical_root = root
        .canonicalize()
        .map_err(|e| format!("无法读取工作区：{e}"))?;
    let resolved = path
        .canonicalize()
        .map_err(|e| format!("无法定位文件 {reported}：{e}"))?;
    if !resolved.starts_with(canonical_root) {
        return Err(format!("文件位于工作区之外：{reported}"));
    }
    Ok(path)
}

impl HiveApp {
    pub(crate) fn artifact_link(
        &self,
        owner: &str,
        index: usize,
        path: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let file = path.to_owned();
        let preview = file.clone();
        let project = self
            .workspaces
            .iter()
            .find(|w| self.workspace.as_ref() == Some(&w.id));
        let root = project.map(|w| PathBuf::from(&w.path));
        let workspace = self.workspace.clone();
        let unavailable = root.is_none();
        let label = if cfg!(target_os = "macos") {
            tr("在访达中显示")
        } else {
            tr("在文件资源管理器中显示")
        };
        let detail = if crate::language::english() {
            format!("{label}: {path}")
        } else {
            format!("{label}：{path}")
        };
        h_flex()
            .w_full()
            .min_w_0()
            .items_center()
            .gap_1p5()
            .px_1p5()
            .py_1()
            .rounded_lg()
            .bg(Color::Inset.get(cx))
            .border_1()
            .border_color(Color::Divider.get(cx))
            .child(
                Button::new(SharedString::from(format!("artifact-{owner}-{index}")))
                    .custom(crate::palette::quiet_icon_variant(cx))
                    .compact()
                    .h(px(24.))
                    .px_1p5()
                    .rounded_md()
                    .min_w_0()
                    .flex_1()
                    .justify_start()
                    .accessibility_label(detail.clone())
                    .tooltip(detail)
                    .child(
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_1p5()
                            .child(
                                Icon::new(Glyph::FolderOpen)
                                    .size(px(13.))
                                    .text_color(Color::Muted.get(cx)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .font_family(Theme::global(cx).mono_font_family.clone())
                                    .text_color(Color::Secondary.get(cx))
                                    .child(path.to_owned()),
                            ),
                    )
                    .disabled(unavailable)
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let Some(root) = root.clone() else { return };
                        let file = file.clone();
                        let workspace = workspace.clone();
                        cx.spawn(async move |this, cx| {
                            let result = smol::unblock(move || artifact_path(&root, &file)).await;
                            let _ = this.update(cx, |this, cx| {
                                if this.workspace != workspace {
                                    return;
                                }
                                match result {
                                    Ok(path) => cx.reveal_path(&path),
                                    Err(error) => {
                                        this.error = Some(error.into());
                                        cx.notify();
                                    }
                                }
                            });
                        })
                        .detach();
                    })),
            )
            .child(
                Button::new(SharedString::from(format!(
                    "preview-artifact-{owner}-{index}"
                )))
                .custom(crate::palette::subtle_icon_variant(cx))
                .compact()
                .h(px(24.))
                .px_2()
                .rounded_md()
                .icon(Glyph::Eye)
                .label(tr("预览"))
                .accessibility_label(format!("{}: {path}", tr("预览文件")))
                .disabled(unavailable)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_file_preview(preview.clone(), window, cx)
                })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::artifact_path;

    #[test]
    fn resolves_reported_paths_only_inside_the_current_project() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("项目");
        std::fs::create_dir(&root).unwrap();
        let file = root.join("报告 1.txt");
        std::fs::write(&file, "report").unwrap();
        assert_eq!(artifact_path(&root, "报告 1.txt").unwrap(), file);
        assert_eq!(artifact_path(&root, file.to_str().unwrap()).unwrap(), file);
        assert!(artifact_path(&root, "已删除.txt").is_err());
        assert!(artifact_path(&root, "..").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temp.path(), root.join("outside")).unwrap();
            assert!(artifact_path(&root, "outside").is_err());
        }
    }
}
