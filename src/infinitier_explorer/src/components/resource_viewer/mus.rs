use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct MusViewer;

impl MusViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for MusViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("MUS Viewer");
    }
}
