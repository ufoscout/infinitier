use iced::Element;

use crate::state::Message;

pub struct UnknownViewer;

impl UnknownViewer {
    pub fn view<'a>(type_id: u16) -> Element<'a, Message> {
        iced::widget::text(format!("Unknown Viewer (type: {type_id:#06x})")).into()
    }
}
