use eframe::egui;

pub struct ChuViewer;

impl ChuViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("CHU Viewer");
    }
}
