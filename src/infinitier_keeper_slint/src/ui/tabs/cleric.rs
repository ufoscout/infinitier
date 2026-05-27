//! Cleric tab — stub. Writes the placeholder body message; the
//! Slint side renders a generic "not implemented yet" `StubTab` when
//! `body-message` is non-empty.

use crate::MainWindow;

pub fn populate(window: &MainWindow) {
    window.set_body_message("Cleric — not implemented yet.".into());
}
