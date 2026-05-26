use eframe::egui;

use crate::state::AppState;

pub struct PartyPanel;

impl PartyPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        egui::Panel::left("keeper_party")
            .resizable(true)
            .default_size(220.0)
            .show_inside(ui, |ui| {
                ui.heading("Party");
                ui.separator();
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
                        let response = ui.selectable_label(selected == Some(i), label);
                        if response.clicked() {
                            selected = Some(i);
                        }
                    }
                    state.selected_party_index = selected;
                });
            });
    }
}
