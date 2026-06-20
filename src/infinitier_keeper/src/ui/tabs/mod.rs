//! Per-character editor tabs.
//!
//! Mirrors EEKeeper's tab strip across the top of the character
//! editor. Each tab is its own module / unit-struct with a `show`
//! method taking the parsed [`Cre`] plus the active [`Game`] — only
//! [`AbilitiesTab`] has real content for now; the rest are stubs we
//! will flesh out in follow-up work.

use eframe::egui;
use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::cre::ImportedCre;
use infinitier_core::imported_resource::gam::{ImportedGam, ImportedGamNpc, NpcCre};
use infinitier_core::resource::Engine;
use infinitier_core::resource::{Game, cre::Cre};

use crate::components::editable_fields::KeeperEditors;
use crate::state::AppState;

mod abilities;
mod appearance;
mod characteristics;
mod cleric;
mod effects;
mod feats;
mod global_variables;
mod innate;
mod inventory;
mod journal_entries;
mod levels;
mod local_variables;
mod memorization;
mod miscellaneous;
mod proficiencies;
mod resistances;
mod spells;
mod wizard;

use abilities::AbilitiesTab;
use appearance::AppearanceTab;
use characteristics::CharacteristicsTab;
use cleric::ClericTab;
use effects::EffectsTab;
use feats::FeatsTab;
use global_variables::GlobalVariablesTab;
use innate::InnateTab;
use inventory::InventoryTab;
use journal_entries::JournalEntriesTab;
use levels::LevelsTab;
use local_variables::LocalVariablesTab;
use memorization::MemorizationTab;
use miscellaneous::MiscellaneousTab;
use proficiencies::ProficienciesTab;
use resistances::ResistancesTab;
use spells::SpellsTab;
use wizard::WizardTab;

/// Identifier for the active per-character tab. Order matches the
/// EEKeeper tab strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterTab {
    Abilities,
    Levels,
    Characteristics,
    Appearance,
    Inventory,
    Spells,
    Feats,
    Memorization,
    Innate,
    Wizard,
    Cleric,
    Proficiencies,
    Resistances,
    Effects,
    LocalVariables,
    GlobalVariables,
    JournalEntries,
    Miscellaneous,
}

impl CharacterTab {
    /// Tabs in EEKeeper display order.
    pub const ALL: &'static [CharacterTab] = &[
        CharacterTab::Abilities,
        CharacterTab::Levels,
        CharacterTab::Characteristics,
        CharacterTab::Appearance,
        CharacterTab::Inventory,
        CharacterTab::Spells,
        CharacterTab::Feats,
        CharacterTab::Memorization,
        CharacterTab::Innate,
        CharacterTab::Wizard,
        CharacterTab::Cleric,
        CharacterTab::Proficiencies,
        CharacterTab::Resistances,
        CharacterTab::Effects,
        CharacterTab::LocalVariables,
        CharacterTab::GlobalVariables,
        CharacterTab::JournalEntries,
        CharacterTab::Miscellaneous,
    ];

    /// The human-readable label rendered on the tab strip.
    /// True when this tab should appear for the given game. Most tabs are
    /// universal; `Levels` is IWD2-only (CRE V2.2 per-class level editor).
    pub fn is_visible_for_game(&self, game: Game) -> bool {
        let is_iwd2 = game.engine() == Engine::Iwd2;
        match self {
            // IWD2-only: per-class levels/kits and the unified spell list.
            CharacterTab::Levels | CharacterTab::Spells | CharacterTab::Feats => is_iwd2,
            // IWD2 folds these into the single Spells tab, so hide the
            // AD&D-style per-spellbook tabs there (they'd be empty
            // anyway — IWD2 uses the V2.2 per-class spell blocks).
            CharacterTab::Memorization
            | CharacterTab::Innate
            | CharacterTab::Proficiencies
            | CharacterTab::Wizard
            | CharacterTab::Cleric => !is_iwd2,
            _ => true,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            CharacterTab::Abilities => "Abilities",
            CharacterTab::Levels => "Levels & Kits",
            CharacterTab::Characteristics => "Characteristics",
            CharacterTab::Appearance => "Appearance",
            CharacterTab::Inventory => "Inventory",
            CharacterTab::Spells => "Spells",
            CharacterTab::Feats => "Feats",
            CharacterTab::Memorization => "Memorization",
            CharacterTab::Innate => "Innate",
            CharacterTab::Wizard => "Wizard",
            CharacterTab::Cleric => "Cleric",
            CharacterTab::Proficiencies => "Proficiencies",
            CharacterTab::Resistances => "Resistances",
            CharacterTab::Effects => "Effects",
            CharacterTab::LocalVariables => "Local Variables",
            CharacterTab::GlobalVariables => "Global Variables",
            CharacterTab::JournalEntries => "Journal Entries",
            CharacterTab::Miscellaneous => "Miscellaneous",
        }
    }
}

/// The immutable references a read-only tab can need, gathered once by
/// [`with_cre`] for the selected party creature. Tabs take whichever
/// subset they use.
struct CreCtx<'a> {
    /// The parsed embedded CRE wrapper (Characteristics / Inventory).
    imported: &'a ImportedCre,
    /// The wrapped CRE — what most read-only tabs read from.
    cre: &'a Cre,
    /// The save's GAM (party-wide tabs, variables, journal, …).
    gam: &'a ImportedGam,
    /// The selected party slot's GAM record (kill stats, name, …).
    member: &'a ImportedGamNpc,
    /// The game variant.
    game: Game,
    /// Resource access for tabs that resolve names / 2DAs / TLK.
    game_data: &'a GameData,
    /// Engine ability/skill ranges (Proficiencies).
    engine_caps: &'a EngineCaps,
    /// `(save-tab, party-slot)` key for the Spells tab's per-character
    /// dropdown memory.
    char_key: u64,
}

/// Resolve the selected party creature and run `f` with its immutable
/// [`CreCtx`]. No-op (the tab simply renders nothing) when the slot is
/// empty or doesn't hold an embedded CRE — this is the single guard
/// every read-only tab shares.
fn with_cre(state: &AppState, f: impl FnOnce(CreCtx)) {
    let active = state.active();
    let Some(idx) = active.selected_party_index else {
        return;
    };
    let Some(member) = active.save.party_npcs.get(idx) else {
        return;
    };
    let Some(NpcCre::Cre(imported)) = member.cre.as_ref() else {
        return;
    };
    f(CreCtx {
        imported,
        cre: imported.cre(),
        gam: &active.save,
        member,
        game: state.game_data.game(),
        game_data: &state.game_data,
        engine_caps: &state.engine_caps,
        char_key: ((state.active_tab as u64) << 32) | idx as u64,
    });
}

/// Render the content area of the active tab. One arm per tab: the
/// editable tabs (Abilities / Levels / Feats) take `&mut AppState`
/// directly; every read-only tab borrows the creature immutably through
/// [`with_cre`], which carries the shared "no creature selected" guard.
pub fn show_tab(ui: &mut egui::Ui, state: &mut AppState, editors: &mut KeeperEditors) {
    // Read the tab out as an owned (Copy) value so no borrow of `state`
    // is held across the match — editable arms need `&mut state`.
    match state.active().selected_tab {
        CharacterTab::Abilities => AbilitiesTab.show(ui, state, editors),
        CharacterTab::Levels => LevelsTab.show(ui, state, editors),
        CharacterTab::Feats => FeatsTab.show(ui, state),
        CharacterTab::Characteristics => with_cre(state, |c| {
            CharacteristicsTab.show(ui, c.imported, c.member, c.game_data)
        }),
        CharacterTab::Appearance => with_cre(state, |c| AppearanceTab.show(ui, c.cre, c.game_data)),
        CharacterTab::Inventory => {
            with_cre(state, |c| InventoryTab.show(ui, c.imported, c.game_data))
        }
        CharacterTab::Spells => with_cre(state, |c| {
            SpellsTab.show(ui, c.cre, c.char_key, c.game_data)
        }),
        CharacterTab::Memorization => {
            with_cre(state, |c| MemorizationTab.show(ui, c.cre, c.gam, c.game))
        }
        CharacterTab::Innate => with_cre(state, |c| InnateTab.show(ui, c.cre, c.game_data)),
        CharacterTab::Wizard => with_cre(state, |c| WizardTab.show(ui, c.cre, c.game_data)),
        CharacterTab::Cleric => with_cre(state, |c| ClericTab.show(ui, c.cre, c.game_data)),
        CharacterTab::Proficiencies => {
            with_cre(state, |c| ProficienciesTab.show(ui, c.cre, c.engine_caps))
        }
        CharacterTab::Resistances => {
            with_cre(state, |c| ResistancesTab.show(ui, c.cre, c.gam, c.game))
        }
        CharacterTab::Effects => with_cre(state, |c| EffectsTab.show(ui, c.cre, c.game_data)),
        CharacterTab::LocalVariables => {
            with_cre(state, |c| LocalVariablesTab.show(ui, c.cre, c.gam, c.game))
        }
        CharacterTab::GlobalVariables => {
            with_cre(state, |c| GlobalVariablesTab.show(ui, c.cre, c.gam, c.game))
        }
        CharacterTab::JournalEntries => {
            with_cre(state, |c| JournalEntriesTab.show(ui, c.gam, c.game_data))
        }
        CharacterTab::Miscellaneous => {
            with_cre(state, |c| MiscellaneousTab.show(ui, c.cre, c.gam, c.member))
        }
    }
}
