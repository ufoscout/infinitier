use eframe::egui;

pub struct PltViewer;

impl PltViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("PLT Viewer");
    }
}
