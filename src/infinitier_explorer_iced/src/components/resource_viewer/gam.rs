use iced::Element;

use crate::state::Message;

pub struct GamViewer;

impl GamViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Gam Viewer").into()
    }
}
