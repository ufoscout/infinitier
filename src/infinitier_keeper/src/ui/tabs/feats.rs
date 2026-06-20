//! Feats tab — IWD2 (CRE V2.2) feat editor.
//!
//! Every IWD2 feat (`FEATS.IDS`, 75 entries) is one row: a **Known**
//! checkbox and, for the 26 stackable feats that carry a count byte, a
//! **Level** input. The two are stored independently on the creature:
//!
//! * **Known** — one bit in the `feats_1/2/3` dwords (header `0x01C0`),
//!   indexed by the feat's `FEATS.IDS` value
//!   ([`Cre::iwd2_feat_known`]).
//! * **Level** — a per-feat count byte (`0x01D8..=0x01F1`), addressed by
//!   slot ([`Cre::iwd2_feat_count`]); only the stackable feats have one.
//!
//! Feat display names are resolved dynamically from `FEATS.IDS` →
//! `feats.2da` `NAMEREF` → `dialog.tlk` (not hardcoded), and the list is
//! shown alphabetically like DaleKeeper2.

use eframe::egui;
use egui_components::scroll_area::ScrollArea;
use egui_components::{Checkbox, Label};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::Cre;

use crate::state::AppState;

/// Map of `FEATS.IDS` symbol → per-feat count-byte slot (see
/// [`Cre::iwd2_feat_count`]) for the 26 stackable feats. Symbols match
/// `FEATS.IDS`; the slot order matches the on-disk `0x01D8` block.
const COUNT_SLOTS: &[(&str, u8)] = &[
    ("MARTIAL_BOW", 0),
    ("SIMPLE_CROSSBOW", 1),
    ("SIMPLE_MISSILE", 2),
    ("MARTIAL_AXE", 3),
    ("SIMPLE_MACE", 4),
    ("MARTIAL_FLAIL", 5),
    ("MARTIAL_POLEARM", 6),
    ("MARTIAL_HAMMER", 7),
    ("SIMPLE_QUARTERSTAFF", 8),
    ("MARTIAL_GREATSWORD", 9),
    ("MARTIAL_LARGESWORD", 10),
    ("SIMPLE_SMALLBLADE", 11),
    ("TOUGHNESS", 12),
    ("ARMORED_ARCANA", 13),
    ("CLEAVE", 14),
    ("ARMOR_PROF", 15),
    ("SPELL_FOCUS_ENCHANTMENT", 16),
    ("SPELL_FOCUS_EVOCATION", 17),
    ("SPELL_FOCUS_NECROMANCY", 18),
    ("SPELL_FOCUS_TRANSMUTE", 19),
    ("SPELL_PENETRATION", 20),
    ("EXTRA_RAGE", 21),
    ("EXTRA_SHAPESHIFTING", 22),
    ("EXTRA_SMITING", 23),
    ("EXTRA_TURNING", 24),
    ("EXOTIC_BASTARD", 25),
];

fn count_slot_for(symbol: &str) -> Option<u8> {
    COUNT_SLOTS
        .iter()
        .find(|(s, _)| *s == symbol)
        .map(|(_, slot)| *slot)
}

/// One feat row in the table: its `FEATS.IDS` index (for the known bit),
/// resolved display name, and optional count slot (for the level).
#[derive(Clone)]
struct FeatRow {
    feat_index: u8,
    name: String,
    count_slot: Option<u8>,
}

pub struct FeatsTab;

impl FeatsTab {
    /// Needs `&mut AppState` to commit edits, and `game_data` (reached
    /// through it) to resolve feat names.
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        // A creature must be selected and be IWD2 (V2.2).
        if selected_cre(state).is_none_or(|c| c.iwd2_feat_known(0).is_none()) {
            ui.label("Feats are only available for IWD2 (CRE V2.2) creatures.");
            return;
        }

        let feats = feat_list(ui, &state.game_data);

        // Snapshot the current known/level for every feat before
        // rendering, so the render loop can mutate the CRE freely.
        let snapshot: Vec<(bool, Option<u8>)> = {
            let cre = selected_cre(state).expect("checked above");
            feats
                .iter()
                .map(|f| {
                    (
                        cre.iwd2_feat_known(f.feat_index).unwrap_or(false),
                        f.count_slot.and_then(|s| cre.iwd2_feat_count(s)),
                    )
                })
                .collect()
        };

        ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("iwd2_feats_table")
                .num_columns(3)
                .striped(true)
                .spacing([24.0, 4.0])
                .show(ui, |ui| {
                    ui.add(Label::new("Feat").strong());
                    ui.add(Label::new("Known").strong());
                    ui.add(Label::new("Level").strong());
                    ui.end_row();

                    for (feat, (known0, level0)) in feats.iter().zip(&snapshot) {
                        ui.add(Label::new(feat.name.as_str()));

                        let mut known = *known0;
                        if ui.add(Checkbox::without_label(&mut known)).changed() {
                            with_selected_cre_mut(state, |c| {
                                c.set_iwd2_feat_known(feat.feat_index, known)
                            });
                        }

                        // Level only exists for stackable feats, and is
                        // editable only once the feat is known.
                        if let Some(slot) = feat.count_slot {
                            let mut level = u32::from(level0.unwrap_or(0));
                            let resp = ui.add_enabled(
                                known,
                                egui::DragValue::new(&mut level).range(0..=u32::from(u8::MAX)),
                            );
                            if resp.changed() {
                                with_selected_cre_mut(state, |c| {
                                    c.set_iwd2_feat_count(slot, level as u8)
                                });
                            }
                        } else {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
        });
    }
}

/// Build (and frame-cache) the alphabetical feat list, resolving each
/// display name from `FEATS.IDS` → `feats.2da` `NAMEREF` → `dialog.tlk`.
/// Cached in the egui frame store because it's stable for the whole
/// session and the name resolution touches the TLK once per feat.
fn feat_list(ui: &mut egui::Ui, game_data: &GameData) -> Vec<FeatRow> {
    let id = egui::Id::new("iwd2_feat_list");
    if let Some(cached) = ui.ctx().data_mut(|d| d.get_temp::<Vec<FeatRow>>(id)) {
        return cached;
    }
    let rows = build_feat_list(game_data);
    ui.ctx().data_mut(|d| d.insert_temp(id, rows.clone()));
    rows
}

fn build_feat_list(game_data: &GameData) -> Vec<FeatRow> {
    let Ok(ids) = game_data.import_ids_by_name("feats") else {
        return Vec::new();
    };
    let two_da = game_data.import_2da_by_name("feats").ok();
    let nameref_col = two_da
        .as_ref()
        .and_then(|t| t.headers.iter().position(|h| h == "NAMEREF"));
    let tlk = game_data.dialog_tlk().ok();

    let mut rows: Vec<FeatRow> = ids
        .entries
        .iter()
        .map(|entry| {
            let symbol = entry.name.as_str();
            // FEATS.IDS value → feat bit/index; clamp into u8 range.
            let feat_index = entry.value as u8;
            let name = resolve_name(symbol, two_da.as_deref(), nameref_col, tlk.as_deref())
                .unwrap_or_else(|| prettify(symbol));
            FeatRow {
                feat_index,
                name,
                count_slot: count_slot_for(symbol),
            }
        })
        .collect();
    rows.sort_by_key(|r| r.name.to_lowercase());
    rows
}

/// Resolve a feat's display name via `feats.2da[symbol].NAMEREF` →
/// `dialog.tlk`. `None` when any step is missing or the entry is empty.
fn resolve_name(
    symbol: &str,
    two_da: Option<&infinitier_core::resource::two_da::TwoDA>,
    nameref_col: Option<usize>,
    tlk: Option<&infinitier_core::resource::tlk::Tlk>,
) -> Option<String> {
    let strref: u32 = two_da?
        .rows
        .get(symbol)?
        .get(nameref_col?)?
        .parse()
        .ok()?;
    tlk?.get(strref).filter(|s| !s.is_empty())
}

/// Fallback title-casing of a `FEATS.IDS` symbol (`AEGIS_OF_RIME` →
/// `Aegis Of Rime`) when no TLK name is available.
fn prettify(symbol: &str) -> String {
    symbol
        .split('_')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The selected party creature, if it's an embedded (parsed) CRE.
fn selected_cre(state: &AppState) -> Option<&Cre> {
    let active = state.active();
    let idx = active.selected_party_index?;
    let member = active.save.party_npcs.get(idx)?;
    match member.cre.as_ref()? {
        NpcCre::Cre(c) => Some(c.cre()),
        NpcCre::Ref(_) => None,
    }
}

/// Run `edit` against the selected party creature's mutable CRE. No-op
/// when the slot is empty or the creature isn't an embedded record.
fn with_selected_cre_mut(state: &mut AppState, edit: impl FnOnce(&mut Cre)) {
    let active = state.active_mut();
    let Some(idx) = active.selected_party_index else {
        return;
    };
    let Some(member) = active.save.party_npcs.get_mut(idx) else {
        return;
    };
    if let Some(NpcCre::Cre(c)) = member.cre.as_mut() {
        edit(c.cre_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_slots_are_unique_and_cover_the_block() {
        // 26 stackable feats, slots 0..=25 each used exactly once.
        assert_eq!(COUNT_SLOTS.len(), 26);
        let mut slots: Vec<u8> = COUNT_SLOTS.iter().map(|(_, s)| *s).collect();
        slots.sort_unstable();
        assert_eq!(slots, (0u8..26).collect::<Vec<_>>());
    }
}
