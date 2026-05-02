use eframe::egui;

pub struct PngViewer;

impl PngViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("PNG Viewer");
    }
}
