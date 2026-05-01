use iced::Element;

use crate::state::Message;

pub struct ProViewer;

impl ProViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Pro Viewer").into()
    }
}
