//! TIS tileset viewer — direct port of the egui original.
//!
//! Two non-obvious choices that fix problems the naive port had:
//!
//! - **Slider feels snappy because recomposing is deferred.** Slint's
//!   `Slider::changed` fires per mouse-delta during a drag (much
//!   faster than the render frame rate). Recomposing a 40 MB tileset
//!   on every event saturates the CPU. So callbacks only mutate
//!   `state.tis_viewer.columns` and refresh the textual properties;
//!   the heavy `compose` runs at most once per playback-timer tick
//!   (~30 Hz) and naturally coalesces adjacent slider events.
//!
//! - **Grid is drawn by Slint, not baked into the source image.**
//!   Baking 1-source-pixel lines and then letting `image-fit: contain`
//!   downscale produced visible aliasing — some lines rounded to 0
//!   screen pixels (gone), others to 1 (visible). Drawing the grid as
//!   Slint `Rectangle`s in screen coordinates pins every line to a
//!   uniform 1 logical pixel regardless of the displayed scale, which
//!   is what the egui original does via `painter.line_segment`.

use std::ops::RangeInclusive;
use std::rc::Rc;

use bytesize::ByteSize;
use image::{ImageBuffer, Rgba};
use infinitier_core::game::GameResource;
use infinitier_core::imported_resource::tis::ImportedTis;
use infinitier_core::resource::tis::Type as TisType;
use slint::{Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};

use crate::MainWindow;
use crate::state::AppState;
use crate::ui::viewers::common;

/// Conservative texture-side cap. Slint's femtovg backend ultimately
/// dispatches to a GL renderer whose `MAX_TEXTURE_SIZE` is 8192 on
/// every desktop GPU we care about. The egui original asks egui for
/// this number; here we just hard-code the floor.
const MAX_TEXTURE_SIDE: u32 = 8192;

pub fn populate(
    window: &MainWindow,
    state: &Rc<AppState>,
    tis: ImportedTis,
    resource: &GameResource,
) {
    let limits = ColumnLimits::new(tis.tile_count, tis.tile_dimension, MAX_TEXTURE_SIDE);
    let columns = tis
        .expected_columns
        .clamp(*limits.range.start(), *limits.range.end());

    let viewer = TisViewerState {
        file_size_text: common::file_size_text(resource),
        origin_text: common::origin_text(resource),
        columns,
        slider_range: limits.range,
        tiles_per_axis: limits.tiles_per_axis,
        truncated: limits.truncated,
        show_grid: false,
        rendered_columns: None,
        tis,
    };
    *state.tis_viewer.borrow_mut() = Some(viewer);

    window.set_viewer_kind("tis".into());
    refresh_props(window, state);
    refresh_bitmap(window, state);
}

/// Slider tick. Called from the playback timer; recomposes only when
/// the columns selection drifted from the last upload.
pub fn tick(window: &MainWindow, state: &Rc<AppState>) {
    let needs_recompose = {
        let guard = state.tis_viewer.borrow();
        let Some(tv) = guard.as_ref() else { return };
        tv.rendered_columns != Some(tv.columns)
    };
    if needs_recompose {
        refresh_bitmap(window, state);
    }
}

pub fn on_columns_changed(window: &MainWindow, state: &Rc<AppState>, value: i32) {
    {
        let mut guard = state.tis_viewer.borrow_mut();
        let Some(tv) = guard.as_mut() else { return };
        let Ok(v) = u32::try_from(value) else { return };
        let v = v.clamp(*tv.slider_range.start(), *tv.slider_range.end());
        if v == tv.columns {
            return;
        }
        tv.columns = v;
    }
    // Light refresh only — the heavy `compose` runs from `tick`.
    refresh_props(window, state);
}

pub fn on_show_grid_changed(window: &MainWindow, state: &Rc<AppState>, value: bool) {
    {
        let mut guard = state.tis_viewer.borrow_mut();
        let Some(tv) = guard.as_mut() else { return };
        if tv.show_grid == value {
            return;
        }
        tv.show_grid = value;
    }
    // Grid is overlaid by Slint in screen pixels, so toggling it never
    // requires a recompose.
    refresh_props(window, state);
}

// ── Property refresh paths ────────────────────────────────────────────────────

/// Cheap pass: every property except `tis_bitmap`. Run on every
/// callback so the slider, label, and grid follow the cursor in real
/// time without waiting for the heavy `compose`.
fn refresh_props(window: &MainWindow, state: &Rc<AppState>) {
    let guard = state.tis_viewer.borrow();
    let Some(tv) = guard.as_ref() else { return };

    let (cols, rows) = grid_dims(tv);
    let tile_dim = tv.tis.tile_dimension;
    let w_px = cols * tile_dim;
    let h_px = rows * tile_dim;

    window.set_tis_dims(format!("{w_px} × {h_px} px").into());
    window.set_tis_file_size(tv.file_size_text.clone().into());
    window.set_tis_variant(
        match tv.tis.variant {
            TisType::Palette => "Palette",
            TisType::Pvrz => "PVRZ",
        }
        .into(),
    );
    window.set_tis_tile_count(format!("{} tiles", tv.tis.tile_count).into());
    window.set_tis_origin(tv.origin_text.clone().into());

    let grid_dims_text = if tv.truncated {
        let visible = tv.columns * tv.tiles_per_axis;
        format!(
            "{} × {} tiles (showing first {} of {})",
            tv.columns,
            tv.tiles_per_axis,
            visible.min(tv.tis.tile_count as u32),
            tv.tis.tile_count,
        )
    } else {
        format!("{} × {} tiles", tv.columns, rows)
    };
    window.set_tis_grid_dims(grid_dims_text.into());

    window.set_tis_columns(tv.columns as i32);
    window.set_tis_columns_min(*tv.slider_range.start() as i32);
    window.set_tis_columns_max(*tv.slider_range.end() as i32);
    window.set_tis_show_grid(tv.show_grid);
    window.set_tis_tile_dim(tile_dim as i32);
    window.set_tis_warning(
        if tv.truncated {
            "⚠ Tileset too large; showing partial view".to_string()
        } else {
            String::new()
        }
        .into(),
    );

    // Grid-line indices. Each entry is "draw a 1-pixel line at column
    // / row `i`" — Slint multiplies by the displayed tile pitch to get
    // the screen-pixel x / y. We push (cols+1) and (rows+1) entries so
    // the outer borders are included.
    let v_indices: Vec<i32> = (0..=cols as i32).collect();
    let h_indices: Vec<i32> = (0..=rows as i32).collect();
    window.set_tis_grid_vlines(ModelRc::new(VecModel::from(v_indices)));
    window.set_tis_grid_hlines(ModelRc::new(VecModel::from(h_indices)));
}

/// Heavy pass: recompose the tileset into an RGBA buffer and upload
/// it to the `tis_bitmap` property. Skipped when the previous upload
/// already matches the current columns.
fn refresh_bitmap(window: &MainWindow, state: &Rc<AppState>) {
    let mut guard = state.tis_viewer.borrow_mut();
    let Some(tv) = guard.as_mut() else { return };
    if tv.rendered_columns == Some(tv.columns) {
        return;
    }
    let img = compose(tv);
    let w = img.width();
    let h = img.height();
    let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(img.as_raw(), w, h);
    window.set_tis_bitmap(Image::from_rgba8(pixels));
    window.set_tis_dims(format!("{w} × {h} px").into());
    tv.rendered_columns = Some(tv.columns);
}

// ── State ─────────────────────────────────────────────────────────────────────

pub struct TisViewerState {
    pub tis: ImportedTis,
    pub file_size_text: String,
    pub origin_text: String,
    pub columns: u32,
    pub slider_range: RangeInclusive<u32>,
    pub tiles_per_axis: u32,
    pub truncated: bool,
    pub show_grid: bool,
    /// `columns` value currently uploaded as `tis_bitmap`. `None`
    /// forces a recompose on the next tick. `show_grid` is *not* part
    /// of the key — the grid lives on the Slint side and toggling it
    /// never needs a new bitmap.
    pub rendered_columns: Option<u32>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// `(cols, rows)` of the composite at the current selection. Truncated
/// tilesets pin rows to `tiles_per_axis`.
fn grid_dims(tv: &TisViewerState) -> (u32, u32) {
    if tv.truncated {
        (tv.columns, tv.tiles_per_axis)
    } else {
        let rows = (tv.tis.tile_count as u32).div_ceil(tv.columns);
        (tv.columns, rows)
    }
}

/// Composite the tileset to an RGBA buffer. The truncated path renders
/// only the leading slice of tiles that fits — see [`ColumnLimits`].
fn compose(tv: &TisViewerState) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    if tv.truncated {
        let visible = (tv.columns as usize) * (tv.tiles_per_axis as usize);
        let count = tv.tis.tile_count.min(visible);
        let stride = (tv.tis.tile_dimension as usize) * (tv.tis.tile_dimension as usize) * 4;
        let view = ImportedTis {
            tile_dimension: tv.tis.tile_dimension,
            tile_count: count,
            tile_pixels: tv.tis.tile_pixels[..count * stride].to_vec(),
            expected_columns: tv.tis.expected_columns,
            variant: tv.tis.variant,
        };
        view.compose(tv.columns)
    } else {
        tv.tis.compose(tv.columns)
    }
}

// ── Slider bounds ─────────────────────────────────────────────────────────────

struct ColumnLimits {
    tiles_per_axis: u32,
    range: RangeInclusive<u32>,
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

impl TisViewerState {
    #[allow(dead_code)]
    pub fn file_size_bytesize(bytes: u64) -> String {
        ByteSize(bytes).to_string()
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
        assert_eq!(*l.range.start(), 19);
        assert_eq!(*l.range.end(), 128);
        assert!(!l.truncated);
    }

    #[test]
    fn column_limits_too_large_tileset_truncates() {
        let l = ColumnLimits::new(20_000, 64, 8192);
        assert_eq!(l.range, 128..=128);
        assert!(l.truncated);
    }
}
