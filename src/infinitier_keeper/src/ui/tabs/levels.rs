//! Levels & Kits tab — IWD2 (CRE V2.2) per-class level + kit editor.
//!
//! Two cards side by side. **Class Levels**: the eleven class/level
//! pairs sorted by name (Barbarian … Wizard), each with an editable
//! numeric input and a read-only "Total" footer; the eleven levels are
//! kept summing to at most 255 (the width of the engine's
//! `total_levels` byte) by clamping the just-edited class against the
//! headroom the other ten leave. **Kits**: a checkbox per IWD2 kit,
//! each toggling its bit in the CRE `kit_bitfield` (offset `0x0270`).

use eframe::egui;
use egui_components::{Card, Checkbox, Label};
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::{CreHeader, Iwd2Kits};

use crate::components::editable_fields::{EditableField, KeeperEditors};
use crate::state::AppState;

const INPUT_WIDTH: f32 = 110.0;
/// Cap the two-card row so the rows don't stretch across the whole
/// window on wide displays.
const CARDS_MAX_WIDTH: f32 = 740.0;

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
            // Keep the two cards to a readable width instead of spanning
            // the whole tab.
            ui.set_max_width(CARDS_MAX_WIDTH);
            ui.columns(2, |cols| {
                // Left: all eleven classes in a single column, sorted by name.
                levels_card(
                    &mut cols[0],
                    "Class Levels",
                    &[
                        (EditableField::BarbarianLevel, "Barbarian"),
                        (EditableField::BardLevel, "Bard"),
                        (EditableField::ClericLevel, "Cleric"),
                        (EditableField::DruidLevel, "Druid"),
                        (EditableField::FighterLevel, "Fighter"),
                        (EditableField::MonkLevel, "Monk"),
                        (EditableField::PaladinLevel, "Paladin"),
                        (EditableField::RangerLevel, "Ranger"),
                        (EditableField::RogueLevel, "Rogue"),
                        (EditableField::SorcererLevel, "Sorcerer"),
                        (EditableField::WizardLevel, "Wizard"),
                    ],
                    state,
                    editors,
                    Some(total),
                );
                // Right: one toggle per kit.
                kits_card(&mut cols[1], state);
            });
        });
    }
}

/// Right-hand card: a checkbox per IWD2 kit, toggling its bit in the
/// selected creature's kit bitfield.
fn kits_card(ui: &mut egui::Ui, state: &mut AppState) {
    let Some(mut kits) = selected_kits(state) else {
        return;
    };
    Card::new().title("Kits").divider().show(ui, |ui| {
        for &(label, kit) in Iwd2Kits::ALL {
            let mut on = kits.contains(kit);
            if ui.add(Checkbox::new(&mut on, label)).changed() {
                // Keep the local snapshot in step so multiple toggles in
                // one frame compose, then write through to the CRE.
                kits.set(kit, on);
                set_selected_kits(state, kits);
            }
        }
    });
}

/// The selected party creature's IWD2 kits, or `None` when there is no
/// V2.2 creature selected.
fn selected_kits(state: &AppState) -> Option<Iwd2Kits> {
    let active = state.active();
    let idx = active.selected_party_index?;
    let member = active.save.party_npcs.get(idx)?;
    let NpcCre::Cre(imported) = member.cre.as_ref()? else {
        return None;
    };
    imported.cre().iwd2_kits()
}

/// Write `kits` back to the selected party creature. No-op when the slot
/// is empty or the creature isn't a V2.2 record.
fn set_selected_kits(state: &mut AppState, kits: Iwd2Kits) {
    let active = state.active_mut();
    let Some(idx) = active.selected_party_index else {
        return;
    };
    let Some(member) = active.save.party_npcs.get_mut(idx) else {
        return;
    };
    let Some(NpcCre::Cre(imported)) = member.cre.as_mut() else {
        return;
    };
    imported.cre_mut().set_iwd2_kits(kits);
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
