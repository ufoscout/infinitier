use eframe::egui;

pub struct MazeViewer;

impl MazeViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("MAZE Viewer");
    }
}
