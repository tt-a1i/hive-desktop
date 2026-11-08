mod files;
use crate::{app::HiveApp, palette::Color};
use base64::Engine;
use gpui_kit::{
    assets::IconName as Glyph,
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    io::Cursor,
    path::PathBuf,
    sync::Arc,
};

pub(crate) use crate::protocol::AttachmentRef;
impl AttachmentRef {
    pub fn is_image(&self) -> bool {
        matches!(
            self.mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        )
    }
    pub fn size_label(&self) -> String {
        if self.size < 1024 {
            format!("{} B", self.size)
        } else if self.size < 1024 * 1024 {
            format!("{:.1} KB", self.size as f64 / 1024.)
        } else {
            format!("{:.1} MB", self.size as f64 / 1048576.)
        }
    }
}
#[derive(Deserialize)]
struct ImageWire {
    data: String,
}
#[derive(Deserialize)]
struct StagedImage {
    attachment: AttachmentRef,
    image: Option<ImageWire>,
}
#[derive(Clone)]
pub(crate) struct ImagePreview {
    pub render: Arc<RenderImage>,
    pub original: Arc<Image>,
}
#[derive(Default)]
pub(crate) struct Attachments {
    pub scroll: ScrollHandle,
    pub drafts: HashMap<String, Vec<AttachmentRef>>,
    previews: HashMap<String, ImagePreview>,
    failed: HashMap<String, String>,
    pub loading: HashSet<String>,
    preparing: HashSet<String>,
    verified: HashSet<String>,
}
fn preview(data: String, renderer: SvgRenderer) -> Result<ImagePreview, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "图片内容无法解码".to_owned())?;
    let decoded = decode_image(&bytes)?.thumbnail(144, 112);
    let mut png = Cursor::new(Vec::new());
    decoded
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    let format = match image::guess_format(&bytes).map_err(|error| error.to_string())? {
        image::ImageFormat::Png => ImageFormat::Png,
        image::ImageFormat::Jpeg => ImageFormat::Jpeg,
        image::ImageFormat::Gif => ImageFormat::Gif,
        image::ImageFormat::WebP => ImageFormat::Webp,
        _ => return Err("不支持的图片格式。".into()),
    };
    let render = Image::from_bytes(ImageFormat::Png, png.into_inner())
        .to_image_data(renderer)
        .map_err(|error| error.to_string())?;
    Ok(ImagePreview {
        render,
        original: Arc::new(Image::from_bytes(format, bytes)),
    })
}
pub(crate) fn decode_image(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8000);
    limits.max_image_height = Some(8000);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|error| format!("无法读取图片：{error}"))
}

impl Attachments {
    pub fn prune(&mut self) {
        self.drafts.retain(|_, items| !items.is_empty());
        let ids = self
            .drafts
            .values()
            .flatten()
            .map(|item| item.id.clone())
            .collect::<HashSet<_>>();
        self.previews.retain(|id, _| ids.contains(id));
        self.failed.retain(|id, _| ids.contains(id));
        self.verified.retain(|id| ids.contains(id));
    }
    pub fn reset_connection(&mut self) {
        self.loading.clear();
        self.preparing.clear();
        self.failed.clear();
        self.verified.clear();
    }
    pub fn refs(&self, key: Option<&String>) -> Vec<AttachmentRef> {
        key.and_then(|key| self.drafts.get(key))
            .cloned()
            .unwrap_or_default()
    }
    pub fn ready(&self, key: Option<&String>) -> bool {
        key.is_none_or(|key| {
            !self.loading.contains(key)
                && self.drafts.get(key).is_none_or(|items| {
                    items.iter().all(|item| {
                        self.previews.contains_key(&item.id) || self.verified.contains(&item.id)
                    })
                })
        })
    }
    pub fn transfer(&mut self, from: &str, to: String) {
        if let Some(items) = self.drafts.remove(from) {
            self.drafts.insert(to, items);
        }
    }
}
impl HiveApp {
    pub(crate) fn choose_images(&mut self, cx: &mut Context<Self>) {
        let target = self.draft_target.clone();
        let workspace = self.workspace.clone();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("选择图片、文档或其他文件".into()),
        });
        cx.spawn(async move |this, cx| match receiver.await {
            Ok(Ok(Some(paths))) => {
                let _ = this.update(cx, |this, cx| {
                    if this.draft_target == target && this.workspace == workspace {
                        this.import_image_paths(paths, cx);
                    } else {
                        this.error = Some("会话已变化，请在目标会话重新选择附件。".into());
                        cx.notify();
                    }
                });
            }
            Ok(Ok(None)) => {}
            error => {
                let _ = this.update(cx, |this, cx| {
                    this.error = Some(format!("选择文件失败：{error:?}"));
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(crate) fn import_image_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.import_image_sources(
            paths
                .into_iter()
                .map(crate::image_input::ImageSource::File)
                .collect(),
            cx,
        );
    }
    pub(crate) fn import_image_sources(
        &mut self,
        sources: Vec<crate::image_input::ImageSource>,
        cx: &mut Context<Self>,
    ) {
        if sources.is_empty() {
            return;
        }
        let (Some(workspace), Some(key), Some(runtime)) = (
            self.workspace.clone(),
            self.draft_target.clone(),
            self.runtime.clone(),
        ) else {
            self.error = Some("请先选择项目，再添加附件。".into());
            cx.notify();
            return;
        };
        if !self.connected
            || self.closing
            || self.loading
            || !self.workspace_writes_allowed()
            || self
                .session
                .as_ref()
                .is_some_and(|session| self.inflight_inputs.contains_key(&session.id))
            || self.attachments.loading.contains(&key)
        {
            self.error = Some("当前会话暂不能添加附件，请稍后再试。".into());
            cx.notify();
            return;
        }
        if sources.len() + self.attachments.refs(Some(&key)).len() > 10 {
            self.error = Some("每条消息最多添加 10 个附件。".into());
            cx.notify();
            return;
        }
        self.attachments.loading.insert(key.clone());
        let connection = self.connection_generation;
        let renderer = cx.svg_renderer();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut ready = Vec::new();
            let mut errors = Vec::new();
            for source in sources {
                let (method, mut params) = match smol::unblock(move || source.prepare()).await {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
                params["workspaceId"] = json!(workspace);
                match runtime.request(method, params).await.and_then(|value| {
                    serde_json::from_value::<StagedImage>(value).map_err(|error| error.to_string())
                }) {
                    Ok(item) => {
                        let image = match item.image {
                            Some(image) => {
                                let render = renderer.clone();
                                smol::unblock(move || preview(image.data, render))
                                    .await
                                    .map(Some)
                            }
                            None => Ok(None),
                        };
                        match image {
                            Ok(image) => ready.push((item.attachment, image)),
                            Err(error) => errors.push(error),
                        }
                    }
                    Err(error) => errors.push(error),
                }
            }
            let _ = this.update(cx, |this, cx| {
                if this.connection_generation != connection {
                    return;
                }
                this.attachments.loading.remove(&key);
                if !this.workspaces.iter().any(|item| item.id == workspace) {
                    return;
                }
                let items = this.attachments.drafts.entry(key).or_default();
                for (item, image) in ready {
                    if items.len() < 10 && !items.iter().any(|existing| existing.id == item.id) {
                        let total = items
                            .iter()
                            .filter(|item| item.is_image())
                            .map(|item| ((item.size + 2) / 3) * 4)
                            .sum::<u64>()
                            + ((item.size + 2) / 3) * 4;
                        if item.is_image() && total > 20_000_000 {
                            errors.push("本条消息图片编码后合计不能超过 20 MB。".into());
                            continue;
                        }
                        if let Some(image) = image {
                            this.attachments.previews.insert(item.id.clone(), image);
                        } else {
                            this.attachments.verified.insert(item.id.clone());
                        }
                        items.push(item);
                    }
                }
                if !errors.is_empty() {
                    this.error = Some(errors.join("\n"));
                }
                this.schedule_draft_save(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn ensure_attachment_previews(&mut self, cx: &mut Context<Self>) {
        if !self.connected {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let refs = self.attachments.refs(self.draft_target.as_ref());
        let connection = self.connection_generation;
        let renderer = cx.svg_renderer();
        for item in refs {
            if self.attachments.previews.contains_key(&item.id)
                || self.attachments.verified.contains(&item.id)
                || self.attachments.failed.contains_key(&item.id)
                || !self.attachments.preparing.insert(item.id.clone())
            {
                continue;
            }
            let runtime = runtime.clone();
            let renderer = renderer.clone();
            cx.spawn(async move |this, cx| {
                let result = match runtime
                    .request(
                        "chat.attachments.get",
                        json!({"workspaceId":item.workspace_id,"ids":[item.id]}),
                    )
                    .await
                    .and_then(|value| {
                        serde_json::from_value::<Vec<StagedImage>>(value)
                            .map_err(|error| error.to_string())
                    }) {
                    Ok(mut images) => match images.pop() {
                        Some(item) => match item.image {
                            Some(image) => smol::unblock(move || preview(image.data, renderer))
                                .await
                                .map(Some),
                            None => Ok(None),
                        },
                        None => Err("附件没有返回内容。".into()),
                    },
                    Err(error) => Err(error),
                };
                let _ = this.update(cx, |this, cx| {
                    if this.connection_generation != connection {
                        return;
                    }
                    this.attachments.preparing.remove(&item.id);
                    match result {
                        Ok(Some(image)) => {
                            this.attachments.previews.insert(item.id, image);
                        }
                        Ok(None) => {
                            this.attachments.verified.insert(item.id);
                        }
                        Err(error) => {
                            this.attachments.failed.insert(item.id, error);
                        }
                    }
                    cx.notify();
                });
            })
            .detach();
        }
    }
    pub(crate) fn render_input_attachments(&self, cx: &Context<Self>) -> impl IntoElement + use<> {
        let refs = self.attachments.refs(self.draft_target.as_ref());
        let busy = self
            .session
            .as_ref()
            .is_some_and(|session| self.inflight_inputs.contains_key(&session.id));
        let empty = refs.is_empty();
        let content = h_flex()
            .flex_shrink_0()
            .w_full()
            .min_w_0()
            .flex_wrap()
            .gap_2()
            .when(
                self.draft_target
                    .as_ref()
                    .is_some_and(|key| self.attachments.loading.contains(key)),
                |row| {
                    row.child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child("正在保存附件…"),
                    )
                },
            )
            .children(refs.into_iter().map(|item| {
                if !item.is_image() {
                    return self
                        .draft_file_attachment(item, busy, cx)
                        .into_any_element();
                }
                let key = self.draft_target.clone();
                let id = item.id.clone();
                v_flex()
                    .w(px(128.))
                    .gap_1()
                    .p_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .when_some(
                        self.attachments.previews.get(&item.id).cloned(),
                        |card, image| {
                            card.child(
                                Button::new(SharedString::from(format!(
                                    "view-draft-image-{}",
                                    item.id
                                )))
                                .ghost()
                                .w(px(112.))
                                .h(px(84.))
                                .accessibility_label(format!("查看图片 {}", item.name))
                                .child(
                                    img(image.render)
                                        .w(px(112.))
                                        .h(px(84.))
                                        .object_fit(ObjectFit::Contain),
                                )
                                .on_click(cx.listener({
                                    let original = image.original;
                                    let name = item.name.clone();
                                    move |this, _, window, cx| {
                                        this.open_image(original.clone(), name.clone(), window, cx)
                                    }
                                })),
                            )
                        },
                    )
                    .when(
                        item.is_image() && !self.attachments.previews.contains_key(&item.id),
                        |card| {
                            card.child(div().text_xs().child(
                                if self.attachments.failed.contains_key(&item.id) {
                                    "预览失败"
                                } else {
                                    "读取图片…"
                                },
                            ))
                        },
                    )
                    .child(div().text_xs().w_full().truncate().child(item.name.clone()))
                    .child(
                        div()
                            .text_xs()
                            .text_color(Color::Muted.get(cx))
                            .child(item.size_label()),
                    )
                    .when_some(self.attachments.failed.get(&item.id), |card, error| {
                        card.child(
                            Button::new(SharedString::from(format!("reload-image-{}", item.id)))
                                .ghost()
                                .compact()
                                .label("重新读取")
                                .tooltip(error.clone())
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        this.attachments.failed.remove(&id);
                                        cx.notify();
                                    }
                                })),
                        )
                    })
                    .child(
                        Button::new(SharedString::from(format!("remove-image-{}", item.id)))
                            .ghost()
                            .compact()
                            .icon(Glyph::X)
                            .accessibility_label(format!(
                                "移除{} {}",
                                if item.is_image() { "图片" } else { "附件" },
                                item.name
                            ))
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(key) = &key {
                                    if let Some(items) = this.attachments.drafts.get_mut(key) {
                                        items.retain(|item| item.id != id);
                                    }
                                }
                                this.attachments.prune();
                                this.schedule_draft_save(cx);
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }));
        if empty
            && !self
                .draft_target
                .as_ref()
                .is_some_and(|key| self.attachments.loading.contains(key))
        {
            return div().into_any_element();
        }
        crate::text_reading::region(
            "draft-attachments".into(),
            "草稿附件，可用方向键或翻页键滚动".into(),
            &self.attachments.scroll,
            px(192.),
            cx,
        )
        .w_full()
        .flex_shrink_0()
        .border_0()
        .rounded_none()
        .p_0()
        .child(content)
        .into_any_element()
    }
}

impl HiveApp {
    pub(crate) fn load_message_images(
        &mut self,
        id: String,
        source: &serde_json::Value,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = self.messages.iter_mut().find(|message| message.id == id) else {
            return;
        };
        if message.images_loading || !message.images.is_empty() || message.images_error.is_some() {
            return;
        }
        let data = source["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| block["type"] == "image")
            .filter_map(|block| block["data"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        if data.is_empty() {
            return;
        }
        message.images_loading = true;
        message.layout_revision += 1;
        let request = self.open_request;
        let renderer = cx.svg_renderer();
        cx.spawn(async move |this, cx| {
            let results = smol::unblock(move || {
                data.into_iter()
                    .map(|data| preview(data, renderer.clone()))
                    .collect::<Vec<_>>()
            })
            .await;
            let _ = this.update(cx, |this, cx| {
                if this.open_request != request {
                    return;
                }
                if let Some(message) = this.messages.iter_mut().find(|message| message.id == id) {
                    message.images_loading = false;
                    for result in results {
                        match result {
                            Ok(image) => {
                                message.images.push(image.render);
                                message.image_sources.push(image.original);
                            }
                            Err(error) => message.images_error = Some(error),
                        }
                    }
                    message.layout_revision += 1;
                }
                cx.notify();
            });
        })
        .detach();
    }
}
