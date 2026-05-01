use iced::Element;

use crate::state::Message;

pub struct PngViewer;

impl PngViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Png Viewer").into()
    }
}
