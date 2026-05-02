use eframe::egui;

pub struct ChrViewer;

impl ChrViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("CHR Viewer");
    }
}
