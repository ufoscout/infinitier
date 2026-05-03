use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::ini::Ini,
};

pub struct IniViewer {
    ini: Ini,
}

impl IniViewer {
    pub fn new(ini: Ini) -> Self {
        Self { ini }
    }
}

impl ResourceViewerTrait for IniViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("INI Viewer");
    }
}
