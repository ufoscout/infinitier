use iced::Element;

use crate::state::Message;

pub struct CreViewer;

impl CreViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Cre Viewer").into()
    }
}
