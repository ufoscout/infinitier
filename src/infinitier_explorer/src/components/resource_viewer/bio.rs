use eframe::egui;

pub struct BioViewer;

impl BioViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("BIO Viewer");
    }
}
