use xilem::{AnyWidgetView, WidgetView};
use xilem::view::label;

use crate::state::AppState;

pub fn view() -> Box<AnyWidgetView<AppState>> {
    label("GLSL Viewer").boxed()
}
