//! Central per-character editor panel.
//!
//! Owns the tab strip across the top and dispatches to the currently
//! selected tab module. Renders an empty / error placeholder when
//! the user has not yet selected a party slot or the slot's embedded
//! CRE blob did not parse.

use eframe::egui;

use crate::save::PartyMember;
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
            let Some(member) = state.save.party.get(idx) else {
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
                Ok(cre) => show_tab(ui, state.selected_tab, cre, &state.save.gam, state.game),
                Err(err) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(180, 90, 90),
                        format!("Could not parse this slot's CRE blob: {err}"),
                    );
                }
            }
        });
    }
}

fn member_title(idx: usize, member: &PartyMember) -> String {
    if member.display_name.is_empty() {
        format!("Party slot {}", idx + 1)
    } else {
        format!("{}. {}", idx + 1, member.display_name)
    }
}
