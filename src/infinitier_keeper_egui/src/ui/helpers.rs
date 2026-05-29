//! Theme-aware layout helpers shared across panels.
//!
//! Most of what the keeper needed used to live here, but
//! `egui-components` now ships first-class `Card`, `Avatar`,
//! `Tooltip` etc. — those replaced the bespoke card helpers
//! directly. What's left is the key/value row inside cards, which
//! has no upstream analog (the gpui-component upstream uses
//! `editable_row` / `read_only_row` baked into each tab).

use eframe::egui;
use egui_components::{Label, LabelTone};

/// One key/value row inside a card: muted label on the left, bold
/// value flush-right against the card's inner edge. Matches the
/// GPUI keeper's `editable_row` read-only variant.
pub fn kv_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), 0.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.add(Label::new(label).tone(LabelTone::Muted));
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.add(Label::new(value).strong());
                },
            );
        },
    );
}
