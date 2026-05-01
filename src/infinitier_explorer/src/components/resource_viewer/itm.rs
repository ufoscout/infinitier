use eframe::egui;

pub struct ItmViewer;

impl ItmViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("ITM Viewer");
    }
}
