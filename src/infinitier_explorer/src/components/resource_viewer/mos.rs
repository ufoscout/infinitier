use eframe::egui;

pub struct MosViewer;

impl MosViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("MOS Viewer");
    }
}
