use eframe::egui;

pub struct BmpViewer;

impl BmpViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("BMP Viewer");
    }
}
