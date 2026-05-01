use iced::Element;

use crate::state::Message;

pub struct BioViewer;

impl BioViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Bio Viewer").into()
    }
}
