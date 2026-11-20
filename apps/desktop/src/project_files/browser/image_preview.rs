use crate::{attachments::render_image, language::tr, palette, project_files::FileError};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        scroll::{ScrollableElement, ScrollbarAxis},
    },
    prelude::FluentBuilder,
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

    pub(super) fn expanded(&self) -> ExpandedImageViewport {
        let scroll = ScrollHandle::new();
        scroll.set_offset(self.scroll.offset());
        ExpandedImageViewport {
            image: self.image.clone(),
            zoom: if self.actual_size {
                ImageZoom::Fixed(100)
            } else {
                ImageZoom::Fit
            },
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
        let view = image_region(div(), id, path, height);
        if self.actual_size {
            let size = actual_image_size(render.size(0), window.scale_factor());
            let scroll = self.scroll.clone();
            view.focusable()
                .overflow_scroll()
                .track_scroll(&self.scroll)
                .scrollbar(&self.scroll, ScrollbarAxis::Both)
                .child(img(render).w(size.width).h(size.height).flex_none())
                .on_key_down(scroll_image_with_keys(scroll))
        } else {
            view.overflow_hidden()
                .child(img(render).size_full().object_fit(ObjectFit::Contain))
        }
    }
}

const ZOOM_STEPS: [u16; 8] = [25, 50, 75, 100, 150, 200, 300, 400];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImageZoom {
    Fit,
    Fixed(u16),
}

#[derive(Clone, Copy)]
pub(super) enum ZoomAction {
    Out,
    ToggleFit,
    In,
}

impl ImageZoom {
    fn ratio(self, actual: Size<Pixels>, viewport: Size<Pixels>) -> f32 {
        match self {
            Self::Fit => (viewport.width / actual.width)
                .min(viewport.height / actual.height)
                .min(1.),
            Self::Fixed(percent) => f32::from(percent) / 100.,
        }
    }

    fn next(
        self,
        action: ZoomAction,
        actual: Size<Pixels>,
        viewport: Size<Pixels>,
    ) -> Option<Self> {
        let current = self.ratio(actual, viewport);
        match action {
            ZoomAction::ToggleFit => Some(if self == Self::Fit {
                Self::Fixed(100)
            } else {
                Self::Fit
            }),
            ZoomAction::Out => ZOOM_STEPS
                .into_iter()
                .rev()
                .find(|step| f32::from(*step) / 100. < current - 0.001)
                .map(Self::Fixed),
            ZoomAction::In => ZOOM_STEPS
                .into_iter()
                .find(|step| f32::from(*step) / 100. > current + 0.001)
                .map(Self::Fixed),
        }
    }
}

pub(super) struct ExpandedImageViewport {
    image: Entity<PreviewImage>,
    zoom: ImageZoom,
    scroll: ScrollHandle,
}

impl ExpandedImageViewport {
    pub(super) fn zoom(&mut self, action: ZoomAction, window: &Window, cx: &App) -> bool {
        let natural = actual_image_size(self.image.read(cx).0.size(0), window.scale_factor());
        let viewport = self.scroll.bounds().size;
        if viewport.width <= px(0.) || viewport.height <= px(0.) {
            return false;
        }
        let Some(next) = self.zoom.next(action, natural, viewport) else {
            return false;
        };
        let previous_size = natural.map(|dim| dim * self.zoom.ratio(natural, viewport));
        let next_size = natural.map(|dim| dim * next.ratio(natural, viewport));
        // The scrollable Div clamps this value against the new content dimensions
        // during layout. Keep this handle separate from the compact preview.
        self.scroll.set_offset(zoom_offset(
            self.scroll.offset(),
            viewport,
            previous_size,
            next_size,
        ));
        self.zoom = next;
        true
    }

    pub(super) fn zoom_controls(
        &self,
        change: impl Fn(ZoomAction, &mut Window, &mut App) + Clone + 'static,
        window: &Window,
        cx: &App,
    ) -> Stateful<Div> {
        let actual = actual_image_size(self.image.read(cx).0.size(0), window.scale_factor());
        let viewport = self.scroll.bounds().size;
        let measured = viewport.width > px(0.) && viewport.height > px(0.);
        let step = |id, label, icon, direction| {
            let change = change.clone();
            Button::new(id)
                .custom(palette::quiet_icon_variant(cx))
                .compact()
                .xsmall()
                .size(px(26.))
                .icon(icon)
                .accessibility_label(tr(label))
                .tooltip(tr(label))
                .disabled(!measured || self.zoom.next(direction, actual, viewport).is_none())
                .on_click(move |_, window, cx| change(direction, window, cx))
        };
        let (label, action) = match self.zoom {
            ImageZoom::Fit => (tr("适应").to_owned(), tr("原尺寸")),
            ImageZoom::Fixed(percent) => (format!("{percent}%"), tr("适应窗口")),
        };
        let out = step(
            "file-reader-zoom-out",
            "缩小图片",
            IconName::Minus,
            ZoomAction::Out,
        );
        let zoom_in = step(
            "file-reader-zoom-in",
            "放大图片",
            IconName::Plus,
            ZoomAction::In,
        );
        h_flex()
            .id("file-reader-image-zoom")
            .items_center()
            .gap_1()
            .child(out)
            .child(
                div()
                    .id("file-reader-zoom-level")
                    .role(Role::Status)
                    .aria_label(label.clone())
                    .child(
                        Button::new("file-reader-image-size")
                            .custom(palette::quiet_icon_variant(cx))
                            .compact()
                            .small()
                            .min_w(px(46.))
                            .label(label)
                            .accessibility_label(action)
                            .tooltip(action)
                            .disabled(!measured)
                            .on_click(move |_, window, cx| {
                                change(ZoomAction::ToggleFit, window, cx)
                            }),
                    ),
            )
            .child(zoom_in)
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
        let actual = actual_image_size(render.size(0), window.scale_factor());
        let content = match self.zoom {
            ImageZoom::Fit => div()
                .size_full()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    img(render)
                        .size_full()
                        .max_w(actual.width)
                        .max_h(actual.height)
                        .object_fit(ObjectFit::Contain),
                ),
            ImageZoom::Fixed(percent) => {
                let size = actual.map(|dim| dim * (f32::from(percent) / 100.));
                div()
                    .w(size.width)
                    .h(size.height)
                    .min_w(relative(1.))
                    .min_h(relative(1.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(img(render).w(size.width).h(size.height).flex_none())
            }
        };
        let scroll = self.scroll.clone();
        let previous_viewport = scroll.bounds().size;
        let view = div().on_children_prepainted(move |_, window, _| {
            if scroll.bounds().size != previous_viewport {
                window.on_next_frame(|window, _| window.refresh());
            }
        });
        image_region(view, id, path, height)
            .focusable()
            .overflow_scroll()
            .track_scroll(&self.scroll)
            .when(self.zoom != ImageZoom::Fit, |view| {
                view.scrollbar(&self.scroll, ScrollbarAxis::Both)
            })
            .on_key_down(scroll_image_with_keys(self.scroll.clone()))
            .child(content)
    }
}

fn image_region(view: Div, id: &'static str, path: &str, height: Pixels) -> Stateful<Div> {
    view.id(id)
        .role(Role::Image)
        .aria_label(format!("{}: {path}", tr("文件图片预览")))
        .w_full()
        .min_w_0()
        .h(height)
        .min_h_0()
        .tab_stop(true)
}

fn scroll_image_with_keys(scroll: ScrollHandle) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) {
    move |event, window, cx| {
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
    }
}

fn zoom_offset(
    previous_offset: Point<Pixels>,
    viewport: Size<Pixels>,
    previous_image: Size<Pixels>,
    next_image: Size<Pixels>,
) -> Point<Pixels> {
    let axis = |offset: Pixels, pane: Pixels, before: Pixels, after: Pixels| {
        let middle = pane / 2.;
        let old_padding = (pane - before).max(px(0.)) / 2.;
        let point_in_image = ((middle - old_padding - offset) / before).clamp(0., 1.);
        let new_padding = (pane - after).max(px(0.)) / 2.;
        (middle - new_padding - after * point_in_image).clamp(-(after - pane).max(px(0.)), px(0.))
    };
    point(
        axis(
            previous_offset.x,
            viewport.width,
            previous_image.width,
            next_image.width,
        ),
        axis(
            previous_offset.y,
            viewport.height,
            previous_image.height,
            next_image.height,
        ),
    )
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
        ImageZoom, RasterFormat, WireImage, ZoomAction, actual_image_size, allowed_dimensions,
        decode, image_scroll_offset, zoom_offset,
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
    fn zoom_steps_follow_the_measured_fit_and_stay_inside_bounds() {
        let actual = size(px(1600.), px(900.));
        let viewport = size(px(1000.), px(600.));
        let next = |zoom: ImageZoom, action| zoom.next(action, actual, viewport);
        assert_eq!(
            next(ImageZoom::Fit, ZoomAction::In),
            Some(ImageZoom::Fixed(75))
        );
        assert_eq!(
            next(ImageZoom::Fit, ZoomAction::Out),
            Some(ImageZoom::Fixed(50))
        );
        assert_eq!(
            next(ImageZoom::Fixed(75), ZoomAction::In),
            Some(ImageZoom::Fixed(100))
        );
        assert_eq!(
            next(ImageZoom::Fixed(100), ZoomAction::In),
            Some(ImageZoom::Fixed(150))
        );
        assert_eq!(next(ImageZoom::Fixed(400), ZoomAction::In), None);
        assert_eq!(next(ImageZoom::Fixed(25), ZoomAction::Out), None);
        assert_eq!(
            next(ImageZoom::Fit, ZoomAction::ToggleFit),
            Some(ImageZoom::Fixed(100))
        );
        assert_eq!(
            next(ImageZoom::Fixed(150), ZoomAction::ToggleFit),
            Some(ImageZoom::Fit)
        );
        assert_eq!(ImageZoom::Fit.ratio(size(px(100.), px(80.)), viewport), 1.,);
    }

    #[test]
    fn zoom_offset_preserves_the_center_image_point_and_clamps_near_edges() {
        let viewport = size(px(600.), px(300.));
        let before = size(px(1000.), px(500.));
        assert_eq!(
            zoom_offset(
                point(px(-325.), px(-100.)),
                viewport,
                before,
                size(px(1500.), px(750.))
            ),
            point(px(-637.5), px(-225.)),
        );
        assert_eq!(
            zoom_offset(
                point(px(-400.), px(-200.)),
                viewport,
                before,
                size(px(800.), px(400.))
            ),
            point(px(-200.), px(-100.)),
        );
        assert_eq!(
            zoom_offset(
                point(px(0.), px(0.)),
                viewport,
                size(px(400.), px(200.)),
                before
            ),
            point(px(-200.), px(-100.)),
        );
        assert_eq!(
            zoom_offset(
                point(px(-400.), px(-200.)),
                viewport,
                before,
                size(px(400.), px(250.))
            ),
            point(px(0.), px(0.)),
        );
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
