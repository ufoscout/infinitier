use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::pvr::PvrzHeader,
};

pub struct PvrzViewer {
    prvz: PvrzHeader,
}

impl PvrzViewer {
    pub fn new(prvz: PvrzHeader) -> Self {
        Self { prvz }
    }
}

impl ResourceViewerTrait for PvrzViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("PVRZ Viewer");
    }
}
