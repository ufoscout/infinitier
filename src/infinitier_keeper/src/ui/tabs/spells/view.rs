//! Rendering for the unified Spells tab — EEKeeper's four-column table
//! (Level · xMem · Spell · Resource) plus a per-row actions menu, for the
//! spells of the currently selected inner tab (any game).
//!
//! Rows arrive already filtered ([`super::data`]); this layer resolves each
//! spell's display name from its SPL generic-name strref via `dialog.tlk`,
//! sorts by (level, name), and paints the table. Both the SPL parse and the
//! TLK parse are expensive, so resolved names are memoised in the egui frame
//! store (keyed by resref) — each spell resolves at most once instead of on
//! every repaint. A row's "Delete" action returns that row's [`SpellDelete`]
//! for the caller to apply.

use std::collections::HashMap;

use eframe::egui;
use egui_components::{Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::resource::tlk::Tlk;

use super::data::{SpellEvent, SpellRow};

/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 560.0;

/// Render the spell table for the already-filtered `rows`. Returns the
/// [`SpellEvent`] (delete / set-memorised) a row requested this frame, for
/// the caller to apply.
pub fn render(ui: &mut egui::Ui, rows: Vec<SpellRow>, game_data: &GameData) -> Option<SpellEvent> {
    if rows.is_empty() {
        ui.add(Label::new("No spells of this type.").strong());
        return None;
    }

    // Resolve names (loading dialog.tlk at most once), then sort by
    // (level, name) to match EEKeeper.
    let names = resolved_names(ui, game_data, &rows);
    let mut named: Vec<(String, SpellRow)> = rows
        .into_iter()
        .map(|row| {
            let name = names
                .get(&row.resref)
                .cloned()
                .unwrap_or_else(|| row.resref.clone());
            (name, row)
        })
        .collect();
    named.sort_by(|(a_name, a), (b_name, b)| {
        a.level
            .cmp(&b.level)
            .then_with(|| a_name.to_lowercase().cmp(&b_name.to_lowercase()))
    });

    spell_table(ui, &named)
}

/// Paint the table (Level · Memorized · Spell · Resource) plus the per-row
/// actions menu, from `(display name, row)` pairs in display order. The
/// Memorized count is editable. Returns the row's requested [`SpellEvent`].
fn spell_table(ui: &mut egui::Ui, named: &[(String, SpellRow)]) -> Option<SpellEvent> {
    let mut event = None;
    let size = egui::vec2(MAX_TABLE_W.min(ui.available_width()), ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("spells")
            .striped(true)
            .max_height(ui.available_height())
            .column(TableColumn::exact(70.0).header("Level"))
            .column(TableColumn::exact(90.0).header("Memorized"))
            .column(
                TableColumn::remainder()
                    .at_least(160.0)
                    .clip(true)
                    .header("Spell"),
            )
            .column(TableColumn::exact(110.0).header("Resource"))
            .column(TableColumn::exact(60.0)) // per-row actions menu
            .show(ui, |body| {
                body.rows(named.len(), |i, mut row| {
                    let (name, r) = &named[i];
                    row.col(|ui| {
                        ui.add(Label::new(r.level.to_string()));
                    });
                    row.col(|ui| {
                        // Editable memorised count. Size the box to the
                        // DragValue's natural height so it stays vertically
                        // centred (see the Inventory tab for the same fix).
                        let mut mem = r.memorized;
                        let h = ui.text_style_height(&egui::TextStyle::Button)
                            + 2.0 * ui.spacing().button_padding.y;
                        let resp = ui.add_sized(
                            [60.0, h],
                            egui::DragValue::new(&mut mem).range(0..=99),
                        );
                        if resp.changed() {
                            event = Some(SpellEvent::SetMemorized(r.spell.clone(), mem));
                        }
                    });
                    row.col(|ui| {
                        ui.add(Label::new(name.as_str()));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(r.resref.as_str()));
                    });
                    row.col(|ui| {
                        if crate::ui::tabs::row_delete_menu(ui) {
                            event = Some(SpellEvent::Delete(r.spell.clone()));
                        }
                    });
                });
            });
    });
    event
}

/// Build the resref → display-name map for every spell in `rows`, memoised
/// in the egui frame store (keyed by resref). `dialog.tlk` is loaded at most
/// once per call, and only when there are resrefs not already cached — so the
/// expensive TLK parse happens once on first open, never per spell and never
/// on a fully-cached repaint.
fn resolved_names(
    ui: &mut egui::Ui,
    game_data: &GameData,
    rows: &[SpellRow],
) -> HashMap<String, String> {
    let id = egui::Id::new("spell_name_cache");
    let cached = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<String, String>>(id))
        .unwrap_or_default();

    let misses: Vec<&str> = rows
        .iter()
        .map(|row| row.resref.as_str())
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

/// Load the SPL, read its generic-name strref, and resolve it through the
/// supplied `dialog.tlk`. `None` if any step fails or the name is empty.
fn resolve_spell_name(game_data: &GameData, tlk: Option<&Tlk>, resref: &str) -> Option<String> {
    let spl = game_data.import_spl_by_name(resref).ok()?;
    let name = tlk?.get(spl.header.name_strref())?;
    (!name.is_empty()).then_some(name)
}
