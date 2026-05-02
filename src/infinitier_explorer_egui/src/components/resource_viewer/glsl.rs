use eframe::egui;

pub struct GlslViewer;

impl GlslViewer {
    pub fn show(ui: &mut egui::Ui) {
        ui.label("GLSL Viewer");
    }
}
