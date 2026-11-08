use super::AttachmentRef;
use crate::app::HiveApp;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    *,
};
use serde_json::json;
impl HiveApp {
    pub(crate) fn file_attachment_link(
        &self,
        owner: &str,
        item: AttachmentRef,
        cx: &Context<Self>,
    ) -> Button {
        let label = if owner == "draft" {
            "显示副本".to_owned()
        } else {
            format!("{} · {}", item.name, item.size_label())
        };
        let workspace = item.workspace_id.clone();
        Button::new(SharedString::from(format!("file-{owner}-{}", item.id)))
            .ghost()
            .compact()
            .min_w_0()
            .icon(Glyph::File)
            .max_w_full()
            .justify_start()
            .child(div().min_w_0().truncate().child(label))
            .tooltip(format!("在文件管理器中显示附件副本：{}", item.name))
            .accessibility_label(format!("显示附件副本 {}", item.name))
            .disabled(!self.connected)
            .on_click(cx.listener(move |this, _, _, cx| {
                let Some(runtime) = this.runtime.clone() else {
                    return;
                };
                let workspace = workspace.clone();
                let id = item.id.clone();
                let connection = this.connection_generation;
                cx.spawn(async move |this, cx| {
                    let result = runtime
                        .request(
                            "chat.attachments.path",
                            json!({"workspaceId":workspace,"id":id}),
                        )
                        .await
                        .and_then(|value| {
                            value["path"]
                                .as_str()
                                .map(std::path::PathBuf::from)
                                .ok_or_else(|| "附件没有返回本地路径。".to_owned())
                        });
                    let _ = this.update(cx, |this, cx| {
                        if this.connection_generation != connection
                            || this.workspace.as_ref() != Some(&workspace)
                        {
                            return;
                        }
                        match result {
                            Ok(path) => cx.reveal_path(&path),
                            Err(error) => {
                                this.error = Some(error);
                                cx.notify();
                            }
                        }
                    });
                })
                .detach();
            }))
    }
}
