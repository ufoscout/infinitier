use eframe::egui;

pub struct TtfViewer;

impl TtfViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("TTF Viewer");
    }
}
