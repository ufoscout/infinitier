//! [`ImportedSpl`] — a higher-level wrapper around a [`Spl`].
//!
//! Like [`ImportedCre`](super::cre::ImportedCre), the [`Spl`] resource stays a
//! pure parser/serialiser; anything that needs *other* game resources to
//! interpret a spell — here, the IWD2 spell-list 2DAs that say which
//! spellbooks a spell belongs to — lives on this wrapper, taking the
//! [`GameData`] to resolve against. Stored in
//! [`ImportedResource::Spl`](super::ImportedResource::Spl); derefs to the
//! wrapped [`Spl`] so callers reach `.header` directly.

use std::ops::Deref;

use infinitier_spl_resource::Spl;

use crate::game::GameData;
use crate::resource::cre::Iwd2Spellbook;

/// One way an IWD2 spell can be added to a creature: which spellbook, at what
/// (1-based) level, storing which list-2DA row index. A single spell yields
/// several placements when it belongs to more than one book (e.g. Bless →
/// Cleric, Paladin and a Domain), exactly like DaleKeeper2's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Iwd2Placement {
    pub book: Iwd2Spellbook,
    pub level: u16,
    pub index: u32,
}

/// An owned [`Spl`] plus the higher-level queries that resolve a spell against
/// other resources (the spell-list 2DAs), each taking the [`GameData`] to
/// resolve against.
#[derive(Debug, Clone)]
pub struct ImportedSpl {
    spl: Box<Spl>,
}

impl ImportedSpl {
    /// Wrap an owned `spl`.
    pub fn new(spl: Spl) -> Self {
        Self { spl: Box::new(spl) }
    }

    /// The wrapped SPL.
    pub fn spl(&self) -> &Spl {
        &self.spl
    }

    /// All the ways the IWD2 spell `resref` can be added to a creature, read
    /// from the spell-list 2DAs. For the seven class books the row in
    /// `listspll` gives both the stored index (its row key) and the per-book
    /// level (columns `BRD CLR DRD PAL RGR SOR WIZ`, 1-based, 0 = absent). A
    /// domain spell reuses that same `listspll` index but takes its level from
    /// `listdomn`'s deity columns. Innate / song / shape books are flat lists
    /// whose row key is the index. Empty when the resref is in no list.
    ///
    /// Keyed by resref (not the SPL contents), so it's an associated function
    /// — no parsed SPL is needed to answer it.
    pub fn iwd2_placements(game_data: &GameData, resref: &str) -> Vec<Iwd2Placement> {
        use Iwd2Spellbook::*;
        let mut out = Vec::new();

        // Class books (+ domain, which shares the listspll index).
        if let Ok(spll) = game_data.import_2da_by_name("listspll")
            && let Some((key, cells)) = spll
                .rows
                .iter()
                .find(|(_, c)| c.last().is_some_and(|r| r.eq_ignore_ascii_case(resref)))
            && let Ok(index) = key.parse::<u32>()
        {
            const BOOKS: [Iwd2Spellbook; 7] =
                [Bard, Cleric, Druid, Paladin, Ranger, Sorcerer, Wizard];
            for (col, &book) in BOOKS.iter().enumerate() {
                if let Some(level) = cells.get(col).and_then(|c| c.parse::<u16>().ok())
                    && level > 0
                {
                    out.push(Iwd2Placement { book, level, index });
                }
            }
            // Domain: same stored index, level from the deity columns.
            if let Some(level) = Self::iwd2_domain_level(game_data, resref) {
                out.push(Iwd2Placement {
                    book: Domain,
                    level,
                    index,
                });
            }
        }

        // Flat books — the row key is the stored index, level is always 1.
        for (table, book) in [
            ("listinnt", Innate),
            ("listsong", Song),
            ("listshap", ShapeChange),
        ] {
            if let Some(index) = Self::iwd2_flat_index(game_data, table, resref) {
                out.push(Iwd2Placement {
                    book,
                    level: 1,
                    index,
                });
            }
        }
        out
    }

    /// The (1-based) level a domain spell `resref` is granted at, from any
    /// non-zero deity column of `listdomn`. `None` if it isn't a domain spell.
    fn iwd2_domain_level(game_data: &GameData, resref: &str) -> Option<u16> {
        let domn = game_data.import_2da_by_name("listdomn").ok()?;
        let cells = domn
            .rows
            .values()
            .find(|c| c.last().is_some_and(|r| r.eq_ignore_ascii_case(resref)))?;
        // Every column but the last is a deity; the spell's level is whichever
        // deity grants it (they agree), so take the first non-zero.
        cells
            .iter()
            .rev()
            .skip(1)
            .filter_map(|c| c.parse::<u16>().ok())
            .find(|&lvl| lvl > 0)
    }

    /// The row index of `resref` in a flat spell-list 2DA (`listinnt` /
    /// `listsong` / `listshap`), whose last column is the resref. `None` when
    /// absent.
    fn iwd2_flat_index(game_data: &GameData, table: &str, resref: &str) -> Option<u32> {
        let t = game_data.import_2da_by_name(table).ok()?;
        t.rows
            .iter()
            .find(|(_, c)| c.last().is_some_and(|r| r.eq_ignore_ascii_case(resref)))
            .and_then(|(key, _)| key.parse::<u32>().ok())
    }
}

impl Deref for ImportedSpl {
    type Target = Spl;

    fn deref(&self) -> &Spl {
        &self.spl
    }
}
