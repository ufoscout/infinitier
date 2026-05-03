use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct PngViewer;

impl PngViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for PngViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("PNG Viewer");
    }
}
