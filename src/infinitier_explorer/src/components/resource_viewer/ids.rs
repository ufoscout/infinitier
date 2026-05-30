use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::ids::Ids,
};

pub struct IdsViewer {
    _ids: Ids,
}

impl IdsViewer {
    pub fn new(_ids: Ids) -> Self {
        Self { _ids }
    }
}

impl ResourceViewerTrait for IdsViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("IDS Viewer");
    }
}
