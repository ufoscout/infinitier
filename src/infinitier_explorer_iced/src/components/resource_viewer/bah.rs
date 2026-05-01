use iced::Element;

use crate::state::Message;

pub struct BahViewer;

impl BahViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Bah Viewer").into()
    }
}
