use eframe::egui;

pub struct AreViewer;

impl AreViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("ARE Viewer");
    }
}
