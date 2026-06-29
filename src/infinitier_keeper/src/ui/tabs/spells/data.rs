//! Spell extraction for the unified Spells tab — for every game.
//!
//! The inner-tab selector is just a **filter** over the creature's spells:
//!
//! * AD&D games (BG / BG2 / IWD / PST + EE) store spells in the flat
//!   `known_spells` list tagged by [`SpellType`] (priest / wizard /
//!   innate). The Innate / Wizard / Cleric inner tabs each select one
//!   type — see [`adnd_rows`].
//! * IWD2 (CRE V2.2) stores spells in per-class, per-level blocks (plus
//!   flat blocks for innate abilities, bard songs and druid shapes),
//!   referencing each spell by a *row index* into a per-category list 2DA.
//!   The per-class inner tabs each select one [`SpellCategory`] — see
//!   [`iwd2_rows`].
//!
//! Both paths flatten down to the same [`SpellRow`] (level · xMem · resref
//! · how-to-delete); the view resolves the resref to a display name. Each
//! row carries the [`SpellDelete`] that removes it, so the view stays
//! game-agnostic.

use infinitier_core::resource::cre::{Cre, Iwd2Spellbook, Iwd2Table, SpellType, SubSections};
use infinitier_core::resource::two_da::TwoDA;

/// How to remove a displayed spell from the creature. Carried by each
/// [`SpellRow`] so the view can request a deletion without knowing the
/// game's storage model.
#[derive(Clone)]
pub enum SpellDelete {
    /// An AD&D known spell, identified by its spellbook type and resref.
    Adnd { spell_type: SpellType, resref: String },
    /// An IWD2 spell slot, identified by its book, level and list-2DA index.
    Iwd2 {
        book: Iwd2Spellbook,
        level: u16,
        index: u32,
    },
}

/// One displayed spell row, before name resolution (done in the view).
pub struct SpellRow {
    /// Spell level shown in the table (1-based).
    pub level: u16,
    /// How many copies are currently memorised ("xMem").
    pub x_mem: u32,
    /// The SPL resref — shown in the Resource column and resolved to a
    /// display name by the view.
    pub resref: String,
    /// The action that removes this spell.
    pub delete: SpellDelete,
}

/// The IWD2 spell categories selectable from the inner tabs, in display
/// order. Each maps to one IWD2 spell block (or array of per-level blocks).
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

    /// Every category with its inner-tab label, in display order.
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

/// The AD&D inner tabs (label + the [`SpellType`] they filter), in display
/// order. Innate / Wizard / Cleric are just three views of one
/// `known_spells` list.
pub const ADND_TABS: &[(&str, SpellType)] = &[
    ("Innate", SpellType::Innate),
    ("Wizard", SpellType::Wizard),
    ("Cleric", SpellType::Priest),
];

/// Build the rows for an AD&D creature's known spells of `spell_type`: one
/// row per distinct spell (duplicates collapsed, like Shadow Keeper), with
/// its memorised-copy count. Empty for a non-V1 creature (IWD2 stores
/// spells differently).
pub fn adnd_rows(cre: &Cre, spell_type: SpellType) -> Vec<SpellRow> {
    let SubSections::V1(sub) = &cre.sub_sections else {
        return Vec::new();
    };
    let mut rows: Vec<SpellRow> = Vec::new();
    for known in &sub.known_spells {
        if known.spell_type != spell_type {
            continue;
        }
        // Collapse duplicate known-spell entries — resrefs are unique per
        // spell.
        if rows.iter().any(|r| r.resref.eq_ignore_ascii_case(&known.spell)) {
            continue;
        }
        let x_mem = sub
            .memorized_spells
            .iter()
            .filter(|m| m.spell.eq_ignore_ascii_case(&known.spell))
            .count() as u32;
        rows.push(SpellRow {
            // The on-disk level is 0-based (a level-1 spell stores 0), so
            // show `level + 1`; innates store 0 and display as 1.
            level: known.level.saturating_add(1),
            x_mem,
            resref: known.spell.clone(),
            delete: SpellDelete::Adnd {
                spell_type,
                resref: known.spell.clone(),
            },
        });
    }
    rows
}

/// Number of distinct known spells of `spell_type` (the AD&D inner-tab
/// count). Zero for a non-V1 creature.
pub fn adnd_count(cre: &Cre, spell_type: SpellType) -> usize {
    adnd_rows(cre, spell_type).len()
}

/// Build the rows for `category` on an IWD2 creature, resolving each slot's
/// list-2DA row index to its SPL resref via `list` (the category's
/// [`SpellCategory::list_2da`], already imported by the caller). Empty for a
/// non-V2.2 creature.
pub fn iwd2_rows(cre: &Cre, category: SpellCategory, list: Option<&TwoDA>) -> Vec<SpellRow> {
    let SubSections::V22(sub) = &cre.sub_sections else {
        return Vec::new();
    };
    let book = category.spellbook();
    // Resolve a list-2DA row index to its resref (the row's last column).
    let resref = |index: u32| -> String {
        list.and_then(|l| l.rows.get(&index.to_string()))
            .and_then(|cells| cells.last())
            .cloned()
            .unwrap_or_default()
    };

    let mut rows = Vec::new();
    // Per-level class/domain books: block `i` holds the level-`i+1` spells.
    let push_leveled = |tables: &[Iwd2Table; 9], rows: &mut Vec<SpellRow>| {
        for (i, table) in tables.iter().enumerate() {
            let level = i as u16 + 1;
            for slot in &table.entries {
                rows.push(SpellRow {
                    level,
                    x_mem: slot.memorized,
                    resref: resref(slot.index),
                    delete: SpellDelete::Iwd2 {
                        book,
                        level,
                        index: slot.index,
                    },
                });
            }
        }
    };
    // Flat books (innate / song / shape): no per-level structure, level 1.
    let push_flat = |table: &Iwd2Table, rows: &mut Vec<SpellRow>| {
        for slot in &table.entries {
            rows.push(SpellRow {
                level: 1,
                x_mem: slot.memorized,
                resref: resref(slot.index),
                delete: SpellDelete::Iwd2 {
                    book,
                    level: 1,
                    index: slot.index,
                },
            });
        }
    };

    match category {
        SpellCategory::Bard => push_leveled(&sub.bard_spells, &mut rows),
        SpellCategory::Cleric => push_leveled(&sub.cleric_spells, &mut rows),
        SpellCategory::Domain => push_leveled(&sub.domain_spells, &mut rows),
        SpellCategory::Druid => push_leveled(&sub.druid_spells, &mut rows),
        SpellCategory::Paladin => push_leveled(&sub.paladin_spells, &mut rows),
        SpellCategory::Ranger => push_leveled(&sub.ranger_spells, &mut rows),
        SpellCategory::Sorcerer => push_leveled(&sub.sorcerer_spells, &mut rows),
        SpellCategory::Wizard => push_leveled(&sub.wizard_spells, &mut rows),
        SpellCategory::Innate => push_flat(&sub.abilities, &mut rows),
        SpellCategory::Song => push_flat(&sub.songs, &mut rows),
        SpellCategory::ShapeChange => push_flat(&sub.shapes, &mut rows),
    }
    rows
}

/// Number of spell slots the creature owns in `category` (across all nine
/// levels for the per-level class books) — the IWD2 inner-tab count. Zero
/// for a non-V2.2 creature. Cheap: only sums slot counts, no row building.
pub fn iwd2_count(cre: &Cre, category: SpellCategory) -> usize {
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
