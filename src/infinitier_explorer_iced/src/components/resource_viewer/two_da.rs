use iced::Element;

use crate::state::Message;

pub struct TwoDAViewer;

impl TwoDAViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("TwoDA Viewer").into()
    }
}
