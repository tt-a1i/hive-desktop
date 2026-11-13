use crate::file_save::save_file as save_original;
use crate::{app::HiveApp, palette::Color, language::tr};
use gpui_kit::{
    component::{
        button::{Button, ButtonVariants},
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::{io::Cursor, path::PathBuf, sync::Arc};

struct ImageViewer {
    original: Arc<Image>,
    name: String,
    directory: PathBuf,
    rendered: Option<Arc<RenderImage>>,
    dimensions: (u32, u32),
    fit: bool,
    saving: bool,
    notice: Option<&'static str>,
    error: Option<(&'static str,String)>,
}

impl HiveApp {
    pub(crate) fn open_image(
        &mut self,
        original: Arc<Image>,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let directory = self
            .workspaces
            .iter()
            .find(|item| Some(&item.id) == self.workspace.as_ref())
            .map(|item| PathBuf::from(&item.path))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let renderer = cx.svg_renderer();
        let viewer = cx.new(|cx| {
            let bytes = original.clone();
            cx.spawn(async move |this, cx| {
                let result = smol::unblock(move || {
                    // Render only the decoded first frame, with the same allocation
                    // limits as thumbnails. Do not let GPUI decode all GIF frames.
                    let decoded = crate::attachments::decode_image(&bytes.bytes)?;
                    let dimensions = (decoded.width(), decoded.height());
                    let mut png = Cursor::new(Vec::new());
                    decoded
                        .write_to(&mut png, image::ImageFormat::Png)
                        .map_err(|error| error.to_string())?;
                    let image = Image::from_bytes(ImageFormat::Png, png.into_inner())
                        .to_image_data(renderer)
                        .map_err(|error| error.to_string())?;
                    Ok::<_, String>((image, dimensions))
                })
                .await;
                let _ = this.update(cx, |this: &mut ImageViewer, cx| {
                    match result {
                        Ok((image, dimensions)) => {
                            this.rendered = Some(image);
                            this.dimensions = dimensions;
                        }
                        Err(error) => this.error = Some(("无法读取原图",error)),
                    }
                    cx.notify();
                });
            })
            .detach();
            ImageViewer {
                original,
                name: name.clone(),
                directory,
                rendered: None,
                dimensions: (0, 0),
                fit: true,
                saving: false,
                notice: None,
                error: None,
            }
        });
        let width = (window.viewport_size().width - px(48.)).min(px(960.));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog.title(name.clone()).w(width).child(viewer.clone())
        });
    }
}

impl ImageViewer {
    fn save(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.saving = true;
        self.notice = None;
        self.error = None;
        let receiver = cx.prompt_for_new_path(&self.directory, Some(&self.name));
        let original = self.original.clone();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = match receiver.await {
                Ok(Ok(Some(path))) => {
                    smol::unblock(move || save_original(&path, &original.bytes).map(|_| true)).await.map_err(|error|("保存失败",error))
                }
                Ok(Ok(None)) => Ok(false),
                error => Err(("无法选择保存位置",format!("{error:?}"))),
            };
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(true) => this.notice = Some("已保存原图".into()),
                    Ok(false) => {}
                    Err(error) => this.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ImageViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let height = (window.viewport_size().height - px(250.))
            .max(px(100.))
            .min(px(620.));
        v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("image-fit")
                            .ghost()
                            .label(if self.fit {
                                tr("原尺寸")
                            } else {
                                tr("适应窗口")
                            })
                            .disabled(self.rendered.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.fit = !this.fit;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("copy-original-image")
                            .ghost()
                            .label(tr("复制原图"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_image(&this.original));
                                this.notice = Some("已复制原图");
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("save-original-image")
                            .ghost()
                            .label(if self.saving {
                                tr("正在保存…")
                            } else {
                                tr("另存为…")
                            })
                            .disabled(self.saving)
                            .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    )
                    .when(self.dimensions.0 > 0, |row| {
                        row.child(
                            div()
                                .id("image-dimensions")
                                .role(Role::Label)
                                .aria_label(format!("{} × {}", self.dimensions.0, self.dimensions.1))
                                .text_xs()
                                .text_color(Color::Muted.get(cx))
                                .child(format!("{} × {}", self.dimensions.0, self.dimensions.1)),
                        )
                    }),
            )
            .child(
                div()
                    .id("image-viewport")
                    .w_full()
                    .min_w_0()
                    .h(height)
                    .overflow_scroll()
                    .rounded_xl()
                    .border_1()
                    .border_color(Color::Divider.get(cx))
                    .bg(Color::Card.get(cx))
                    .when_some(self.rendered.clone(), |view, image| {
                        view.child(if self.fit {
                            img(image)
                                .w_full()
                                .h(height)
                                .object_fit(ObjectFit::Contain)
                                .into_any_element()
                        } else {
                            img(image)
                                .w(px(self.dimensions.0 as f32 / window.scale_factor()))
                                .h(px(self.dimensions.1 as f32 / window.scale_factor()))
                                .into_any_element()
                        })
                    })
                    .when(self.rendered.is_none() && self.error.is_none(), |view| {
                        view.child(div().id("image-loading").role(Role::Label).aria_label(tr("正在读取原图…")).p_4().child(tr("正在读取原图…")))
                    }),
            )
            .when_some(self.notice.map(tr), |view, text| {
                view.child(div().id("image-notice").role(Role::Label).aria_label(text).text_xs().child(text))
            })
            .when_some(self.error.as_ref().map(|(key, error)| format!("{}: {}", tr(key), crate::language::localize_owned_error(error))), |view, text| {
                view.child(div().id("image-error").role(Role::Label).aria_label(text.clone()).text_xs().text_color(Color::Error.get(cx)).child(text))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::save_original;
    #[test]
    fn save_preserves_original_bytes_and_failed_save_preserves_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("截图.png");
        std::fs::write(&path, b"old image").unwrap();
        let original = b"immutable original image bytes";
        save_original(&path, original).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let existing_directory = directory.path().join("folder");
        std::fs::create_dir(&existing_directory).unwrap();
        let sentinel = existing_directory.join("keep.txt");
        std::fs::write(&sentinel, b"preserved").unwrap();
        assert!(save_original(&existing_directory, b"replacement").is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"preserved");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
