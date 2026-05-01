use eframe::egui;

pub struct StoViewer;

impl StoViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("STO Viewer");
    }
}
