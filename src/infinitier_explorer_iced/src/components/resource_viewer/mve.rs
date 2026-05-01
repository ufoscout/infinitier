use iced::Element;

use crate::state::Message;

pub struct MveViewer;

impl MveViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Mve Viewer").into()
    }
}
