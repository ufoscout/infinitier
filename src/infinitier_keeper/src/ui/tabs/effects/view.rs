//! Read-only rendering for the Effects tab — EEKeeper's wide
//! effect table (Type · Name · Parameter 1 · Parameter 2 · Resource
//! 0..3 · Time · Flags · Target).
//!
//! The opcode → "Type" name comes from [`super::opcode_names`]; each
//! resource resref is resolved to its `SPELL.IDS` symbol (title-cased,
//! e.g. `SPWI923` → "Wizard Summon Planatar Good [SPWI923]"), memoised
//! per resref in the egui frame store; timing mode and target are
//! mapped to EEKeeper's labels. The table is wide, so it lives in a
//! both-axis scroll area with fixed, resizable columns.

use std::collections::HashMap;

use eframe::egui;
use egui_components::Label;
use egui_extras::{Column, TableBuilder};
use infinitier_core::game::GameData;
use infinitier_core::resource::ids::Ids;

use super::data::EffectRow;
use super::opcode_names::effect_name;

/// egui frame-store key for the resref → display-name cache.
const RESOURCE_CACHE: &str = "effects_resource_cache";

pub fn render(ui: &mut egui::Ui, rows: &[EffectRow], game_data: &GameData) {
    let row_h = ui.text_style_height(&egui::TextStyle::Body);

    // Resolve each distinct, non-empty resref to its display name,
    // memoised per resref in the egui frame store. `SPELL.IDS` is
    // loaded at most once, and only when there are uncached resrefs.
    let resolved = resolve_resources(ui, game_data, rows);

    egui::ScrollArea::both().show(ui, |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .vscroll(false)
            .column(
                Column::initial(250.0)
                    .at_least(120.0)
                    .clip(true)
                    .resizable(true),
            ) // Type
            .column(Column::initial(70.0).clip(true).resizable(true)) // Name
            .column(Column::initial(78.0).resizable(true)) // Parameter 1
            .column(Column::initial(78.0).resizable(true)) // Parameter 2
            .column(Column::initial(160.0).clip(true).resizable(true)) // Resource 0
            .column(Column::initial(110.0).clip(true).resizable(true)) // Resource 1
            .column(Column::initial(110.0).clip(true).resizable(true)) // Resource 2
            .column(Column::initial(160.0).clip(true).resizable(true)) // Resource 3
            .column(Column::initial(80.0).resizable(true)) // Time
            .column(Column::initial(150.0).clip(true).resizable(true)) // Flags
            .column(Column::initial(140.0).clip(true).resizable(true)) // Target
            .header(row_h, |mut header| {
                for title in [
                    "Type",
                    "Name",
                    "Parameter 1",
                    "Parameter 2",
                    "Resource 0",
                    "Resource 1",
                    "Resource 2",
                    "Resource 3",
                    "Time",
                    "Flags",
                    "Target",
                ] {
                    header.col(|ui| {
                        ui.add(Label::new(title).strong());
                    });
                }
            })
            .body(|mut body| {
                for row in rows {
                    body.row(row_h, |mut tr| {
                        tr.col(|ui| {
                            ui.add(Label::new(type_text(row.opcode)));
                        });
                        tr.col(|ui| {
                            ui.add(Label::new(row.name.as_str()));
                        });
                        tr.col(|ui| {
                            ui.add(Label::new(row.param1.to_string()));
                        });
                        tr.col(|ui| {
                            ui.add(Label::new(row.param2.to_string()));
                        });
                        for resref in &row.resources {
                            tr.col(|ui| {
                                let text = resolved.get(resref).map_or("", String::as_str);
                                ui.add(Label::new(text));
                            });
                        }
                        tr.col(|ui| {
                            ui.add(Label::new(row.time.to_string()));
                        });
                        tr.col(|ui| {
                            ui.add(Label::new(timing_text(row.timing_mode)));
                        });
                        tr.col(|ui| {
                            ui.add(Label::new(target_text(row.target)));
                        });
                    });
                }
            });
    });
}

/// EEKeeper's "Type" text for an opcode, e.g.
/// `"Spell: Wizard Spell Slots Modifier [42]"`. Falls back to a
/// bracketed number for opcodes absent from the table.
fn type_text(opcode: u32) -> String {
    effect_name(opcode).map_or_else(|| format!("Unknown [{opcode}]"), str::to_owned)
}

/// Map an effect timing mode to EEKeeper's "Flags" label.
fn timing_text(mode: u32) -> String {
    let label = match mode {
        0 => "Duration",
        1 => "Permanent",
        2 => "While Equipped",
        3 => "Delayed Duration",
        4 => "Delayed",
        5 => "Delayed, Equipped",
        6 => "Delayed, Absolute Duration",
        7 => "Delayed, Permanent",
        8 => "Equipped, Permanent",
        9 => "Instant, Permanent",
        // EE absolute-duration flag (0x1000): the duration field holds
        // an absolute game-time rather than a relative span.
        4096 => "Absolute Duration",
        other => return other.to_string(),
    };
    label.to_owned()
}

/// Map an effect target dispatcher to EEKeeper's "Target" label.
fn target_text(target: u32) -> String {
    let label = match target {
        0 => "None",
        1 => "Self",
        2 => "Preset Target",
        3 => "Party",
        4 => "Everyone",
        5 => "Everyone Except Party",
        6 => "Caster Group",
        7 => "Target Group",
        8 => "Everyone Except Self",
        9 => "Original Caster",
        other => return other.to_string(),
    };
    label.to_owned()
}

/// Build the resref → display-name map for every resource referenced
/// by `rows`. Results are memoised in the egui frame store; `SPELL.IDS`
/// is loaded at most once per call, and only if there are resrefs not
/// already cached.
fn resolve_resources(
    ui: &egui::Ui,
    game_data: &GameData,
    rows: &[EffectRow],
) -> HashMap<String, String> {
    let id = egui::Id::new(RESOURCE_CACHE);
    let cached = ui
        .ctx()
        .data_mut(|d| d.get_temp::<HashMap<String, String>>(id))
        .unwrap_or_default();

    // Distinct non-empty resrefs not yet resolved.
    let misses: Vec<&str> = rows
        .iter()
        .flat_map(|r| r.resources.iter())
        .map(String::as_str)
        .filter(|r| !r.is_empty() && !cached.contains_key(*r))
        .collect();

    if misses.is_empty() {
        return cached;
    }

    let spell_ids = game_data.import_ids_by_name("SPELL").ok();
    ui.ctx().data_mut(|d| {
        let map = d.get_temp_mut_or_default::<HashMap<String, String>>(id);
        for resref in misses {
            map.entry(resref.to_owned())
                .or_insert_with(|| resource_display(spell_ids.as_deref(), resref));
        }
        map.clone()
    })
}

/// Render a resource resref the way EEKeeper does: `"Spell Name
/// [RESREF]"` when the resref maps to a `SPELL.IDS` symbol, otherwise
/// the bare resref.
fn resource_display(spell_ids: Option<&Ids>, resref: &str) -> String {
    match spell_ids
        .zip(spell_ids_value(resref))
        .and_then(|(ids, value)| ids.of_value(value))
    {
        Some(symbol) => format!("{} [{resref}]", title_case(symbol)),
        None => resref.to_owned(),
    }
}

/// Map a spell resref to its `SPELL.IDS` numeric value: the two-letter
/// school code after `SP` selects a thousands digit (PR→1 priest,
/// WI→2 wizard, IN→3 innate, CL→4 special) and the trailing number is
/// added (e.g. `SPWI923` → 2923). `None` for non-spell resrefs.
fn spell_ids_value(resref: &str) -> Option<i32> {
    let body = resref
        .get(..2)
        .filter(|p| p.eq_ignore_ascii_case("SP"))
        .map(|_| &resref[2..])?;
    let (code, number) = body.split_at_checked(2)?;
    let thousands = match code.to_ascii_uppercase().as_str() {
        "PR" => 1,
        "WI" => 2,
        "IN" => 3,
        "CL" => 4,
        _ => return None,
    };
    let n: i32 = number.parse().ok()?;
    Some(thousands * 1000 + n)
}

/// Title-case a `SPELL.IDS` symbol: `WIZARD_SUMMON_PLANATAR_GOOD` →
/// `Wizard Summon Planatar Good`.
fn title_case(symbol: &str) -> String {
    symbol
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

