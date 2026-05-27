use gpui::{AnyElement, Context, Window};
use infinitier_core::{
    game::{GameResource, ResourceId},
    resource::ini::Ini,
};

use super::{ResourceViewerTrait, label};
use crate::app::ExplorerApp;

pub struct IniViewer {
    _ini: Ini,
}

impl IniViewer {
    pub fn new(_ini: Ini) -> Self {
        Self { _ini }
    }
}

impl ResourceViewerTrait for IniViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        label("INI Viewer")
    }
}
