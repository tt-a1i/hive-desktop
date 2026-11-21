use crate::app::HiveApp;
use crate::attachments::clipboard_text::{MAX_BYTES, MIN_BYTES};
use base64::Engine;
use gpui_kit::component::WindowExt;
use gpui_kit::*;
use serde_json::{Value, json};
use std::{io::Cursor, path::PathBuf};

pub(crate) enum ImageSource {
    File(PathBuf),
    ProjectFile {
        relative_path: String,
        session_id: Option<String>,
        expected_sha256: Option<String>,
    },
    Clipboard(Image),
}

#[derive(Debug)]
pub(crate) enum PrepareError { ClipboardTooLarge, UnsupportedFormat, EncodedTooLarge, Decode(String) }
impl From<String> for PrepareError {
    fn from(error: String) -> Self { Self::Decode(error) }
}

impl ImageSource {
    pub fn prepare(self) -> Result<(&'static str, Value), PrepareError> {
        match self {
            Self::File(path) => Ok(("chat.attachments.add", json!({"path":path}))),
            Self::ProjectFile { relative_path, session_id, expected_sha256 } => {
                let mut params = json!({"relativePath":relative_path});
                if let Some(session_id) = session_id {
                    params["sessionId"] = json!(session_id);
                }
                if let Some(expected_sha256) = expected_sha256 {
                    params["expectedSha256"] = json!(expected_sha256);
                }
                Ok(("project.files.attach", params))
            }
            Self::Clipboard(image) => {
                // Windows DIB arrives as BMP; some macOS apps offer TIFF.
                // Only these clipboard-only formats need conversion.
                let (bytes, mime) = match image.format {
                    ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Gif | ImageFormat::Webp => {
                        (image.bytes, image.format.mime_type())
                    }
                    ImageFormat::Bmp | ImageFormat::Tiff => {
                        if image.bytes.len() > 64 * 1024 * 1024 {
                            return Err(PrepareError::ClipboardTooLarge);
                        }
                        let decoded = crate::attachments::decode_image(&image.bytes)?;
                        let mut png = Cursor::new(Vec::new());
                        decoded
                            .write_to(&mut png, image::ImageFormat::Png)
                            .map_err(|error| error.to_string())?;
                        (png.into_inner(), "image/png")
                    }
                    _ => {
                        return Err(PrepareError::UnsupportedFormat);
                    }
                };
                if bytes.len() > 3_750_000 {
                    return Err(PrepareError::EncodedTooLarge);
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
    pub(crate) fn paste_composer_clipboard(&mut self, item: &ClipboardItem, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if item.entries().iter().any(|entry| !matches!(entry, ClipboardEntry::String(_))) {
            return self.paste_images(item, cx);
        }
        let Some(text) = item.text() else {
            return false;
        };
        let composing = self.input.update(cx, |state, cx| state.marked_text_range(window, cx).is_some());
        let existing_count = self.attachments.refs(self.draft_target.as_ref()).len();
        let has_pending = self.has_pending_clipboard_text();
        match classify_clipboard_paste(
            text.len(),
            self.workspace.is_some(),
            composing,
            existing_count,
            has_pending,
        ) {
            ClipboardPasteDecision::Native => false,
            ClipboardPasteDecision::RejectTooLarge => {
                self.error = Some("粘贴的文本超过 8 MiB 附件上限，请精简或保存为文件后添加。".into());
                cx.notify();
                true
            }
            ClipboardPasteDecision::RejectTooManyAttachments => {
                self.error = Some("每条消息最多添加 10 个附件。".into());
                cx.notify();
                true
            }
            ClipboardPasteDecision::RejectPendingSave => {
                self.error = Some("上一份粘贴文本仍在保存中，请稍候或先处理待保存文本。".into());
                cx.notify();
                true
            }
            ClipboardPasteDecision::Stage => self.stage_clipboard_text(text, cx),
        }
    }

    pub(crate) fn paste_composer_text(
        &mut self,
        _: &crate::PasteComposerText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.conversation_is_read_only() || window.has_active_dialog(cx)
            || !self.input.read(cx).focus_handle(cx).is_focused(window)
            || self.draft_target != self.draft_owner
        {
            cx.propagate();
            return;
        }
        let Some(text) = cx.read_from_clipboard().and_then(|item| {
            let text = item.entries().iter().filter_map(|entry| match entry {
                ClipboardEntry::String(value) => Some(value.text()),
                _ => None,
            }).map(String::as_str).collect::<String>();
            (!text.is_empty()).then_some(text)
        }) else { cx.propagate(); return; };
        let inserted = self.input.update(cx, |input, cx| {
            if input.marked_text_range(window, cx).is_some() { return false; }
            input.replace(text, window, cx);
            true
        });
        if inserted { cx.stop_propagation(); } else { cx.propagate(); }
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClipboardPasteDecision {
    Native,
    Stage,
    RejectTooLarge,
    RejectTooManyAttachments,
    RejectPendingSave,
}

pub(crate) fn classify_clipboard_paste(
    byte_len: usize,
    workspace_ready: bool,
    composing: bool,
    existing_count: usize,
    has_pending_clipboard_text: bool,
) -> ClipboardPasteDecision {
    if !workspace_ready || composing || byte_len < MIN_BYTES {
        return ClipboardPasteDecision::Native;
    }
    if byte_len > MAX_BYTES {
        return ClipboardPasteDecision::RejectTooLarge;
    }
    if existing_count >= 10 {
        return ClipboardPasteDecision::RejectTooManyAttachments;
    }
    if has_pending_clipboard_text {
        return ClipboardPasteDecision::RejectPendingSave;
    }
    ClipboardPasteDecision::Stage
}

#[cfg(test)]
fn should_stage_text(text: &str) -> bool {
    (MIN_BYTES..=MAX_BYTES).contains(&text.len())
}

#[cfg(test)]
mod tests {
    use super::{
        ClipboardPasteDecision, ImageSource, MAX_BYTES, MIN_BYTES, PrepareError,
        classify_clipboard_paste, should_stage_text,
    };
    use base64::Engine;
    use gpui_kit::{Image, ImageFormat};
    use std::io::Cursor;

    #[test]
    fn long_paste_threshold_uses_exact_utf8_bytes() {
        let near_limit = "界".repeat(MIN_BYTES / 3);
        assert!(!should_stage_text(&near_limit));
        assert!(should_stage_text(&(near_limit + "ab")));
        assert!(!should_stage_text(&"x".repeat(MAX_BYTES + 1)));
    }

    #[test]
    fn clipboard_paste_classification_respects_thresholds_and_limits() {
        assert_eq!(
            classify_clipboard_paste(MIN_BYTES - 1, true, false, 0, false),
            ClipboardPasteDecision::Native
        );
        assert_eq!(
            classify_clipboard_paste(MIN_BYTES, false, false, 0, false),
            ClipboardPasteDecision::Native
        );
        assert_eq!(
            classify_clipboard_paste(MIN_BYTES, true, true, 0, false),
            ClipboardPasteDecision::Native
        );
        assert_eq!(
            classify_clipboard_paste(MIN_BYTES, true, false, 0, false),
            ClipboardPasteDecision::Stage
        );
        assert_eq!(
            classify_clipboard_paste(MAX_BYTES, true, false, 9, false),
            ClipboardPasteDecision::Stage
        );
        assert_eq!(
            classify_clipboard_paste(MAX_BYTES + 1, true, false, 0, false),
            ClipboardPasteDecision::RejectTooLarge
        );
        assert_eq!(
            classify_clipboard_paste(MIN_BYTES, true, false, 10, false),
            ClipboardPasteDecision::RejectTooManyAttachments
        );
        assert_eq!(
            classify_clipboard_paste(MIN_BYTES, true, false, 0, true),
            ClipboardPasteDecision::RejectPendingSave
        );
    }
    #[test]
    fn project_file_attachment_pins_optional_session_and_preview_digest() {
        let path = "docs/shared.txt";
        let sha256 = "a".repeat(64);
        for (session_id, expected_sha256, expected) in [
            (
                Some("worktree-a".into()),
                Some(sha256.clone()),
                serde_json::json!({"relativePath":path,"sessionId":"worktree-a","expectedSha256":sha256}),
            ),
            (
                Some("worktree-a".into()),
                None,
                serde_json::json!({"relativePath":path,"sessionId":"worktree-a"}),
            ),
            (None, None, serde_json::json!({"relativePath":path})),
        ] {
            let source = ImageSource::ProjectFile {
                relative_path: path.into(),
                session_id,
                expected_sha256,
            };
            assert_eq!(source.prepare().unwrap(), ("project.files.attach", expected));
        }
    }

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
        assert!(matches!(ImageSource::Clipboard(Image::from_bytes(ImageFormat::Png,vec![0;3_750_001])).prepare(),Err(PrepareError::EncodedTooLarge)));
        assert!(matches!(ImageSource::Clipboard(Image::from_bytes(ImageFormat::Svg,b"<svg/>".to_vec())).prepare(),Err(PrepareError::UnsupportedFormat)));
    }
}
