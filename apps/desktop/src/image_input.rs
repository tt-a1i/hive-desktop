use crate::app::HiveApp;
use base64::Engine;
use gpui_kit::*;
use serde_json::{Value, json};
use std::{io::Cursor, path::PathBuf};

pub(crate) enum ImageSource {
    File(PathBuf),
    Clipboard(Image),
}

impl ImageSource {
    pub fn prepare(self) -> Result<(&'static str, Value), String> {
        match self {
            Self::File(path) => Ok(("chat.attachments.add", json!({"path":path}))),
            Self::Clipboard(image) => {
                // Windows DIB arrives as BMP; some macOS apps offer TIFF.
                // Only these clipboard-only formats need conversion.
                let (bytes, mime) = match image.format {
                    ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::Webp => {
                        (image.bytes, image.format.mime_type())
                    }
                    ImageFormat::Bmp | ImageFormat::Tiff => {
                        if image.bytes.len() > 64 * 1024 * 1024 {
                            return Err("剪贴板图片过大，请先缩小图片。".into());
                        }
                        let decoded = crate::attachments::decode_image(&image.bytes)?;
                        let mut png = Cursor::new(Vec::new());
                        decoded
                            .write_to(&mut png, image::ImageFormat::Png)
                            .map_err(|error| error.to_string())?;
                        (png.into_inner(), "image/png")
                    }
                    _ => {
                        return Err(
                            "暂不支持该剪贴板图片格式，请粘贴 PNG、JPEG、GIF 或 WebP。".into()
                        );
                    }
                };
                if bytes.len() > 3_750_000 {
                    return Err("图片过大，编码后不能超过 5 MB。".into());
                }
                Ok((
                    "chat.attachments.clipboard",
                    json!({"image":{
                        "type":"image", "mimeType":mime,
                        "data":base64::engine::general_purpose::STANDARD.encode(bytes)
                    }}),
                ))
            }
        }
    }
}

impl HiveApp {
    pub(crate) fn paste_images(&mut self, item: &ClipboardItem, cx: &mut Context<Self>) -> bool {
        let sources = item
            .entries()
            .iter()
            .flat_map(|entry| match entry {
                ClipboardEntry::Image(image) => vec![ImageSource::Clipboard(image.clone())],
                ClipboardEntry::ExternalPaths(paths) => paths
                    .paths()
                    .iter()
                    .cloned()
                    .map(ImageSource::File)
                    .collect(),
                ClipboardEntry::String(_) => vec![],
            })
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return false;
        }
        let copied_files = item
            .entries()
            .iter()
            .any(|entry| matches!(entry, ClipboardEntry::ExternalPaths(_)));
        self.import_image_sources(sources, cx);
        // File clipboards also carry a path string. Do not insert that path into
        // the message; independent image/text payloads keep normal text insertion.
        copied_files || item.text().is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::ImageSource;
    use base64::Engine;
    use gpui_kit::{Image, ImageFormat};
    use std::io::Cursor;
    #[test]
    fn clipboard_preserves_supported_bytes_and_normalizes_native_bitmap_formats() {
        let pixels = image::RgbImage::from_fn(8, 4, |x, _| {
            if x < 4 {
                image::Rgb([30, 120, 240])
            } else {
                image::Rgb([240, 110, 130])
            }
        });
        let bitmap = image::DynamicImage::ImageRgb8(pixels.clone());
        for (encoded_format, native_format) in [
            (image::ImageFormat::Png, ImageFormat::Png),
            (image::ImageFormat::Bmp, ImageFormat::Bmp),
            (image::ImageFormat::Tiff, ImageFormat::Tiff),
        ] {
            let mut encoded = Cursor::new(Vec::new());
            bitmap.write_to(&mut encoded, encoded_format).unwrap();
            let original = encoded.into_inner();
            let (method, payload) =
                ImageSource::Clipboard(Image::from_bytes(native_format, original.clone()))
                    .prepare()
                    .unwrap();
            assert_eq!(method, "chat.attachments.clipboard");
            assert_eq!(payload["image"]["mimeType"], "image/png");
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload["image"]["data"].as_str().unwrap())
                .unwrap();
            assert_eq!(
                crate::attachments::decode_image(&bytes).unwrap().to_rgb8(),
                pixels
            );
            if native_format == ImageFormat::Png {
                assert_eq!(bytes, original);
            }
        }
        assert!(
            ImageSource::Clipboard(Image::from_bytes(
                ImageFormat::Bmp,
                b"broken bitmap".to_vec()
            ))
            .prepare()
            .is_err()
        );
        assert!(
            ImageSource::Clipboard(Image::from_bytes(ImageFormat::Png, vec![0; 3_750_001]))
                .prepare()
                .is_err()
        );
        assert!(
            ImageSource::Clipboard(Image::from_bytes(ImageFormat::Svg, b"<svg/>".to_vec()))
                .prepare()
                .is_err()
        );
    }
}
