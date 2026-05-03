use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct WavViewer;

impl WavViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for WavViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("WAV Viewer");
    }
}
