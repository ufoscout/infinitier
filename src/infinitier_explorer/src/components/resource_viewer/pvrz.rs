use eframe::egui;

pub struct PvrzViewer;

impl PvrzViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("PVRZ Viewer");
    }
}
