//! Read-only extraction of a creature's innate abilities for the
//! Innate tab.
//!
//! EEKeeper's Innate tab lists the creature's *known* innate spells
//! (`spell_type == 2`), one row per distinct spell, with:
//!
//! * **Level** — the known spell's level. The on-disk level field is
//!   0-based (a level-1 wizard spell stores `0`), so it is shown as
//!   `level + 1`; innates store `0` and therefore display as `1`,
//!   matching EEKeeper.
//! * **xMem** — how many copies are currently memorised, i.e. the
//!   number of daily uses. Counted from the flat `memorized_spells`
//!   list by resref (Shadow Keeper's `nTimesMemorized`).
//! * **Resource** — the SPL resref (e.g. `SPCL905`).
//!
//! The display name is resolved from the SPL's generic-name strref in
//! the view layer (it needs `GameData` + `dialog.tlk`); this module
//! stays pure so it is cheap to call every repaint and easy to test.

use infinitier_core::resource::cre::{Cre, SubSections};

/// Spell-type code for an innate ability (`0` priest, `1` wizard,
/// `2` innate — same scheme as [`KnownSpell::spell_type`]).
const SPELL_TYPE_INNATE: u16 = 2;

/// One Innate-tab row, before name resolution.
pub struct InnateRow {
    pub level: u16,
    pub x_mem: u32,
    pub resource: String,
}

/// Build the innate rows for a creature: the distinct known innate
/// spells with their memorised-copy counts. IWD2 (V2.2) stores
/// abilities in a separate block, so it yields an empty table here.
pub fn innate_rows(cre: &Cre) -> Vec<InnateRow> {
    let SubSections::V1(sub) = &cre.sub_sections else {
        return Vec::new();
    };

    let mut rows: Vec<InnateRow> = Vec::new();
    for known in &sub.known_spells {
        if known.spell_type != SPELL_TYPE_INNATE {
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
        rows.push(InnateRow {
            level: known.level.saturating_add(1),
            x_mem,
            resource: known.spell.clone(),
        });
    }
    rows
}
