use eframe::egui;

pub struct TwoDAViewer;

impl TwoDAViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("2DA Viewer");
    }
}
