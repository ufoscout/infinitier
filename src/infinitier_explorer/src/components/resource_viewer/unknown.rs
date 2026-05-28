use gpui::{AnyElement, Context, Window};
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::ResourceType,
};

use super::{ResourceViewerTrait, label};
use crate::app::ExplorerApp;

pub struct UnknownViewer;

impl UnknownViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for UnknownViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        let type_id = if let ResourceType::Unknown(id) = resource.r#type {
            id
        } else {
            0
        };
        label(format!("Unknown Viewer (type: {type_id:#06x})"))
    }
}
