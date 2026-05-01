use iced::Element;

use crate::state::Message;

pub struct TtfViewer;

impl TtfViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Ttf Viewer").into()
    }
}
