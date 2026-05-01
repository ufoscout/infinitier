use eframe::egui;

pub struct MenuViewer;

impl MenuViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("MENU Viewer");
    }
}
