use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct IdsViewer;

impl IdsViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for IdsViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("IDS Viewer");
    }
}
