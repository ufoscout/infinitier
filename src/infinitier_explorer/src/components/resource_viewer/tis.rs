use super::ResourceViewerTrait;
use bytesize::ByteSize;
use eframe::egui::{self, TextureHandle};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::tis::ImportedTis,
    resource::tis::Type as TisType,
};

/// TIS tileset viewer.
///
/// Mirrors NearInfinity's `TisResource` panel:
/// - top bar with a "tiles per row" slider and a "Show grid" checkbox,
/// - the composite tileset image rendered below,
/// - bottom info bar with dimensions, file size, variant, and origin.
///
/// The slider's default value comes from
/// [`ImportedTis::expected_columns`] — sourced from the area's WED when
/// one is registered, otherwise the NearInfinity fallback
/// (`min(tile_count, 5)`).
pub struct TisViewer {
    tis: ImportedTis,
    texture: TextureHandle,
    /// Currently-selected tiles-per-row. Always inside `slider_range`.
    columns: u32,
    /// Inclusive bounds for the slider, picked so the composite never
    /// exceeds the renderer's `max_texture_side` on either axis.
    slider_range: std::ops::RangeInclusive<u32>,
    /// Tiles per axis that fit in `max_texture_side`. Cached on the
    /// struct because the truncation path needs it at compose time
    /// and it doesn't change for the viewer's lifetime.
    tiles_per_axis: u32,
    /// `true` when the tileset is so large (>`tiles_per_axis²`) that
    /// even the squarest arrangement overflows the texture limit, so
    /// what's rendered is the leading slice that fits. Surfaced in
    /// the bottom info bar.
    truncated: bool,
    /// Whether to draw a tile-boundary grid on top of the image.
    show_grid: bool,
    /// `Some(cols)` while the texture matches that column count;
    /// `None` forces a re-composite on the next show.
    rendered_columns: Option<u32>,
}

impl TisViewer {
    pub fn new(tis: ImportedTis, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
        let max_side = ui.ctx().input(|i| i.max_texture_side) as u32;
        let limits = ColumnLimits::new(tis.tile_count, tis.tile_dimension, max_side);
        let columns = tis
            .expected_columns
            .clamp(*limits.range.start(), *limits.range.end());

        // Placeholder texture — `refresh_texture` immediately replaces
        // it with the composite at `columns`.
        let texture = ui.ctx().load_texture(
            format!("tis_{resource_id}"),
            egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]),
            egui::TextureOptions::default(),
        );

        let mut view = Self {
            tis,
            texture,
            columns,
            slider_range: limits.range,
            tiles_per_axis: limits.tiles_per_axis,
            truncated: limits.truncated,
            show_grid: false,
            rendered_columns: None,
        };
        view.refresh_texture();
        view
    }

    /// Re-composite tiles into a new image and upload it to the GPU
    /// texture, but only if the columns selection changed since the
    /// last upload. Tile compositing is a CPU memcpy per tile so this
    /// is cheap for typical tilesets; doing it lazily still spares us
    /// from re-uploading on every frame.
    fn refresh_texture(&mut self) {
        if self.rendered_columns == Some(self.columns) {
            return;
        }
        // The truncated path renders only the leading slice of tiles
        // that fits — see `ColumnLimits`. For everything else just
        // hand the whole tileset to `compose`.
        let image = if self.truncated {
            let visible = (self.columns as usize) * (self.tiles_per_axis as usize);
            let count = self.tis.tile_count.min(visible);
            let stride =
                (self.tis.tile_dimension as usize) * (self.tis.tile_dimension as usize) * 4;
            // Compose works off a borrowed view: clone only the leading
            // `count * stride` bytes into a smaller `ImportedTis`.
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
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        self.texture.set(color, egui::TextureOptions::default());
        self.rendered_columns = Some(self.columns);
    }

    fn variant_label(&self) -> &'static str {
        match self.tis.variant {
            TisType::Palette => "Palette",
            TisType::Pvrz => "PVRZ",
        }
    }
}

/// Composite-image limits derived from the renderer's
/// `max_texture_side` and the fixed 64-pixel TIS tile size.
struct ColumnLimits {
    /// Tiles per axis that fit in a single texture.
    tiles_per_axis: u32,
    /// Inclusive bounds the slider must stay within.
    range: std::ops::RangeInclusive<u32>,
    /// `true` when not every tile fits — see [`TisViewer::truncated`].
    truncated: bool,
}

impl ColumnLimits {
    /// Compute the safe `(min..=max)` columns range so that both
    /// `columns * tile_dim` (width) and `rows * tile_dim` (height) stay
    /// within the renderer's limit. When the tileset is so large that
    /// no value satisfies both, falls back to the widest fitting grid
    /// and flags the result as truncated.
    fn new(tile_count: usize, tile_dim: u32, max_side: u32) -> Self {
        let tile_count = tile_count.max(1) as u32;
        let tiles_per_axis = (max_side / tile_dim.max(1)).max(1);

        // Width:  columns ≤ tiles_per_axis           ⇒ width ≤ max_side
        // Height: rows = ceil(tile_count / columns) ≤ tiles_per_axis
        //         ⇒ columns ≥ ceil(tile_count / tiles_per_axis)
        let max_cols = tile_count.min(tiles_per_axis);
        let min_cols = tile_count.div_ceil(tiles_per_axis);

        if min_cols > max_cols {
            // tile_count > tiles_per_axis² — pin to the widest fitting
            // grid. The renderer will paint only the first
            // `tiles_per_axis²` tiles; the rest are hidden.
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

impl ResourceViewerTrait for TisViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        // ── Bottom info bar (rendered first so the central area takes
        //     the rest of the space) ──────────────────────────────────
        let composed_size = self.texture.size();
        egui::Panel::bottom("tis_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{} × {} px", composed_size[0], composed_size[1]));
                ui.separator();
                match resource.file_size {
                    Some(size) => {
                        ui.label(ByteSize(size).to_string());
                    }
                    None => {
                        ui.label("? B");
                    }
                }
                ui.separator();
                ui.label("TIS");
                ui.separator();
                ui.label(self.variant_label());
                ui.separator();
                ui.label(format!("{} tiles", self.tis.tile_count));
                ui.separator();
                if self.truncated {
                    // The displayed grid is the first
                    // `columns × tiles_per_axis` tiles; the rest don't
                    // fit in the renderer's max texture side.
                    let visible = self.columns * self.tiles_per_axis;
                    ui.label(format!(
                        "{} × {} tiles (showing first {} of {})",
                        self.columns,
                        self.tiles_per_axis,
                        visible.min(self.tis.tile_count as u32),
                        self.tis.tile_count,
                    ));
                } else {
                    let rows = (self.tis.tile_count as u32).div_ceil(self.columns);
                    ui.label(format!("{} × {} tiles", self.columns, rows));
                }
                ui.separator();
                match &resource.data_origin {
                    DataOrigin::Bif { name } => {
                        ui.label(format!("BIF: {name}"));
                    }
                    DataOrigin::Dir { name, path } => {
                        ui.label(format!("{name}: {}", path.path().display()));
                    }
                    DataOrigin::Missing => {
                        ui.label("Missing");
                    }
                }
            });
        });

        // ── Top control bar: tiles-per-row slider + grid toggle ──────
        egui::Panel::top("tis_controls_panel").show_inside(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Tiles per row:");
                // Slider range is clamped so the composite never
                // exceeds `max_texture_side` on either axis — see
                // `ColumnLimits`. Without this clamp, dragging the
                // slider toward 1 caused the height to overflow the
                // GPU limit (e.g. 15168 > 8192) and panicked the
                // renderer in wgpu's `create_texture`.
                let range = self.slider_range.clone();
                let max = *range.end();
                ui.add(
                    egui::Slider::new(&mut self.columns, range)
                        .integer()
                        .text(format!("/ {max}")),
                );
                ui.separator();
                ui.checkbox(&mut self.show_grid, "Show grid");
                if self.truncated {
                    ui.separator();
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 160, 0),
                        "⚠ Tileset too large; showing partial view",
                    );
                }
            });
            ui.add_space(4.0);
        });

        // Recompose if the slider moved this tick.
        self.refresh_texture();

        // ── Center: the composite image, fit to the available space ──
        let available = ui.available_size();
        let natural = egui::Vec2::new(composed_size[0] as f32, composed_size[1] as f32);
        // Allow the image to scale up only if needed; otherwise keep
        // its natural size so tiles render pixel-exact.
        let scale = (available.x / natural.x)
            .min(available.y / natural.y)
            .min(1.0);
        let display = natural * scale;

        let y_offset = ((available.y - display.y) / 2.0).max(0.0);
        if y_offset > 0.0 {
            ui.add_space(y_offset);
        }
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            let response = ui.add(egui::Image::new(&self.texture).fit_to_exact_size(display));

            if self.show_grid {
                paint_tile_grid(ui, response.rect, self.tis.tile_dimension as f32 * scale);
            }
        });
    }
}

/// Draw a thin grid over `image_rect` with one line per tile boundary,
/// at the same scale the image is displayed at. The outermost border
/// is included so the bottom and right edges of the tileset are also
/// marked.
fn paint_tile_grid(ui: &mut egui::Ui, image_rect: egui::Rect, tile_size: f32) {
    if tile_size <= 0.5 {
        // At this zoom level the grid would be denser than the pixels
        // it's supposed to outline — skip it to avoid a moiré pattern.
        return;
    }
    let painter = ui.painter_at(image_rect);
    let stroke = egui::Stroke::new(
        1.0,
        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180),
    );

    // Vertical lines at column boundaries.
    let mut x = image_rect.left();
    while x <= image_rect.right() + 0.5 {
        painter.line_segment(
            [
                egui::pos2(x, image_rect.top()),
                egui::pos2(x, image_rect.bottom()),
            ],
            stroke,
        );
        x += tile_size;
    }
    // Horizontal lines at row boundaries.
    let mut y = image_rect.top();
    while y <= image_rect.bottom() + 0.5 {
        painter.line_segment(
            [
                egui::pos2(image_rect.left(), y),
                egui::pos2(image_rect.right(), y),
            ],
            stroke,
        );
        y += tile_size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_limits_small_tileset_fits_with_no_min_constraint() {
        // 6 tiles, 64 px tiles, 8192 px max → tiles_per_axis = 128.
        // ceil(6 / 128) = 1, so slider can go from 1 to 6.
        let l = ColumnLimits::new(6, 64, 8192);
        assert_eq!(l.tiles_per_axis, 128);
        assert_eq!(l.range, 1..=6);
        assert!(!l.truncated);
    }

    #[test]
    fn column_limits_medium_tileset_enforces_min() {
        // 2400 tiles, 64 px tiles, 8192 px max → tiles_per_axis = 128.
        // ceil(2400 / 128) = 19, so the slider can't go below 19 (else
        // rows × 64 would exceed 8192 — the original wgpu panic).
        let l = ColumnLimits::new(2400, 64, 8192);
        assert_eq!(l.tiles_per_axis, 128);
        assert_eq!(*l.range.start(), 19);
        assert_eq!(*l.range.end(), 128);
        assert!(!l.truncated);
    }

    #[test]
    fn column_limits_too_large_tileset_truncates() {
        // tiles_per_axis = 128, tile_count = 20_000 > 128² = 16_384.
        // No columns value lets every tile fit — collapse the slider
        // to the widest fitting grid and flag truncation.
        let l = ColumnLimits::new(20_000, 64, 8192);
        assert_eq!(l.tiles_per_axis, 128);
        assert_eq!(l.range, 128..=128);
        assert!(l.truncated);
    }

    #[test]
    fn column_limits_clamps_to_at_least_one() {
        // Pathological inputs (zero tiles or absurdly small limits)
        // must not produce an empty slider range — that would crash
        // `Slider::new` with an unsatisfiable bound.
        let l = ColumnLimits::new(0, 64, 8192);
        assert_eq!(l.range, 1..=1);

        let tiny = ColumnLimits::new(10, 64, 32); // max_side < tile_dim
        assert_eq!(tiny.tiles_per_axis, 1);
        assert!(*tiny.range.start() >= 1);
    }

    #[test]
    fn column_limits_specific_wgpu_panic_case() {
        // The original bug report: a 2.4k-ish tileset, slider down to
        // a tile-per-row that puts the composite at 15168 px tall —
        // wgpu rejects anything > 8192. With the clamp the slider's
        // minimum must produce a height ≤ 8192.
        let tile_count = 2400usize;
        let l = ColumnLimits::new(tile_count, 64, 8192);
        let min_cols = *l.range.start();
        let height_at_min = (tile_count as u32).div_ceil(min_cols) * 64;
        assert!(
            height_at_min <= 8192,
            "min_cols={min_cols} ⇒ height={height_at_min} (should be ≤ 8192)"
        );
    }
}
