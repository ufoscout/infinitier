//! Central per-character editor panel.
//!
//! Owns the tab strip across the top and dispatches to the currently
//! selected tab module. Renders an empty / error placeholder when the
//! user has not yet selected a party slot, the slot is empty, or it
//! references an external CRE we haven't resolved (those don't carry
//! an in-band record to show).

use eframe::egui;
use egui_components::Tabs;
use infinitier_core::imported_resource::gam::{ImportedGamNpc, NpcCre};

use crate::components::editable_fields::KeeperEditors;
use crate::state::AppState;
use crate::ui::tabs::{CharacterTab, show_tab};

pub struct CharacterPanel;

impl CharacterPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState, editors: &mut KeeperEditors) {
        egui::CentralPanel::default().show(ui, |ui| {
            let active = state.active();
            let Some(idx) = active.selected_party_index else {
                ui.label("Select a party member on the left to view their data.");
                return;
            };

            // Header (display name + slot label) — pulled into local
            // owned values so the `&active.save.party_npcs` borrow is
            // released before `show_tab` takes `&mut state`.
            let header: Option<HeaderInfo> = active.save.party_npcs.get(idx).map(HeaderInfo::from);
            let Some(header) = header else {
                ui.colored_label(
                    egui::Color32::RED,
                    "Stale selection — party member not found.",
                );
                return;
            };

            // Tab strip — segmented variant gives a single bordered
            // bar with each tab as a button-style cell.
            // Filter to game-specific tabs (e.g. Levels only for IWD2).
            let game = state.game_data.game();
            let visible_tabs: Vec<CharacterTab> = CharacterTab::ALL
                .iter()
                .copied()
                .filter(|t| t.is_visible_for_game(game))
                .collect();
            let mut selected_idx = visible_tabs
                .iter()
                .position(|t| *t == active.selected_tab)
                .unwrap_or(0);
            let labels: Vec<&'static str> = visible_tabs.iter().map(|t| t.label()).collect();
            ui.add(Tabs::new(&mut selected_idx).tabs(labels).segmented());
            state.active_mut().selected_tab = visible_tabs[selected_idx];
            ui.add_space(8.0);

            match header.cre_kind {
                CreKind::Embedded => show_tab(ui, state, editors),
                CreKind::ExternalRef(resref) => {
                    ui.label(format!(
                        "External CRE '{resref}' — embedded record not present in this GAM.",
                    ));
                }
                CreKind::Empty => {
                    ui.label("Empty party slot — no creature record to edit.");
                }
            }
        });
    }
}

/// Owned summary of the active party-member row. Decouples the
/// header-render path from the `&state.save` borrow so `show_tab`
/// can take `&mut state` next.
struct HeaderInfo {
    cre_kind: CreKind,
}

enum CreKind {
    Embedded,
    ExternalRef(String),
    Empty,
}

impl HeaderInfo {
    fn from(member: &ImportedGamNpc) -> Self {
        let cre_kind = match member.cre.as_ref() {
            Some(NpcCre::Cre(_)) => CreKind::Embedded,
            Some(NpcCre::Ref(resref)) => CreKind::ExternalRef(resref.clone()),
            None => CreKind::Empty,
        };
        Self { cre_kind }
    }
}
