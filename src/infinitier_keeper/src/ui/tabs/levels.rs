//! Levels tab — IWD2 (CRE V2.2) per-class level editor.
//!
//! Mirrors EEKeeper's layout: two columns of class/level pairs, six on
//! the left (Barbarian … Monk) and five on the right (Paladin … Wizard),
//! each with an editable numeric input and a read-only "Total" footer.

use eframe::egui;
use egui_components::{Card, Label};
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::CreHeader;

use crate::components::editable_fields::{EditableField, KeeperEditors};
use crate::state::AppState;

const INPUT_WIDTH: f32 = 110.0;

pub struct LevelsTab;

impl LevelsTab {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState, editors: &mut KeeperEditors) {
        // Snapshot total_levels from the V22 header before releasing the borrow.
        let total = {
            let active = state.active();
            let Some(idx) = active.selected_party_index else {
                ui.label("Empty party slot — no creature record to edit.");
                return;
            };
            let Some(member) = active.save.party_npcs.get(idx) else {
                ui.label("Empty party slot — no creature record to edit.");
                return;
            };
            let Some(NpcCre::Cre(imported)) = member.cre.as_ref() else {
                ui.label("Empty party slot — no creature record to edit.");
                return;
            };
            match &imported.header {
                CreHeader::V22(h) => h.total_levels,
                _ => {
                    ui.label("Levels are only available for IWD2 (CRE V2.2) creatures.");
                    return;
                }
            }
        };

        egui_components::scroll_area::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(2, |cols| {
                levels_card(
                    &mut cols[0],
                    "Classes (left)",
                    &[
                        (EditableField::BarbarianLevel, "Barbarian"),
                        (EditableField::BardLevel, "Bard"),
                        (EditableField::ClericLevel, "Cleric"),
                        (EditableField::DruidLevel, "Druid"),
                        (EditableField::FighterLevel, "Fighter"),
                        (EditableField::MonkLevel, "Monk"),
                    ],
                    state,
                    editors,
                    Some(total),
                );
                levels_card(
                    &mut cols[1],
                    "Classes (right)",
                    &[
                        (EditableField::PaladinLevel, "Paladin"),
                        (EditableField::RangerLevel, "Ranger"),
                        (EditableField::RogueLevel, "Rogue"),
                        (EditableField::SorcererLevel, "Sorcerer"),
                        (EditableField::WizardLevel, "Wizard"),
                    ],
                    state,
                    editors,
                    None,
                );
            });
        });
    }
}

fn levels_card(
    ui: &mut egui::Ui,
    title: &str,
    fields: &[(EditableField, &'static str)],
    state: &mut AppState,
    editors: &mut KeeperEditors,
    total: Option<u8>,
) {
    Card::new().title(title).divider().show(ui, |ui| {
        for &(field, label) in fields {
            level_row(ui, field, label, state, editors);
        }
        if let Some(t) = total {
            read_only_row(ui, "Total", &t.to_string());
        }
    });
}

fn level_row(
    ui: &mut egui::Ui,
    field: EditableField,
    label: &str,
    state: &mut AppState,
    editors: &mut KeeperEditors,
) {
    let avail_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(avail_w, 0.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            editors.show_input(ui, field, state, INPUT_WIDTH);
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let theme = egui_components::theme::Theme::get(ui.ctx());
                let rich = egui::RichText::new(label)
                    .color(theme.colors.muted_foreground)
                    .font(egui::FontId::proportional(theme.metrics.font_size_md));
                ui.add(egui::Label::new(rich).truncate());
            });
        },
    );
}

fn read_only_row(ui: &mut egui::Ui, label: &str, value: &str) {
    let avail_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(avail_w, 0.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.add(Label::new(value).strong());
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let theme = egui_components::theme::Theme::get(ui.ctx());
                let rich = egui::RichText::new(label)
                    .color(theme.colors.muted_foreground)
                    .font(egui::FontId::proportional(theme.metrics.font_size_md));
                ui.add(egui::Label::new(rich).truncate());
            });
        },
    );
}
