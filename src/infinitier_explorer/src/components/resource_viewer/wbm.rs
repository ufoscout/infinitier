use eframe::egui;

pub struct WbmViewer;

impl WbmViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("WBM Viewer");
    }
}
