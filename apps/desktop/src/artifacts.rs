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

use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtifactSource {
    pub workspace_id: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dispatch_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_session_id: Option<String>,
}
impl ArtifactSource {
    pub(crate) fn report(
        workspace: &str,
        path: &str,
        dispatch: Option<&str>,
        agent: Option<&str>,
    ) -> Self {
        Self {
            workspace_id: workspace.into(),
            path: path.into(),
            dispatch_id: dispatch.filter(|v| !v.is_empty()).map(str::to_owned),
            source_agent_id: agent.filter(|v| !v.is_empty()).map(str::to_owned),
            source_session_id: None,
        }
    }
    pub(crate) fn session(workspace: &str, session: &str, path: &str) -> Self {
        Self {
            workspace_id: workspace.into(),
            path: path.into(),
            dispatch_id: None,
            source_agent_id: None,
            source_session_id: Some(session.into()),
        }
    }
}
#[derive(Clone)]
pub(crate) struct ArtifactTarget {
    pub runtime: Arc<crate::transport::Runtime>,
    pub owner: WeakEntity<HiveApp>,
    workspace: String,
    connection: u64,
}
impl ArtifactTarget {
    pub(crate) fn current(&self, app: &HiveApp) -> bool {
        app.connected
            && !app.closing
            && app.connection_generation == self.connection
            && app.workspace.as_ref() == Some(&self.workspace)
            && app
                .runtime
                .as_ref()
                .is_some_and(|runtime| Arc::ptr_eq(runtime, &self.runtime))
    }
    pub(crate) fn current_owner(&self, cx: &App) -> bool {
        self.owner
            .upgrade()
            .is_some_and(|owner| self.current(owner.read(cx)))
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactResolution {
    path: PathBuf,
    root: PathBuf,
    #[serde(default)]
    legacy: bool,
    warning: Option<String>,
    source_session_id: Option<String>,
}
pub(crate) struct ResolvedArtifact {
    pub path: PathBuf,
    pub root: Option<PathBuf>,
    pub warning: Option<String>,
}
fn validated_resolution(value: ArtifactResolution) -> Result<ResolvedArtifact, String> {
    if !value.root.is_absolute() || !value.path.is_absolute() {
        return Err(tr("产物没有返回有效的绝对路径和来源目录。").into());
    }
    let reported = value
        .path
        .to_str()
        .ok_or_else(|| tr("文件路径无效").to_owned())?;
    let path = artifact_path(&value.root, reported)?;
    let warning = value
        .warning
        .filter(|warning| !warning.trim().is_empty())
        .or_else(|| {
            value
                .legacy
                .then(|| tr("旧产物记录未保存执行来源，已按团队工作区解析。").to_owned())
        });
    Ok(ResolvedArtifact {
        path,
        root: Some(value.root),
        warning,
    })
}
pub(crate) async fn resolve_artifact(
    runtime: &Arc<crate::transport::Runtime>,
    source: &ArtifactSource,
) -> Result<ResolvedArtifact, String> {
    let params = serde_json::to_value(source).map_err(|error| error.to_string())?;
    let value = runtime.request("artifact.resolve", params).await?;
    let resolution: ArtifactResolution =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    if source
        .source_session_id
        .as_ref()
        .zip(resolution.source_session_id.as_ref())
        .is_some_and(|(expected, actual)| expected != actual)
    {
        return Err(tr("产物返回的会话来源不匹配。").into());
    }
    smol::unblock(move || validated_resolution(resolution)).await
}

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
    pub(crate) fn artifact_target(
        &self,
        workspace: &str,
        cx: &Context<Self>,
    ) -> Option<ArtifactTarget> {
        if !self.connected || self.closing || self.workspace.as_deref() != Some(workspace) {
            return None;
        }
        Some(ArtifactTarget {
            runtime: self.runtime.clone()?,
            owner: cx.entity().downgrade(),
            workspace: workspace.into(),
            connection: self.connection_generation,
        })
    }
    pub(crate) fn artifact_link(
        &self,
        owner: &str,
        index: usize,
        path: &str,
        dispatch_id: Option<&str>,
        source_agent_id: Option<&str>,
        cx: &Context<Self>,
    ) -> impl IntoElement + use<> {
        let source = self
            .workspace
            .as_ref()
            .map(|workspace| ArtifactSource::report(workspace, path, dispatch_id, source_agent_id));
        let target = source
            .as_ref()
            .and_then(|source| self.artifact_target(&source.workspace_id, cx));
        let preview_source = source.clone();
        let preview_target = target.clone();
        let unavailable = target.is_none();
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let (Some(source), Some(target)) = (source.clone(), target.clone()) else {
                            return;
                        };
                        if !target.current(this) {
                            return;
                        }
                        cx.spawn(async move |this, cx| {
                            let result = resolve_artifact(&target.runtime, &source).await;
                            let _ = this.update(cx, |this, cx| {
                                if !target.current(this) {
                                    return;
                                }
                                match result {
                                    Ok(resolved) => {
                                        if let Some(warning) = resolved.warning {
                                            this.error = Some(
                                                format!(
                                                    "{}: {}",
                                                    tr("产物来源提示"),
                                                    crate::language::localize_owned_error(&warning)
                                                )
                                                .into(),
                                            );
                                            cx.notify();
                                        }
                                        cx.reveal_path(&resolved.path);
                                    }
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
                    if let (Some(source), Some(target)) =
                        (preview_source.clone(), preview_target.clone())
                    {
                        this.open_artifact_preview(source, target, window, cx);
                    }
                })),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtifactResolution, ArtifactSource, artifact_path, validated_resolution};

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
    #[test]
    fn source_serialization_preserves_dispatch_recipient_and_omits_absent_fields() {
        let report = serde_json::to_value(ArtifactSource::report(
            "workspace",
            "proof.txt",
            Some("dispatch"),
            Some("worker"),
        ))
        .unwrap();
        assert_eq!(report["workspaceId"], "workspace");
        assert_eq!(report["dispatchId"], "dispatch");
        assert_eq!(report["sourceAgentId"], "worker");
        assert!(report.get("sourceSessionId").is_none());
        let session = serde_json::to_value(ArtifactSource::session(
            "workspace",
            "original-session",
            "same.txt",
        ))
        .unwrap();
        assert_eq!(session["sourceSessionId"], "original-session");
        assert!(session.get("dispatchId").is_none());
        assert!(session.get("sourceAgentId").is_none());
        let legacy =
            serde_json::to_value(ArtifactSource::report("workspace", "old.txt", None, None))
                .unwrap();
        assert_eq!(legacy.as_object().unwrap().len(), 2);
    }
    #[test]
    fn resolved_artifacts_use_only_returned_root_and_make_legacy_visible() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("project");
        let worker = dir.path().join("worker");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&worker).unwrap();
        std::fs::write(original.join("same.txt"), "original").unwrap();
        std::fs::write(worker.join("same.txt"), "worker").unwrap();
        let resolution =
            |root: std::path::PathBuf, path: std::path::PathBuf, legacy| ArtifactResolution {
                root,
                path,
                legacy,
                warning: None,
                source_session_id: None,
            };
        let resolved =
            validated_resolution(resolution(worker.clone(), worker.join("same.txt"), false))
                .unwrap();
        assert_eq!(std::fs::read_to_string(resolved.path).unwrap(), "worker");
        assert!(
            validated_resolution(resolution(original.clone(), worker.join("same.txt"), false))
                .is_err()
        );
        assert!(
            validated_resolution(resolution(worker.clone(), "same.txt".into(), false)).is_err()
        );
        assert!(
            validated_resolution(resolution(
                "relative".into(),
                worker.join("same.txt"),
                false
            ))
            .is_err()
        );
        let old = validated_resolution(resolution(
            original.clone(),
            original.join("same.txt"),
            true,
        ))
        .unwrap();
        assert!(old.warning.is_some());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&original, worker.join("outside")).unwrap();
            assert!(
                validated_resolution(resolution(
                    worker.clone(),
                    worker.join("outside/same.txt"),
                    false
                ))
                .is_err()
            );
        }
        std::fs::remove_dir_all(&worker).unwrap();
        assert!(
            validated_resolution(resolution(worker.clone(), worker.join("same.txt"), false))
                .is_err()
        );
    }
}
