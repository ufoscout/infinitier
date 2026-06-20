//! Per-character editor tabs.
//!
//! Mirrors EEKeeper's tab strip across the top of the character
//! editor. Each tab is its own module / unit-struct with a `show`
//! method taking the parsed [`Cre`] plus the active [`Game`] — only
//! [`AbilitiesTab`] has real content for now; the rest are stubs we
//! will flesh out in follow-up work.

use eframe::egui;
use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
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

/// Render the content area of the active tab. The Abilities tab
/// needs mutable access to commit edits, so it's dispatched
/// directly with `&mut AppState + &mut KeeperEditors`. Every other
/// tab is still read-only — we pull the immutable triple
/// `(cre, gam, game)` out of `state` once and forward it to the
/// stub.
pub fn show_tab(ui: &mut egui::Ui, state: &mut AppState, editors: &mut KeeperEditors) {
    let active = state.active();
    if active.selected_tab == CharacterTab::Abilities {
        AbilitiesTab.show(ui, state, editors);
        return;
    }
    if active.selected_tab == CharacterTab::Levels {
        LevelsTab.show(ui, state, editors);
        return;
    }
    // Feats edits the CRE in place, so it needs `&mut AppState`.
    if active.selected_tab == CharacterTab::Feats {
        FeatsTab.show(ui, state);
        return;
    }
    let Some(idx) = active.selected_party_index else {
        return;
    };
    let Some(member) = active.save.party_npcs.get(idx) else {
        return;
    };
    let Some(NpcCre::Cre(imported)) = member.cre.as_ref() else {
        return;
    };
    let cre: &Cre = imported.cre();
    let gam: &ImportedGam = &active.save;
    let game: Game = state.game_data.game();
    match active.selected_tab {
        CharacterTab::Abilities | CharacterTab::Levels | CharacterTab::Feats => unreachable!(),
        // Characteristics resolves IDS-backed names (class / race /
        // kit / …) and the strongest-kill strref, so it needs
        // `game_data`; the kill statistics live in the GAM party
        // slot's raw struct, so it also takes `member`.
        CharacterTab::Characteristics => {
            CharacteristicsTab.show(ui, imported, member, &state.game_data)
        }
        // Appearance resolves the animation id via ANIMATE.IDS and the
        // colour-gradient swatches via the palette BMP, so it needs
        // `game_data`.
        CharacterTab::Appearance => AppearanceTab.show(ui, cre, &state.game_data),
        // Inventory resolves each item slot's ITM (for the name + icon
        // BAM) and `dialog.tlk`, so it needs `game_data`.
        CharacterTab::Inventory => InventoryTab.show(ui, imported, &state.game_data),
        // IWD2-only single spell list with a category dropdown; resolves
        // spell names via their SPL files and `dialog.tlk`. The
        // (save-tab, party-slot) pair keys the per-character dropdown
        // memory so switching creatures re-defaults the selection.
        CharacterTab::Spells => {
            let char_key = ((state.active_tab as u64) << 32) | idx as u64;
            SpellsTab.show(ui, cre, char_key, &state.game_data)
        }
        CharacterTab::Memorization => MemorizationTab.show(ui, cre, gam, game),
        CharacterTab::Innate => InnateTab.show(ui, cre, &state.game_data),
        CharacterTab::Wizard => WizardTab.show(ui, cre, &state.game_data),
        CharacterTab::Cleric => ClericTab.show(ui, cre, &state.game_data),
        CharacterTab::Proficiencies => ProficienciesTab.show(ui, cre, &state.engine_caps),
        CharacterTab::Resistances => ResistancesTab.show(ui, cre, gam, game),
        // Effects resolves resource resrefs to spell names via their
        // SPL files and `dialog.tlk`, so it needs `game_data`.
        CharacterTab::Effects => EffectsTab.show(ui, cre, &state.game_data),
        CharacterTab::LocalVariables => LocalVariablesTab.show(ui, cre, gam, game),
        CharacterTab::GlobalVariables => GlobalVariablesTab.show(ui, cre, gam, game),
        // The journal belongs to the savegame, not a character, and
        // resolves each entry's text via `dialog.tlk`.
        CharacterTab::JournalEntries => JournalEntriesTab.show(ui, gam, &state.game_data),
        CharacterTab::Miscellaneous => MiscellaneousTab.show(ui, cre, gam, member),
    }
}
