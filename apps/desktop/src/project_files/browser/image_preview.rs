use crate::{attachments::render_image, language::tr, project_files::FileError};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::*;
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

pub(super) struct PreviewImage(Arc<RenderImage>);

impl PreviewImage {
    pub(super) fn retain(render: Arc<RenderImage>, cx: &mut App) -> Entity<Self> {
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

pub(super) fn region(
    id: &'static str,
    image: &Entity<PreviewImage>,
    path: &str,
    height: Pixels,
    cx: &App,
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Image)
        .aria_label(format!("{}: {path}", tr("文件图片预览")))
        .w_full()
        .h(height)
        .min_h_0()
        .overflow_hidden()
        .child(
            img(image.read(cx).0.clone())
                .w_full()
                .h_full()
                .object_fit(ObjectFit::Contain),
        )
}

#[cfg(test)]
mod tests {
    use super::{RasterFormat, WireImage, allowed_dimensions, decode};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
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
}
