use eframe::egui;

pub struct TotViewer;

impl TotViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("TOT Viewer");
    }
}
