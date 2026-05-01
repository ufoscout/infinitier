use eframe::egui;

pub struct MusViewer;

impl MusViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("MUS Viewer");
    }
}
