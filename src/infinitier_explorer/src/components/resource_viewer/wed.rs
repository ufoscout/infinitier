use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::wed::Wed,
};

pub struct WedViewer {
    _wed: Wed,
}

impl WedViewer {
    pub fn new(_wed: Wed) -> Self {
        Self { _wed }
    }
}

impl ResourceViewerTrait for WedViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("WED Viewer");
    }
}
