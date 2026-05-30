use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct ErrorViewer {
    message: String,
}

impl ErrorViewer {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl ResourceViewerTrait for ErrorViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.centered_and_justified(|ui| {
            ui.label(&self.message);
        });
    }
}
