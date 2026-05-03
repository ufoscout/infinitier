use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct PltViewer;

impl PltViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for PltViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("PLT Viewer");
    }
}
