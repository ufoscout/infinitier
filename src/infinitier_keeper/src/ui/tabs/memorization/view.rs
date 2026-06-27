//! Rendering for the Memorization tab — EEKeeper's three-column table
//! (Type · Level · Max Can Memorise). The "Max Can Memorise" cell is
//! **editable** (it writes the slot's `num_memorizable_total`); the type
//! and level are fixed identifiers and stay read-only.

use eframe::egui;
use egui_components::{Label, Table, TableColumn};

use super::data::MemRow;
use super::with_selected_cre_mut;
use crate::state::AppState;

/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 480.0;
/// Width of the editable "Max Can Memorise" drag input.
const MAX_INPUT_W: f32 = 60.0;

pub fn render(ui: &mut egui::Ui, rows: &[MemRow], state: &mut AppState) {
    let size = egui::vec2(MAX_TABLE_W.min(ui.available_width()), ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("memorization")
            .striped(true)
            .max_height(ui.available_height())
            .column(
                TableColumn::remainder()
                    .at_least(140.0)
                    .clip(true)
                    .header("Type"),
            )
            .column(TableColumn::exact(80.0).header("Level"))
            .column(TableColumn::exact(150.0).header("Max Can Memorise"))
            .show(ui, |body| {
                body.rows(rows.len(), |i, mut row| {
                    let r = &rows[i];
                    row.col(|ui| {
                        ui.add(Label::new(r.type_name));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(r.level.to_string()));
                    });
                    row.col(|ui| {
                        // The row order matches the CRE's
                        // `spell_memorization_info`, so `i` is the slot index.
                        let mut max = r.max;
                        let h = ui.spacing().interact_size.y;
                        let resp =
                            ui.add_sized([MAX_INPUT_W, h], egui::DragValue::new(&mut max));
                        if resp.changed() {
                            with_selected_cre_mut(state, |c| {
                                c.set_spell_memorization_total(i, max)
                            });
                        }
                    });
                });
            });
    });
}
