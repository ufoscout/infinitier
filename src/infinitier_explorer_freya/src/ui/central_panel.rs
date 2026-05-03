use freya::prelude::*;

use crate::components::resource_viewer::ResourceViewer;

#[derive(PartialEq)]
pub struct CentralPanel;

impl Component for CentralPanel {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .height(Size::fill())
            .child(ResourceViewer)
    }
}
