use eframe::egui;

pub struct WavViewer;

impl WavViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("WAV Viewer");
    }
}
