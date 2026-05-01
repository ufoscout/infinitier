use iced::Element;

use crate::state::Message;

pub struct MusViewer;

impl MusViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Mus Viewer").into()
    }
}
