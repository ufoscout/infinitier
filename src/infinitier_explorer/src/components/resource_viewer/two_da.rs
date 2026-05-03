use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct TwoDAViewer;

impl TwoDAViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for TwoDAViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("2DA Viewer");
    }
}
