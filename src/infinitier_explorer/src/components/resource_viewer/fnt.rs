use bytesize::ByteSize;
use eframe::egui::{self, RichText};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::fnt::Fnt,
};

use super::ResourceViewerTrait;

/// FNT (bitmap font) viewer.
///
/// Shows what the importer can reliably extract from the
/// Enhanced-Edition pre-2.0 FNT format:
///
/// - Header summary (glyph count + the unknown small header fields,
///   surfaced for hex-comparison work).
/// - Character coverage — every Unicode code point the font claims to
///   support, grouped by Unicode block, with printable code points
///   rendered next to their numeric value.
/// - A truncated hex dump of the un-parsed body bytes (per-glyph
///   metrics + pixel/coverage floats, an undocumented format — see
///   `infinitier_fnt_resource`'s module docs).
///
/// Bitmap glyph rendering is **not** implemented: the body section
/// uses an SDF/coverage-mask float layout that isn't documented
/// anywhere I could find, and shipping a half-right renderer that
/// silently distorts glyphs would be worse than not rendering them at
/// all. NearInfinity itself only parses the *post*-2.0 stub variant.
pub struct FntViewer {
    fnt: Fnt,
}

impl FntViewer {
    pub fn new(fnt: Fnt) -> Self {
        Self { fnt }
    }
}

impl ResourceViewerTrait for FntViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        // ── Bottom info bar ────────────────────────────────────────
        egui::Panel::bottom("fnt_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("FNT");
                ui.separator();
                ui.label(format!("{} glyphs", self.fnt.glyph_count));
                ui.separator();
                ui.label(format!(
                    "header: f4={} f6={} f8={} fC={}",
                    self.fnt.field_4, self.fnt.field_6, self.fnt.field_8, self.fnt.field_c
                ));
                ui.separator();
                match resource.file_size {
                    Some(size) => ui.label(ByteSize(size).to_string()),
                    None => ui.label("? B"),
                };
                ui.separator();
                ui.label(format!("body: {} B", self.fnt.body().len()));
                ui.separator();
                match &resource.data_origin {
                    DataOrigin::Bif { name } => ui.label(format!("BIF: {name}")),
                    DataOrigin::Dir { name, path } => {
                        ui.label(format!("{name}: {}", path.path().display()))
                    }
                    DataOrigin::Missing => ui.label("Missing"),
                };
            });
        });

        // ── Main scroll area: warning + coverage + hex dump ────────
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 0),
                "⚠ FNT bitmap rendering is not implemented. The per-glyph metric and \
                 coverage-mask sections use an undocumented float layout.",
            );
            ui.add_space(8.0);

            self.show_coverage(ui);
            ui.separator();
            self.show_body_preview(ui);
        });
    }
}

impl FntViewer {
    /// Character coverage grid: for every code point in the font, show
    /// the codepoint in hex and (if printable) the glyph rendered in
    /// egui's default font.
    ///
    /// We intentionally do NOT render in the loaded FNT — the body
    /// section's bitmaps aren't parsed yet. Showing the same glyph in
    /// the default font lets the user verify *which* characters the
    /// font is supposed to cover.
    fn show_coverage(&self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Character coverage")
                .strong()
                .size(16.0),
        );
        ui.add_space(4.0);
        ui.label(
            "Code points covered by this font, rendered in egui's default font \
             (FNT bitmap data is not yet decoded).",
        );
        ui.add_space(8.0);

        // Lay out as a grid of "U+XXXX  c" cells, ~12 per row, in code
        // order — matches the file's natural ordering and makes it
        // easy to spot gaps.
        const COLUMNS: usize = 12;
        egui::Grid::new("fnt_coverage_grid")
            .num_columns(COLUMNS)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (i, &code) in self.fnt.character_codes.iter().enumerate() {
                    let ch = char::from_u32(code);
                    let glyph = ch
                        .filter(|c| !c.is_control())
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "·".to_string());
                    ui.label(
                        RichText::new(format!("U+{code:04X} {glyph}"))
                            .monospace(),
                    );
                    if (i + 1) % COLUMNS == 0 {
                        ui.end_row();
                    }
                }
                // Close the last incomplete row.
                if !self.fnt.character_codes.len().is_multiple_of(COLUMNS) {
                    ui.end_row();
                }
            });
    }

    /// First N bytes of the un-parsed body as a hex dump. Useful while
    /// the format is being reverse-engineered.
    fn show_body_preview(&self, ui: &mut egui::Ui) {
        const PREVIEW_BYTES: usize = 256;

        ui.label(
            RichText::new("Body (un-parsed metric + bitmap section)")
                .strong()
                .size(16.0),
        );
        ui.add_space(4.0);
        let body = self.fnt.body();
        let shown = body.len().min(PREVIEW_BYTES);
        ui.label(format!(
            "Showing first {shown} of {} bytes (offset 0x{:X} in file).",
            body.len(),
            self.fnt.body_offset
        ));
        ui.add_space(4.0);

        // 16 bytes per row, "offset  hex  ascii".
        let mut dump = String::with_capacity(shown * 4);
        for (i, chunk) in body[..shown].chunks(16).enumerate() {
            dump.push_str(&format!("{:08x}  ", self.fnt.body_offset + i * 16));
            for b in chunk {
                dump.push_str(&format!("{:02x} ", b));
            }
            // Pad shorter rows to keep columns aligned.
            for _ in chunk.len()..16 {
                dump.push_str("   ");
            }
            dump.push(' ');
            for &b in chunk {
                dump.push(if (32..127).contains(&b) { b as char } else { '.' });
            }
            dump.push('\n');
        }
        ui.add(
            egui::TextEdit::multiline(&mut dump.as_str())
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
    }
}
