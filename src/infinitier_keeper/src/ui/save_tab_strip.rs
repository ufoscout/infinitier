//! Save-tab strip — one tab per open save game.
//!
//! Sits between the header button bar and the character editor. Each
//! tab shows the on-disk save folder name; clicking it switches
//! [`AppState::active_tab`]. The strip is always painted, even when
//! only one save is open, so the structure is discoverable as more
//! tabs get added via the Load action (yet-to-be-wired).

use eframe::egui;
use egui_components::Tabs;
use egui_components::theme::Theme;

use crate::state::AppState;

pub struct SaveTabStrip;

impl SaveTabStrip {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        let theme = Theme::get(ui.ctx());
        egui::Panel::top("keeper_save_tabs")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.background)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show_inside(ui, |ui| {
                // Snapshot the labels — the `Tabs` widget needs an
                // owned iterator and we want to release the borrow on
                // `state.tabs` before re-borrowing it mutably to write
                // back `active_tab`.
                let labels: Vec<String> = state.tabs.iter().map(|t| t.save_name.clone()).collect();
                let mut selected = state.active_tab.min(labels.len().saturating_sub(1));
                ui.add(Tabs::new(&mut selected).tabs(labels).underline());
                state.active_tab = selected;
            });
    }
}
