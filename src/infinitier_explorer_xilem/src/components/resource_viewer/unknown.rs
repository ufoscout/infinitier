use xilem::{AnyWidgetView, WidgetView};
use xilem::view::label;

use crate::state::AppState;

pub fn view(type_id: u16) -> Box<AnyWidgetView<AppState>> {
    label(format!("Unknown Viewer (type: {type_id:#06x})")).boxed()
}
