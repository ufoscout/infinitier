use eframe::egui;
use infinitier_cre_resource::Cre;

use crate::save::PartyMember;
use crate::state::AppState;

pub struct AbilitiesPanel;

impl AbilitiesPanel {
    pub fn show(&self, ui: &mut egui::Ui, state: &AppState) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let Some(idx) = state.selected_party_index else {
                ui.label("Select a party member on the left to view their abilities.");
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
            match &member.cre {
                Ok(cre) => render_abilities(ui, cre),
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

fn render_abilities(ui: &mut egui::Ui, cre: &Cre) {
    ui.horizontal(|ui| {
        ui.strong("CRE version:");
        ui.label(format!("{:?}", cre.version));
    });
    ui.add_space(8.0);

    egui::Grid::new("ability_scores")
        .num_columns(2)
        .spacing([24.0, 6.0])
        .striped(true)
        .show(ui, |ui| {
            ability_row(ui, "Strength", cre.strength());
            // Strength % (the AD&D 18/01..18/00 bonus) only exists
            // pre-d20. IWD2 (CRE V2.2) omits it; the typed accessor
            // returns `None` there, and we render the row as "—" so
            // the grid stays uniform across versions.
            match cre.strength_bonus() {
                Some(bonus) => ability_row(ui, "Strength %", bonus),
                None => {
                    ui.label("Strength %");
                    ui.label("—");
                    ui.end_row();
                }
            }
            ability_row(ui, "Dexterity", cre.dexterity());
            ability_row(ui, "Constitution", cre.constitution());
            ability_row(ui, "Intelligence", cre.intelligence());
            ability_row(ui, "Wisdom", cre.wisdom());
            ability_row(ui, "Charisma", cre.charisma());
        });
}

fn ability_row(ui: &mut egui::Ui, label: &str, value: u8) {
    ui.label(label);
    ui.strong(value.to_string());
    ui.end_row();
}
