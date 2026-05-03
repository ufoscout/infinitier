use freya::prelude::*;

pub fn unknown_viewer(type_id: u16) -> Element {
    label()
        .text(format!("Unknown Viewer (type: {type_id:#06x})"))
        .into()
}
