use iced::Element;

use crate::state::Message;

pub struct StoViewer;

impl StoViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Sto Viewer").into()
    }
}
