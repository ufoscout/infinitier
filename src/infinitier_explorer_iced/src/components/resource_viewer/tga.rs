use iced::Element;

use crate::state::Message;

pub struct TgaViewer;

impl TgaViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Tga Viewer").into()
    }
}
