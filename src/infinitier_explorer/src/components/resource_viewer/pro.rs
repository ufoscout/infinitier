use eframe::egui;

pub struct ProViewer;

impl ProViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("PRO Viewer");
    }
}
