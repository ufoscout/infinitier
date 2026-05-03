use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::two_da::TwoDA,
};

pub struct TwoDAViewer {
    twoda: TwoDA,
}

impl TwoDAViewer {
    pub fn new(twoda: TwoDA) -> Self {
        Self { twoda }
    }
}

impl ResourceViewerTrait for TwoDAViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("2DA Viewer");
    }
}
