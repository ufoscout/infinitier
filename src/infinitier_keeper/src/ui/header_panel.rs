//! Top button bar.
//!
//! Left-aligned action cluster (Load, Save, …) and a right-aligned
//! theme-toggle pill. The save-game metadata that used to live here
//! now lives in the window title (see `AppState::window_title`), so
//! the bar is purely actions.

use eframe::egui;
use egui_components::Button;
use egui_components::theme::{Theme, ThemeMode};

/// Click signals emitted by the header bar this frame. Booleans
/// keep the host side trivial (`if header_action.save_clicked {
/// … }`) and let new buttons be added without revisiting the
/// caller's match arm.
#[derive(Default, Clone, Copy)]
pub struct HeaderAction {
    pub load_clicked: bool,
    pub save_clicked: bool,
}

pub struct HeaderPanel;

impl HeaderPanel {
    /// Paint the button bar and report which buttons were clicked
    /// this frame.
    pub fn show(&self, ui: &mut egui::Ui) -> HeaderAction {
        let theme = Theme::get(ui.ctx());
        let mut action = HeaderAction::default();
        egui::Panel::top("keeper_header")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.muted_background)
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if ui.add(Button::primary("Load").small()).clicked() {
                        action.load_clicked = true;
                    }
                    if ui.add(Button::primary("Save").small()).clicked() {
                        action.save_clicked = true;
                    }
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            theme_toggle_button(ui);
                        },
                    );
                });
            });
        action
    }
}

/// Pill that swaps the active theme between light and dark. Label
/// shows the *target* mode (same convention as the explorer's left
/// rail), so it reads as a switch rather than a destination.
fn theme_toggle_button(ui: &mut egui::Ui) {
    let theme = Theme::get(ui.ctx());
    let target_label = match theme.mode {
        ThemeMode::Dark => "Light",
        ThemeMode::Light => "Dark",
    };
    if ui.add(Button::ghost(target_label).small()).clicked() {
        let next = match theme.mode {
            ThemeMode::Dark => Theme::light(),
            ThemeMode::Light => Theme::dark(),
        };
        next.install(ui.ctx());
    }
}
