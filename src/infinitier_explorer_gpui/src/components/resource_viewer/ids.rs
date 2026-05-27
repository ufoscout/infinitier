use gpui::{AnyElement, Context, Window};
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::ids::Ids,
};

use super::{ResourceViewerTrait, label};
use crate::app::ExplorerApp;

pub struct IdsViewer {
    _ids: Ids,
}

impl IdsViewer {
    pub fn new(_ids: Ids) -> Self {
        Self { _ids }
    }
}

impl ResourceViewerTrait for IdsViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        label("IDS Viewer")
    }
}
