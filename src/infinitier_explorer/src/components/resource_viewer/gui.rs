use eframe::egui;

pub struct GuiViewer;

impl GuiViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("GUI Viewer");
    }
}
