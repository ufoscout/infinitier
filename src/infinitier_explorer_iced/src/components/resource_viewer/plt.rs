use iced::Element;

use crate::state::Message;

pub struct PltViewer;

impl PltViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Plt Viewer").into()
    }
}
