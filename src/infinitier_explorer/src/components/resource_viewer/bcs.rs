use super::ResourceViewerTrait;
use bytesize::ByteSize;
use eframe::egui;
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::bcs::ImportedBcs,
};

pub struct BcsViewer {
    bcs: ImportedBcs,
}

impl BcsViewer {
    pub fn new(bcs: ImportedBcs) -> Self {
        Self { bcs }
    }
}

impl ResourceViewerTrait for BcsViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        // ── Bottom info bar ───────────────────────────────────────────────────
        let cr_count = self.bcs.bcs.condition_responses.len();
        let baf_lines = self.bcs.baf.lines().count();

        egui::Panel::bottom("bcs_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("{cr_count} CR blocks"));
                ui.separator();
                ui.label(format!("{baf_lines} BAF lines"));
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

        // ── BAF source ────────────────────────────────────────────────────────
        // The decompiled script can be long; wrap in a vertical scroll area
        // and use a read-only multiline TextEdit so users can select/copy.
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.bcs.baf.as_str())
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .code_editor()
                        .interactive(true),
                );
            });
    }
}
