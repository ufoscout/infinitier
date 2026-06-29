//! Spells tab — every game's unified spell view.
//!
//! One Spells tab for all games, with an inner tab strip that selects the
//! spell type. The AD&D games (BG / BG2 / IWD / PST and their Enhanced
//! Editions) show **Innate / Wizard / Cleric**, each delegating to that
//! spellbook's existing read-only view. IWD2 (CRE V2.2) shows its
//! per-class categories (Bard, Cleric, Domain, … Wizard) — what used to
//! live in a dropdown — one inner tab each; [`data`] flattens the chosen
//! block into rows and [`view`] resolves names and paints the table.

mod data;
mod view;

use eframe::egui;
use egui_components::Tabs;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::Engine;
use infinitier_core::resource::cre::{Cre, Iwd2Spellbook, SpellType};

use self::data::SpellCategory;
use crate::state::AppState;

/// A spell-removal request raised by a row's "Delete" action, applied to
/// the selected creature after rendering (rendering only borrows the CRE
/// immutably).
enum SpellDelete {
    /// An AD&D known spell, identified by its spellbook type and resref.
    Adnd { spell_type: SpellType, resref: String },
    /// An IWD2 spell slot, identified by its book, level and list-2DA index.
    Iwd2 {
        book: Iwd2Spellbook,
        level: u16,
        index: u32,
    },
}

pub struct SpellsTab;

impl SpellsTab {
    /// Takes `&mut AppState` so a row's "Delete" action can remove the spell
    /// from the creature: the tables are rendered against an immutable borrow
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
                self.show_iwd2(ui, cre, char_key, &state.game_data)
            } else {
                self.show_adnd(ui, cre, char_key, &state.game_data)
            }
        };
        if let Some(req) = delete {
            apply_delete(state, req);
        }
    }

    /// AD&D spellbooks: Innate / Wizard / Cleric inner tabs, each
    /// delegating to that spellbook's own view. Each tab shows its
    /// known-spell count, like the IWD2 categories.
    fn show_adnd(
        &self,
        ui: &mut egui::Ui,
        cre: &Cre,
        char_key: u64,
        game_data: &GameData,
    ) -> Option<SpellDelete> {
        const TYPES: [&str; 3] = ["Innate", "Wizard", "Cleric"];
        let counts = [
            super::innate::InnateTab.count(cre),
            super::wizard::WizardTab.count(cre),
            super::cleric::ClericTab.count(cre),
        ];
        let mut selected = remembered_index(ui, "adnd_spell_type", char_key, 0).min(TYPES.len() - 1);
        ui.add(Tabs::new(&mut selected).tabs(tab_labels(&TYPES, &counts)).segmented());
        store_index(ui, "adnd_spell_type", char_key, selected);

        ui.add_space(8.0);
        let (spell_type, resref) = match selected {
            0 => (SpellType::Innate, super::innate::InnateTab.show(ui, cre, game_data)),
            1 => (SpellType::Wizard, super::wizard::WizardTab.show(ui, cre, game_data)),
            _ => (SpellType::Priest, super::cleric::ClericTab.show(ui, cre, game_data)),
        };
        resref.map(|resref| SpellDelete::Adnd { spell_type, resref })
    }

    /// IWD2 (V2.2) per-class spell categories — one inner tab each, with a
    /// spell count, replacing the old dropdown.
    fn show_iwd2(
        &self,
        ui: &mut egui::Ui,
        cre: &Cre,
        char_key: u64,
        game_data: &GameData,
    ) -> Option<SpellDelete> {
        // How many spells the creature has of each category — shown on each
        // tab and used to default the selection.
        let counts: Vec<usize> = SpellCategory::ALL
            .iter()
            .map(|(c, _)| data::spell_count(cre, *c))
            .collect();
        // On a freshly-shown creature, default to the first category it
        // actually has spells in, so the table isn't empty on open.
        let default_idx = counts.iter().position(|&n| n > 0).unwrap_or(0);
        let mut selected = remembered_index(ui, "iwd2_spell_category", char_key, default_idx)
            .min(SpellCategory::ALL.len() - 1);

        let names: Vec<&str> = SpellCategory::ALL.iter().map(|(_, label)| *label).collect();
        ui.add(Tabs::new(&mut selected).tabs(tab_labels(&names, &counts)).segmented());
        store_index(ui, "iwd2_spell_category", char_key, selected);

        ui.add_space(8.0);
        let category = SpellCategory::ALL[selected].0;
        let rows = data::spell_rows(cre, category);
        view::render(ui, &rows, category, game_data).map(|(level, index)| SpellDelete::Iwd2 {
            book: category.spellbook(),
            level,
            index,
        })
    }
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
/// store, re-defaulting to `default_idx` when a different character is
/// shown (so switching party members doesn't carry a stale tab).
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
