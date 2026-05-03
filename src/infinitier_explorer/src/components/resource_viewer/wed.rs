use eframe::egui;

pub struct WedViewer;

impl WedViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("WED Viewer");
    }
}
