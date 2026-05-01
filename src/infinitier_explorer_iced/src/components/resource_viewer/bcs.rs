use iced::Element;

use crate::state::Message;

pub struct BcsViewer;

impl BcsViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Bcs Viewer").into()
    }
}
