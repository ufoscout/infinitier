//! Per-character editor tabs. Same set/order as the egui keeper; only
//! Abilities has real (read-only) content, the rest are stubs.

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
