use freya::prelude::*;

use crate::components::selected_file_info::SelectedFileInfo;

#[derive(PartialEq)]
pub struct BottomPanel;

impl Component for BottomPanel {
    fn render(&self) -> impl IntoElement {
        rect()
            .width(Size::fill())
            .height(Size::px(28.))
            .padding(Gaps::new_all(4.))
            .background((240u8, 240u8, 240u8))
            .child(SelectedFileInfo)
    }
}
