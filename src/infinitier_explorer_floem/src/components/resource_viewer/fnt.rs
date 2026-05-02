use floem::views::label;
use floem::{AnyView, IntoView};

pub fn view() -> AnyView {
    label(|| "FNT Viewer".to_string()).into_any()
}
