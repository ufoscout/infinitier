use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct MveViewer;

impl MveViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for MveViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("MVE Viewer");
    }
}
