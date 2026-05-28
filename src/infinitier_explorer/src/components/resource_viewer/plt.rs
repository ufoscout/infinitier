use gpui::{AnyElement, Context, Window};
use infinitier_core::game::{GameResource, ResourceId};

use super::{ResourceViewerTrait, label};
use crate::app::ExplorerApp;

pub struct PltViewer;

impl PltViewer {
    pub fn new() -> Self {
        Self
    }
}

impl ResourceViewerTrait for PltViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        label("PLT Viewer")
    }
}
