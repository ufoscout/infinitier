use eframe::egui;

pub struct TgaViewer;

impl TgaViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("TGA Viewer");
    }
}
