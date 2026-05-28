use gpui::{AnyElement, Context, Window};
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::two_da::TwoDA,
};

use super::{ResourceViewerTrait, label};
use crate::app::ExplorerApp;

pub struct TwoDAViewer {
    _twoda: TwoDA,
}

impl TwoDAViewer {
    pub fn new(_twoda: TwoDA) -> Self {
        Self { _twoda }
    }
}

impl ResourceViewerTrait for TwoDAViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        label("2DA Viewer")
    }
}
