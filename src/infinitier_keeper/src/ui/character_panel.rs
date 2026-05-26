//! Central per-character editor panel.
//!
//! Owns the tab strip across the top and dispatches to the currently
//! selected tab module. Renders an empty / error placeholder when the
//! user has not yet selected a party slot, the slot is empty, or it
//! references an external CRE we haven't resolved (those don't carry
//! an in-band record to show).

use eframe::egui;
use infinitier_core::imported_resource::gam::{ImportedGamNpc, NpcCre};

use crate::state::AppState;
use crate::ui::tabs::{CharacterTab, show_tab};

pub struct CharacterPanel;

impl CharacterPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let Some(idx) = state.selected_party_index else {
                ui.label("Select a party member on the left to view their data.");
                return;
            };
            let Some(member) = state.save.party_npcs.get(idx) else {
                ui.colored_label(
                    egui::Color32::RED,
                    "Stale selection — party member not found.",
                );
                return;
            };
            ui.heading(member_title(idx, member));
            ui.separator();

            // Tab strip
            ui.horizontal_wrapped(|ui| {
                for tab in CharacterTab::ALL {
                    if ui
                        .selectable_label(state.selected_tab == *tab, tab.label())
                        .clicked()
                    {
                        state.selected_tab = *tab;
                    }
                }
            });
            ui.separator();

            match &member.cre {
                Some(NpcCre::Cre(cre)) => show_tab(
                    ui,
                    state.selected_tab,
                    cre,
                    &state.save.gam,
                    state.game_data.game(),
                ),
                Some(NpcCre::Ref(resref)) => {
                    ui.label(format!(
                        "External CRE '{resref}' — embedded record not present in this GAM.",
                    ));
                }
                None => {
                    ui.label("Empty party slot — no creature record to edit.");
                }
            }
        });
    }
}

fn member_title(idx: usize, member: &ImportedGamNpc) -> String {
    if member.display_name.is_empty() {
        format!("Party slot {}", idx + 1)
    } else {
        format!("{}. {}", idx + 1, member.display_name)
    }
}
