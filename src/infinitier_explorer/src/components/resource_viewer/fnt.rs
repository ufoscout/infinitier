use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct FntViewer;

impl FntViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for FntViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("FNT Viewer");
    }
}
