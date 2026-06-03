//! Read-only extraction of a creature's arcane spellbook for the
//! Wizard tab.
//!
//! EEKeeper's Wizard tab lists the creature's *known* wizard spells
//! (`spell_type == 1`), one row per distinct spell, with:
//!
//! * **Level** — the spell level (1..=9). The on-disk level field is
//!   0-based (a level-1 spell stores `0`), so it is shown as
//!   `level + 1`.
//! * **xMem** — how many copies are currently memorised. Counted from
//!   the flat `memorized_spells` list by resref (Shadow Keeper's
//!   `nTimesMemorized`).
//! * **Resource** — the SPL resref (e.g. `SPWI112`).
//!
//! The display name is resolved from the SPL's generic-name strref in
//! the view layer (it needs `GameData` + `dialog.tlk`); this module
//! stays pure so it is cheap to call every repaint and easy to test.

use infinitier_core::resource::cre::{Cre, SubSections};

/// Spell-type code for an arcane (wizard) spell (`0` priest, `1`
/// wizard, `2` innate — same scheme as [`KnownSpell::spell_type`]).
const SPELL_TYPE_WIZARD: u16 = 1;

/// One Wizard-tab row, before name resolution.
pub struct WizardRow {
    pub level: u16,
    pub x_mem: u32,
    pub resource: String,
}

/// Build the wizard rows for a creature: the distinct known arcane
/// spells with their memorised-copy counts. IWD2 (V2.2) stores spells
/// in per-class blocks, so it yields an empty table here.
pub fn wizard_rows(cre: &Cre) -> Vec<WizardRow> {
    let SubSections::V1(sub) = &cre.sub_sections else {
        return Vec::new();
    };

    let mut rows: Vec<WizardRow> = Vec::new();
    for known in &sub.known_spells {
        if known.spell_type != SPELL_TYPE_WIZARD {
            continue;
        }
        // Collapse duplicate known-spell entries (Shadow Keeper does
        // the same) — resrefs are unique per spell.
        if rows
            .iter()
            .any(|r| r.resource.eq_ignore_ascii_case(&known.spell))
        {
            continue;
        }
        let x_mem = sub
            .memorized_spells
            .iter()
            .filter(|m| m.spell.eq_ignore_ascii_case(&known.spell))
            .count() as u32;
        rows.push(WizardRow {
            level: known.level.saturating_add(1),
            x_mem,
            resource: known.spell.clone(),
        });
    }
    rows
}
