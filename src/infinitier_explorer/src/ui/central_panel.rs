use eframe::egui;

use crate::components::resource_viewer::ResourceViewer;
use crate::state::AppState;

pub struct CentralPanel {
    viewer: ResourceViewer,
}

impl CentralPanel {
    pub fn new() -> Self {
        Self {
            viewer: ResourceViewer::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &AppState) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.viewer.show(ui, state);
        });
    }
}
