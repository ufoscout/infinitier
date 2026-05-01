use iced::Element;

use crate::state::Message;

pub struct ChuViewer;

impl ChuViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Chu Viewer").into()
    }
}
