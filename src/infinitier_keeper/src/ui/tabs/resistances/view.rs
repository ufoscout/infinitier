//! Rendering for the Resistances tab.
//!
//! AD&D creatures (V1.0 / V1.2 / V9.0 — including the Enhanced Editions)
//! are **editable**: a "Resistances" box and a "Saving Throws" box side
//! by side, with an "Armor Class Modifiers" box below the resistances.
//! Each field is a numeric input bound to its storage type, so values
//! clamp to the byte/word range automatically. IWD2 (V2.2) mirrors
//! DaleKeeper2's "Resistances/Saves" layout, read-only, in subtle
//! framed boxes.

use eframe::egui;
use egui_components::Card;
use egui_components::scroll_area::ScrollArea;
use egui_components::theme::Theme;
use egui_components::{Label, LabelTone};
use infinitier_core::resource::cre::Cre;

use super::data::{AcModifiers, Iwd2Saves, ResistData, Resistances, SavingThrows};
use super::with_selected_cre_mut;
use crate::state::AppState;

/// Width of a numeric box / input — narrow, since every value is at
/// most a few characters (a percent, a save, or a signed AC modifier).
const NUM_BOX_W: f32 = 48.0;
/// Cap the IWD2 two-column layout so it doesn't stretch the whole tab.
const IWD2_MAX_WIDTH: f32 = 460.0;

pub fn render(ui: &mut egui::Ui, data: &ResistData, state: &mut AppState) {
    ScrollArea::vertical().show(ui, |ui| match data {
        ResistData::Adnd {
            resistances,
            saving_throws,
            ac_modifiers,
        } => {
            ui.columns(2, |cols| {
                adnd_resistances_card(&mut cols[0], resistances, state);
                cols[0].add_space(8.0);
                ac_modifiers_card(&mut cols[0], ac_modifiers, state);

                saving_throws_card(&mut cols[1], saving_throws, state);
            });
        }
        ResistData::Iwd2 {
            resistances,
            magic_damage,
            saves,
        } => {
            ui.set_max_width(IWD2_MAX_WIDTH);
            iwd2_resistances_card(ui, resistances, *magic_damage);
            ui.add_space(8.0);
            iwd2_saves_card(ui, saves);
        }
    });
}

// ── AD&D (editable) ──────────────────────────────────────────────────

fn adnd_resistances_card(ui: &mut egui::Ui, r: &Resistances, state: &mut AppState) {
    Card::new().title("Resistances").divider().show(ui, |ui| {
        // Two label/input pairs per row, matching EEKeeper's two columns.
        // The right column's last cell (opposite "Piercing") is empty.
        egui::Grid::new("resistances_grid")
            .num_columns(4)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                resist_pair(ui, state, "Acid", r.acid, Cre::set_resist_acid, Some(("Slashing", r.slashing, Cre::set_resist_slashing)));
                resist_pair(ui, state, "Cold", r.cold, Cre::set_resist_cold, Some(("Missile", r.missile, Cre::set_resist_missile)));
                resist_pair(ui, state, "Electricity", r.electricity, Cre::set_resist_electricity, Some(("Magic", r.magic, Cre::set_resist_magic)));
                resist_pair(ui, state, "Fire", r.fire, Cre::set_resist_fire, Some(("Magic Fire", r.magic_fire, Cre::set_resist_magic_fire)));
                resist_pair(ui, state, "Crushing", r.crushing, Cre::set_resist_crushing, Some(("Magic Cold", r.magic_cold, Cre::set_resist_magic_cold)));
                resist_pair(ui, state, "Piercing", r.piercing, Cre::set_resist_piercing, None);
            });
    });
}

fn saving_throws_card(ui: &mut egui::Ui, s: &SavingThrows, state: &mut AppState) {
    Card::new().title("Saving Throws").divider().show(ui, |ui| {
        egui::Grid::new("saving_throws_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                save_edit_row(ui, state, "Paralyzation, Poison, Death", s.paralyze_poison_death, Cre::set_save_vs_death);
                save_edit_row(ui, state, "Rod, Staff, Wand", s.rod_staff_wand, Cre::set_save_vs_wands);
                save_edit_row(ui, state, "Petrification, Polymorph", s.petrify_polymorph, Cre::set_save_vs_polymorph);
                save_edit_row(ui, state, "Breath Weapons", s.breath, Cre::set_save_vs_breath);
                save_edit_row(ui, state, "Spells", s.spells, Cre::set_save_vs_spells);
            });
    });
}

fn ac_modifiers_card(ui: &mut egui::Ui, ac: &AcModifiers, state: &mut AppState) {
    Card::new()
        .title("Armor Class Modifiers")
        .divider()
        .show(ui, |ui| {
            egui::Grid::new("ac_modifiers_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ac_edit_row(ui, state, "Slashing", ac.slashing, Cre::set_ac_slashing_modifier);
                    ac_edit_row(ui, state, "Missile", ac.missile, Cre::set_ac_missile_modifier);
                    ac_edit_row(ui, state, "Crushing", ac.crushing, Cre::set_ac_crushing_modifier);
                    ac_edit_row(ui, state, "Piercing", ac.piercing, Cre::set_ac_piercing_modifier);
                });
        });
}

// ── IWD2 (read-only) ─────────────────────────────────────────────────

fn iwd2_resistances_card(ui: &mut egui::Ui, r: &Resistances, magic_damage: i8) {
    Card::new().title("Resistances").divider().show(ui, |ui| {
        // Twelve resistances in two alphabetical columns (DaleKeeper2);
        // `magic` is spell resistance ("Spells") on IWD2.
        egui::Grid::new("iwd2_resistances_grid")
            .num_columns(4)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                value_pair(ui, "Acid", r.acid, "Magic Damage", Some(magic_damage));
                value_pair(ui, "Cold", r.cold, "Magic Fire", Some(r.magic_fire));
                value_pair(ui, "Crushing", r.crushing, "Missile", Some(r.missile));
                value_pair(ui, "Electricity", r.electricity, "Piercing", Some(r.piercing));
                value_pair(ui, "Fire", r.fire, "Slashing", Some(r.slashing));
                value_pair(ui, "Magic Cold", r.magic_cold, "Spells", Some(r.magic));
            });
    });
}

fn iwd2_saves_card(ui: &mut egui::Ui, s: &Iwd2Saves) {
    Card::new().title("Saving Throws").divider().show(ui, |ui| {
        egui::Grid::new("iwd2_saves_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                save_row(ui, "Fortitude", s.fortitude);
                save_row(ui, "Reflex", s.reflex);
                save_row(ui, "Will", s.will);
            });
    });
}

// ── Editable cells ───────────────────────────────────────────────────

/// One resistance cell: label, current value, and the CRE setter.
type ResistCell<'a> = (&'a str, i8, fn(&mut Cre, i8));

/// A `label [i8] [ label [i8] ]` grid row. The right pair is omitted
/// (blank cells) when `right` is `None`. Each input is bound to an `i8`,
/// so it clamps to the signed-byte range automatically.
fn resist_pair(
    ui: &mut egui::Ui,
    state: &mut AppState,
    left_label: &str,
    left_value: i8,
    left_set: fn(&mut Cre, i8),
    right: Option<ResistCell>,
) {
    ui.add(Label::new(left_label));
    edit_i8(ui, state, left_value, left_set);
    match right {
        Some((label, value, set)) => {
            ui.add(Label::new(label));
            edit_i8(ui, state, value, set);
        }
        None => {
            ui.label("");
            ui.label("");
        }
    }
    ui.end_row();
}

fn save_edit_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    label: &str,
    value: u8,
    set: fn(&mut Cre, u8),
) {
    ui.add(Label::new(label));
    let mut v = value;
    if numeric_input(ui, &mut v).changed() {
        with_selected_cre_mut(state, |c| set(c, v));
    }
    ui.end_row();
}

fn ac_edit_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    label: &str,
    value: i16,
    set: fn(&mut Cre, i16),
) {
    ui.add(Label::new(label));
    let mut v = value;
    if numeric_input(ui, &mut v).changed() {
        with_selected_cre_mut(state, |c| set(c, v));
    }
    ui.end_row();
}

fn edit_i8(ui: &mut egui::Ui, state: &mut AppState, value: i8, set: fn(&mut Cre, i8)) {
    let mut v = value;
    if numeric_input(ui, &mut v).changed() {
        with_selected_cre_mut(state, |c| set(c, v));
    }
}

/// A fixed-width numeric drag input. Bound to the value's own integer
/// type, so it clamps to that type's range (the "proper clamp" — a
/// resistance can't overflow its signed byte, a save its `u8`, etc.).
fn numeric_input<T: egui::emath::Numeric>(ui: &mut egui::Ui, value: &mut T) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.add_sized([NUM_BOX_W, h], egui::DragValue::new(value))
}

// ── Read-only widgets (IWD2) ─────────────────────────────────────────

/// One grid row of `label : value` (unsigned).
fn save_row(ui: &mut egui::Ui, label: &str, value: u8) {
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

/// Render a value inside a subtle, non-editable framed box.
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
