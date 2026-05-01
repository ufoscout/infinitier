use iced::Element;

use crate::state::Message;

pub struct FntViewer;

impl FntViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Fnt Viewer").into()
    }
}
