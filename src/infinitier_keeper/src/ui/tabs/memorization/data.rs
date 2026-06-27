//! Extraction of spell-memorisation slots for the Memorization tab.
//!
//! Each [`SpellMemorizationInfo`] record in the CRE is one row: a
//! spell type (Cleric / Wizard / Innate), a spell level, and the
//! number of spells memorisable at that slot (`num_memorizable_total`
//! — EEKeeper's "Max Can Memorise" column). Records are listed in
//! file order, which for BG/BG2/EE saves is the fixed sequence
//! EEKeeper shows: Cleric levels 1..=7, Wizard levels 1..=9, then a
//! single Innate level-1 slot.
//!
//! The on-disk `level` is 0-based (Shadow Keeper displays `level + 1`),
//! so it is incremented here to match the 1-based levels in the UI.

use infinitier_core::resource::cre::{Cre, SpellType, SubSections};

/// One table row: a spell slot's type, level and memorisation cap.
pub struct MemRow {
    pub type_name: &'static str,
    pub level: u16,
    pub max: u16,
}

/// Build the table rows for a creature, one per spell-memorisation
/// slot. The IWD2 (V2.2) layout stores per-class spell tables instead
/// of `spell_memorization_info`, so it yields an empty table.
pub fn memorization_rows(cre: &Cre) -> Vec<MemRow> {
    let SubSections::V1(sub) = &cre.sub_sections else {
        return Vec::new();
    };
    sub.spell_memorization_info
        .iter()
        .map(|info| MemRow {
            type_name: spell_type_name(info.spell_type),
            level: info.level.saturating_add(1),
            max: info.num_memorizable_total,
        })
        .collect()
}

/// Map a spell type to EEKeeper's column label. [`SpellType::Priest`]
/// is shown as "Cleric" to match EEKeeper's UI.
fn spell_type_name(spell_type: SpellType) -> &'static str {
    match spell_type {
        SpellType::Priest => "Cleric",
        SpellType::Wizard => "Wizard",
        SpellType::Innate => "Innate",
        SpellType::Unknown(_) => "Unknown",
    }
}
