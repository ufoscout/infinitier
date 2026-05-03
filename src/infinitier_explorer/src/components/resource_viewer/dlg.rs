use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct DlgViewer;

impl DlgViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for DlgViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("DLG Viewer");
    }
}
