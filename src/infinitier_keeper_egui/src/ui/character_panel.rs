//! Central per-character editor panel.
//!
//! Owns the tab strip across the top and dispatches to the currently
//! selected tab module. Renders an empty / error placeholder when the
//! user has not yet selected a party slot, the slot is empty, or it
//! references an external CRE we haven't resolved (those don't carry
//! an in-band record to show).

use eframe::egui;
use egui_components::{Avatar, Label, LabelTone, Size, Tabs};
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
            // Header: initials avatar + bold name + muted "Slot N".
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                let display = member_display_name(member, idx);
                ui.add(Avatar::from_name(&display).size(44.0));
                ui.vertical(|ui| {
                    ui.add(Label::new(&display).strong().size(Size::Large));
                    ui.add(
                        Label::new(format!("Party slot {}", idx + 1))
                            .tone(LabelTone::Muted)
                            .size(Size::Small),
                    );
                });
            });
            ui.add_space(8.0);

            // Tab strip — pill variant matches the GPUI keeper's tab
            // chips, with the active tab using the accent fill.
            let mut selected_idx = CharacterTab::ALL
                .iter()
                .position(|t| *t == state.selected_tab)
                .unwrap_or(0);
            let labels: Vec<&'static str> =
                CharacterTab::ALL.iter().map(|t| t.label()).collect();
            ui.add(Tabs::new(&mut selected_idx).tabs(labels).pill());
            state.selected_tab = CharacterTab::ALL[selected_idx];
            ui.add_space(8.0);

            match &member.cre {
                Some(NpcCre::Cre(cre)) => show_tab(
                    ui,
                    state.selected_tab,
                    cre,
                    &state.save,
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

fn member_display_name(member: &ImportedGamNpc, idx: usize) -> String {
    if member.display_name.is_empty() {
        format!("Slot {}", idx + 1)
    } else {
        member.display_name.clone()
    }
}
