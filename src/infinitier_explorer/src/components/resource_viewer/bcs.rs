use eframe::egui;

pub struct BcsViewer;

impl BcsViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("BCS Viewer");
    }
}
