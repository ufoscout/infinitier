use eframe::egui;

pub struct WmpViewer;

impl WmpViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("WMP Viewer");
    }
}
