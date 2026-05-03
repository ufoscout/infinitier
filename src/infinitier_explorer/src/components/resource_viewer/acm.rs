use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct AcmViewer;

impl AcmViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for AcmViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("ACM Viewer");
    }
}
