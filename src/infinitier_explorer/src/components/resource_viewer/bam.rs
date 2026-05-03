use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct BamViewer;

impl BamViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for BamViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("BAM Viewer");
    }
}
