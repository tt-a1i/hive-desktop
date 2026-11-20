use crate::{attachments::render_image, language::tr, palette, project_files::FileError};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::{
    assets::IconName,
    component::{
        Sizable,
        button::{Button, ButtonVariants},
        scroll::{ScrollableElement, ScrollbarAxis},
    },
    *,
};
use image::{ImageDecoder, ImageFormat, ImageReader};
use serde::Deserialize;
use serde_json::Value;
use std::{io::Cursor, sync::Arc};

const MAX_ENCODED_BYTES: usize = 4 * 1024 * 1024;
const MAX_BASE64_BYTES: usize = 4 * MAX_ENCODED_BYTES.div_ceil(3);
const MAX_DIMENSION: u32 = 8192;
const MAX_PIXELS: u64 = 9_000_000;
const IMAGE_ERROR: &str = "项目文件图片无法显示。";
static DECODERS: smol::lock::Semaphore = smol::lock::Semaphore::new(2);

struct PreviewImage(Arc<RenderImage>);

impl PreviewImage {
    fn retain(render: Arc<RenderImage>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            cx.on_release(|image: &mut Self, cx| {
                let render = image.0.clone();
                cx.defer(move |cx| cx.drop_image(render, None));
            })
            .detach();
            Self(render)
        })
    }
}

pub(super) struct ImageViewport {
    image: Entity<PreviewImage>,
    actual_size: bool,
    scroll: ScrollHandle,
}

impl ImageViewport {
    pub(super) fn new(render: Arc<RenderImage>, cx: &mut App) -> Self {
        Self {
            image: PreviewImage::retain(render, cx),
            actual_size: false,
            scroll: ScrollHandle::new(),
        }
    }

    pub(super) fn expanded(&self) -> Self {
        let scroll = ScrollHandle::new();
        scroll.set_offset(self.scroll.offset());
        Self {
            image: self.image.clone(),
            actual_size: self.actual_size,
            scroll,
        }
    }

    pub(super) fn toggle_size(&mut self) {
        self.actual_size = !self.actual_size;
        self.scroll = ScrollHandle::new();
    }

    pub(super) fn size_button(&self, id: &'static str, cx: &App) -> Button {
        let label = if self.actual_size {
            tr("适应窗口")
        } else {
            tr("原尺寸")
        };
        let button = Button::new(id)
            .custom(palette::quiet_icon_variant(cx))
            .compact()
            .xsmall()
            .size(px(26.))
            .accessibility_label(label)
            .tooltip(label);
        if self.actual_size {
            button.icon(IconName::Minimize2)
        } else {
            button.label("1:1")
        }
    }

    pub(super) fn region(
        &self,
        id: &'static str,
        path: &str,
        height: Pixels,
        window: &Window,
        cx: &App,
    ) -> Stateful<Div> {
        let render = self.image.read(cx).0.clone();
        let view = div()
            .id(id)
            .role(Role::Image)
            .aria_label(format!("{}: {path}", tr("文件图片预览")))
            .w_full()
            .min_w_0()
            .h(height)
            .min_h_0()
            .tab_stop(true);
        if self.actual_size {
            let size = actual_image_size(render.size(0), window.scale_factor());
            let scroll = self.scroll.clone();
            view.focusable()
                .overflow_scroll()
                .track_scroll(&self.scroll)
                .scrollbar(&self.scroll, ScrollbarAxis::Both)
                .child(img(render).w(size.width).h(size.height).flex_none())
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.modifiers != Modifiers::default() {
                        return;
                    }
                    if let Some(offset) = image_scroll_offset(
                        event.keystroke.key.as_str(),
                        scroll.offset(),
                        scroll.max_offset(),
                    ) {
                        scroll.set_offset(offset);
                        window.refresh();
                        cx.stop_propagation();
                    }
                })
        } else {
            view.overflow_hidden()
                .child(img(render).size_full().object_fit(ObjectFit::Contain))
        }
    }
}

fn actual_image_size(size: Size<DevicePixels>, display_scale: f32) -> Size<Pixels> {
    size.map(|dimension| px(dimension.0 as f32 / display_scale))
}

fn image_scroll_offset(
    key: &str,
    offset: Point<Pixels>,
    maximum: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let step = px(36.);
    let delta = match key {
        "left" => point(step, px(0.)),
        "right" => point(-step, px(0.)),
        "up" => point(px(0.), step),
        "down" => point(px(0.), -step),
        _ => return None,
    };
    Some(point(
        (offset.x + delta.x).clamp(-maximum.x, px(0.)),
        (offset.y + delta.y).clamp(-maximum.y, px(0.)),
    ))
}

#[derive(Deserialize)]
struct WireImage {
    format: RasterFormat,
    data: String,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RasterFormat {
    Png,
    Jpeg,
}

impl RasterFormat {
    fn image_format(self) -> ImageFormat {
        match self {
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
        }
    }
}

fn allowed_dimensions(width: u32, height: u32) -> bool {
    (1..=MAX_DIMENSION).contains(&width)
        && (1..=MAX_DIMENSION).contains(&height)
        && u64::from(width) * u64::from(height) <= MAX_PIXELS
}

pub(super) async fn prepare(value: Value) -> Result<Arc<RenderImage>, FileError> {
    let wire =
        serde_json::from_value::<WireImage>(value).map_err(|_| FileError::Owned(IMAGE_ERROR))?;
    if wire.data.len() > MAX_BASE64_BYTES {
        return Err(FileError::Owned(IMAGE_ERROR));
    }
    let permit = DECODERS.acquire().await;
    smol::unblock(move || {
        // Already-started blocking work keeps its slot if the browser drops its task.
        let _permit = permit;
        decode(wire).ok_or(FileError::Owned(IMAGE_ERROR))
    })
    .await
}

fn decode(wire: WireImage) -> Option<Arc<RenderImage>> {
    let bytes = STANDARD.decode(wire.data).ok()?;
    let format = wire.format.image_format();
    if bytes.len() > MAX_ENCODED_BYTES || image::guess_format(&bytes).ok()? != format {
        return None;
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(64 * 1024 * 1024);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder().ok()?;
    let (width, height) = decoder.dimensions();
    if !allowed_dimensions(width, height) {
        return None;
    }
    limits.reserve(decoder.total_bytes()).ok()?;
    decoder.set_limits(limits).ok()?;
    let image = image::DynamicImage::from_decoder(decoder).ok()?;
    Some(render_image(image))
}

#[cfg(test)]
mod tests {
    use super::{
        RasterFormat, WireImage, actual_image_size, allowed_dimensions, decode, image_scroll_offset,
    };
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use gpui_kit::{DevicePixels, point, px, size};
    use std::io::Cursor;

    #[test]
    fn image_decoding_checks_format_and_decoded_size() {
        let make = |format, data: &str| WireImage {
            format,
            data: data.to_owned(),
        };
        let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC";
        let red = decode(make(RasterFormat::Png, png)).expect("valid PNG");
        assert_eq!(red.as_bytes(0), Some(&[0, 0, 255, 255][..]));
        let mut jpeg = Cursor::new(Vec::new());
        image::RgbImage::from_pixel(4, 3, image::Rgb([0, 0, 255]))
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .unwrap();
        let blue = decode(make(
            RasterFormat::Jpeg,
            &STANDARD.encode(jpeg.into_inner()),
        ))
        .expect("valid JPEG");
        let pixel = &blue.as_bytes(0).unwrap()[..4];
        assert!(pixel[0] > 245 && pixel[1] < 10 && pixel[2] < 10 && pixel[3] == 255);
        assert!(decode(make(RasterFormat::Jpeg, png)).is_none());
        assert!(decode(make(RasterFormat::Png, "iVBORw0KGgo=")).is_none());
        assert!(allowed_dimensions(3840, 2160));
        assert!(!allowed_dimensions(4096, 2304));
        assert!(!allowed_dimensions(8193, 1));
    }

    #[test]
    fn valid_image_larger_than_the_text_limit_decodes() {
        let mut state = 12345u32;
        let pixels = image::RgbImage::from_fn(700, 700, |_, _| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let [r, g, b, _] = state.to_le_bytes();
            image::Rgb([r, g, b])
        });
        let mut png = Cursor::new(Vec::new());
        pixels.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let bytes = png.into_inner();
        assert!(bytes.len() > 1024 * 1024);
        let image = decode(WireImage {
            format: RasterFormat::Png,
            data: STANDARD.encode(bytes),
        })
        .expect("valid PNG above the text limit");
        assert_eq!(image.as_bytes(0).unwrap().len(), 700 * 700 * 4);
    }

    #[test]
    fn actual_size_uses_device_pixels_and_scrolls_both_axes() {
        let resolution = size(DevicePixels(3200), DevicePixels(1800));
        assert_eq!(
            actual_image_size(resolution, 1.),
            size(px(3200.), px(1800.))
        );
        assert_eq!(actual_image_size(resolution, 2.), size(px(1600.), px(900.)));
        assert_eq!(
            actual_image_size(resolution, 1.25),
            size(px(2560.), px(1440.))
        );
        let maximum = point(px(80.), px(100.));
        let start = point(px(-70.), px(-90.));
        assert_eq!(
            image_scroll_offset("right", start, maximum),
            Some(point(px(-80.), px(-90.)))
        );
        assert_eq!(
            image_scroll_offset("down", start, maximum),
            Some(point(px(-70.), px(-100.)))
        );
        assert_eq!(
            image_scroll_offset("up", point(px(-25.), px(-10.)), maximum),
            Some(point(px(-25.), px(0.)))
        );
        assert_eq!(image_scroll_offset("escape", start, maximum), None);
    }
}
