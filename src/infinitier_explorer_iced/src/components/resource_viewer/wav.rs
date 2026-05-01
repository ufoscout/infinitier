use iced::Element;

use crate::state::Message;

pub struct WavViewer;

impl WavViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Wav Viewer").into()
    }
}
