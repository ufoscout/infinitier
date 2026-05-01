use iced::Element;

use crate::state::Message;

pub struct SplViewer;

impl SplViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Spl Viewer").into()
    }
}
