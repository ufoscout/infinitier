use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::bam::Bam,
};

pub struct BamViewer {
    _bam: Bam,
}

impl BamViewer {
    pub fn new(bam: Bam) -> Self {
        Self { _bam: bam }
    }
}

impl ResourceViewerTrait for BamViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("BAM Viewer");
    }
}
