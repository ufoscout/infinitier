//! Per-character-tab modules. One Rust file per tab — same shape
//! the egui keeper uses. Only `abilities.rs` is non-stub; the
//! others just write a `body-message` placeholder.

use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::cre::Cre;

use crate::MainWindow;

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

/// Identifier for the active per-character tab. Order matches the
/// `TabLabel` strip the user sees, and mirrors the egui keeper's
/// `CharacterTab::ALL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Set the abilities-grid `ModelRc`s back to empty. Called whenever
/// the active tab isn't Abilities, or the selected slot has no
/// parsed CRE — keeps stale rows from flashing through during a
/// re-selection.
pub fn clear_abilities(window: &MainWindow) {
    let empty = || super::key_value_model(Vec::new());
    window.set_ability_scores(empty());
    window.set_combat_stats(empty());
    window.set_experience_levels(empty());
    window.set_morale_rows(empty());
    window.set_skill_rows(empty());
    window.set_skills_title("Skills".into());
}

/// Drive the body content for the active tab.
pub fn dispatch(window: &MainWindow, tab: CharacterTab, cre: &Cre, gam: &ImportedGam) {
    // Every non-Abilities tab uses the StubTab body; only the
    // Abilities branch needs the parsed CRE + ImportedGam. We still
    // call the stub `populate(window)` per tab so each module can
    // grow into a real implementation without touching this file.
    match tab {
        CharacterTab::Abilities => {
            window.set_body_message("".into());
            abilities::populate(window, cre, gam);
        }
        CharacterTab::Characteristics => {
            clear_abilities(window);
            characteristics::populate(window);
        }
        CharacterTab::Appearance => {
            clear_abilities(window);
            appearance::populate(window);
        }
        CharacterTab::Inventory => {
            clear_abilities(window);
            inventory::populate(window);
        }
        CharacterTab::Memorization => {
            clear_abilities(window);
            memorization::populate(window);
        }
        CharacterTab::Innate => {
            clear_abilities(window);
            innate::populate(window);
        }
        CharacterTab::Wizard => {
            clear_abilities(window);
            wizard::populate(window);
        }
        CharacterTab::Cleric => {
            clear_abilities(window);
            cleric::populate(window);
        }
        CharacterTab::Proficiencies => {
            clear_abilities(window);
            proficiencies::populate(window);
        }
        CharacterTab::Resistances => {
            clear_abilities(window);
            resistances::populate(window);
        }
        CharacterTab::Effects => {
            clear_abilities(window);
            effects::populate(window);
        }
        CharacterTab::LocalVariables => {
            clear_abilities(window);
            local_variables::populate(window);
        }
        CharacterTab::GlobalVariables => {
            clear_abilities(window);
            global_variables::populate(window);
        }
        CharacterTab::JournalEntries => {
            clear_abilities(window);
            journal_entries::populate(window);
        }
        CharacterTab::Miscellaneous => {
            clear_abilities(window);
            miscellaneous::populate(window);
        }
    }
}
