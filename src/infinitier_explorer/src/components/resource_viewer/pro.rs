use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct ProViewer;

impl ProViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for ProViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("PRO Viewer");
    }
}
