use eframe::egui;

pub struct AcmViewer;

impl AcmViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("ACM Viewer");
    }
}
