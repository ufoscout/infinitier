use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct GlslViewer;

impl GlslViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for GlslViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("GLSL Viewer");
    }
}
