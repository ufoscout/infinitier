use iced::Element;

use crate::state::Message;

pub struct TohViewer;

impl TohViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Toh Viewer").into()
    }
}
