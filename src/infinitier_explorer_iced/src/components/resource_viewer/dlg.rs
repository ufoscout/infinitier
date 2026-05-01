use iced::Element;

use crate::state::Message;

pub struct DlgViewer;

impl DlgViewer {
    pub fn view<'a>() -> Element<'a, Message> {
        iced::widget::text("Dlg Viewer").into()
    }
}
