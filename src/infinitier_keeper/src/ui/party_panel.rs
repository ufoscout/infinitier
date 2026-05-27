//! Left rail listing party-NPC slots. Matches the Slint keeper's
//! `PartyPanel`: an `alternate_background` band with `accent`-coloured
//! pills for the current selection.

use eframe::egui;
use infinitier_egui_common::theme;

use crate::state::AppState;

pub struct PartyPanel;

impl PartyPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        let palette = theme::active();

        egui::Panel::left("keeper_party")
            .resizable(true)
            .default_size(240.0)
            .frame(
                egui::Frame::new()
                    .fill(palette.alternate_background)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show_inside(ui, |ui| {
                ui.label(theme::card_title("Party"));
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                if state.save.party_npcs.is_empty() {
                    ui.label("No party members in this save.");
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut selected = state.selected_party_index;
                    for (i, member) in state.save.party_npcs.iter().enumerate() {
                        let label = if member.display_name.is_empty() {
                            format!("Slot {}", i + 1)
                        } else {
                            format!("{}. {}", i + 1, member.display_name)
                        };
                        let response =
                            theme::chip(ui, &label, selected == Some(i), theme::ChipKind::Row);
                        if response.clicked() {
                            selected = Some(i);
                        }
                    }
                    state.selected_party_index = selected;
                });
            });
    }
}
