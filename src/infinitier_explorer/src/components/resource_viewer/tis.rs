use eframe::egui;

pub struct TisViewer;

impl TisViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("TIS Viewer");
    }
}
