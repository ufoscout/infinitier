use eframe::egui;

pub struct SqlViewer;

impl SqlViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("SQL Viewer");
    }
}
