use iced::Element;

use crate::state::Message;

pub struct AcmViewer;

impl AcmViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Acm Viewer").into()
    }
}
