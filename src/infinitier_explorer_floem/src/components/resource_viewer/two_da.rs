use floem::views::label;
use floem::{AnyView, IntoView};

pub fn view() -> AnyView {
    label(|| "2DA Viewer".to_string()).into_any()
}
