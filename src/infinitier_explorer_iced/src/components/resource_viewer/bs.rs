use iced::Element;

use crate::state::Message;

pub struct BsViewer;

impl BsViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Bs Viewer").into()
    }
}
