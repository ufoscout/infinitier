use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{game::{GameResource, ResourceId}, resource::acm::AcmDecoder};

pub struct AcmViewer{
    acm: AcmDecoder
}

impl AcmViewer {
    pub fn new(acm: AcmDecoder) -> Self {
        Self { acm }
    }
}

impl ResourceViewerTrait for AcmViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("ACM Viewer");
    }
}
