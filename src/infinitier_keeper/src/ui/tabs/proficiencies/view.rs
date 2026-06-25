//! Rendering for the Proficiencies tab — EEKeeper's three-column table
//! (Proficiency · First Class · Second Class). Editable on the Enhanced
//! Edition engine; read-only elsewhere.

use eframe::egui;
use egui_components::{Label, Table, TableColumn};
use infinitier_core::resource::cre::WeaponProficiency;

use super::data::ProfRow;
use super::with_selected_cre_mut;
use crate::state::AppState;

const VALUE_COL_W: f32 = 110.0;
const MAX_TABLE_W: f32 = 480.0;
/// Proficiency pips max out at 5 (EEKeeper's cap);
const MAX_POINTS: u8 = 5;

pub fn render(ui: &mut egui::Ui, rows: &[ProfRow], editable: bool, state: &mut AppState) {
    let width = MAX_TABLE_W.min(ui.available_width());
    let size = egui::vec2(width, ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("proficiencies")
            .striped(true)
            .max_height(ui.available_height())
            .column(
                TableColumn::remainder()
                    .at_least(140.0)
                    .clip(true)
                    .header("Proficiency"),
            )
            .column(TableColumn::exact(VALUE_COL_W).header("First Class"))
            .column(TableColumn::exact(VALUE_COL_W).header("Second Class"))
            .show(ui, |body| {
                body.rows(rows.len(), |i, mut row_ui| {
                    let row = &rows[i];
                    row_ui.col(|ui| {
                        ui.add(Label::new(row.name.clone()));
                    });
                    row_ui.col(|ui| {
                        if editable {
                            edit_cell(ui, state, row, Slot::First);
                        } else {
                            value_cell(ui, row.first);
                        }
                    });
                    row_ui.col(|ui| {
                        // The second-class slot only applies to a dual-classed
                        // character's weapons; elsewhere it stays read-only.
                        if editable && row.has_second_class {
                            edit_cell(ui, state, row, Slot::Second);
                        } else {
                            value_cell(ui, row.second);
                        }
                    });
                });
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
    let resp = ui.add(egui::DragValue::new(&mut value).range(0..=MAX_POINTS));
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
    if points != 0 {
        ui.add(Label::new(points.to_string()));
    }
}
