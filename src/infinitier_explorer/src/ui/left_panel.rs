//! Left rail — resource tree grouped by file extension.
//!
//! Mirrors the GPUI explorer's `ui::left_panel` shape: a muted
//! sidebar with a top strip (theme-toggle pill + bold "Resources"
//! label), a divider, and the virtualized tree underneath. Colors
//! and typography come from `egui_components::theme`, so light↔dark
//! toggles stay consistent without local color logic.

use eframe::egui;
use egui_components::button::Button;
use egui_components::heading::{Heading, HeadingLevel};
use egui_components::scroll_area::ScrollArea;
use egui_components::separator::Separator;
use egui_components::theme::{Theme, ThemeMode};
use infinitier_core::game::GameData;

use crate::components::key_file_tree_view::KeyFileTreeView;
use crate::state::AppState;

pub struct LeftPanel {
    tree_view: KeyFileTreeView,
}

impl LeftPanel {
    pub fn new(game_data: &GameData) -> Self {
        Self {
            tree_view: KeyFileTreeView::new(game_data),
        }
    }

    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        let theme = Theme::get(ui.ctx());
        egui::Panel::left("resource_panel")
            .resizable(true)
            .default_size(260.0)
            .frame(
                egui::Frame::new()
                    .fill(theme.colors.muted_background)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme_toggle_button(ui);
                    ui.add(Heading::new("Resources").level(HeadingLevel::H3));
                });
                ui.add_space(4.0);
                ui.add(Separator::horizontal());
                ui.add_space(4.0);
                ScrollArea::vertical().show(ui, |ui| {
                    self.tree_view.show(ui, state);
                });
            });
    }
}

/// Small pill at the top-left that swaps the active theme between
/// light and dark. Label shows the *target* mode (so it reads as a
/// switch rather than a destination — same convention the GPUI
/// explorer + keeper use).
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
