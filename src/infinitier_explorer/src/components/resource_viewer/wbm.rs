use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct WbmViewer;

impl WbmViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for WbmViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("WBM Viewer");
    }
}
