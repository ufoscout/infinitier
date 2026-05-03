use eframe::egui;

pub struct FntViewer;

impl FntViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("FNT Viewer");
    }
}
