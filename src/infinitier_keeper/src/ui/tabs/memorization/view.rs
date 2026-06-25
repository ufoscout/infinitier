//! Read-only rendering for the Memorization tab — EEKeeper's
//! three-column table (Type · Level · Max Can Memorise).

use eframe::egui;
use egui_components::{Label, Table, TableColumn};

use super::data::MemRow;

/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 480.0;

pub fn render(ui: &mut egui::Ui, rows: &[MemRow]) {
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
                        ui.add(Label::new(r.max.to_string()));
                    });
                });
            });
    });
}
