use eframe::egui;

pub struct BamViewer;

impl BamViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("BAM Viewer");
    }
}
