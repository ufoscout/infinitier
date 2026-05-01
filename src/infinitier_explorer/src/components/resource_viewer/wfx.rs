use eframe::egui;

pub struct WfxViewer;

impl WfxViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("WFX Viewer");
    }
}
