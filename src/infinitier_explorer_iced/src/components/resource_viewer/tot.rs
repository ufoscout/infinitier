use iced::Element;

use crate::state::Message;

pub struct TotViewer;

impl TotViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Tot Viewer").into()
    }
}
