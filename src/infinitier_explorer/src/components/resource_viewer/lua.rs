use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use super::ResourceViewerTrait;

pub struct LuaViewer;

impl LuaViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for LuaViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, _resource: &GameResource) {
        ui.label("LUA Viewer");
    }
}
