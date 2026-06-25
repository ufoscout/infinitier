//! Read-only rendering for the Global Variables tab — EEKeeper's
//! two-column table (Name · Value).
//!
//! Uses [`egui_components::Table`] for a sticky header and virtualised
//! rows (only the visible slice is laid out each frame — the list runs
//! to ~2000 entries).

use eframe::egui;
use egui_components::{Label, Table, TableColumn};

use super::data::GlobalVar;

/// Initial width of the name column.
const NAME_COL_W: f32 = 300.0;
/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 480.0;

pub fn render(ui: &mut egui::Ui, rows: &[GlobalVar]) {
    let size = egui::vec2(MAX_TABLE_W.min(ui.available_width()), ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("global_vars")
            .max_height(ui.available_height())
            .column(
                TableColumn::initial(NAME_COL_W)
                    .at_least(120.0)
                    .clip(true)
                    .header("Name"),
            )
            .column(TableColumn::remainder().header("Value"))
            .show(ui, |body| {
                body.rows(rows.len(), |i, mut row| {
                    let var = &rows[i];
                    row.col(|ui| {
                        ui.add(Label::new(var.name));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(var.value.to_string()));
                    });
                });
            });
    });
}
