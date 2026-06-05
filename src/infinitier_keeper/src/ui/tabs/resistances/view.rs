//! Read-only rendering for the Resistances tab.
//!
//! Mirrors EEKeeper's layout: a "Resistances" box and a "Saving
//! Throws" box side by side, with an "Armor Class Modifiers" box below
//! the resistances. Values are shown in subtle non-editable framed
//! boxes, matching the other read-only tabs.

use eframe::egui;
use egui_components::Card;
use egui_components::scroll_area::ScrollArea;
use egui_components::theme::Theme;
use egui_components::{Label, LabelTone};

use super::data::{AcModifiers, ResistData, Resistances, SavingThrows};

/// Width of a numeric value box — narrow, since every value is at most
/// a few characters (a percent, a save, or a signed AC modifier).
const NUM_BOX_W: f32 = 48.0;

pub fn render(ui: &mut egui::Ui, data: &ResistData) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.columns(2, |cols| {
            resistances_card(&mut cols[0], &data.resistances);
            cols[0].add_space(8.0);
            ac_modifiers_card(&mut cols[0], &data.ac_modifiers);

            if let Some(saves) = &data.saving_throws {
                saving_throws_card(&mut cols[1], saves);
            }
        });
    });
}

fn resistances_card(ui: &mut egui::Ui, r: &Resistances) {
    Card::new().title("Resistances").divider().show(ui, |ui| {
        // Two label/value pairs per row, matching EEKeeper's two
        // columns. The right column runs one row short, so its last
        // cell (opposite "Piercing") is left empty. The three
        // right-column "Magic" rows are Magic / Magic Fire / Magic
        // Cold respectively.
        egui::Grid::new("resistances_grid")
            .num_columns(4)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                value_pair(ui, "Acid", r.acid, "Slashing", Some(r.slashing));
                value_pair(ui, "Cold", r.cold, "Missile", Some(r.missile));
                value_pair(ui, "Electricity", r.electricity, "Magic", Some(r.magic));
                value_pair(ui, "Fire", r.fire, "Magic Fire", Some(r.magic_fire));
                value_pair(ui, "Crushing", r.crushing, "Magic Cold", Some(r.magic_cold));
                value_pair(ui, "Piercing", r.piercing, "", None);
            });
    });
}

fn saving_throws_card(ui: &mut egui::Ui, s: &SavingThrows) {
    Card::new().title("Saving Throws").divider().show(ui, |ui| {
        egui::Grid::new("saving_throws_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                save_row(ui, "Paralyzation, Poison, Death", s.paralyze_poison_death);
                save_row(ui, "Rod, Staff, Wand", s.rod_staff_wand);
                save_row(ui, "Petrification, Polymorph", s.petrify_polymorph);
                save_row(ui, "Breath Weapons", s.breath);
                save_row(ui, "Spells", s.spells);
            });
    });
}

fn ac_modifiers_card(ui: &mut egui::Ui, ac: &AcModifiers) {
    Card::new()
        .title("Armor Class Modifiers")
        .divider()
        .show(ui, |ui| {
            egui::Grid::new("ac_modifiers_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    signed_row(ui, "Slashing", ac.slashing);
                    signed_row(ui, "Missile", ac.missile);
                    signed_row(ui, "Crushing", ac.crushing);
                    signed_row(ui, "Piercing", ac.piercing);
                });
        });
}

// ── Shared widgets ───────────────────────────────────────────────────

/// One grid row of `label : value` (unsigned).
fn save_row(ui: &mut egui::Ui, label: &str, value: u8) {
    ui.add(Label::new(label));
    num_box(ui, &value.to_string());
    ui.end_row();
}

/// One grid row of `label : value` (signed AC modifier).
fn signed_row(ui: &mut egui::Ui, label: &str, value: i16) {
    ui.add(Label::new(label));
    num_box(ui, &value.to_string());
    ui.end_row();
}

/// A `label : value [ label : value ]` grid row. The right pair is
/// omitted (blank cells) when `right_value` is `None`.
fn value_pair(
    ui: &mut egui::Ui,
    left_label: &str,
    left_value: i8,
    right_label: &str,
    right_value: Option<i8>,
) {
    ui.add(Label::new(left_label));
    num_box(ui, &left_value.to_string());
    match right_value {
        Some(v) => {
            ui.add(Label::new(right_label));
            num_box(ui, &v.to_string());
        }
        None => {
            ui.label("");
            ui.label("");
        }
    }
    ui.end_row();
}

/// Render a value inside a subtle, non-editable framed box so the
/// display reads like EEKeeper's (disabled) field controls.
fn num_box(ui: &mut egui::Ui, text: &str) {
    let theme = Theme::get(ui.ctx());
    egui::Frame::new()
        .fill(theme.colors.muted_background)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.set_min_width(NUM_BOX_W);
            ui.add(Label::new(text).tone(LabelTone::Default));
        });
}
