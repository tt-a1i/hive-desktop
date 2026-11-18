use crate::language::{english,tr};
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
async fn prepare_preview(data: String) -> Result<ImagePreview, String> {
    static DECODERS: smol::lock::Semaphore = smol::lock::Semaphore::new(2);
    let permit = DECODERS.acquire().await;
    smol::unblock(move || {
        // The blocking work owns the permit even if its caller leaves the chat.
        let _permit = permit;
        preview(data)
    }).await
}

fn preview(data: String) -> Result<ImagePreview, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|_| "图片内容无法解码".to_owned())?;
    let decoded = decode_image(&bytes)?.thumbnail(144, 112);
    let format = match image::guess_format(&bytes).map_err(|error| error.to_string())? {
        image::ImageFormat::Png => ImageFormat::Png,
        image::ImageFormat::Jpeg => ImageFormat::Jpeg,
        image::ImageFormat::Gif => ImageFormat::Gif,
        image::ImageFormat::WebP => ImageFormat::Webp,
        _ => return Err("不支持的图片格式。".into()),
    };
    Ok(ImagePreview {
        render: render_image(decoded),
        original: Arc::new(Image::from_bytes(format, bytes)),
    })
}
pub(crate) fn render_image(image: image::DynamicImage) -> Arc<RenderImage> {
    let mut pixels = image.into_rgba8();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Arc::new(RenderImage::new([image::Frame::new(pixels)]))
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
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        self.failed.retain(|id, _| ids.contains(id.as_str()));
        self.verified.retain(|id| ids.contains(id.as_str()));
    }
    fn retain_previews(&mut self, active: Option<&str>) -> Vec<Arc<RenderImage>> {
        let items = active.and_then(|key| self.drafts.get(key));
        let mut released = Vec::new();
        self.previews.retain(|id, image| {
            let keep = items.is_some_and(|items| items.iter().any(|item| item.id == *id));
            if !keep {
                released.push(image.render.clone());
            }
            keep
        });
        released
    }
    fn contains(&self, key: Option<&str>, attachment: &AttachmentRef) -> bool {
        key.and_then(|key| self.drafts.get(key)).is_some_and(|items| {
            items.iter().any(|item| item.id == attachment.id && item.workspace_id == attachment.workspace_id)
        })
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
    pub(crate) fn release_attachment_previews(&mut self, keep_active: bool, cx: &mut Context<Self>) {
        let active = if keep_active { self.draft_target.as_deref() } else { None };
        let released = self.attachments.retain_previews(active);
        if !released.is_empty() {
            cx.defer(move |cx| {
                for image in released {
                    cx.drop_image(image, None);
                }
            });
        }
    }

    pub(crate) fn prune_draft_attachments(&mut self, cx: &mut Context<Self>) {
        self.attachments.prune();
        self.release_attachment_previews(true, cx);
    }

    pub(crate) fn choose_images(&mut self, cx: &mut Context<Self>) {
        let target = self.draft_target.clone();
        let workspace = self.workspace.clone();
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr("选择图片、文档或其他文件").into()),
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
                    this.error = Some(crate::app::ErrorNotice::Detail("选择文件失败",format!("{error:?}")));
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
        cx.notify();
        cx.spawn(async move |this, cx| {
            let mut ready = Vec::new();
            let mut errors: Vec<crate::app::ErrorNotice> = Vec::new();
            for source in sources {
                let (method, mut params) = match smol::unblock(move || source.prepare()).await {
                    Ok(prepared) => prepared,
                    Err(error) => {
                        errors.push(error.into());
                        continue;
                    }
                };
                params["workspaceId"] = json!(workspace);
                match runtime.request(method, params).await.and_then(|value| {
                    serde_json::from_value::<StagedImage>(value).map_err(|error| error.to_string())
                }) {
                    Ok(item) => {
                        let image = match item.image {
                            Some(image) => prepare_preview(image.data).await.map(Some),
                            None => Ok(None),
                        };
                        match image {
                            Ok(image) => ready.push((item.attachment, image)),
                            Err(error) => errors.push(error.into()),
                        }
                    }
                    Err(error) => errors.push(error.into()),
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
                let active = this.draft_target.as_ref() == Some(&key);
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
                            if active {
                                this.attachments.previews.insert(item.id.clone(), image);
                            }
                        } else {
                            this.attachments.verified.insert(item.id.clone());
                        }
                        items.push(item);
                    }
                }
                if !errors.is_empty() {
                    this.error = Some(crate::app::ErrorNotice::Batch(errors));
                }
                this.schedule_draft_save(cx);
                cx.notify();
            });
        })
        .detach();
    }
    pub(crate) fn ensure_attachment_previews(&mut self, cx: &mut Context<Self>) {
        static PREVIEW_READS: smol::lock::Semaphore = smol::lock::Semaphore::new(2);
        if !self.connected {
            return;
        }
        let Some(runtime) = self.runtime.clone() else {
            return;
        };
        let refs = self.attachments.refs(self.draft_target.as_ref());
        let connection = self.connection_generation;
        for item in refs {
            if self.attachments.previews.contains_key(&item.id)
                || self.attachments.verified.contains(&item.id)
                || self.attachments.failed.contains_key(&item.id)
                || !self.attachments.preparing.insert(item.id.clone())
            {
                continue;
            }
            let runtime = runtime.clone();
            cx.spawn(async move |this, cx| {
                let _read = PREVIEW_READS.acquire().await;
                let wanted = this.update(cx, |this, _| {
                    if this.connection_generation != connection { return false; }
                    if this.attachments.contains(this.draft_target.as_deref(), &item) { return true; }
                    this.attachments.preparing.remove(&item.id);
                    false
                }).unwrap_or(false);
                if !wanted { return; }
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
                            Some(image) => prepare_preview(image.data).await.map(Some),
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
                    if !this.attachments.contains(this.draft_target.as_deref(), &item) {
                        // The draft or this attachment can leave the window
                        // while its RPC or decoder still owns the result.
                        return;
                    }
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
    pub(crate) fn render_input_attachments(&self, cx: &Context<Self>) -> Option<AnyElement> {
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
                            .child(tr("正在保存附件…")),
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
                let name = item.name.clone();
                let size = item.size_label();
                let is_image = item.is_image();
                v_flex()
                    .w(px(116.))
                    .gap_1()
                    .p_1p5()
                    .rounded_lg()
                    .bg(Color::Inset.get(cx))
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
                                .custom(crate::palette::quiet_icon_variant(cx))
                                .w_full()
                                .h(px(64.))
                                .p_0()
                                .rounded_md()
                                .overflow_hidden()
                                .bg(Color::Card.get(cx))
                                .accessibility_label(format!("{}: {}", tr("查看图片"), name))
                                .child(
                                    img(image.render)
                                        .w_full()
                                        .h(px(64.))
                                        .object_fit(ObjectFit::Contain),
                                )
                                .on_click(cx.listener({
                                    let original = image.original;
                                    let name = name.clone();
                                    move |this, _, window, cx| {
                                        this.open_image(original.clone(), name.clone(), window, cx)
                                    }
                                })),
                            )
                        },
                    )
                    .when(
                        is_image && !self.attachments.previews.contains_key(&item.id),
                        |card| {
                            card.child(
                                div()
                                    .w_full()
                                    .h(px(64.))
                                    .rounded_md()
                                    .bg(Color::Card.get(cx))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .px_1()
                                    .text_xs()
                                    .text_color(if self.attachments.failed.contains_key(&item.id) {
                                        Color::Error.get(cx)
                                    } else {
                                        Color::Muted.get(cx)
                                    })
                                    .child(if self.attachments.failed.contains_key(&item.id) {
                                        tr("预览失败")
                                    } else {
                                        tr("读取图片…")
                                    }),
                            )
                        },
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .items_center()
                            .justify_between()
                            .gap_1()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_xs()
                                            .w_full()
                                            .truncate()
                                            .text_color(Color::Secondary.get(cx))
                                            .child(name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.5))
                                            .text_color(Color::Muted.get(cx))
                                            .child(size),
                                    ),
                            )
                            .child(
                                Button::new(SharedString::from(format!("remove-image-{}", item.id)))
                                    .custom(crate::palette::subtle_icon_variant(cx))
                                    .compact()
                                    .h(px(22.))
                                    .w(px(22.))
                                    .rounded_md()
                                    .icon(Glyph::X)
                                    .accessibility_label(if english() {
                                        format!(
                                            "Remove {}: {}",
                                            if is_image { tr("图片") } else { tr("附件") },
                                            name
                                        )
                                    } else {
                                        format!(
                                            "移除{} {}",
                                            if is_image { tr("图片") } else { tr("附件") },
                                            name
                                        )
                                    })
                                    .disabled(busy)
                                    .on_click(cx.listener({
                                        let remove_id = id.clone();
                                        move |this, _, _, cx| {
                                            if let Some(key) = &key {
                                                if let Some(items) =
                                                    this.attachments.drafts.get_mut(key)
                                                {
                                                    items.retain(|item| item.id != remove_id);
                                                }
                                            }
                                            this.prune_draft_attachments(cx);
                                            this.schedule_draft_save(cx);
                                            cx.notify();
                                        }
                                    })),
                            ),
                    )
                    .when_some(self.attachments.failed.get(&item.id), |card, error| {
                        card.child(
                            Button::new(SharedString::from(format!("reload-image-{}", item.id)))
                                .custom(crate::palette::subtle_icon_variant(cx))
                                .compact()
                                .h(px(22.))
                                .rounded_md()
                                .label(tr("重新读取"))
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
                    .into_any_element()
            }));
        if empty
            && !self
                .draft_target
                .as_ref()
                .is_some_and(|key| self.attachments.loading.contains(key))
        {
            return None;
        }
        Some(crate::text_reading::region(
            "draft-attachments".into(),
            tr("草稿附件，可用方向键或翻页键滚动").into(),
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
        .into_any_element())
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
        message.image_task = Some(cx.spawn(async move |this, cx| {
            let mut results = Vec::with_capacity(data.len());
            for data in data {
                results.push(prepare_preview(data).await);
            }
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
        }));
    }
}
