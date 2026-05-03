use eframe::egui;

pub struct VefViewer;

impl VefViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("VEF Viewer");
    }
}
