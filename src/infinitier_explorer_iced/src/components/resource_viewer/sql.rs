use iced::Element;

use crate::state::Message;

pub struct SqlViewer;

impl SqlViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Sql Viewer").into()
    }
}
