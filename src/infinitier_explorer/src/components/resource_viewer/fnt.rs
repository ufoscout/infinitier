use bytesize::ByteSize;
use eframe::egui::{self, RichText};
use egui_components::scroll_area::ScrollArea;
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::fnt::{Fnt, HEADER_LEN},
};

use super::ResourceViewerTrait;

/// FNT viewer modelled on NearInfinity's `FntResource` struct view.
///
/// FNT is a 4-byte "font envelope": just a `# extra letters` count.
/// The `Letters` BAM and `Extra letters` BMP that NI shows are not
/// stored in the file — they're synthesised from the FNT's own
/// resource name (`DIALOG.FNT` ⇒ `DIALOG.BAM` + `DIALOG.BMP`). The
/// importer does the same.
///
/// Layout:
/// - **Top**: a four-column `Attribute / Value / Offset / Size`
///   grid mirroring NI's "Edit" tab (the screenshot the user shared).
/// - **Middle**: an annotated header (in case the file is bigger than
///   4 bytes — vanilla EE FNTs are 23–100 KB) telling the user that
///   the trailing bytes are engine-internal opaque data.
/// - **Bottom info bar**: file size, body size, data origin — same
///   shape as the other viewers.
///
/// No bitmap rendering: NI itself doesn't render the FNT glyphs (the
/// linked BAM/BMP are opened as their own resources), and the opaque
/// trailing bytes use an undocumented float layout.
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
                ui.label(format!("# extra letters: {}", self.fnt.extra_letters_count));
                ui.separator();
                match resource.file_size {
                    Some(size) => ui.label(ByteSize(size).to_string()),
                    None => ui.label("? B"),
                };
                ui.separator();
                ui.label(format!("body: {} B (opaque)", self.fnt.body().len()));
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

        // ── Main scroll area ───────────────────────────────────────
        ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(8.0);
            self.show_struct_table(ui);
            ui.add_space(12.0);
            self.show_format_note(ui);
            ui.add_space(12.0);
            self.show_body_preview(ui);
        });
    }
}

impl FntViewer {
    /// NI-style struct table — same columns as the screenshot.
    fn show_struct_table(&self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        egui::Grid::new("fnt_struct_grid")
            .num_columns(4)
            .striped(true)
            .spacing([16.0, 4.0])
            .show(ui, |ui| {
                // Header row.
                ui.label(RichText::new("Attribute").strong());
                ui.label(RichText::new("Value").strong());
                ui.label(RichText::new("Offset").strong());
                ui.label(RichText::new("Size").strong());
                ui.end_row();

                // # extra letters — the only field actually read from the file.
                ui.label("# extra letters");
                ui.label(self.fnt.extra_letters_count.to_string());
                ui.label("0 h");
                ui.label(HEADER_LEN.to_string());
                ui.end_row();

                // Letters — synthesised BAM ref. NI shows offset 0 / size 8
                // because that's a `ResourceRef`'s nominal layout, even
                // though the bytes never appear in the FNT.
                ui.label("Letters");
                ui.label(&self.fnt.letters_bam);
                ui.label("0 h");
                ui.label("8");
                ui.end_row();

                // Extra letters — synthesised BMP ref, same convention.
                ui.label("Extra letters");
                ui.label(&self.fnt.extra_letters_bmp);
                ui.label("0 h");
                ui.label("8");
                ui.end_row();
            });
    }

    /// One-line clarifier — keeps users from wondering why the file is
    /// 100 KB on disk but the viewer only shows three fields.
    fn show_format_note(&self, ui: &mut egui::Ui) {
        let body_len = self.fnt.body().len();
        if body_len == 0 {
            return;
        }
        ui.colored_label(
            egui::Color32::from_rgb(160, 160, 160),
            format!(
                "Note: FNT is a stub. Glyph data lives in {} and {}; \
                 the {body_len} bytes past offset 0x04 in this file are \
                 engine-internal and not parsed (NearInfinity treats them \
                 the same way).",
                self.fnt.letters_bam, self.fnt.extra_letters_bmp,
            ),
        );
    }

    /// First N bytes of the un-parsed body as a hex dump — same
    /// content NI's "Raw" tab would surface.
    fn show_body_preview(&self, ui: &mut egui::Ui) {
        const PREVIEW_BYTES: usize = 256;

        ui.label(RichText::new("Raw (post-header body)").strong().size(16.0));
        ui.add_space(4.0);
        let body = self.fnt.body();
        if body.is_empty() {
            ui.label("(no body bytes)");
            return;
        }
        let shown = body.len().min(PREVIEW_BYTES);
        ui.label(format!(
            "Showing first {shown} of {} bytes (offset 0x{HEADER_LEN:X} in file).",
            body.len(),
        ));
        ui.add_space(4.0);

        let mut dump = String::with_capacity(shown * 4);
        for (i, chunk) in body[..shown].chunks(16).enumerate() {
            dump.push_str(&format!("{:08x}  ", HEADER_LEN + i * 16));
            for b in chunk {
                dump.push_str(&format!("{:02x} ", b));
            }
            for _ in chunk.len()..16 {
                dump.push_str("   ");
            }
            dump.push(' ');
            for &b in chunk {
                dump.push(if (32..127).contains(&b) {
                    b as char
                } else {
                    '.'
                });
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
