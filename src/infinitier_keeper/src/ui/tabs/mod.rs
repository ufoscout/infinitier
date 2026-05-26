//! Per-character editor tabs.
//!
//! Mirrors EEKeeper's tab strip across the top of the character
//! editor. Each tab is its own module / unit-struct with a `show`
//! method taking the parsed [`Cre`] plus the active [`Game`] — only
//! [`AbilitiesTab`] has real content for now; the rest are stubs we
//! will flesh out in follow-up work.

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::{Game, cre::Cre};

mod abilities;
mod appearance;
mod characteristics;
mod cleric;
mod effects;
mod global_variables;
mod innate;
mod inventory;
mod journal_entries;
mod local_variables;
mod memorization;
mod miscellaneous;
mod proficiencies;
mod resistances;
mod wizard;

use abilities::AbilitiesTab;
use appearance::AppearanceTab;
use characteristics::CharacteristicsTab;
use cleric::ClericTab;
use effects::EffectsTab;
use global_variables::GlobalVariablesTab;
use innate::InnateTab;
use inventory::InventoryTab;
use journal_entries::JournalEntriesTab;
use local_variables::LocalVariablesTab;
use memorization::MemorizationTab;
use miscellaneous::MiscellaneousTab;
use proficiencies::ProficienciesTab;
use resistances::ResistancesTab;
use wizard::WizardTab;

/// Identifier for the active per-character tab. Order matches the
/// EEKeeper tab strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterTab {
    Abilities,
    Characteristics,
    Appearance,
    Inventory,
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
        CharacterTab::Characteristics,
        CharacterTab::Appearance,
        CharacterTab::Inventory,
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
    pub fn label(&self) -> &'static str {
        match self {
            CharacterTab::Abilities => "Abilities",
            CharacterTab::Characteristics => "Characteristics",
            CharacterTab::Appearance => "Appearance",
            CharacterTab::Inventory => "Inventory",
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

/// Render the content area of the active tab. The tab strip itself
/// is owned by the parent panel — this just dispatches.
pub fn show_tab(ui: &mut egui::Ui, tab: CharacterTab, cre: &Cre, gam: &ImportedGam, game: Game) {
    match tab {
        CharacterTab::Abilities => AbilitiesTab.show(ui, cre, gam, game),
        CharacterTab::Characteristics => CharacteristicsTab.show(ui, cre, gam, game),
        CharacterTab::Appearance => AppearanceTab.show(ui, cre, gam, game),
        CharacterTab::Inventory => InventoryTab.show(ui, cre, gam, game),
        CharacterTab::Memorization => MemorizationTab.show(ui, cre, gam, game),
        CharacterTab::Innate => InnateTab.show(ui, cre, gam, game),
        CharacterTab::Wizard => WizardTab.show(ui, cre, gam, game),
        CharacterTab::Cleric => ClericTab.show(ui, cre, gam, game),
        CharacterTab::Proficiencies => ProficienciesTab.show(ui, cre, gam, game),
        CharacterTab::Resistances => ResistancesTab.show(ui, cre, gam, game),
        CharacterTab::Effects => EffectsTab.show(ui, cre, gam, game),
        CharacterTab::LocalVariables => LocalVariablesTab.show(ui, cre, gam, game),
        CharacterTab::GlobalVariables => GlobalVariablesTab.show(ui, cre, gam, game),
        CharacterTab::JournalEntries => JournalEntriesTab.show(ui, cre, gam, game),
        CharacterTab::Miscellaneous => MiscellaneousTab.show(ui, cre, gam, game),
    }
}
