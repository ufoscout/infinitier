use eframe::egui;

pub struct SplViewer;

impl SplViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("SPL Viewer");
    }
}
