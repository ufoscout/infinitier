use eframe::egui;

pub struct DlgViewer;

impl DlgViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("DLG Viewer");
    }
}
