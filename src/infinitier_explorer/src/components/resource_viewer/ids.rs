use eframe::egui;

pub struct IdsViewer;

impl IdsViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("IDS Viewer");
    }
}
