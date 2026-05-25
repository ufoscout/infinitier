#![doc = include_str!("../readme.md")]
//!
//! ## On-disk layout (per version)
//!
//! Every CRE file opens with an 8-byte file prefix
//! (`<CRE_SIGNATURE><version.as_bytes()>`) followed by a fixed-width
//! header. The header carries every primitive field (stats, AC, HP,
//! resists, saves, kits, scripts, sounds, alignment, race, …) plus
//! the **section table** — a block of `(offset, count)` pairs that
//! point at the variable-length sub-section bodies appended after
//! the header.
//!
//! | Version | Header size | Section-table base | Sections layout    |
//! |---------|-------------|--------------------|--------------------|
//! | `V1.0`  | `0x02D4` (724 B)  | `0x02A0`     | V1 spell/item/effect block |
//! | `V1.2`  | `0x0378` (888 B)  | `0x0344`     | V1 block + PST-specific extras (overlay etc.) preserved in header bytes |
//! | `V9.0`  | `0x033C` (828 B)  | `0x0308`     | V1 spell/item/effect block |
//! | `V2.2`  | `0x062E` (1582 B) | `0x03BA`/`0x05FA` | IWD2 d20 block: per-class spell tables, abilities, songs, shapes, items, effects |

mod exporter;
mod header_generated;
mod importer;

pub use exporter::CreExporter;
pub use header_generated::{CreHeaderV10, CreHeaderV12, CreHeaderV22, CreHeaderV90};
pub use importer::CreImporter;

/// 4-byte signature at offset 0 of every CRE file (note the trailing
/// space — every IE signature is exactly 4 bytes).
pub const CRE_SIGNATURE: &[u8; 4] = b"CRE ";

/// Known on-disk version tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreVersion {
    /// `V1.0` — BG1, BG2, BG:EE, BG2:EE, IWD:EE, PST:EE, EET.
    V1_0,
    /// `V1.2` — PST (vanilla).
    V1_2,
    /// `V9.0` — IWD (vanilla / HoW / TotL).
    V9_0,
    /// `V2.2` — IWD2 (d20 system).
    V2_2,
}

impl CreVersion {
    /// The 4-byte tag stored at offset 0x04 of every CRE file.
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            CreVersion::V1_0 => b"V1.0",
            CreVersion::V1_2 => b"V1.2",
            CreVersion::V9_0 => b"V9.0",
            CreVersion::V2_2 => b"V2.2",
        }
    }

    /// Total byte length of this version's fixed header (including
    /// the 8-byte signature/version prefix). Sub-sections start at
    /// or after this offset.
    pub fn header_len(&self) -> usize {
        match self {
            CreVersion::V1_0 => 0x02D4,
            CreVersion::V1_2 => 0x0378,
            CreVersion::V9_0 => 0x033C,
            CreVersion::V2_2 => 0x062E,
        }
    }
}

/// A parsed CRE file.
///
/// Every primitive field documented in IESDP — stats, AC, HP, saves,
/// alignment, race, class, sounds, scripts, item-slot pointers,
/// section pointers, … — is parsed into a typed field on the
/// version-specific [`CreHeader`] variant. Gaps between documented
/// fields are preserved as `_padding_NN: Vec<u8>` so the whole header
/// round-trips byte-for-byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cre {
    /// On-disk version tag (`V1.0` / `V1.2` / `V9.0` / `V2.2`).
    pub version: CreVersion,
    /// The parsed fixed-width header. The variant matches
    /// [`Self::version`]; see the per-version structs for the field
    /// list (~120–330 typed fields depending on version).
    pub header: CreHeader,
    /// Variable-length sub-section bodies. V1.0 / V1.2 / V9.0 share
    /// the same shape; V2.2 has its own d20 layout.
    pub sub_sections: SubSections,
}

/// Per-version typed header. The fixed byte layout is determined by
/// [`CreVersion`] — we use a discriminated enum so the parser /
/// writer can dispatch statically on the variant.
///
/// `V12` and `V22` are boxed because [`CreHeaderV12`] (PST, 169
/// fields) and [`CreHeaderV22`] (IWD2, 329 fields) are several
/// times larger than the V10 / V90 variants; inline storage would
/// force every `Cre` to carry that much memory even for the more
/// common V1.0 / V9.0 creatures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreHeader {
    V10(CreHeaderV10),
    V12(Box<CreHeaderV12>),
    V90(CreHeaderV90),
    V22(Box<CreHeaderV22>),
}

impl CreHeader {
    /// The on-disk version this header was parsed from / will serialise to.
    pub fn version(&self) -> CreVersion {
        match self {
            CreHeader::V10(_) => CreVersion::V1_0,
            CreHeader::V12(_) => CreVersion::V1_2,
            CreHeader::V90(_) => CreVersion::V9_0,
            CreHeader::V22(_) => CreVersion::V2_2,
        }
    }
}

impl Cre {
    /// Strength score (1..=25), present on every CRE version.
    pub fn strength(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.strength,
            CreHeader::V12(h) => h.strength,
            CreHeader::V90(h) => h.strength,
            CreHeader::V22(h) => h.strength,
        }
    }

    /// Strength % bonus (the AD&D 18/01..18/00 modifier). `None` on
    /// IWD2 (CRE V2.2) since the d20 system has no exceptional-Strength
    /// bonus.
    pub fn strength_bonus(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.strength_bonus),
            CreHeader::V12(h) => Some(h.strength_bonus),
            CreHeader::V90(h) => Some(h.strength_bonus),
            CreHeader::V22(_) => None,
        }
    }

    /// Intelligence score (1..=25).
    pub fn intelligence(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.intelligence,
            CreHeader::V12(h) => h.intelligence,
            CreHeader::V90(h) => h.intelligence,
            CreHeader::V22(h) => h.intelligence,
        }
    }

    /// Wisdom score (1..=25).
    pub fn wisdom(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.wisdom,
            CreHeader::V12(h) => h.wisdom,
            CreHeader::V90(h) => h.wisdom,
            CreHeader::V22(h) => h.wisdom,
        }
    }

    /// Dexterity score (1..=25).
    pub fn dexterity(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.dexterity,
            CreHeader::V12(h) => h.dexterity,
            CreHeader::V90(h) => h.dexterity,
            CreHeader::V22(h) => h.dexterity,
        }
    }

    /// Constitution score (1..=25).
    pub fn constitution(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.constitution,
            CreHeader::V12(h) => h.constitution,
            CreHeader::V90(h) => h.constitution,
            CreHeader::V22(h) => h.constitution,
        }
    }

    /// Charisma score (1..=25).
    pub fn charisma(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.charisma,
            CreHeader::V12(h) => h.charisma,
            CreHeader::V90(h) => h.charisma,
            CreHeader::V22(h) => h.charisma,
        }
    }

    /// Current hit points.
    pub fn current_hit_points(&self) -> u16 {
        match &self.header {
            CreHeader::V10(h) => h.current_hit_points,
            CreHeader::V12(h) => h.current_hit_points,
            CreHeader::V90(h) => h.current_hit_points,
            CreHeader::V22(h) => h.current_hit_points,
        }
    }

    /// Maximum hit points.
    pub fn maximum_hit_points(&self) -> u16 {
        match &self.header {
            CreHeader::V10(h) => h.maximum_hit_points,
            CreHeader::V12(h) => h.maximum_hit_points,
            CreHeader::V90(h) => h.maximum_hit_points,
            CreHeader::V22(h) => h.maximum_hit_points,
        }
    }

    /// 4-byte `strref` pointing into `dialog.tlk` for the creature's
    /// long-name (proper-noun display name).
    pub fn long_name_strref(&self) -> u32 {
        match &self.header {
            CreHeader::V10(h) => h.long_name,
            CreHeader::V12(h) => h.long_name,
            CreHeader::V90(h) => h.long_name,
            CreHeader::V22(h) => h.long_name,
        }
    }

    /// 4-byte `strref` pointing into `dialog.tlk` for the creature's
    /// short-name (tooltip).
    pub fn short_name_strref(&self) -> u32 {
        match &self.header {
            CreHeader::V10(h) => h.short_name_tooltip,
            CreHeader::V12(h) => h.short_name_tooltip,
            CreHeader::V90(h) => h.short_name_tooltip,
            CreHeader::V22(h) => h.short_name_tooltip,
        }
    }

    /// Effect-record version flag stored at header offset 0x33:
    /// `0` => V1 (48-byte effects), `1` => V2 (264-byte effects).
    pub fn eff_version(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.eff_structure_version_0_version_1,
            CreHeader::V12(h) => h.eff_structure_version_0_version_1,
            CreHeader::V90(h) => h.eff_structure_version_0_version_1,
            CreHeader::V22(h) => h.eff_structure_version_0_version_1,
        }
    }
}

/// Variable-length sub-sections, dispatched by version family.
///
/// The V2.2 variant is boxed because its struct is ~16× larger than
/// the V1 one (seven 9-level class-spell arrays + domain spells +
/// the rest); inline storage would force every `Cre` to carry that
/// much memory even for V1.0 / V1.2 / V9.0 files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubSections {
    /// Used by V1.0, V1.2, and V9.0 — all three share the same
    /// known-spells / spell-memorisation / memorised-spells / items
    /// / item-slots / effects layout. Differences live only in the
    /// header itself.
    V1(V1SubSections),
    /// IWD2 (V2.2) — d20 layout with per-class spell tables plus
    /// abilities / songs / shapes blocks.
    V22(Box<V22SubSections>),
}

/// Sub-sections shared by V1.0 / V1.2 / V9.0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V1SubSections {
    /// Spells the creature knows (priest / wizard / innate). 12 B
    /// per record on disk.
    pub known_spells: Vec<KnownSpell>,
    /// One row per spell level / type slot, carrying memorisation
    /// counts. 16 B per record on disk.
    pub spell_memorization_info: Vec<SpellMemorizationInfo>,
    /// Flat list of currently-memorised spells (referenced by the
    /// `spell_table_index` / `count` cursor in each
    /// [`SpellMemorizationInfo`]). 12 B per record.
    pub memorized_spells: Vec<MemorizedSpell>,
    /// Items the creature carries. 20 B per record.
    pub items: Vec<Item>,
    /// Item-slot lookup table — version-specific in size (V1.0 /
    /// V9.0 = 80 B = 40 × `i16` slot indices, V1.2 / PST = 96 B,
    /// V2.2 = 104 B). Stored as raw bytes because the inner layout
    /// is a flat list of `i16` indices into [`Self::items`].
    pub item_slots: Vec<u8>,
    /// Effect records on the creature. EFF byte at header offset
    /// 0x33 selects whether the records are 48-byte V1 or 264-byte
    /// V2; the parser handles both transparently.
    pub effects: EffectList,
}

/// Sub-sections specific to V2.2 (IWD2).
///
/// Every IWD2 spell / ability / song / shape block on disk is a
/// [`Iwd2Table`] — a variable list of [`Iwd2Slot`] records (16 B
/// each, same shape across all four categories) followed by an
/// 8-byte trailer carrying the `num_memorizable` / `num_remaining`
/// counters. The trailer is always present, even for empty lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V22SubSections {
    /// Bard's per-level spell tables (levels 1..=9).
    pub bard_spells: [Iwd2Table; 9],
    /// Cleric's per-level spell tables.
    pub cleric_spells: [Iwd2Table; 9],
    /// Druid's per-level spell tables.
    pub druid_spells: [Iwd2Table; 9],
    /// Paladin's per-level spell tables.
    pub paladin_spells: [Iwd2Table; 9],
    /// Ranger's per-level spell tables.
    pub ranger_spells: [Iwd2Table; 9],
    /// Sorcerer's per-level spell tables.
    pub sorcerer_spells: [Iwd2Table; 9],
    /// Wizard's per-level spell tables.
    pub wizard_spells: [Iwd2Table; 9],
    /// Cleric-domain spell tables (9 domains, IWD2 cleric kit).
    pub domain_spells: [Iwd2Table; 9],
    /// Innate / racial abilities.
    pub abilities: Iwd2Table,
    /// Bard songs.
    pub songs: Iwd2Table,
    /// Druid wild-shape forms.
    pub shapes: Iwd2Table,
    /// Items the creature carries (20 B records, same shape as V1
    /// items).
    pub items: Vec<Item>,
    /// Item-slot index table (104 B = 52 × `i16` slot indices).
    pub item_slots: Vec<u8>,
    /// Effect records (V1 or V2 depending on header flag).
    pub effects: EffectList,
}

/// One known-spell record (12 B on disk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownSpell {
    /// 0x00: 8-byte ASCIIZ SPL resref. Trailing NULs stripped.
    pub spell: String,
    /// 0x08: spell level (1..=9).
    pub level: u16,
    /// 0x0A: spell type — `0` priest, `1` wizard, `2` innate.
    pub spell_type: u16,
}

/// One spell-memorisation row (16 B on disk). Carries the
/// `spell_table_index`/`spell_count` cursor that selects which
/// [`MemorizedSpell`]s belong to this slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellMemorizationInfo {
    /// 0x00: spell level (1..=9).
    pub level: u16,
    /// 0x02: # spells memorisable in total.
    pub num_memorizable_total: u16,
    /// 0x04: # currently memorisable (after effects).
    pub num_memorizable_current: u16,
    /// 0x06: spell type — same `0`/`1`/`2` scheme as [`KnownSpell`].
    pub spell_type: u16,
    /// 0x08: index into [`V1SubSections::memorized_spells`] where
    /// this slot's spells start.
    pub spell_table_index: u32,
    /// 0x0C: number of [`MemorizedSpell`]s belonging to this slot.
    pub spell_count: u32,
}

/// One memorised-spell entry (12 B on disk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorizedSpell {
    /// 0x00: 8-byte ASCIIZ SPL resref.
    pub spell: String,
    /// 0x08: memorisation flag bitfield (bit 0 = memorised, bit 1 =
    /// disabled, bit 2+ = "already cast" in some EE saves).
    pub memorization_flags: u16,
    /// 0x0A: unknown / padding (NI calls this "Unknown"). Preserved
    /// verbatim for round-trip.
    pub padding: u16,
}

/// One carried-item record (20 B on disk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 0x00: 8-byte ASCIIZ ITM resref.
    pub item: String,
    /// 0x08: item-expiration duration (EE) or "unknown" (vanilla).
    /// Stored verbatim.
    pub duration: u16,
    /// 0x0A: primary quantity / charges.
    pub quantity1: u16,
    /// 0x0C: secondary quantity / charges (e.g. wand charges).
    pub quantity2: u16,
    /// 0x0E: tertiary quantity / charges.
    pub quantity3: u16,
    /// 0x10: item flags bitfield (`identified`, `unstealable`,
    /// `stolen`, `undroppable`, …).
    pub flags: u32,
}

/// Effect-record list. EFF byte at header offset 0x33 (`0` => V1,
/// `1` => V2) selects the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectList {
    /// 48-byte V1 effect records (BG1 / vanilla IWD / vanilla PST).
    V1(Vec<EffectV1>),
    /// 264-byte V2 effect records (BG2 / EE / IWD2).
    V2(Vec<EffectV2>),
}

impl EffectList {
    /// Record size on disk for this variant.
    pub fn record_size(&self) -> usize {
        match self {
            EffectList::V1(_) => 48,
            EffectList::V2(_) => 264,
        }
    }

    /// Number of effect records.
    pub fn len(&self) -> usize {
        match self {
            EffectList::V1(v) => v.len(),
            EffectList::V2(v) => v.len(),
        }
    }

    /// `true` when there are no effect records.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One V1 (48-byte) effect record. Inner-parameter parsing depends
/// on 300+ opcodes so the bytes are kept verbatim — callers that
/// know the opcode can pick the fields they need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectV1 {
    /// Raw 48 bytes of the record.
    pub raw: [u8; 48],
}

/// One V2 (264-byte) effect record. Same opacity reasoning as
/// [`EffectV1`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectV2 {
    /// Raw 264 bytes of the record.
    pub raw: Vec<u8>,
}

/// One 16-byte IWD2 sub-section slot. The on-disk layout is the
/// same for class spells, domain spells, abilities, songs and
/// shapes — NI uses four distinct Java classes for UI labelling but
/// the bytes are identical.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Iwd2Slot {
    /// 0x00: 8-byte ASCIIZ SPL resref.
    pub spell: String,
    /// 0x08: memorised flag (NI: "memorized" Bitmap).
    pub memorized: u32,
    /// 0x0C: free uses remaining / "unknown" (NI). Mirrors the
    /// table-level trailer in some entries.
    pub remaining: u32,
}

/// A complete IWD2 sub-section on disk: `entries` × 16 B, followed
/// by an 8-byte trailer (`num_memorizable` + `num_remaining`). The
/// trailer is always present even when `entries` is empty.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Iwd2Table {
    /// Records in file order.
    pub entries: Vec<Iwd2Slot>,
    /// Trailer u32 — total memorisable count for this slot.
    pub num_memorizable: u32,
    /// Trailer u32 — free uses remaining for this slot.
    pub num_remaining: u32,
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::{Cre, CreImporter};

    /// Recursively collect every `.cre` file under `assets/cre/`.
    pub fn all_cre_fixtures() -> Vec<PathBuf> {
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.eq_ignore_ascii_case("cre"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        visit(&get_assets_path().join("cre"), &mut out);
        out.sort();
        out
    }

    /// Imports a fixture path relative to `assets/cre/`.
    pub fn import_fixture(rel_path: &str) -> Cre {
        let path = get_assets_path().join("cre").join(rel_path);
        CreImporter { name: rel_path }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("import {rel_path}: {e}"))
    }
}
