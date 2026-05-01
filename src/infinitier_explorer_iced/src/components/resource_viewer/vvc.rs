use iced::Element;

use crate::state::Message;

pub struct VvcViewer;

impl VvcViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Vvc Viewer").into()
    }
}
