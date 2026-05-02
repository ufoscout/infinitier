use eframe::egui;

pub struct SrcViewer;

impl SrcViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("SRC Viewer");
    }
}
