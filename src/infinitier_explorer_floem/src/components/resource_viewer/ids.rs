use floem::views::label;
use floem::{AnyView, IntoView};

pub fn view() -> AnyView {
    label(|| "IDS Viewer".to_string()).into_any()
}
