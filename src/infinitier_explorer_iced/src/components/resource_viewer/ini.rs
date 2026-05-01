use iced::Element;

use crate::state::Message;

pub struct IniViewer;

impl IniViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Ini Viewer").into()
    }
}
