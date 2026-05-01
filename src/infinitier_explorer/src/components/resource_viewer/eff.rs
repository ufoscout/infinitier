use eframe::egui;

pub struct EffViewer;

impl EffViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("EFF Viewer");
    }
}
