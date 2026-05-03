use super::ResourceViewerTrait;
use eframe::egui;
use infinitier_core::game::{GameResource, ResourceId};
use infinitier_core::resource::ResourceType;

pub struct UnknownViewer;

impl UnknownViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for UnknownViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        let type_id = if let ResourceType::Unknown(id) = resource.r#type {
            id
        } else {
            0
        };
        ui.label(format!("Unknown Viewer (type: {type_id:#06x})"));
    }
}
