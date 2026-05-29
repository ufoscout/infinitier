//! Top-of-window metadata strip. A muted band carrying a theme-toggle
//! pill and four caption / value columns (Game, Folder, Save, GAM).

use eframe::egui;
use egui_components::{Button, Label, LabelTone, Size, Tooltip};
use egui_components_theme::{Theme, ThemeMode};

use crate::state::AppState;

pub struct HeaderPanel;

impl HeaderPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &AppState) {
        let theme = Theme::get(ui.ctx());
        egui::Panel::top("keeper_header")
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.muted_background)
                    .inner_margin(egui::Margin::symmetric(14, 10)),
            )
            .show_inside(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    theme_toggle_button(ui);
                    ui.add_space(16.0);
                    field(ui, "Game", &format!("{:?}", state.game_data.game()));
                    ui.add_space(24.0);
                    field(
                        ui,
                        "Folder",
                        &state
                            .game_data
                            .fs()
                            .get_roots()
                            .iter()
                            .map(|p| p.display().to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    );
                    ui.add_space(24.0);
                    field(ui, "Save", &state.save_name);
                    ui.add_space(24.0);
                    field(ui, "GAM", &format!("{:?}", state.save.version));
                });
            });
    }
}

/// Small pill on the left of the header that swaps the active theme.
/// Label shows the *target* mode (matches the GPUI keeper convention).
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

/// One header column: muted caption above a bold value. A hover
/// tooltip echoes the full `value` so long entries (e.g. the
/// comma-joined folder list) remain readable when the horizontal
/// strip truncates the visible label.
fn field(ui: &mut egui::Ui, caption: &str, value: &str) {
    ui.vertical(|ui| {
        ui.add(Label::new(caption).tone(LabelTone::Muted).size(Size::Small));
        let response = ui.add(Label::new(value).strong().size(Size::Medium));
        Tooltip::new(value).attach(response);
    });
}
