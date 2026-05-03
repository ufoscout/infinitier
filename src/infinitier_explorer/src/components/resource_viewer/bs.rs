use eframe::egui;

pub struct BsViewer;

impl BsViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("BS Viewer");
    }
}
