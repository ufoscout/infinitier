//! TIS tileset viewer. GPUI port of the egui `TisViewer`.
//!
//! Mirrors NearInfinity's `TisResource` panel as the egui version
//! does:
//! - Top control strip with prev/next "tiles per row" buttons (the
//!   egui version uses a Slider; gpui-component's `Slider` widget
//!   wants its own `Entity<SliderState>` so we use buttons for the
//!   same reason `bam.rs` uses buttons instead of a ComboBox).
//! - Center: the composite image, scaled-to-fit / never-upscale and
//!   centred via the absolute-pinned `img` trick used by the image
//!   viewer.
//! - Bottom info bar: dimensions, file size, TIS variant, tile count,
//!   layout, origin.
//!
//! Texture cache: one `Arc<RenderImage>` keyed on the current
//! `columns` setting. `ImportedTis::compose` is a CPU memcpy per
//! tile — cheap to redo when the layout changes, but no point
//! re-uploading every frame.
//!
//! Grid overlay: a toggle button on the control strip flips
//! `show_grid`. When on, a `canvas()` element layered on top of the
//! image computes the actually-displayed image rect from its
//! intrinsic dimensions (re-running gpui's `ObjectFit::ScaleDown`
//! math against the slot's resolved bounds) and paints 1-px lines at
//! every tile boundary inside that rect. Same shape the egui
//! viewer's `paint_tile_grid` paints, just driven by gpui's
//! `paint_quad` instead of egui's `painter.line_segment`.

use std::sync::Arc;

use bytesize::ByteSize;
use gpui::{
    AnyElement, Bounds, Context, IntoElement, ObjectFit, ParentElement, Pixels, Point, RenderImage,
    Size, Styled, StyledImage as _, Window, canvas, div, fill, hsla, img, px,
};
use gpui_component::{ActiveTheme, Sizable, button::Button, h_flex, v_flex};
use image::Frame;
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::tis::ImportedTis,
    resource::tis::Type as TisType,
};
use smallvec::SmallVec;

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

/// GPU texture-side cap. gpui doesn't expose the live renderer limit
/// the way egui does, so we pick a value every desktop GPU supports
/// (matches the lower-bound wgpu defaults).
const MAX_TEXTURE_SIDE: u32 = 8192;

pub struct TisViewer {
    tis: ImportedTis,
    /// Currently-selected tiles-per-row. Always inside `slider_range`.
    columns: u32,
    /// Inclusive bounds for the columns selector, picked so the
    /// composite never exceeds `MAX_TEXTURE_SIDE` on either axis.
    slider_range: std::ops::RangeInclusive<u32>,
    /// Tiles per axis that fit in `MAX_TEXTURE_SIDE`. Used by the
    /// truncated compose path.
    tiles_per_axis: u32,
    /// `true` when the tileset is so large (>`tiles_per_axis²`) that
    /// even the squarest arrangement overflows the texture limit, so
    /// the displayed image is the leading slice that fits.
    truncated: bool,
    /// Cached composite for the currently-selected `columns`.
    cached: Option<CachedTexture>,
    /// `true` when the user has the grid overlay enabled (toggle
    /// button in the control strip). Painted by the `canvas`
    /// element layered on top of the picture area.
    show_grid: bool,
}

struct CachedTexture {
    columns: u32,
    width: u32,
    height: u32,
    image: Arc<RenderImage>,
}

impl TisViewer {
    pub fn new(tis: ImportedTis) -> Self {
        let limits = ColumnLimits::new(tis.tile_count, tis.tile_dimension, MAX_TEXTURE_SIDE);
        let columns = tis
            .expected_columns
            .clamp(*limits.range.start(), *limits.range.end());
        Self {
            tis,
            columns,
            slider_range: limits.range,
            tiles_per_axis: limits.tiles_per_axis,
            truncated: limits.truncated,
            cached: None,
            show_grid: false,
        }
    }

    fn variant_label(&self) -> &'static str {
        match self.tis.variant {
            TisType::Palette => "Palette",
            TisType::Pvrz => "PVRZ",
        }
    }

    /// (Re)build the composite when the columns selection changes.
    /// Cheap because `ImportedTis::compose` is just a per-tile memcpy.
    /// Buffer is converted RGBA → BGRA in place — gpui's renderer
    /// expects BGRA (see `gpui/elements/img.rs`).
    fn ensure_cached(&mut self) {
        if let Some(c) = &self.cached
            && c.columns == self.columns
        {
            return;
        }
        let composed = if self.truncated {
            let visible = (self.columns as usize) * (self.tiles_per_axis as usize);
            let count = self.tis.tile_count.min(visible);
            let stride =
                (self.tis.tile_dimension as usize) * (self.tis.tile_dimension as usize) * 4;
            let view = ImportedTis {
                tile_dimension: self.tis.tile_dimension,
                tile_count: count,
                tile_pixels: self.tis.tile_pixels[..count * stride].to_vec(),
                expected_columns: self.tis.expected_columns,
                variant: self.tis.variant,
            };
            view.compose(self.columns)
        } else {
            self.tis.compose(self.columns)
        };
        let width = composed.width();
        let height = composed.height();
        let mut buffer = composed;
        for pixel in buffer.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let frame = Frame::new(buffer);
        let image = Arc::new(RenderImage::new(SmallVec::from_elem(frame, 1)));
        self.cached = Some(CachedTexture {
            columns: self.columns,
            width,
            height,
            image,
        });
    }
}

impl ResourceViewerTrait for TisViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        _window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        self.ensure_cached();
        let border = cx.theme().border;

        let image_area = picture_area(
            self.cached.as_ref().map(|c| c.image.clone()),
            self.cached.as_ref().map(|c| (c.width, c.height)),
            self.tis.tile_dimension,
            self.show_grid,
        );
        let controls = control_strip(self, cx);
        let info = info_bar(self, resource, cx);

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(controls)
            .child(div().h_px().bg(border))
            .child(image_area)
            .child(div().h_px().bg(border))
            .child(info)
            .into_any_element()
    }
}

/// Scaled-to-fit, centred picture area. Same `absolute + inset_0`
/// trick the image viewer uses to stop taffy from expanding the slot
/// to satisfy the composite's intrinsic aspect ratio. When
/// `show_grid` is on we layer a `canvas` over the image and paint
/// tile-boundary lines inside the displayed image rect.
fn picture_area(
    image: Option<Arc<RenderImage>>,
    image_dims: Option<(u32, u32)>,
    tile_dim: u32,
    show_grid: bool,
) -> impl IntoElement {
    let mut slot = div()
        .flex_1()
        .min_h_0()
        .w_full()
        .relative()
        .overflow_hidden();
    if let Some(tex) = image {
        slot = slot.child(
            img(tex)
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .size_full()
                .object_fit(ObjectFit::ScaleDown),
        );
    }
    if show_grid && let Some((iw, ih)) = image_dims {
        slot = slot.child(grid_overlay(iw, ih, tile_dim));
    }
    slot
}

/// `canvas` overlay that paints a 1-px tile-boundary grid inside the
/// displayed image rect. The displayed rect is computed at paint
/// time by re-running gpui's `ObjectFit::ScaleDown` math against the
/// canvas's resolved bounds — same logic the underlying `img`
/// element uses, so the grid stays aligned at every window size.
fn grid_overlay(image_w: u32, image_h: u32, tile_dim: u32) -> impl IntoElement {
    canvas(
        // No prepaint work; we only need the bounds at paint time.
        |_bounds, _window, _cx| (),
        move |bounds: Bounds<Pixels>, _, window, _cx| {
            paint_tile_grid(bounds, image_w, image_h, tile_dim, window);
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .right_0()
    .bottom_0()
}

/// Replicate `ObjectFit::ScaleDown` to find where the image actually
/// paints inside `bounds`, then paint one filled 1-px quad per tile
/// boundary (vertical and horizontal). The outermost border on the
/// right / bottom is included so the tileset's outer edge is also
/// marked, matching the egui viewer's `paint_tile_grid`.
fn paint_tile_grid(
    bounds: Bounds<Pixels>,
    image_w: u32,
    image_h: u32,
    tile_dim: u32,
    window: &mut Window,
) {
    if image_w == 0 || image_h == 0 || tile_dim == 0 {
        return;
    }
    // `Pixels(pub(crate) f32)` — extract the underlying float via the
    // public `From<Pixels> for f32` impl.
    let bounds_w = f32::from(bounds.size.width);
    let bounds_h = f32::from(bounds.size.height);
    let img_w = image_w as f32;
    let img_h = image_h as f32;

    // ScaleDown: shrink-to-fit but never upscale, then centre.
    let scale = (bounds_w / img_w).min(bounds_h / img_h).min(1.0);
    let drawn_w = img_w * scale;
    let drawn_h = img_h * scale;
    let origin_x = f32::from(bounds.origin.x) + (bounds_w - drawn_w) / 2.0;
    let origin_y = f32::from(bounds.origin.y) + (bounds_h - drawn_h) / 2.0;

    let tile_px = tile_dim as f32 * scale;
    // At sub-pixel tile sizes the grid would be denser than the
    // pixels it's supposed to outline — same skip the egui viewer
    // does to avoid a moiré pattern.
    if tile_px <= 0.5 {
        return;
    }

    // Translucent white lines, same colour the egui viewer uses.
    let line = hsla(0.0, 0.0, 1.0, 0.70);

    // Vertical lines at column boundaries (0..=columns).
    let cols = (image_w / tile_dim).max(1);
    for col in 0..=cols {
        let x = origin_x + (col as f32) * tile_px;
        let rect = Bounds {
            origin: Point {
                x: px(x),
                y: px(origin_y),
            },
            size: Size {
                width: px(1.0),
                height: px(drawn_h),
            },
        };
        window.paint_quad(fill(rect, line));
    }
    // Horizontal lines at row boundaries (0..=rows).
    let rows = (image_h / tile_dim).max(1);
    for row in 0..=rows {
        let y = origin_y + (row as f32) * tile_px;
        let rect = Bounds {
            origin: Point {
                x: px(origin_x),
                y: px(y),
            },
            size: Size {
                width: px(drawn_w),
                height: px(1.0),
            },
        };
        window.paint_quad(fill(rect, line));
    }
}

/// Top control strip: prev/next columns + the truncation warning.
fn control_strip(viewer: &TisViewer, cx: &mut Context<ExplorerApp>) -> impl IntoElement + use<> {
    let theme = cx.theme();
    let max = *viewer.slider_range.end();
    let min = *viewer.slider_range.start();
    let label = format!("Tiles per row: {} / {max}", viewer.columns);
    // Single-column slider range = nothing to drag; disable the
    // buttons rather than have them no-op.
    let can_adjust = min < max;
    let truncated = viewer.truncated;

    let mut row = h_flex()
        .w_full()
        .px_2()
        .py_1()
        .gap_2()
        .items_center()
        .bg(theme.secondary)
        .child(
            Button::new("tis-prev-cols")
                .label("◀")
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = tis_viewer_mut(this);
                    let start = *viewer.slider_range.start();
                    if viewer.columns > start {
                        viewer.columns -= 1;
                        cx.notify();
                    }
                })),
        )
        .child(div().min_w(px(180.)).child(label))
        .child(
            Button::new("tis-next-cols")
                .label("▶")
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = tis_viewer_mut(this);
                    let end = *viewer.slider_range.end();
                    if viewer.columns < end {
                        viewer.columns += 1;
                        cx.notify();
                    }
                })),
        )
        .child(div().w_2())
        .child(
            Button::new("tis-grid")
                .label(if viewer.show_grid {
                    "Hide grid"
                } else {
                    "Show grid"
                })
                .small()
                .on_click(cx.listener(|this, _, _, cx| {
                    let viewer = tis_viewer_mut(this);
                    viewer.show_grid = !viewer.show_grid;
                    cx.notify();
                })),
        );
    if !can_adjust {
        // Visual hint that the layout is locked; no functional change.
        row = row.child(
            div()
                .text_color(theme.muted_foreground)
                .child("(layout fixed)"),
        );
    }
    if truncated {
        // Same warning the egui viewer surfaces inline.
        row = row.child(div().flex_1()).child(
            div()
                .text_color(gpui::Hsla {
                    h: 40.0 / 360.0,
                    s: 1.0,
                    l: 0.5,
                    a: 1.0,
                })
                .child("⚠ Tileset too large; showing partial view"),
        );
    }
    row
}

/// Bottom info bar — same cells the egui viewer paints.
fn info_bar(
    viewer: &TisViewer,
    resource: &GameResource,
    cx: &mut Context<ExplorerApp>,
) -> impl IntoElement + use<> {
    let theme = cx.theme();

    let (composed_w, composed_h) = viewer
        .cached
        .as_ref()
        .map(|c| (c.width, c.height))
        .unwrap_or((0, 0));
    let file_size = match resource.file_size {
        Some(size) => ByteSize(size).to_string(),
        None => "? B".to_string(),
    };
    let layout_cell = if viewer.truncated {
        let visible = viewer.columns * viewer.tiles_per_axis;
        format!(
            "{} × {} tiles (showing first {} of {})",
            viewer.columns,
            viewer.tiles_per_axis,
            visible.min(viewer.tis.tile_count as u32),
            viewer.tis.tile_count,
        )
    } else {
        let rows = (viewer.tis.tile_count as u32).div_ceil(viewer.columns);
        format!("{} × {} tiles", viewer.columns, rows)
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
        .child(cell(format!("{composed_w} × {composed_h} px")))
        .child(separator(theme.border))
        .child(cell(file_size))
        .child(separator(theme.border))
        .child(cell("TIS".to_string()))
        .child(separator(theme.border))
        .child(cell(viewer.variant_label().to_string()))
        .child(separator(theme.border))
        .child(cell(format!("{} tiles", viewer.tis.tile_count)))
        .child(separator(theme.border))
        .child(cell(layout_cell))
        .child(separator(theme.border))
        .child(cell(origin))
}

fn cell(text: String) -> impl IntoElement {
    div().child(text)
}

fn separator(color: gpui::Hsla) -> impl IntoElement {
    div().w_px().h_4().bg(color)
}

/// Pull the cached `TisViewer` back out of the dispatcher cache so
/// click handlers (which only get `&mut ExplorerApp`) can mutate
/// viewer state. Same downcast trick `bam.rs` uses; works because
/// `ResourceViewerTrait` extends `Any`.
fn tis_viewer_mut(app: &mut ExplorerApp) -> &mut TisViewer {
    let trait_obj = &mut app
        .viewer
        .inner
        .as_mut()
        .expect("TIS click fired without an active viewer")
        .viewer;
    (trait_obj.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<TisViewer>()
        .expect("active viewer is not a TisViewer")
}

// ── Column-limits helper, ported verbatim from the egui viewer ──────

/// Composite-image limits derived from the renderer's max texture
/// side and the fixed 64-pixel TIS tile size.
struct ColumnLimits {
    /// Tiles per axis that fit in a single texture.
    tiles_per_axis: u32,
    /// Inclusive bounds the columns selector must stay within.
    range: std::ops::RangeInclusive<u32>,
    /// `true` when not every tile fits — see [`TisViewer::truncated`].
    truncated: bool,
}

impl ColumnLimits {
    fn new(tile_count: usize, tile_dim: u32, max_side: u32) -> Self {
        let tile_count = tile_count.max(1) as u32;
        let tiles_per_axis = (max_side / tile_dim.max(1)).max(1);

        let max_cols = tile_count.min(tiles_per_axis);
        let min_cols = tile_count.div_ceil(tiles_per_axis);

        if min_cols > max_cols {
            ColumnLimits {
                tiles_per_axis,
                range: max_cols..=max_cols,
                truncated: true,
            }
        } else {
            ColumnLimits {
                tiles_per_axis,
                range: min_cols.max(1)..=max_cols.max(1),
                truncated: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_limits_small_tileset_fits_with_no_min_constraint() {
        let l = ColumnLimits::new(6, 64, 8192);
        assert_eq!(l.tiles_per_axis, 128);
        assert_eq!(l.range, 1..=6);
        assert!(!l.truncated);
    }

    #[test]
    fn column_limits_medium_tileset_enforces_min() {
        let l = ColumnLimits::new(2400, 64, 8192);
        assert_eq!(l.tiles_per_axis, 128);
        assert_eq!(*l.range.start(), 19);
        assert_eq!(*l.range.end(), 128);
        assert!(!l.truncated);
    }

    #[test]
    fn column_limits_too_large_tileset_truncates() {
        let l = ColumnLimits::new(20_000, 64, 8192);
        assert_eq!(l.tiles_per_axis, 128);
        assert_eq!(l.range, 128..=128);
        assert!(l.truncated);
    }

    #[test]
    fn column_limits_clamps_to_at_least_one() {
        let l = ColumnLimits::new(0, 64, 8192);
        assert_eq!(l.range, 1..=1);

        let tiny = ColumnLimits::new(10, 64, 32);
        assert_eq!(tiny.tiles_per_axis, 1);
        assert!(*tiny.range.start() >= 1);
    }
}
