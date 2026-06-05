//! Read-only rendering for the Cleric tab — EEKeeper's four-column
//! table (Level · xMem · Spell · Resource).
//!
//! The spell display name is resolved per resref by loading the SPL
//! and looking its generic-name strref up in `dialog.tlk`. Both the
//! SPL parse and the TLK parse are expensive, so resolved names are
//! memoised in the egui frame store (keyed by resref) — we resolve
//! each spell at most once instead of on every repaint. Rows are then
//! ordered by (level, display name), the way EEKeeper sorts the list.

use std::collections::HashMap;

use eframe::egui;
use egui_components::Label;
use egui_components::scroll_area::ScrollArea;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::resource::ResourceType;

use super::data::ClericRow;

pub fn render(ui: &mut egui::Ui, rows: &[ClericRow], game_data: &GameData) {
    // Resolve names first, then sort by (level, name) to match EEKeeper.
    let mut named: Vec<(String, &ClericRow)> = rows
        .iter()
        .map(|row| (spell_name(ui, game_data, &row.resource), row))
        .collect();
    named.sort_by(|(a_name, a), (b_name, b)| {
        a.level
            .cmp(&b.level)
            .then_with(|| a_name.to_lowercase().cmp(&b_name.to_lowercase()))
    });

    ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("cleric_table")
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

/// Resolve a spell resref to its display name (the SPL's generic-name
/// strref, looked up in `dialog.tlk`), memoising the result per resref
/// in the egui frame store. Falls back to the resref itself when the
/// SPL or string can't be resolved.
fn spell_name(ui: &mut egui::Ui, game_data: &GameData, resref: &str) -> String {
    let id = egui::Id::new("cleric_spell_name_cache");
    if let Some(hit) = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<String, String>>(id))
        .and_then(|m| m.get(resref).cloned())
    {
        return hit;
    }

    let resolved = resolve_spell_name(game_data, resref).unwrap_or_else(|| resref.to_string());
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<HashMap<String, String>>(id)
            .insert(resref.to_string(), resolved.clone());
    });
    resolved
}

/// Load the SPL, read its generic-name strref, and resolve it through
/// `dialog.tlk`. `None` if any step fails or the name is empty.
fn resolve_spell_name(game_data: &GameData, resref: &str) -> Option<String> {
    let imported = game_data
        .import_by_name_and_type(resref, ResourceType::Spl)
        .ok()?;
    let ImportedResource::Spl(spl) = imported.as_ref() else {
        return None;
    };
    let name = game_data.dialog_tlk().ok()?.get(spl.header.name_strref())?;
    (!name.is_empty()).then_some(name)
}
