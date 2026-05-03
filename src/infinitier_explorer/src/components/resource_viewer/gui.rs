use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};

pub struct GuiViewer;

impl GuiViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for GuiViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("GUI Viewer");
    }
}
