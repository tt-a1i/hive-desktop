use crate::app::HiveApp;
use gpui_kit::*;
use serde_json::json;

impl HiveApp {
    pub fn choose_workspace_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择项目文件夹".into()),
        });
        let window = window.window_handle();
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    if let Some(path) = path.to_str() {
                        let _ = window.update(cx, |_, window, cx| {
                            let _ = this.update(cx, |this, cx| {
                                if this.connected {
                                    this.folder
                                        .update(cx, |input, cx| input.set_value(path, window, cx));
                                    this.add_workspace(window, cx);
                                }
                            });
                        });
                    } else {
                        let _ = this.update(cx, |this, cx| {
                            this.error = Some("所选文件夹路径无法显示为文本".into());
                            cx.notify();
                        });
                    }
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("选择文件夹失败：{error}"));
                    cx.notify();
                });
            }
            Err(error) => {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("文件夹选择器已关闭：{error}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn open_workspace_target(
        &mut self,
        workspace: String,
        target: &'static str,
        cx: &mut Context<Self>,
    ) {
        if !self.connected {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let result = runtime.request("hive.request", json!({"method":"POST", "path":format!("/api/workspaces/{workspace}/open"), "body":{"target_id":target}})).await;
            let _ = this.update(cx, |this, cx| {
                if this.workspace.as_ref() == Some(&workspace) {
                    if let Err(error) = result { this.error = Some(format!("无法通过 {target} 打开项目，请确认目标应用已安装：{error}")); }
                    cx.notify();
                }
            });
        }).detach();
    }
}
