use eframe::egui;

pub struct CreViewer;

impl CreViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("CRE Viewer");
    }
}
