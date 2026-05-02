use floem::views::label;
use floem::{AnyView, IntoView};

pub fn view(type_id: u16) -> AnyView {
    label(move || format!("Unknown Viewer (type: {type_id:#06x})")).into_any()
}
