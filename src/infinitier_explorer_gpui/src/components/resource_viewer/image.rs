//! Raster-image viewer (BMP / PVRZ / MOS / PNG). Port of the egui
//! `ImageViewer`: same info bar + scale-to-fit, never-upscale picture
//! area, but the texture upload goes through `gpui::RenderImage` /
//! `img()` instead of `egui::TextureHandle`.
//!
//! The decoded image data is converted from RGBA → BGRA in-place at
//! construction time (gpui's renderer expects BGRA, see
//! `gpui/elements/img.rs`), wrapped in an `Arc<RenderImage>`, and
//! reused on every frame.

use std::sync::Arc;

use bytesize::ByteSize;
use gpui::{
    AnyElement, Context, IntoElement, ObjectFit, ParentElement, RenderImage, StyledImage as _,
    Window, img,
};
use gpui::{Styled, div};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use image::Frame;
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::image::ImportedImage,
};
use smallvec::SmallVec;

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

pub struct ImageViewer {
    /// BGRA texture cached for the lifetime of this viewer.
    cached: Arc<RenderImage>,
    width: u32,
    height: u32,
    /// Short uppercase label of the source format (e.g. `"BMP"`).
    format_label: &'static str,
    /// Human-readable detail line (bit depth / compression / DXT variant).
    format_description: String,
}

impl ImageViewer {
    pub fn new(img: ImportedImage) -> Self {
        let width = img.width();
        let height = img.height();
        let format_label = img.format_label();
        let format_description = img.format_description();

        // gpui's renderer wants BGRA; `ImportedImage` always lands as
        // RGBA8 regardless of source. Swap R↔B in place — same trick
        // gpui itself uses in `elements/img.rs` for decoded buffers.
        let mut buffer = img.image;
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }

        let frame = Frame::new(buffer);
        let cached = Arc::new(RenderImage::new(SmallVec::from_elem(frame, 1)));

        Self {
            cached,
            width,
            height,
            format_label,
            format_description,
        }
    }
}

impl ResourceViewerTrait for ImageViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        _window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        let theme = cx.theme();
        // `flex_1 + min_h_0` (not `size_full`) is what makes the
        // picture-area shrink-to-fit work — `size_full` resolves the
        // percentage against the central panel's *content* box, but a
        // descendant `flex_1` then claims that full height and the
        // image, centred inside the oversized slot, falls below the
        // viewport. Same shape `keeper_gpui::ui::character` uses for
        // its tab body.
        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(picture_area(self.cached.clone()))
            .child(div().h_px().bg(theme.border))
            .child(info_bar(self, resource, cx))
            .into_any_element()
    }
}

/// The scale-to-fit, centred picture area. We pin the `img` element
/// to the four edges of a `relative + overflow_hidden` container with
/// `.absolute()` so taffy can't expand the slot to satisfy the image's
/// intrinsic aspect ratio — without that escape hatch the picture
/// area grew taller than the viewport and `ObjectFit::ScaleDown`'s
/// centring placed the image below the visible window.
/// `ObjectFit::ScaleDown` itself handles the actual scale + centre
/// of the pixels within the pinned bounds.
fn picture_area(image: Arc<RenderImage>) -> impl IntoElement {
    div()
        .flex_1()
        .min_h_0()
        .w_full()
        .relative()
        .overflow_hidden()
        .child(
            // `size_full()` *and* the four-edge inset are both
            // required. Without explicit width/height, `Img` falls
            // through to natural image dimensions (see its
            // `request_layout`) and the insets get ignored — the
            // image then paints at its native size in the top-left.
            // Without the insets, an absolute child has no defined
            // containing block to size against.
            img(image)
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .size_full()
                .object_fit(ObjectFit::ScaleDown),
        )
}

/// Bottom info bar — dimensions, file size, format label, format
/// description, data origin. Same row of cells the egui viewer paints.
fn info_bar(
    viewer: &ImageViewer,
    resource: &GameResource,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement {
    let theme = cx.theme();

    let file_size = match resource.file_size {
        Some(size) => ByteSize(size).to_string(),
        None => "? B".to_string(),
    };
    let origin = match &resource.data_origin {
        DataOrigin::Bif { name } => format!("BIF: {name}"),
        DataOrigin::Dir { name, path } => format!("{name}: {}", path.path().display()),
        DataOrigin::Missing => "Missing".to_string(),
    };

    h_flex()
        .w_full()
        .px_3()
        .py_1p5()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(cell(format!("{} × {} px", viewer.width, viewer.height)))
        .child(separator(theme.border))
        .child(cell(file_size))
        .child(separator(theme.border))
        .child(cell(viewer.format_label.to_string()))
        .child(separator(theme.border))
        .child(cell(viewer.format_description.clone()))
        .child(separator(theme.border))
        .child(cell(origin))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}
