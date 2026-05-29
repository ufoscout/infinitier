//! Per-tab modules + the `CharacterTab` enum that drives the tab
//! strip. The 15 variants mirror the Slint spike one-for-one;
//! `Abilities` is the only fully-implemented tab — everything else is
//! a "not implemented yet" stub.

use gpui::{AnyElement, Context, IntoElement};
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::cre::Cre;

use crate::app::KeeperApp;

pub mod abilities;
pub mod stub;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

    pub fn label(self) -> &'static str {
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

/// Route to the right tab body. `Abilities` gets the full Slint-port
/// rendering; every other variant falls through to `stub::render`
/// with a "not implemented yet" message.
pub fn dispatch(
    this: &KeeperApp,
    tab: CharacterTab,
    cre: &Cre,
    gam: &ImportedGam,
    cx: &mut Context<KeeperApp>,
) -> AnyElement {
    match tab {
        CharacterTab::Abilities => abilities::render(this, cre, gam, cx).into_any_element(),
        other => {
            stub::render(format!("{} — not implemented yet.", other.label())).into_any_element()
        }
    }
}
