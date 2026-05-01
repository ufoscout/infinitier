use eframe::egui;

pub struct IniViewer;

impl IniViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("INI Viewer");
    }
}
