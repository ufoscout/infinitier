use eframe::egui;

pub struct GamViewer;

impl GamViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("GAM Viewer");
    }
}
