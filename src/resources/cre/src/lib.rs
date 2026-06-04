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
pub use header_generated::{
    CreHeaderV10, CreHeaderV12, CreHeaderV22, CreHeaderV90, NumberOfAttacks,
};
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

    // ── Setters ────────────────────────────────────────────────────
    //
    // Same per-variant dispatch as the getters above. Callers are
    // expected to clamp to the engine's gameplay range
    // (`infinitier_core::engine_caps::ability_caps`) before writing;
    // we don't enforce it here so the importer can stay bit-exact
    // round-trip with whatever the file contains.

    /// Overwrite the strength score.
    pub fn set_strength(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.strength = value,
            CreHeader::V12(h) => h.strength = value,
            CreHeader::V90(h) => h.strength = value,
            CreHeader::V22(h) => h.strength = value,
        }
    }

    /// Overwrite the AD&D extraordinary-strength percentile.
    /// Silently no-ops on V2.2 headers (IWD2 d20 has no such field).
    pub fn set_strength_bonus(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.strength_bonus = value,
            CreHeader::V12(h) => h.strength_bonus = value,
            CreHeader::V90(h) => h.strength_bonus = value,
            CreHeader::V22(_) => {}
        }
    }

    /// Overwrite the intelligence score.
    pub fn set_intelligence(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.intelligence = value,
            CreHeader::V12(h) => h.intelligence = value,
            CreHeader::V90(h) => h.intelligence = value,
            CreHeader::V22(h) => h.intelligence = value,
        }
    }

    /// Overwrite the wisdom score.
    pub fn set_wisdom(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.wisdom = value,
            CreHeader::V12(h) => h.wisdom = value,
            CreHeader::V90(h) => h.wisdom = value,
            CreHeader::V22(h) => h.wisdom = value,
        }
    }

    /// Overwrite the dexterity score.
    pub fn set_dexterity(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.dexterity = value,
            CreHeader::V12(h) => h.dexterity = value,
            CreHeader::V90(h) => h.dexterity = value,
            CreHeader::V22(h) => h.dexterity = value,
        }
    }

    /// Overwrite the constitution score.
    pub fn set_constitution(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.constitution = value,
            CreHeader::V12(h) => h.constitution = value,
            CreHeader::V90(h) => h.constitution = value,
            CreHeader::V22(h) => h.constitution = value,
        }
    }

    /// Overwrite the charisma score.
    pub fn set_charisma(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.charisma = value,
            CreHeader::V12(h) => h.charisma = value,
            CreHeader::V90(h) => h.charisma = value,
            CreHeader::V22(h) => h.charisma = value,
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

/// The spellbook a [`KnownSpell`] belongs to. Stored on disk as a
/// `u16` (`0` priest, `1` wizard, `2` innate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellType {
    /// Divine / priest spell (on-disk `0`).
    Priest,
    /// Arcane / wizard spell (on-disk `1`).
    Wizard,
    /// Innate ability (on-disk `2`).
    Innate,
    /// Any other on-disk value, preserved verbatim for round-trip.
    Unknown(u16),
}

impl SpellType {
    /// Decode the on-disk `u16` type code.
    pub fn from_u16(value: u16) -> Self {
        match value {
            0 => SpellType::Priest,
            1 => SpellType::Wizard,
            2 => SpellType::Innate,
            other => SpellType::Unknown(other),
        }
    }

    /// Encode back to the on-disk `u16` type code.
    pub fn to_u16(self) -> u16 {
        match self {
            SpellType::Priest => 0,
            SpellType::Wizard => 1,
            SpellType::Innate => 2,
            SpellType::Unknown(other) => other,
        }
    }
}

/// One known-spell record (12 B on disk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownSpell {
    /// 0x00: 8-byte ASCIIZ SPL resref. Trailing NULs stripped.
    pub spell: String,
    /// 0x08: spell level (1..=9).
    pub level: u16,
    /// 0x0A: spell type (priest / wizard / innate).
    pub spell_type: SpellType,
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
    /// 0x06: spell type (priest / wizard / innate).
    pub spell_type: SpellType,
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

bitflags::bitflags! {
    /// Per-item flags at CRE item offset 0x10 (a 4-byte field).
    ///
    /// Only the low four bits are defined; any other bits are kept via
    /// [`ItemFlags::from_bits_retain`] so the record round-trips
    /// byte-for-byte.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ItemFlags: u32 {
        /// The item has been identified.
        const Identified = 1 << 0;
        /// The item cannot be stolen.
        const Unstealable = 1 << 1;
        /// The item is flagged as stolen.
        const Stolen = 1 << 2;
        /// The item cannot be dropped.
        const Undroppable = 1 << 3;
    }
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
    /// 0x10: item flags (`identified`, `unstealable`, `stolen`,
    /// `undroppable`).
    pub flags: ItemFlags,
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

/// One V2 (264-byte) effect record, parsed into a typed variant.
///
/// Three opcodes get their own minimal, easily-editable representation
/// — [`LocalVariable`] (`op187`) and [`Proficiency`] (`op233`), each
/// rebuilt from a canonical template, and every other opcode as a
/// fully-parsed [`Effect`] (all fields modelled, so it round-trips
/// without keeping the raw bytes). [`EffectV2::Raw`] only survives for
/// records too short to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectV2 {
    /// A record kept byte-for-byte — only used when a record is shorter
    /// than the 264-byte V2 layout (it otherwise never occurs).
    Raw(Vec<u8>),
    /// A creature-local script variable (`op187`), parsed into its
    /// editable [`name`](LocalVariable::name) / [`value`](LocalVariable::value).
    LocalVariable(LocalVariable),
    /// A weapon-proficiency setter (`op233`), parsed into its editable
    /// [`proficiency`](Proficiency::proficiency) / [`points`](Proficiency::points).
    Proficiency(Proficiency),
    /// Any other effect, fully parsed into editable fields. Boxed to
    /// keep the enum (and the per-creature `Vec<EffectV2>`) small.
    Effect(Box<Effect>),
}

impl EffectV2 {
    /// Effect opcode that sets a creature-local script variable.
    pub const LOCAL_VARIABLE_OPCODE: u32 = 187;
    /// Effect opcode that sets a weapon proficiency.
    pub const PROFICIENCY_OPCODE: u32 = 233;

    /// The effect opcode (dword at record offset 0x08).
    pub fn opcode(&self) -> u32 {
        match self {
            EffectV2::Raw(r) => u32::from_le_bytes([r[0x08], r[0x09], r[0x0A], r[0x0B]]),
            EffectV2::LocalVariable(_) => Self::LOCAL_VARIABLE_OPCODE,
            EffectV2::Proficiency(_) => Self::PROFICIENCY_OPCODE,
            EffectV2::Effect(e) => e.opcode,
        }
    }

    /// The 264-byte on-disk record for this effect. `LocalVariable` /
    /// `Proficiency` are rebuilt from their templates; a full
    /// [`Effect`] serialises every field; `Raw` is emitted verbatim.
    pub(crate) fn to_record(&self) -> Vec<u8> {
        match self {
            EffectV2::Raw(r) => r.clone(),
            EffectV2::LocalVariable(lv) => lv.to_record(),
            EffectV2::Proficiency(p) => p.to_record(),
            EffectV2::Effect(e) => e.to_record(),
        }
    }
}

/// A creature-local script variable, parsed from an `op187` ("set
/// local variable") V2 effect record.
///
/// EE games store each `LOCALS` variable as one such effect: the value
/// in `param1` (record offset 0x14) and the 32-byte name in the
/// feature block's EE-only variable-name field (record offset 0xA0).
/// Only those two carry the variable's identity, so they are the only
/// thing kept here — making the value trivially editable. The exporter
/// rebuilds a complete record from [`Self::TEMPLATE`].
///
/// The remaining effect fields are application metadata (target,
/// timing, caster position, time applied). Their structural constants
/// — permanent timing, 100% probability, "no target" markers — are
/// preserved by the template; the genuinely per-instance bits (the
/// caster's map position, the application timestamp) are *not* part of
/// the variable and are normalised to zero on write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVariable {
    /// Variable name (`LOCALS` script identifier).
    pub name: String,
    /// Signed value — the engine's 32-bit script integer.
    pub value: i32,
}

impl LocalVariable {
    /// Record offset / length of the 32-byte variable-name field.
    const NAME_OFFSET: usize = 0xA0;
    const NAME_LEN: usize = 32;
    /// Record offset of `param1`, the variable's value.
    const VALUE_OFFSET: usize = 0x14;

    /// Canonical 264-byte `op187` record with the per-instance fields
    /// (name, value, caster position, time applied) zeroed. Captured
    /// from a BG2EE save; the surviving non-zero bytes are the
    /// structural constants every "set local variable" effect shares —
    /// opcode `187` (0x08), permanent timing mode `9` (0x1C), 100%
    /// probability (0x24), and the `0xFF` "no target" markers — so a
    /// rebuilt effect is engine-equivalent to the original.
    #[rustfmt::skip]
    const TEMPLATE: [u8; 264] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xbb, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// Parse an `op187` record into its name / value. The caller must
    /// have verified the opcode is [`EffectV2::LOCAL_VARIABLE_OPCODE`];
    /// `record` must be at least 264 bytes.
    pub(crate) fn from_record(record: &[u8]) -> Self {
        let name_bytes = &record[Self::NAME_OFFSET..Self::NAME_OFFSET + Self::NAME_LEN];
        let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(name_bytes);
        let o = Self::VALUE_OFFSET;
        LocalVariable {
            name: decoded.trim_end_matches('\0').to_owned(),
            value: i32::from_le_bytes([record[o], record[o + 1], record[o + 2], record[o + 3]]),
        }
    }

    /// Serialise back to a 264-byte `op187` record: the canonical
    /// [`Self::TEMPLATE`] with `name` (WINDOWS-1252, NUL-padded) and
    /// `value` written in.
    pub(crate) fn to_record(&self) -> Vec<u8> {
        let mut r = Self::TEMPLATE.to_vec();
        let o = Self::VALUE_OFFSET;
        r[o..o + 4].copy_from_slice(&self.value.to_le_bytes());
        let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(&self.name);
        let n = encoded.len().min(Self::NAME_LEN);
        r[Self::NAME_OFFSET..Self::NAME_OFFSET + n].copy_from_slice(&encoded[..n]);
        r
    }
}

/// A weapon-proficiency setter, parsed from an `op233` ("set
/// proficiency") V2 effect record.
///
/// `param1` is the number of points and `param2` the `IE_PROFICIENCY*`
/// stat the points apply to. Those two are the only editable values;
/// the exporter rebuilds the rest from [`Self::TEMPLATE`] (op233's
/// other fields — permanent timing, 100% probability, "no target"
/// markers — are constant, so this round-trips faithfully).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Proficiency {
    /// `param2` — the `IE_PROFICIENCY*` stat the points apply to.
    pub proficiency: u32,
    /// `param1` — the number of proficiency points granted.
    pub points: u32,
}

impl Proficiency {
    /// `param1` (points) / `param2` (proficiency stat) record offsets.
    const POINTS_OFFSET: usize = 0x14;
    const PROFICIENCY_OFFSET: usize = 0x18;

    /// Canonical 264-byte `op233` record with `param1` / `param2`
    /// zeroed. Captured from a BG2EE save; the surviving bytes are the
    /// structural constants every proficiency effect shares — opcode
    /// `233` (0x08), permanent timing mode `9` (0x1C), 100% probability
    /// (0x24), and the `0xFF` "no target" markers.
    #[rustfmt::skip]
    const TEMPLATE: [u8; 264] = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe9, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x09, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    /// Parse an `op233` record into its proficiency / points. The
    /// caller must have verified the opcode is
    /// [`EffectV2::PROFICIENCY_OPCODE`]; `record` must be ≥ 264 bytes.
    pub(crate) fn from_record(record: &[u8]) -> Self {
        Proficiency {
            points: rd_u32(record, Self::POINTS_OFFSET),
            proficiency: rd_u32(record, Self::PROFICIENCY_OFFSET),
        }
    }

    /// Serialise back to a 264-byte `op233` record: the canonical
    /// [`Self::TEMPLATE`] with `points` (`param1`) and `proficiency`
    /// (`param2`) written in.
    pub(crate) fn to_record(self) -> Vec<u8> {
        let mut r = Self::TEMPLATE.to_vec();
        r[Self::POINTS_OFFSET..Self::POINTS_OFFSET + 4].copy_from_slice(&self.points.to_le_bytes());
        r[Self::PROFICIENCY_OFFSET..Self::PROFICIENCY_OFFSET + 4]
            .copy_from_slice(&self.proficiency.to_le_bytes());
        r
    }
}

/// A fully-parsed EE (V2, 264-byte) effect record — every field is
/// modelled, so it is freely editable and round-trips without keeping
/// the original bytes.
///
/// Field offsets follow the EE feature-block layout (cf. NearInfinity's
/// `BaseOpcode`), taken relative to the opcode at record offset 0x08.
/// [`header`](Self::header) (0x00, the standalone-`EFF` signature /
/// version, zero for embedded records) and [`trailing`](Self::trailing)
/// (0xD0, reserved) are kept verbatim so nothing is lost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    /// 0x00: 8-byte signature/version prefix (zero for CRE-embedded).
    pub header: [u8; 8],
    /// 0x08: effect opcode.
    pub opcode: u32,
    /// 0x0C: target dispatcher.
    pub target: u32,
    /// 0x10: power.
    pub power: u32,
    /// 0x14: parameter 1.
    pub param1: u32,
    /// 0x18: parameter 2.
    pub param2: u32,
    /// 0x1C: timing mode.
    pub timing_mode: u32,
    /// 0x20: duration.
    pub duration: u32,
    /// 0x24: probability (upper).
    pub probability1: u16,
    /// 0x26: probability (lower).
    pub probability2: u16,
    /// 0x28: primary resource (8-byte resref).
    pub resource: String,
    /// 0x30: dice thrown / max level.
    pub dice_thrown: u32,
    /// 0x34: dice sides / min level.
    pub dice_sides: u32,
    /// 0x38: saving-throw type.
    pub save_type: u32,
    /// 0x3C: saving-throw bonus.
    pub save_bonus: u32,
    /// 0x40: special / "is variable".
    pub special: u32,
    /// 0x44: primary type (school).
    pub primary_type: u32,
    /// 0x48: unknown.
    pub unknown_48: u32,
    /// 0x4C: minimum level (parent resource).
    pub parent_lowest_level: u32,
    /// 0x50: maximum level (parent resource).
    pub parent_highest_level: u32,
    /// 0x54: dispel / resistance.
    pub resistance: u32,
    /// 0x58: parameter 3.
    pub param3: u32,
    /// 0x5C: parameter 4.
    pub param4: u32,
    /// 0x60: parameter 5.
    pub param5: u32,
    /// 0x64: time applied (game-time when the effect was applied).
    pub time_applied: u32,
    /// 0x68: secondary resource (8-byte resref).
    pub resource2: String,
    /// 0x70: tertiary resource (8-byte resref).
    pub resource3: String,
    /// 0x78: caster location X.
    pub caster_x: u32,
    /// 0x7C: caster location Y.
    pub caster_y: u32,
    /// 0x80: target location X.
    pub target_x: u32,
    /// 0x84: target location Y.
    pub target_y: u32,
    /// 0x88: parent-resource type.
    pub parent_resource_type: u32,
    /// 0x8C: parent resource (8-byte resref).
    pub parent_resource: String,
    /// 0x94: parent-resource flags.
    pub parent_resource_flags: u32,
    /// 0x98: projectile.
    pub projectile: u32,
    /// 0x9C: parent-resource slot.
    pub parent_resource_slot: u32,
    /// 0xA0: variable name (32-byte field).
    pub variable: String,
    /// 0xC0: caster level.
    pub caster_level: u32,
    /// 0xC4: first-apply flag.
    pub first_apply: u32,
    /// 0xC8: secondary type.
    pub secondary_type: u32,
    /// 0xCC: unknown.
    pub unknown_cc: u32,
    /// 0xD0: 56-byte reserved trailer, kept verbatim.
    pub trailing: [u8; 56],
}

impl Effect {
    /// Parse a 264-byte EE V2 effect record into typed fields. `record`
    /// must be at least 264 bytes.
    pub(crate) fn from_record(r: &[u8]) -> Self {
        Effect {
            header: r[0x00..0x08].try_into().unwrap(),
            opcode: rd_u32(r, 0x08),
            target: rd_u32(r, 0x0C),
            power: rd_u32(r, 0x10),
            param1: rd_u32(r, 0x14),
            param2: rd_u32(r, 0x18),
            timing_mode: rd_u32(r, 0x1C),
            duration: rd_u32(r, 0x20),
            probability1: rd_u16(r, 0x24),
            probability2: rd_u16(r, 0x26),
            resource: rd_resref(r, 0x28),
            dice_thrown: rd_u32(r, 0x30),
            dice_sides: rd_u32(r, 0x34),
            save_type: rd_u32(r, 0x38),
            save_bonus: rd_u32(r, 0x3C),
            special: rd_u32(r, 0x40),
            primary_type: rd_u32(r, 0x44),
            unknown_48: rd_u32(r, 0x48),
            parent_lowest_level: rd_u32(r, 0x4C),
            parent_highest_level: rd_u32(r, 0x50),
            resistance: rd_u32(r, 0x54),
            param3: rd_u32(r, 0x58),
            param4: rd_u32(r, 0x5C),
            param5: rd_u32(r, 0x60),
            time_applied: rd_u32(r, 0x64),
            resource2: rd_resref(r, 0x68),
            resource3: rd_resref(r, 0x70),
            caster_x: rd_u32(r, 0x78),
            caster_y: rd_u32(r, 0x7C),
            target_x: rd_u32(r, 0x80),
            target_y: rd_u32(r, 0x84),
            parent_resource_type: rd_u32(r, 0x88),
            parent_resource: rd_resref(r, 0x8C),
            parent_resource_flags: rd_u32(r, 0x94),
            projectile: rd_u32(r, 0x98),
            parent_resource_slot: rd_u32(r, 0x9C),
            variable: rd_resref_n(r, 0xA0, 32),
            caster_level: rd_u32(r, 0xC0),
            first_apply: rd_u32(r, 0xC4),
            secondary_type: rd_u32(r, 0xC8),
            unknown_cc: rd_u32(r, 0xCC),
            trailing: r[0xD0..0x108].try_into().unwrap(),
        }
    }

    /// Serialise back to a 264-byte EE V2 effect record.
    pub(crate) fn to_record(&self) -> Vec<u8> {
        let mut r = vec![0u8; 264];
        r[0x00..0x08].copy_from_slice(&self.header);
        wr_u32(&mut r, 0x08, self.opcode);
        wr_u32(&mut r, 0x0C, self.target);
        wr_u32(&mut r, 0x10, self.power);
        wr_u32(&mut r, 0x14, self.param1);
        wr_u32(&mut r, 0x18, self.param2);
        wr_u32(&mut r, 0x1C, self.timing_mode);
        wr_u32(&mut r, 0x20, self.duration);
        wr_u16(&mut r, 0x24, self.probability1);
        wr_u16(&mut r, 0x26, self.probability2);
        wr_resref(&mut r, 0x28, &self.resource);
        wr_u32(&mut r, 0x30, self.dice_thrown);
        wr_u32(&mut r, 0x34, self.dice_sides);
        wr_u32(&mut r, 0x38, self.save_type);
        wr_u32(&mut r, 0x3C, self.save_bonus);
        wr_u32(&mut r, 0x40, self.special);
        wr_u32(&mut r, 0x44, self.primary_type);
        wr_u32(&mut r, 0x48, self.unknown_48);
        wr_u32(&mut r, 0x4C, self.parent_lowest_level);
        wr_u32(&mut r, 0x50, self.parent_highest_level);
        wr_u32(&mut r, 0x54, self.resistance);
        wr_u32(&mut r, 0x58, self.param3);
        wr_u32(&mut r, 0x5C, self.param4);
        wr_u32(&mut r, 0x60, self.param5);
        wr_u32(&mut r, 0x64, self.time_applied);
        wr_resref(&mut r, 0x68, &self.resource2);
        wr_resref(&mut r, 0x70, &self.resource3);
        wr_u32(&mut r, 0x78, self.caster_x);
        wr_u32(&mut r, 0x7C, self.caster_y);
        wr_u32(&mut r, 0x80, self.target_x);
        wr_u32(&mut r, 0x84, self.target_y);
        wr_u32(&mut r, 0x88, self.parent_resource_type);
        wr_resref(&mut r, 0x8C, &self.parent_resource);
        wr_u32(&mut r, 0x94, self.parent_resource_flags);
        wr_u32(&mut r, 0x98, self.projectile);
        wr_u32(&mut r, 0x9C, self.parent_resource_slot);
        wr_resref_n(&mut r, 0xA0, 32, &self.variable);
        wr_u32(&mut r, 0xC0, self.caster_level);
        wr_u32(&mut r, 0xC4, self.first_apply);
        wr_u32(&mut r, 0xC8, self.secondary_type);
        wr_u32(&mut r, 0xCC, self.unknown_cc);
        r[0xD0..0x108].copy_from_slice(&self.trailing);
        r
    }
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// Read an 8-byte ASCIIZ resref (WINDOWS-1252, NUL-trimmed).
fn rd_resref(b: &[u8], o: usize) -> String {
    rd_resref_n(b, o, 8)
}

/// Read an `n`-byte ASCIIZ string field (WINDOWS-1252, NUL-trimmed).
fn rd_resref_n(b: &[u8], o: usize, n: usize) -> String {
    let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(&b[o..o + n]);
    decoded.trim_end_matches('\0').to_owned()
}

fn wr_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}

fn wr_u16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn wr_resref(b: &mut [u8], o: usize, s: &str) {
    wr_resref_n(b, o, 8, s);
}

/// Write an `n`-byte ASCIIZ string field (WINDOWS-1252, NUL-padded).
fn wr_resref_n(b: &mut [u8], o: usize, n: usize, s: &str) {
    let (encoded, _, _) = encoding_rs::WINDOWS_1252.encode(s);
    let len = encoded.len().min(n);
    b[o..o + len].copy_from_slice(&encoded[..len]);
}

impl Cre {
    /// The creature's local script variables (`LOCALS` scope), in CRE
    /// file order. These are stored as `op187` effects, parsed by the
    /// importer into [`EffectV2::LocalVariable`]. Empty for creatures
    /// whose effects are the classic 48-byte (V1) records — those are
    /// too small to carry the variable-name field, so pre-EE games
    /// don't persist creature locals this way.
    pub fn local_variables(&self) -> impl Iterator<Item = &LocalVariable> {
        let effects = match &self.sub_sections {
            SubSections::V1(s) => &s.effects,
            SubSections::V22(s) => &s.effects,
        };
        let list: &[EffectV2] = match effects {
            EffectList::V2(list) => list,
            EffectList::V1(_) => &[],
        };
        list.iter().filter_map(|e| match e {
            EffectV2::LocalVariable(lv) => Some(lv),
            _ => None,
        })
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic 264-byte `op187` ("set local variable")
    /// record with the given name / value.
    fn op187_record(name: &str, value: i32) -> Vec<u8> {
        let mut r = vec![0u8; 264];
        r[0x08..0x0C].copy_from_slice(&EffectV2::LOCAL_VARIABLE_OPCODE.to_le_bytes());
        r[0x14..0x18].copy_from_slice(&value.to_le_bytes());
        let nb = name.as_bytes();
        r[0xA0..0xA0 + nb.len()].copy_from_slice(nb);
        r
    }

    #[test]
    fn local_variable_parses_name_and_value() {
        let lv = LocalVariable::from_record(&op187_record("KELDORNESTATE", 2));
        assert_eq!(lv.name, "KELDORNESTATE"); // trailing NULs trimmed
        assert_eq!(lv.value, 2);
    }

    #[test]
    fn local_variable_value_is_signed() {
        let lv = LocalVariable::from_record(&op187_record("DELTA", -7));
        assert_eq!(lv.value, -7);
    }

    /// A `LocalVariable` rebuilds to a record that re-parses to the
    /// same name / value, and the rebuilt record is a valid op187
    /// (correct opcode, permanent timing) — the property the exporter
    /// relies on.
    #[test]
    fn local_variable_record_round_trips_name_and_value() {
        let original = LocalVariable {
            name: "MARIAFIGHT".to_owned(),
            value: 2,
        };
        let record = original.to_record();
        assert_eq!(record.len(), 264);
        // Rebuilt record carries the right opcode + permanent timing.
        assert_eq!(
            u32::from_le_bytes([record[0x08], record[0x09], record[0x0A], record[0x0B]]),
            EffectV2::LOCAL_VARIABLE_OPCODE,
        );
        assert_eq!(record[0x1C], 9, "permanent timing mode");
        // Re-parsing yields an equal variable.
        assert_eq!(LocalVariable::from_record(&record), original);
    }

    #[test]
    fn effect_v2_opcode_for_variants() {
        let lv = EffectV2::LocalVariable(LocalVariable {
            name: "X".to_owned(),
            value: 1,
        });
        assert_eq!(lv.opcode(), EffectV2::LOCAL_VARIABLE_OPCODE);
        assert_eq!(lv.to_record().len(), 264);

        let prof = EffectV2::Proficiency(Proficiency {
            proficiency: 89,
            points: 2,
        });
        assert_eq!(prof.opcode(), EffectV2::PROFICIENCY_OPCODE);

        let raw = EffectV2::Raw(vec![0u8; 16]);
        assert_eq!(raw.opcode(), 0);
    }

    #[test]
    fn proficiency_round_trips_and_is_op233() {
        let original = Proficiency {
            proficiency: 89,
            points: 2,
        };
        let record = original.to_record();
        assert_eq!(record.len(), 264);
        assert_eq!(
            u32::from_le_bytes([record[0x08], record[0x09], record[0x0A], record[0x0B]]),
            EffectV2::PROFICIENCY_OPCODE,
        );
        assert_eq!(record[0x1C], 9, "permanent timing mode");
        assert_eq!(Proficiency::from_record(&record), original);
    }

    /// Every byte of a V2 record is read and written back by
    /// [`Effect`] — fill a record with a NUL-free pattern (so the
    /// resref / variable fields survive intact) and assert a parse →
    /// serialise round-trip reproduces it byte-for-byte. This proves no
    /// field offset is missed and nothing is silently dropped.
    #[test]
    fn effect_full_round_trip_is_byte_exact() {
        let original: Vec<u8> = (0..264u32)
            .map(|i| 0x20 + (i % 0x5E) as u8) // printable ASCII, no NUL
            .collect();
        let effect = Effect::from_record(&original);
        let produced = effect.to_record();
        assert_eq!(produced, original, "Effect must cover every byte");
    }
}
