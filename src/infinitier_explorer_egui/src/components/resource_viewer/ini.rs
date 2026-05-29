use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::ini::Ini,
};

pub struct IniViewer {
    _ini: Ini,
}

impl IniViewer {
    pub fn new(_ini: Ini) -> Self {
        Self { _ini }
    }
}

impl ResourceViewerTrait for IniViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("INI Viewer");
    }
}
