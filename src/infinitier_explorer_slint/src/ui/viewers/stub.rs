//! Stub viewers for resource types the egui original only renders as
//! a single label (`ui.label("X Viewer")`). Mirrors that minimal
//! behaviour exactly — the dispatcher passes the label text in.

use infinitier_core::game::GameResource;
use infinitier_core::resource::ResourceType;

use crate::MainWindow;
use crate::ui::viewers::message;

/// One-line "X Viewer" stub.
pub fn label(window: &MainWindow, text: &str) {
    message::populate(window, text);
}

/// Unknown-type fallback. Mirrors the egui `UnknownViewer` exactly:
/// shows the raw `Unknown(id)` discriminator when present.
pub fn unknown(window: &MainWindow, resource: &GameResource) {
    let type_id = match resource.r#type {
        ResourceType::Unknown(id) => id,
        _ => 0,
    };
    message::populate(window, &format!("Unknown Viewer (type: {type_id:#06x})"));
}
