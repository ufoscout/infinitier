use eframe::egui;

pub struct BahViewer;

impl BahViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("BAH Viewer");
    }
}
