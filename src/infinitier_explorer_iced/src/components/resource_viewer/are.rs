use iced::Element;

use crate::state::Message;

pub struct AreViewer;

impl AreViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Are Viewer").into()
    }
}
