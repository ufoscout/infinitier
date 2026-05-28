use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use infinitier_core::game::{GameResource, ResourceId};

use super::ResourceViewerTrait;
use crate::app::ExplorerApp;

pub struct ErrorViewer {
    message: String,
}

impl ErrorViewer {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl ResourceViewerTrait for ErrorViewer {
    fn render(
        &mut self,
        _resource_id: ResourceId,
        _resource: &GameResource,
        _window: &mut Window,
        _cx: &mut Context<ExplorerApp>,
    ) -> AnyElement {
        div()
            .w_full()
            .p_6()
            .child(self.message.clone())
            .into_any_element()
    }
}
