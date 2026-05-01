use eframe::egui;

pub struct VvcViewer;

impl VvcViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("VVC Viewer");
    }
}
