//! Read-only rendering for the Innate tab — EEKeeper's four-column
//! table (Level · xMem · Spell · Resource).
//!
//! The spell display name is resolved per resref by loading the SPL
//! and looking its generic-name strref up in `dialog.tlk`. Both the
//! SPL parse and the TLK parse are expensive, so resolved names are
//! memoised in the egui frame store (keyed by resref) — we resolve
//! each spell at most once instead of on every repaint. Rows are then
//! ordered by display name, the way EEKeeper sorts the list.

use std::collections::HashMap;

use eframe::egui;
use egui_components::Label;
use egui_components::scroll_area::ScrollArea;
use infinitier_core::game::GameData;
use infinitier_core::resource::tlk::Tlk;

use super::data::InnateRow;

pub fn render(ui: &mut egui::Ui, rows: &[InnateRow], game_data: &GameData) {
    // Resolve names first (loading dialog.tlk at most once), then sort
    // by name to match EEKeeper.
    let names = resolved_names(ui, game_data, rows);
    let mut named: Vec<(String, &InnateRow)> = rows
        .iter()
        .map(|row| {
            let name = names
                .get(&row.resource)
                .cloned()
                .unwrap_or_else(|| row.resource.clone());
            (name, row)
        })
        .collect();
    named.sort_by_key(|(name, _)| name.to_lowercase());

    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("innate_table")
            .num_columns(4)
            .striped(true)
            .spacing([24.0, 5.0])
            .show(ui, |ui| {
                ui.add(Label::new("Level").strong());
                ui.add(Label::new("xMem").strong());
                ui.add(Label::new("Spell").strong());
                ui.add(Label::new("Resource").strong());
                ui.end_row();

                for (name, row) in &named {
                    ui.add(Label::new(row.level.to_string()));
                    ui.add(Label::new(row.x_mem.to_string()));
                    ui.add(Label::new(name.as_str()));
                    ui.add(Label::new(row.resource.as_str()));
                    ui.end_row();
                }
            });
    });
}

/// Build the resref → display-name map for every spell in `rows`,
/// memoised in the egui frame store (keyed by resref). `dialog.tlk` is
/// loaded at most once per call, and only when there are resrefs not
/// already cached — so the expensive TLK parse happens once on first
/// open, never per spell and never on a fully-cached repaint.
fn resolved_names(
    ui: &mut egui::Ui,
    game_data: &GameData,
    rows: &[InnateRow],
) -> HashMap<String, String> {
    let id = egui::Id::new("innate_spell_name_cache");
    let cached = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<String, String>>(id))
        .unwrap_or_default();

    let misses: Vec<&str> = rows
        .iter()
        .map(|row| row.resource.as_str())
        .filter(|r| !r.is_empty() && !cached.contains_key(*r))
        .collect();
    if misses.is_empty() {
        return cached;
    }

    // One TLK load for every miss, instead of one per spell.
    let tlk = game_data.dialog_tlk().ok();
    let tlk = tlk.as_deref();
    ui.ctx().data_mut(|d| {
        let map = d.get_temp_mut_or_default::<HashMap<String, String>>(id);
        for resref in misses {
            map.entry(resref.to_string()).or_insert_with(|| {
                resolve_spell_name(game_data, tlk, resref).unwrap_or_else(|| resref.to_string())
            });
        }
        map.clone()
    })
}

/// Load the SPL, read its generic-name strref, and resolve it through
/// the supplied `dialog.tlk`. `None` if any step fails or the name is
/// empty.
fn resolve_spell_name(game_data: &GameData, tlk: Option<&Tlk>, resref: &str) -> Option<String> {
    let spl = game_data.import_spl_by_name(resref).ok()?;
    let name = tlk?.get(spl.header.name_strref())?;
    (!name.is_empty()).then_some(name)
}
