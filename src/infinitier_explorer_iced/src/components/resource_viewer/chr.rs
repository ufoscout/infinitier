use iced::Element;

use crate::state::Message;

pub struct ChrViewer;

impl ChrViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Chr Viewer").into()
    }
}
