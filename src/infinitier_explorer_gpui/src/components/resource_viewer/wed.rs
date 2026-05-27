use gpui::{AnyElement, Context, Window};
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::wed::Wed,
};

use super::{ResourceViewerTrait, label};
use crate::app::ExplorerApp;

pub struct WedViewer {
    _wed: Wed,
}

impl WedViewer {
    pub fn new(_wed: Wed) -> Self {
        Self { _wed }
    }
}

impl ResourceViewerTrait for WedViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        label("WED Viewer")
    }
}
