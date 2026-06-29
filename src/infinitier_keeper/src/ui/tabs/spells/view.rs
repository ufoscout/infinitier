//! Read-only rendering for the IWD2 Spells tab — the four-column table
//! (Level · xMem · Spell · Resource) for the spells of the currently
//! selected category.
//!
//! Each slot stores a row index into the category's list 2DA
//! ([`SpellCategory::list_2da`]); the row's last column is the SPL
//! resref, which is then resolved to a display name via the SPL's
//! generic-name strref and `dialog.tlk`. The 2DA is preloaded/cached by
//! `GameData`, and resolved names are memoised in the egui frame store
//! (keyed by resref), so each spell resolves at most once instead of on
//! every repaint. Rows are ordered by (level, display name).

use std::collections::HashMap;

use eframe::egui;
use egui_components::{Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::resource::tlk::Tlk;

use super::data::{SpellCategory, SpellRow};

/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 560.0;

/// Render the IWD2 spell table for `category`. Returns `(level, list-2DA
/// index)` of a spell whose row "Delete" action was chosen this frame, for
/// the caller to remove from the creature.
pub fn render(
    ui: &mut egui::Ui,
    rows: &[SpellRow],
    category: SpellCategory,
    game_data: &GameData,
) -> Option<(u16, u32)> {
    if rows.is_empty() {
        ui.add(Label::new("No spells of this type.").strong());
        return None;
    }

    // Resolve each slot's list-2DA row index to a SPL resref, then to a
    // display name (loading dialog.tlk at most once), and sort by
    // (level, name).
    let resrefs = resolved_resrefs(game_data, category, rows);
    let names = resolved_names(ui, game_data, &resrefs);
    let mut named: Vec<(String, String, &SpellRow)> = rows
        .iter()
        .map(|row| {
            let resref = resrefs.get(&row.index).cloned().unwrap_or_default();
            let name = names
                .get(&resref)
                .cloned()
                .unwrap_or_else(|| resref.clone());
            (name, resref, row)
        })
        .collect();
    named.sort_by(|(a_name, _, a), (b_name, _, b)| {
        a.level
            .cmp(&b.level)
            .then_with(|| a_name.to_lowercase().cmp(&b_name.to_lowercase()))
    });

    let mut to_delete = None;
    let size = egui::vec2(MAX_TABLE_W.min(ui.available_width()), ui.available_height());
    ui.allocate_ui_with_layout(size, egui::Layout::top_down(egui::Align::Min), |ui| {
        Table::new("iwd2_spells")
            .striped(true)
            .max_height(ui.available_height())
            .column(TableColumn::exact(70.0).header("Level"))
            .column(TableColumn::exact(70.0).header("xMem"))
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
                    let (name, resref, r) = &named[i];
                    row.col(|ui| {
                        ui.add(Label::new(r.level.to_string()));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(r.x_mem.to_string()));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(name.as_str()));
                    });
                    row.col(|ui| {
                        ui.add(Label::new(resref.as_str()));
                    });
                    row.col(|ui| {
                        if crate::ui::tabs::row_delete_menu(ui) {
                            to_delete = Some((r.level, r.index));
                        }
                    });
                });
            });
    });
    to_delete
}

/// Map each distinct slot index to its SPL resref via the category's
/// list 2DA (the row's last column). The 2DA is cached by `GameData`,
/// so this is cheap to call per repaint.
fn resolved_resrefs(
    game_data: &GameData,
    category: SpellCategory,
    rows: &[SpellRow],
) -> HashMap<u32, String> {
    let Ok(list) = game_data.import_2da_by_name(category.list_2da()) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for row in rows {
        out.entry(row.index).or_insert_with(|| {
            list.rows
                .get(&row.index.to_string())
                .and_then(|cells| cells.last())
                .cloned()
                .unwrap_or_default()
        });
    }
    out
}

/// Build the resref → display-name map, memoised in the egui frame store
/// (keyed by resref). `dialog.tlk` is loaded at most once per call, and
/// only when there are resrefs not already cached — so the expensive TLK
/// parse happens once on first open, never per spell and never on a
/// fully-cached repaint.
fn resolved_names(
    ui: &mut egui::Ui,
    game_data: &GameData,
    resrefs: &HashMap<u32, String>,
) -> HashMap<String, String> {
    let id = egui::Id::new("iwd2_spell_name_cache");
    let cached = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<String, String>>(id))
        .unwrap_or_default();

    let misses: Vec<&str> = resrefs
        .values()
        .map(String::as_str)
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
