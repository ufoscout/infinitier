//! Spells tab — every game's unified spell view.
//!
//! One Spells tab and module for all games. An inner tab strip selects
//! which spells to show, acting purely as a **filter**:
//!
//! * AD&D (BG / BG2 / IWD / PST + EE): Innate / Wizard / Cleric — the three
//!   [`SpellType`]s of the flat `known_spells` list.
//! * IWD2 (CRE V2.2): the per-class categories (Bard, Cleric, Domain, …
//!   Wizard) of its per-class/level spell blocks.
//!
//! Either way [`data`] flattens the selected filter into uniform rows and
//! [`view`] resolves names and paints one table. A row's "Delete" action
//! removes the spell from the creature.

mod data;
mod view;

use eframe::egui;
use egui_components::Tabs;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::Engine;
use infinitier_core::resource::cre::Cre;

use self::data::{SpellCategory, SpellDelete};
use crate::state::AppState;

pub struct SpellsTab;

impl SpellsTab {
    /// Takes `&mut AppState` so a row's "Delete" action can remove the spell
    /// from the creature: the table is rendered against an immutable borrow
    /// that yields an optional [`SpellDelete`], which is then applied through
    /// a fresh mutable borrow.
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        let game = state.game_data.game();
        let delete = {
            let Some(char_key) = char_key(state) else {
                return;
            };
            let Some(cre) = selected_cre(state) else {
                return;
            };
            if game.engine() == Engine::Iwd2 {
                show_iwd2(ui, cre, char_key, &state.game_data)
            } else {
                show_adnd(ui, cre, char_key, &state.game_data)
            }
        };
        if let Some(req) = delete {
            apply_delete(state, req);
        }
    }
}

/// AD&D inner tabs: Innate / Wizard / Cleric — each filters `known_spells`
/// by [`SpellType`]. Each tab shows its known-spell count.
fn show_adnd(
    ui: &mut egui::Ui,
    cre: &Cre,
    char_key: u64,
    game_data: &GameData,
) -> Option<SpellDelete> {
    let counts: Vec<usize> = data::ADND_TABS
        .iter()
        .map(|(_, ty)| data::adnd_count(cre, *ty))
        .collect();
    let names: Vec<&str> = data::ADND_TABS.iter().map(|(label, _)| *label).collect();

    let mut selected =
        remembered_index(ui, "adnd_spell_type", char_key, 0).min(data::ADND_TABS.len() - 1);
    ui.add(Tabs::new(&mut selected).tabs(tab_labels(&names, &counts)).segmented());
    store_index(ui, "adnd_spell_type", char_key, selected);

    ui.add_space(8.0);
    let spell_type = data::ADND_TABS[selected].1;
    view::render(ui, data::adnd_rows(cre, spell_type), game_data)
}

/// IWD2 inner tabs: the per-class spell categories — each filters that
/// book's per-level slots. Each tab shows its slot count.
fn show_iwd2(
    ui: &mut egui::Ui,
    cre: &Cre,
    char_key: u64,
    game_data: &GameData,
) -> Option<SpellDelete> {
    // How many spells the creature has of each category — shown on each
    // tab and used to default the selection.
    let counts: Vec<usize> = SpellCategory::ALL
        .iter()
        .map(|(c, _)| data::iwd2_count(cre, *c))
        .collect();
    // On a freshly-shown creature, default to the first category it actually
    // has spells in, so the table isn't empty on open.
    let default_idx = counts.iter().position(|&n| n > 0).unwrap_or(0);
    let mut selected = remembered_index(ui, "iwd2_spell_category", char_key, default_idx)
        .min(SpellCategory::ALL.len() - 1);

    let names: Vec<&str> = SpellCategory::ALL.iter().map(|(_, label)| *label).collect();
    ui.add(Tabs::new(&mut selected).tabs(tab_labels(&names, &counts)).segmented());
    store_index(ui, "iwd2_spell_category", char_key, selected);

    ui.add_space(8.0);
    let category = SpellCategory::ALL[selected].0;
    // Resolve the category's list 2DA once, so `data` can turn slot indices
    // into resrefs.
    let list = game_data.import_2da_by_name(category.list_2da()).ok();
    view::render(ui, data::iwd2_rows(cre, category, list.as_deref()), game_data)
}

/// The selected party creature, if it's an embedded (parsed) CRE.
fn selected_cre(state: &AppState) -> Option<&Cre> {
    let active = state.active();
    let member = active.save.party_npcs.get(active.selected_party_index?)?;
    match member.cre.as_ref()? {
        NpcCre::Cre(c) => Some(c.cre()),
        NpcCre::Ref(_) => None,
    }
}

/// `(save-tab, party-slot)` key for the per-character inner-tab memory.
fn char_key(state: &AppState) -> Option<u64> {
    let idx = state.active().selected_party_index?;
    Some(((state.active_tab as u64) << 32) | idx as u64)
}

/// Apply a delete request to the selected creature's mutable CRE.
fn apply_delete(state: &mut AppState, req: SpellDelete) {
    let active = state.active_mut();
    let Some(idx) = active.selected_party_index else {
        return;
    };
    if let Some(member) = active.save.party_npcs.get_mut(idx)
        && let Some(NpcCre::Cre(c)) = member.cre.as_mut()
    {
        let cre = c.cre_mut();
        match req {
            SpellDelete::Adnd { spell_type, resref } => {
                cre.remove_known_spell(spell_type, &resref);
            }
            SpellDelete::Iwd2 { book, level, index } => {
                cre.remove_iwd2_spell(book, level as usize, index);
            }
        }
    }
}

/// Build inner-tab labels, appending the spell count when non-zero
/// (`"Wizard (3)"`) — bare name otherwise. `names` and `counts` are
/// parallel, in tab order.
fn tab_labels(names: &[&str], counts: &[usize]) -> Vec<String> {
    names
        .iter()
        .zip(counts)
        .map(|(name, &n)| {
            if n > 0 {
                format!("{name} ({n})")
            } else {
                name.to_string()
            }
        })
        .collect()
}

/// Read the inner-tab index remembered for `char_key` in the egui frame
/// store, re-defaulting to `default_idx` when a different character is shown
/// (so switching party members doesn't carry a stale tab).
fn remembered_index(ui: &egui::Ui, key: &'static str, char_key: u64, default_idx: usize) -> usize {
    let anchor = ui
        .ctx()
        .data_mut(|d| d.get_temp::<u64>(egui::Id::new((key, "anchor"))));
    if anchor == Some(char_key) {
        ui.ctx()
            .data_mut(|d| d.get_temp::<usize>(egui::Id::new(key)))
            .unwrap_or(default_idx)
    } else {
        default_idx
    }
}

/// Persist the selected inner-tab index for `char_key`.
fn store_index(ui: &egui::Ui, key: &'static str, char_key: u64, idx: usize) {
    ui.ctx().data_mut(|d| {
        d.insert_temp(egui::Id::new(key), idx);
        d.insert_temp(egui::Id::new((key, "anchor")), char_key);
    });
}
