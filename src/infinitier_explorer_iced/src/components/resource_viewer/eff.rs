use iced::Element;

use crate::state::Message;

pub struct EffViewer;

impl EffViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Eff Viewer").into()
    }
}
