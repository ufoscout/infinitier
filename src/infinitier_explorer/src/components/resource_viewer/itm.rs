use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct ItmViewer;

impl ItmViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for ItmViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("ITM Viewer");
    }
}
