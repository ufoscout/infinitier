use freya::prelude::*;

use crate::components::resource_tree::ResourceTree;

#[derive(PartialEq)]
pub struct LeftPanel;

impl Component for LeftPanel {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::px(280.))
            .height(Size::fill())
            .direction(Direction::Vertical)
            .background((245u8, 245u8, 245u8))
            .padding(Gaps::new_all(4.))
            .child(label().text("Resources").font_size(16.))
            .child(
                rect()
                    .width(Size::fill())
                    .height(Size::px(1.))
                    .background((204u8, 204u8, 204u8)),
            )
            .child(ResourceTree)
    }
}
