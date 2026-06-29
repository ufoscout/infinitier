//! Read-only extraction of an IWD2 (CRE V2.2) creature's spells for the
//! Spells tab.
//!
//! IWD2 stores spells in per-class, per-level blocks (plus flat blocks
//! for innate abilities, bard songs and druid shapes) rather than the
//! AD&D `known_spells` / `memorized_spells` lists. Each slot references
//! a spell by a *row index* into a per-category list 2DA (`listspll`
//! for the class books, `listdomn` / `listinnt` / `listsong` /
//! `listshap` for the rest); that row's resref column names the SPL.
//! This module flattens the block the dropdown selects into one row per
//! spell slot:
//!
//! * **Level** — the spell level (1..=9). For the per-level class blocks
//!   it is the block index + 1; the flat innate / song / shape blocks
//!   carry no level, shown as `1` (matching DaleKeeper2).
//! * **xMem** — the slot's memorised count ([`Iwd2Slot::memorized`]).
//! * **index** — the list-2DA row index, resolved to a resref + display
//!   name in the view layer (it needs `GameData` + `dialog.tlk`); this
//!   module stays pure so it is cheap to call every repaint.

use infinitier_core::resource::cre::{Cre, Iwd2Spellbook, Iwd2Table, SubSections};

/// The spell categories selectable from the tab's dropdown, in display
/// order. Each maps to one IWD2 spell block (or array of per-level
/// blocks) on the creature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellCategory {
    Bard,
    Cleric,
    Domain,
    Druid,
    Innate,
    Paladin,
    Ranger,
    ShapeChange,
    Song,
    Sorcerer,
    Wizard,
}

impl SpellCategory {
    /// The IWD2 list 2DA whose rows the slot indices reference; the
    /// row's last column is the spell resref.
    pub fn list_2da(self) -> &'static str {
        match self {
            // The seven class spellbooks all index the shared spell list.
            SpellCategory::Bard
            | SpellCategory::Cleric
            | SpellCategory::Druid
            | SpellCategory::Paladin
            | SpellCategory::Ranger
            | SpellCategory::Sorcerer
            | SpellCategory::Wizard => "listspll",
            SpellCategory::Domain => "listdomn",
            SpellCategory::Innate => "listinnt",
            SpellCategory::Song => "listsong",
            SpellCategory::ShapeChange => "listshap",
        }
    }

    /// The core [`Iwd2Spellbook`] this category removes from.
    pub fn spellbook(self) -> Iwd2Spellbook {
        match self {
            SpellCategory::Bard => Iwd2Spellbook::Bard,
            SpellCategory::Cleric => Iwd2Spellbook::Cleric,
            SpellCategory::Domain => Iwd2Spellbook::Domain,
            SpellCategory::Druid => Iwd2Spellbook::Druid,
            SpellCategory::Innate => Iwd2Spellbook::Innate,
            SpellCategory::Paladin => Iwd2Spellbook::Paladin,
            SpellCategory::Ranger => Iwd2Spellbook::Ranger,
            SpellCategory::ShapeChange => Iwd2Spellbook::ShapeChange,
            SpellCategory::Song => Iwd2Spellbook::Song,
            SpellCategory::Sorcerer => Iwd2Spellbook::Sorcerer,
            SpellCategory::Wizard => Iwd2Spellbook::Wizard,
        }
    }

    /// Every category with its dropdown label, in display order.
    pub const ALL: &'static [(SpellCategory, &'static str)] = &[
        (SpellCategory::Bard, "Bard"),
        (SpellCategory::Cleric, "Cleric"),
        (SpellCategory::Domain, "Domain"),
        (SpellCategory::Druid, "Druid"),
        (SpellCategory::Innate, "Innate"),
        (SpellCategory::Paladin, "Paladin"),
        (SpellCategory::Ranger, "Ranger"),
        (SpellCategory::ShapeChange, "Shape Change"),
        (SpellCategory::Song, "Song"),
        (SpellCategory::Sorcerer, "Sorcerer"),
        (SpellCategory::Wizard, "Wizard"),
    ];
}

/// One Spells-tab row, before name resolution.
pub struct SpellRow {
    pub level: u16,
    pub x_mem: u32,
    /// Row index into the category's [`SpellCategory::list_2da`].
    pub index: u32,
}

/// Number of spell slots the creature owns in `category` (across all
/// nine levels for the per-level class books). Zero for a non-IWD2
/// creature. Cheap — it only sums slot counts, no row building.
pub fn spell_count(cre: &Cre, category: SpellCategory) -> usize {
    let SubSections::V22(sub) = &cre.sub_sections else {
        return 0;
    };
    let leveled = |tables: &[Iwd2Table; 9]| tables.iter().map(|t| t.entries.len()).sum();
    match category {
        SpellCategory::Bard => leveled(&sub.bard_spells),
        SpellCategory::Cleric => leveled(&sub.cleric_spells),
        SpellCategory::Domain => leveled(&sub.domain_spells),
        SpellCategory::Druid => leveled(&sub.druid_spells),
        SpellCategory::Paladin => leveled(&sub.paladin_spells),
        SpellCategory::Ranger => leveled(&sub.ranger_spells),
        SpellCategory::Sorcerer => leveled(&sub.sorcerer_spells),
        SpellCategory::Wizard => leveled(&sub.wizard_spells),
        SpellCategory::Innate => sub.abilities.entries.len(),
        SpellCategory::Song => sub.songs.entries.len(),
        SpellCategory::ShapeChange => sub.shapes.entries.len(),
    }
}

/// Build the rows for `category` on `cre`. Yields an empty table for a
/// non-IWD2 (non-V2.2) creature, which stores spells differently.
pub fn spell_rows(cre: &Cre, category: SpellCategory) -> Vec<SpellRow> {
    let SubSections::V22(sub) = &cre.sub_sections else {
        return Vec::new();
    };
    match category {
        SpellCategory::Bard => leveled(&sub.bard_spells),
        SpellCategory::Cleric => leveled(&sub.cleric_spells),
        SpellCategory::Domain => leveled(&sub.domain_spells),
        SpellCategory::Druid => leveled(&sub.druid_spells),
        SpellCategory::Paladin => leveled(&sub.paladin_spells),
        SpellCategory::Ranger => leveled(&sub.ranger_spells),
        SpellCategory::Sorcerer => leveled(&sub.sorcerer_spells),
        SpellCategory::Wizard => leveled(&sub.wizard_spells),
        SpellCategory::Innate => flat(&sub.abilities),
        SpellCategory::Song => flat(&sub.songs),
        SpellCategory::ShapeChange => flat(&sub.shapes),
    }
}

/// Flatten nine per-level blocks: block index `i` holds the level-`i+1`
/// spells.
fn leveled(tables: &[Iwd2Table; 9]) -> Vec<SpellRow> {
    let mut rows = Vec::new();
    for (i, table) in tables.iter().enumerate() {
        for slot in &table.entries {
            rows.push(SpellRow {
                level: i as u16 + 1,
                x_mem: slot.memorized,
                index: slot.index,
            });
        }
    }
    rows
}

/// Flatten a single block (innate / song / shape) — no per-level
/// structure, so every row is shown at level 1.
fn flat(table: &Iwd2Table) -> Vec<SpellRow> {
    table
        .entries
        .iter()
        .map(|slot| SpellRow {
            level: 1,
            x_mem: slot.memorized,
            index: slot.index,
        })
        .collect()
}
