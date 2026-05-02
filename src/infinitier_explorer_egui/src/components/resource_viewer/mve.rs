use eframe::egui;

pub struct MveViewer;

impl MveViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("MVE Viewer");
    }
}
