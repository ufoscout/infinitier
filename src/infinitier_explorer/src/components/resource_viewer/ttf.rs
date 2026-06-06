use std::sync::Arc;

use bytesize::ByteSize;
use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, RichText};
use egui_components::scroll_area::ScrollArea;
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::ttf::Ttf,
};

use super::ResourceViewerTrait;

/// TTF viewer.
///
/// Shows the font's `name`-table metadata and lets the user type a
/// sample sentence rendered in the actual loaded font at several sizes.
/// Rendering goes through egui's own text pipeline: at construction
/// time we install the font into the egui context under a unique
/// family name (derived from `resource_id`) and reference it via
/// [`FontFamily::Name`] when drawing sample lines. That avoids pulling
/// in a separate rasteriser and stays pure-Rust.
pub struct TtfViewer {
    ttf: Ttf,
    /// Custom font family we installed into `egui::Context`'s
    /// `FontDefinitions`. Used for every sample-text draw — but only
    /// once [`TtfViewer::active_family`] confirms egui has actually
    /// rebuilt its atlas around it (see comment there).
    font_family: FontFamily,
    sample_text: String,
    sizes: Vec<f32>,
}

impl TtfViewer {
    pub fn new(ttf: Ttf, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
        let key = format!("ttf_viewer_{resource_id}");
        install_font_in_egui(ui.ctx(), &key, &ttf);

        Self {
            ttf,
            font_family: FontFamily::Name(key.into()),
            sample_text: "The quick brown fox jumps over the lazy dog. 0123456789".to_string(),
            sizes: vec![12.0, 16.0, 24.0, 36.0, 48.0],
        }
    }

    /// Returns the font family the sample text should be drawn with on
    /// this frame.
    ///
    /// `Context::set_fonts` is deferred: it queues the new font
    /// definitions for the *next* frame's atlas rebuild, so the very
    /// first `show()` after we call it would otherwise panic with
    /// `FontFamily::Name(...) is not bound to any fonts` when we try
    /// to lay out text. Checking the current `Fonts::families()` and
    /// falling back to `Proportional` until our family appears makes
    /// the first frame degrade gracefully; `request_repaint` ensures
    /// egui re-renders as soon as the atlas catches up.
    fn active_family(&self, ctx: &egui::Context) -> FontFamily {
        let ready = ctx.fonts(|f| f.families().contains(&self.font_family));
        if ready {
            self.font_family.clone()
        } else {
            ctx.request_repaint();
            FontFamily::Proportional
        }
    }
}

impl ResourceViewerTrait for TtfViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        // ── Bottom info bar ────────────────────────────────────────
        egui::Panel::bottom("ttf_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("TTF");
                ui.separator();
                ui.label(format!(
                    "{} {}",
                    self.ttf.family_name, self.ttf.subfamily_name
                ));
                ui.separator();
                ui.label(format!("{} glyphs", self.ttf.glyph_count));
                ui.separator();
                ui.label(format!(
                    "em={} asc={} desc={} line_gap={}",
                    self.ttf.units_per_em, self.ttf.ascender, self.ttf.descender, self.ttf.line_gap,
                ));
                ui.separator();
                if self.ttf.is_monospaced {
                    ui.label("monospaced");
                    ui.separator();
                }
                match resource.file_size {
                    Some(size) => ui.label(ByteSize(size).to_string()),
                    None => ui.label("? B"),
                };
                ui.separator();
                match &resource.data_origin {
                    DataOrigin::Bif { name } => ui.label(format!("BIF: {name}")),
                    DataOrigin::Dir { name, path } => {
                        ui.label(format!("{name}: {}", path.path().display()))
                    }
                    DataOrigin::Unhardcoded { folder } => ui.label(format!("unhardcoded/{folder}")),
                    DataOrigin::Missing => ui.label("Missing"),
                };
            });
        });

        // Resolve which family we can actually use this frame — see
        // `active_family` for the deferred-atlas-rebuild dance.
        let family = self.active_family(ui.ctx());

        // ── Main content: header card + sample-text playground ─────
        ScrollArea::vertical().show(ui, |ui| {
            self.show_header(ui, &family);
            ui.separator();
            self.show_sample_text(ui, &family);
        });
    }
}

impl TtfViewer {
    /// Header card: the typeface's display name in its own font, plus
    /// the optional designer / copyright / version strings underneath.
    fn show_header(&self, ui: &mut egui::Ui, family: &FontFamily) {
        ui.add_space(8.0);
        ui.label(RichText::new(&self.ttf.full_name).font(FontId::new(36.0, family.clone())));
        ui.add_space(4.0);

        let mut details = Vec::new();
        if let Some(v) = &self.ttf.version {
            details.push(("Version", v.clone()));
        }
        if let Some(d) = &self.ttf.designer {
            details.push(("Designer", d.clone()));
        }
        if let Some(m) = &self.ttf.manufacturer {
            details.push(("Foundry", m.clone()));
        }
        if let Some(c) = &self.ttf.copyright {
            details.push(("Copyright", c.clone()));
        }
        if let Some(ps) = &self.ttf.postscript_name {
            details.push(("PostScript", ps.clone()));
        }
        egui::Grid::new("ttf_metadata_grid")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (label, value) in details {
                    ui.label(RichText::new(label).strong());
                    ui.label(value);
                    ui.end_row();
                }
            });
    }

    /// Sample-text panel: an editable string + one rendered line per
    /// configured size. The same string is drawn at every size so the
    /// reader sees how the font looks across the typical UI range.
    fn show_sample_text(&mut self, ui: &mut egui::Ui, family: &FontFamily) {
        ui.label(RichText::new("Sample text").strong());
        ui.add(
            egui::TextEdit::singleline(&mut self.sample_text)
                .desired_width(f32::INFINITY)
                .hint_text("Type a sample sentence…"),
        );
        ui.add_space(8.0);

        for &size in &self.sizes {
            let label_text = if self.sample_text.is_empty() {
                // egui collapses empty Text widgets; show a placeholder
                // glyph row instead so the size sizing stays visible.
                "(empty)".to_string()
            } else {
                self.sample_text.clone()
            };
            ui.label(format!("{size:>3.0} px:"));
            ui.label(RichText::new(label_text).font(FontId::new(size, family.clone())));
            ui.add_space(8.0);
        }
    }
}

/// Install `ttf` into `ctx`'s font definitions under `key`, preserving
/// every default font already configured. The font becomes available
/// to draw calls via [`FontFamily::Name`]`(key)`.
///
/// Each call rebuilds egui's font atlas (the price of a font swap is
/// paid by `set_fonts`). The viewer calls this exactly once at
/// construction time, so the cost is paid on resource open rather than
/// every frame.
fn install_font_in_egui(ctx: &egui::Context, key: &str, ttf: &Ttf) {
    let mut fonts = FontDefinitions::default();
    // Clone the raw bytes for egui — the Arc'd payload stays shared
    // with anything else holding the `Ttf` (we don't take it from
    // under the importer).
    let data = FontData::from_owned((*ttf.raw).clone());
    fonts.font_data.insert(key.to_string(), Arc::new(data));
    fonts.families.insert(
        FontFamily::Name(key.to_string().into()),
        vec![key.to_string()],
    );
    ctx.set_fonts(fonts);
}
