use iced::Element;

use crate::state::Message;

pub struct TisViewer;

impl TisViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Tis Viewer").into()
    }
}
