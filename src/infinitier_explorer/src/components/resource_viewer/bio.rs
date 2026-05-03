use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct BioViewer;

impl BioViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for BioViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("BIO Viewer");
    }
}
