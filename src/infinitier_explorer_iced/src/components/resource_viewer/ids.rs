use iced::Element;

use crate::state::Message;

pub struct IdsViewer;

impl IdsViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Ids Viewer").into()
    }
}
