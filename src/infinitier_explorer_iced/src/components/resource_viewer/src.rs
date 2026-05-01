use iced::Element;

use crate::state::Message;

pub struct SrcViewer;

impl SrcViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Src Viewer").into()
    }
}
