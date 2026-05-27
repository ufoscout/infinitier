//! Centred-message viewer (errors, "not implemented" stubs, …).

use crate::MainWindow;

pub fn populate(window: &MainWindow, text: &str) {
    window.set_viewer_kind("message".into());
    window.set_message_text(text.into());
}
