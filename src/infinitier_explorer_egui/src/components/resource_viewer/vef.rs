use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct VefViewer;

impl VefViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for VefViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("VEF Viewer");
    }
}
