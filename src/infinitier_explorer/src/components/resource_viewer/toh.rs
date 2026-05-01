use eframe::egui;

pub struct TohViewer;

impl TohViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("TOH Viewer");
    }
}
