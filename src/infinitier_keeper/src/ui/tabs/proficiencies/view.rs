//! Rendering for the Proficiencies tab — EEKeeper's three-column table
//! (Proficiency · First Class · Second Class). Editable on the Enhanced
//! Edition engine; read-only elsewhere.

use eframe::egui;
use egui_components::scroll_area::ScrollArea;
use egui_components::{Label, LabelTone};
use infinitier_core::resource::cre::WeaponProficiency;

use super::data::ProfRow;
use super::with_selected_cre_mut;
use crate::state::AppState;

const VALUE_COL_W: f32 = 90.0;
/// Proficiency pips max out at 5 (EEKeeper's cap); the engine packs each
/// slot into three bits, so the stored value never exceeds this.
const MAX_POINTS: u8 = 5;

pub fn render(ui: &mut egui::Ui, rows: &[ProfRow], editable: bool, state: &mut AppState) {
    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("proficiencies_table")
            .num_columns(3)
            .striped(true)
            .spacing([24.0, 5.0])
            .show(ui, |ui| {
                ui.add(Label::new("Proficiency").strong());
                ui.add(Label::new("First Class").strong());
                ui.add(Label::new("Second Class").strong());
                ui.end_row();

                for row in rows {
                    ui.add(Label::new(row.name.clone()));
                    if editable {
                        edit_cell(ui, state, row, Slot::First);
                    } else {
                        value_cell(ui, row.first);
                    }
                    // The second-class slot only applies to a dual-classed
                    // character's weapons; elsewhere it stays read-only.
                    if editable && row.has_second_class {
                        edit_cell(ui, state, row, Slot::Second);
                    } else {
                        value_cell(ui, row.second);
                    }
                    ui.end_row();
                }
            });
    });
}

/// Which class slot a cell edits.
#[derive(Clone, Copy)]
enum Slot {
    First,
    Second,
}

/// An editable proficiency-point cell. A change rewrites the whole packed
/// byte for the row's stat, preserving the other slot's value.
fn edit_cell(ui: &mut egui::Ui, state: &mut AppState, row: &ProfRow, slot: Slot) {
    let mut value = match slot {
        Slot::First => row.first as u8,
        Slot::Second => row.second as u8,
    };
    let resp = ui.add_sized(
        [VALUE_COL_W, ui.spacing().interact_size.y],
        egui::DragValue::new(&mut value).range(0..=MAX_POINTS),
    );
    if resp.changed() {
        let points = match slot {
            Slot::First => WeaponProficiency {
                first_class: value,
                second_class: row.second as u8,
            },
            Slot::Second => WeaponProficiency {
                first_class: row.first as u8,
                second_class: value,
            },
        };
        let stat = row.stat;
        with_selected_cre_mut(state, |c| c.set_proficiency(stat, points));
    }
}

/// A read-only proficiency-point cell: the number, or blank for zero
/// (matching EEKeeper, which leaves untrained proficiencies empty).
fn value_cell(ui: &mut egui::Ui, points: u32) {
    let text = if points == 0 {
        String::new()
    } else {
        points.to_string()
    };
    ui.add_sized(
        [VALUE_COL_W, ui.spacing().interact_size.y],
        Label::new(text).tone(LabelTone::Default),
    );
}
