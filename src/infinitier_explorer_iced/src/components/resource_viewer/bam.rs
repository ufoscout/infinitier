use iced::Element;

use crate::state::Message;

pub struct BamViewer;

impl BamViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Bam Viewer").into()
    }
}
