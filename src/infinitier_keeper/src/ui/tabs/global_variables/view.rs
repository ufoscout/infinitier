//! Read-only rendering for the Global Variables tab — EEKeeper's
//! two-column table (Name · Value).
//!
//! Uses [`egui_extras::TableBuilder`] for real, resizable columns with
//! a sticky header and virtualised rows (only the visible slice is
//! laid out each frame — the list runs to ~2000 entries).

use eframe::egui;
use egui_components::Label;
use egui_extras::{Column, TableBuilder};

use super::data::GlobalVar;

/// Initial width of the name column (resizable by the user).
const NAME_COL_W: f32 = 300.0;

pub fn render(ui: &mut egui::Ui, rows: &[GlobalVar]) {
    let row_h = ui.text_style_height(&egui::TextStyle::Body);

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .column(Column::initial(NAME_COL_W).at_least(120.0).clip(true))
        .column(Column::remainder())
        .header(row_h, |mut header| {
            header.col(|ui| {
                ui.add(Label::new("Name").strong());
            });
            header.col(|ui| {
                ui.add(Label::new("Value").strong());
            });
        })
        .body(|body| {
            body.rows(row_h, rows.len(), |mut row| {
                let var = &rows[row.index()];
                row.col(|ui| {
                    ui.add(Label::new(var.name));
                });
                row.col(|ui| {
                    ui.add(Label::new(var.value.to_string()));
                });
            });
        });
}
