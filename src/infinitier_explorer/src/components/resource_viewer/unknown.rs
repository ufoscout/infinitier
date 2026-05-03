use eframe::egui;

pub struct UnknownViewer;

impl UnknownViewer {
    pub fn show(ui: &mut egui::Ui, type_id: u16) {
        ui.label(format!("Unknown Viewer (type: {type_id:#06x})"));
    }
}
