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
mod header;
mod importer;

pub use exporter::CreExporter;
pub use header::{
    CreHeaderV10, CreHeaderV12, CreHeaderV22, CreHeaderV90, NumberOfAttacks, TrackingTarget,
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

    /// Race — an index into `RACE.IDS` (e.g. `1` = `HUMAN` in BG/BG2).
    pub fn race(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.race_race_ids,
            CreHeader::V12(h) => h.race_race_ids,
            CreHeader::V90(h) => h.race_race_ids,
            CreHeader::V22(h) => h.race_race_ids,
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

    /// Whether this creature is *dual*-classed (as opposed to
    /// multi-classed) — relevant on the AD&D engines, where dual- and
    /// multi-class characters compute hit points differently (a dual
    /// freezes its first class until the second surpasses it; a multi
    /// splits each level's HP across its classes).
    ///
    /// The engine records the former class of a dual with exactly one
    /// `MC_WAS_*` bit (`0x08`..=`0x100`) in the creature-flags field at
    /// offset `0x10`; a true multi-class has none set. Mirrors GemRB's
    /// `Actor::IsDualClassed` (`CountBits(flags & MC_WAS_ANY) == 1`).
    ///
    /// Always `false` for V2.2 (IWD2), whose 3e rules have no
    /// dual-classing and reuse those flag bits.
    pub fn is_dual_classed(&self) -> bool {
        self.dual_class_original_class().is_some()
    }

    /// For a dual-classed creature, the class it dual-classed *out of*
    /// — identified by the single `MC_WAS_*` bit set in the creature-
    /// flags dword. Returns `None` for a true multi-class (no such bit),
    /// an ambiguous record (more than one set), a single-class creature,
    /// or any V2.2 (IWD2) file, whose 3e rules have no dual-classing and
    /// reuse those bits.
    pub fn dual_class_original_class(&self) -> Option<OriginalClass> {
        // V2.2 (IWD2) reuses these bits for non-dual-class purposes.
        if matches!(self.header, CreHeader::V22(_)) {
            return None;
        }
        let flags = self.raw_creature_flags();
        let mut found = None;
        for class in OriginalClass::ALL {
            if flags & class.bit() != 0 {
                if found.is_some() {
                    // More than one MC_WAS_* bit ⇒ not a clean dual.
                    return None;
                }
                found = Some(class);
            }
        }
        found
    }

    /// Whether the creature is a fallen paladin or fallen ranger (has
    /// lost its class abilities), per the dedicated creature-flags bits.
    pub fn is_fallen(&self) -> bool {
        self.creature_flags()
            .intersects(CreatureFlags::FallenPaladin | CreatureFlags::FallenRanger)
    }

    /// Raw creature-flags dword (header offset 0x010), all bits, every
    /// version.
    fn raw_creature_flags(&self) -> u32 {
        match &self.header {
            CreHeader::V10(h) => h.creature_flags,
            CreHeader::V12(h) => h.creature_flags,
            CreHeader::V90(h) => h.creature_flags,
            CreHeader::V22(h) => h.creature_flags,
        }
    }

    /// The creature-flags dword (header offset 0x010) decoded into named
    /// bits. Bits not modelled by [`CreatureFlags`] — including the
    /// dual-class `MC_WAS_*` markers consumed by [`Self::is_dual_classed`]
    /// — are preserved in the returned value (via `from_bits_retain`),
    /// so reading, toggling one flag, and writing back never clobbers
    /// them.
    pub fn creature_flags(&self) -> CreatureFlags {
        CreatureFlags::from_bits_retain(self.raw_creature_flags())
    }

    /// Overwrite the creature-flags dword (header offset 0x010) on every
    /// header variant. The full bit pattern — including any unmodelled
    /// bits carried along in `flags` — is written verbatim. To flip a
    /// single flag without disturbing the rest, start from
    /// [`Self::creature_flags`]:
    ///
    /// ```ignore
    /// let mut f = cre.creature_flags();
    /// f.set(CreatureFlags::Exportable, true);
    /// cre.set_creature_flags(f);
    /// ```
    pub fn set_creature_flags(&mut self, flags: CreatureFlags) {
        let bits = flags.bits();
        match &mut self.header {
            CreHeader::V10(h) => h.creature_flags = bits,
            CreHeader::V12(h) => h.creature_flags = bits,
            CreHeader::V90(h) => h.creature_flags = bits,
            CreHeader::V22(h) => h.creature_flags = bits,
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
    /// The CRE creature-flags dword at header offset 0x010.
    ///
    /// Each named bit was confirmed empirically against EEKeeper-edited
    /// BG:EE saves (toggling exactly one "Miscellaneous" checkbox on the
    /// protagonist's embedded CRE). The dual-class `MC_WAS_*` original-
    /// class markers (0x08..=0x100) are deliberately **not** modelled
    /// here — they're consumed by [`Cre::is_dual_classed`] — and any
    /// other undefined bit is preserved via
    /// [`CreatureFlags::from_bits_retain`] so the dword round-trips
    /// byte-for-byte and a single-flag edit never disturbs the rest.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct CreatureFlags: u32 {
        /// bit 0: show the long name in the tooltip ("identified").
        const Identified = 1 << 0;
        /// bit 1: leave no corpse when the creature dies.
        const NoCorpse = 1 << 1;
        /// bit 2: keep the corpse permanently.
        const PermanentCorpse = 1 << 2;
        /// bit 11: the creature can be exported.
        const Exportable = 1 << 11;
        /// bit 15: the creature has been in the party.
        const BeenInParty = 1 << 15;
        /// bit 31 (Enhanced Edition): damage does not interrupt the
        /// creature's current action ("uninterruptible").
        const Uninterruptible = 1 << 31;

        /// bit 9: the creature is a fallen paladin. Not a "miscellaneous"
        /// checkbox (it's surfaced via [`Cre::is_fallen`]), so it is not
        /// in [`CreatureFlags::MISC`].
        const FallenPaladin = 1 << 9;
        /// bit 10: the creature is a fallen ranger. See
        /// [`Self::FallenPaladin`].
        const FallenRanger = 1 << 10;
    }
}

impl CreatureFlags {
    /// The user-facing "Miscellaneous" flags, in display order, each
    /// paired with a human-readable label. Lets a UI render the
    /// checkbox list without hardcoding any bit values — the bit
    /// semantics live here, on the resource type, not in the view.
    ///
    /// Deliberately excludes bits that are surfaced through dedicated
    /// accessors instead of a raw checkbox: the dual-class `MC_WAS_*`
    /// markers (see [`Cre::dual_class_original_class`]) and the
    /// fallen-paladin / fallen-ranger bits (see [`Cre::is_fallen`]).
    pub const MISC: &'static [(&'static str, CreatureFlags)] = &[
        ("Identified", CreatureFlags::Identified),
        ("No Corpse", CreatureFlags::NoCorpse),
        ("Permanent Corpse", CreatureFlags::PermanentCorpse),
        ("Exportable", CreatureFlags::Exportable),
        ("Been In Party", CreatureFlags::BeenInParty),
        ("Uninterruptible", CreatureFlags::Uninterruptible),
    ];
}

/// The class a dual-classed creature originally belonged to, recorded
/// by the single `MC_WAS_*` bit set in its [`CreatureFlags`] dword.
/// Mirrors GemRB `ie_stats.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginalClass {
    Fighter,
    Mage,
    Cleric,
    Thief,
    Druid,
    Ranger,
}

impl OriginalClass {
    /// The `CLASS.IDS`-style uppercase symbol for this class.
    pub fn symbol(self) -> &'static str {
        match self {
            OriginalClass::Fighter => "FIGHTER",
            OriginalClass::Mage => "MAGE",
            OriginalClass::Cleric => "CLERIC",
            OriginalClass::Thief => "THIEF",
            OriginalClass::Druid => "DRUID",
            OriginalClass::Ranger => "RANGER",
        }
    }

    /// The `MC_WAS_*` creature-flags bit that marks this original class.
    fn bit(self) -> u32 {
        match self {
            OriginalClass::Fighter => 0x0008,
            OriginalClass::Mage => 0x0010,
            OriginalClass::Cleric => 0x0020,
            OriginalClass::Thief => 0x0040,
            OriginalClass::Druid => 0x0080,
            OriginalClass::Ranger => 0x0100,
        }
    }

    /// All variants, in `MC_WAS_*` bit order.
    const ALL: [OriginalClass; 6] = [
        OriginalClass::Fighter,
        OriginalClass::Mage,
        OriginalClass::Cleric,
        OriginalClass::Thief,
        OriginalClass::Druid,
        OriginalClass::Ranger,
    ];
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

/// One V1 (48-byte) effect record, parsed into a typed variant —
/// mirroring [`EffectV2`].
///
/// `op233` gets the editable [`Proficiency`] representation; every
/// other opcode is a fully-parsed [`EffectV1Body`] (all 48 bytes
/// modelled, so it round-trips without keeping raw bytes). There is no
/// `LocalVariable` variant: the classic 48-byte record has no room for
/// the 32-byte variable name, so pre-EE games don't store creature
/// locals as effects. [`EffectV1::Raw`] only survives for records too
/// short to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectV1 {
    /// A record kept byte-for-byte — only for records shorter than the
    /// 48-byte V1 layout (it otherwise never occurs).
    Raw(Vec<u8>),
    /// A weapon-proficiency setter (`op233`), parsed into its editable
    /// [`proficiency`](Proficiency::proficiency) / [`points`](Proficiency::points).
    Proficiency(Proficiency),
    /// Any other effect, fully parsed into editable fields.
    Effect(EffectV1Body),
}

impl EffectV1 {
    /// Effect opcode that sets a weapon proficiency.
    pub const PROFICIENCY_OPCODE: u32 = EffectV2::PROFICIENCY_OPCODE;

    /// The effect opcode (word at record offset 0x00).
    pub fn opcode(&self) -> u32 {
        match self {
            EffectV1::Raw(r) => u32::from(u16::from_le_bytes([r[0], r[1]])),
            EffectV1::Proficiency(_) => Self::PROFICIENCY_OPCODE,
            EffectV1::Effect(e) => u32::from(e.opcode),
        }
    }

    /// The 48-byte on-disk record for this effect.
    pub(crate) fn to_record(&self) -> Vec<u8> {
        match self {
            EffectV1::Raw(r) => r.clone(),
            EffectV1::Proficiency(p) => p.to_v1_record(),
            EffectV1::Effect(e) => e.to_record(),
        }
    }
}

/// A fully-parsed classic (V1, 48-byte) effect record — every field is
/// modelled, so it is freely editable and round-trips without keeping
/// the original bytes.
///
/// Field offsets follow the classic feature-block layout (cf.
/// NearInfinity's `BaseOpcode`): opcode 0x00, target 0x02, power 0x03,
/// param1 0x04, param2 0x08, timing 0x0C, resistance 0x0D, duration
/// 0x0E, probabilities 0x12/0x13, resource 0x14, dice 0x1C/0x20, save
/// type/bonus 0x24/0x28, special 0x2C.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectV1Body {
    /// 0x00: effect opcode.
    pub opcode: u16,
    /// 0x02: target dispatcher.
    pub target: u8,
    /// 0x03: power.
    pub power: u8,
    /// 0x04: parameter 1.
    pub param1: u32,
    /// 0x08: parameter 2.
    pub param2: u32,
    /// 0x0C: timing mode.
    pub timing_mode: u8,
    /// 0x0D: dispel / resistance.
    pub resistance: u8,
    /// 0x0E: duration.
    pub duration: u32,
    /// 0x12: probability (upper).
    pub probability1: u8,
    /// 0x13: probability (lower).
    pub probability2: u8,
    /// 0x14: resource (8-byte resref).
    pub resource: String,
    /// 0x1C: dice thrown / max level.
    pub dice_thrown: u32,
    /// 0x20: dice sides / min level.
    pub dice_sides: u32,
    /// 0x24: saving-throw type.
    pub save_type: u32,
    /// 0x28: saving-throw bonus.
    pub save_bonus: u32,
    /// 0x2C: special.
    pub special: u32,
}

impl EffectV1Body {
    /// Parse a 48-byte classic effect record into typed fields.
    /// `record` must be at least 48 bytes.
    pub(crate) fn from_record(r: &[u8]) -> Self {
        EffectV1Body {
            opcode: rd_u16(r, 0x00),
            target: r[0x02],
            power: r[0x03],
            param1: rd_u32(r, 0x04),
            param2: rd_u32(r, 0x08),
            timing_mode: r[0x0C],
            resistance: r[0x0D],
            duration: rd_u32(r, 0x0E),
            probability1: r[0x12],
            probability2: r[0x13],
            resource: rd_resref(r, 0x14),
            dice_thrown: rd_u32(r, 0x1C),
            dice_sides: rd_u32(r, 0x20),
            save_type: rd_u32(r, 0x24),
            save_bonus: rd_u32(r, 0x28),
            special: rd_u32(r, 0x2C),
        }
    }

    /// Serialise back to a 48-byte classic effect record.
    pub(crate) fn to_record(&self) -> Vec<u8> {
        let mut r = vec![0u8; 48];
        wr_u16(&mut r, 0x00, self.opcode);
        r[0x02] = self.target;
        r[0x03] = self.power;
        wr_u32(&mut r, 0x04, self.param1);
        wr_u32(&mut r, 0x08, self.param2);
        r[0x0C] = self.timing_mode;
        r[0x0D] = self.resistance;
        wr_u32(&mut r, 0x0E, self.duration);
        r[0x12] = self.probability1;
        r[0x13] = self.probability2;
        wr_resref(&mut r, 0x14, &self.resource);
        wr_u32(&mut r, 0x1C, self.dice_thrown);
        wr_u32(&mut r, 0x20, self.dice_sides);
        wr_u32(&mut r, 0x24, self.save_type);
        wr_u32(&mut r, 0x28, self.save_bonus);
        wr_u32(&mut r, 0x2C, self.special);
        r
    }
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

/// Weapon-proficiency points for one weapon stat, split by class slot.
///
/// The Infinity Engine packs both slots into a single byte: the
/// first-class points in bits 0–2 and the second-class (dual/multi-class)
/// points in bits 3–5. Both fields are `0..=7` (in practice `0..=5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeaponProficiency {
    pub first_class: u8,
    pub second_class: u8,
}

impl WeaponProficiency {
    /// Unpack the engine's packed proficiency byte.
    pub fn from_packed(byte: u8) -> Self {
        Self {
            first_class: byte & 0x07,
            second_class: (byte >> 3) & 0x07,
        }
    }

    /// Repack into the engine's byte; each slot is clamped to `0..=7`.
    pub fn to_packed(self) -> u8 {
        (self.first_class & 0x07) | ((self.second_class & 0x07) << 3)
    }

    /// `true` when both slots are zero.
    pub fn is_empty(self) -> bool {
        self.first_class == 0 && self.second_class == 0
    }
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

    /// `param1` (points) / `param2` (proficiency stat) offsets in the
    /// classic 48-byte (V1) record.
    const V1_POINTS_OFFSET: usize = 0x04;
    const V1_PROFICIENCY_OFFSET: usize = 0x08;

    /// Parse a classic (V1) `op233` record into its proficiency /
    /// points. Caller must have verified the opcode is
    /// [`EffectV1::PROFICIENCY_OPCODE`]; `record` must be ≥ 48 bytes.
    pub(crate) fn from_v1_record(record: &[u8]) -> Self {
        Proficiency {
            points: rd_u32(record, Self::V1_POINTS_OFFSET),
            proficiency: rd_u32(record, Self::V1_PROFICIENCY_OFFSET),
        }
    }

    /// Serialise to a classic 48-byte `op233` record. No V1 `op233`
    /// reference save was available, so the non-value fields are a
    /// constructed-but-valid template: opcode `233` (0x00), "while
    /// equipped" timing (0x0C = 2), 100% probability (0x12).
    pub(crate) fn to_v1_record(self) -> Vec<u8> {
        let mut r = vec![0u8; 48];
        wr_u16(&mut r, 0x00, EffectV1::PROFICIENCY_OPCODE as u16);
        r[0x0C] = 2;
        r[0x12] = 100;
        wr_u32(&mut r, Self::V1_POINTS_OFFSET, self.points);
        wr_u32(&mut r, Self::V1_PROFICIENCY_OFFSET, self.proficiency);
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

    /// The CRE header's weapon proficiency for an `IE_PROFICIENCY*` stat,
    /// oriented into first/second-class points (see [`Self::proficiency`]
    /// for the dual-class orientation).
    ///
    /// Only the V1.0 header carries this packed block (20 bytes covering
    /// stats `89..=108`); other versions model proficiencies differently
    /// (PST / IWD V9.0 use older single-category slots, IWD2 is d20), so
    /// this returns an empty proficiency there.
    pub fn header_proficiency(&self, stat: u8) -> WeaponProficiency {
        self.orient_proficiency(stat, self.header_proficiency_byte(stat))
    }

    /// The weapon proficiency an `op233` ("set proficiency") effect sets
    /// for a stat. Save-game party members store their proficiencies here
    /// (with a zeroed header); empty when no effect targets the stat.
    pub fn effect_proficiency(&self, stat: u8) -> WeaponProficiency {
        self.orient_proficiency(stat, self.effect_proficiency_byte(stat))
    }

    /// The effective weapon proficiency the engine resolves for a stat:
    /// the header base with any `op233` effect applied on top (real saves
    /// store a proficiency in the header XOR in an effect, so the larger
    /// packed byte matches the engine for both layouts).
    ///
    /// The packed byte's two 3-bit slots map to first/second class the
    /// way EEKeeper shows them: for a **dual-classed** character's weapon
    /// the slots are swapped (the engine keeps the active class in the
    /// low slot and the suspended original — EEKeeper's "First Class" —
    /// in the high slot); single/multi-class characters and fighting
    /// styles (stats `111..=114`) are not swapped.
    pub fn proficiency(&self, stat: u8) -> WeaponProficiency {
        let byte = self
            .header_proficiency_byte(stat)
            .max(self.effect_proficiency_byte(stat));
        self.orient_proficiency(stat, byte)
    }

    /// Set the weapon proficiency for an `IE_PROFICIENCY*` stat, matching
    /// EEKeeper: edits go to an `op233` ("set proficiency") effect when
    /// the creature already stores proficiencies as effects (every
    /// save-game party member does) — updating the existing effect for
    /// the stat or **creating** one. Only a standalone CRE that has no
    /// proficiency effects is edited in its V1.0 header. (Writing the
    /// header for a party member is the classic "proficiencies vanish on
    /// save" bug — the effect overrides it.)
    pub fn set_proficiency(&mut self, stat: u8, value: WeaponProficiency) {
        let packed = self.pack_proficiency(stat, value);
        if self.has_proficiency_effects() {
            self.set_or_add_effect_proficiency(stat, packed);
        } else if let CreHeader::V10(h) = &mut self.header
            && let Some(byte) = v10_proficiency_byte_mut(h, stat)
        {
            *byte = packed;
        }
    }

    /// Whether the packed byte's first/second-class slots are swapped for
    /// `stat`: yes for a dual-classed character's weapons, no for single/
    /// multi-class characters or fighting styles (`111..=114`).
    fn proficiency_slots_swapped(&self, stat: u8) -> bool {
        const FIRST_STYLE: u8 = 111;
        const LAST_STYLE: u8 = 114;
        let is_style = (FIRST_STYLE..=LAST_STYLE).contains(&stat);
        self.is_dual_classed() && !is_style
    }

    /// Orient a packed byte into first/second-class points for `stat`.
    fn orient_proficiency(&self, stat: u8, byte: u8) -> WeaponProficiency {
        let low = byte & 0x07;
        let high = (byte >> 3) & 0x07;
        if self.proficiency_slots_swapped(stat) {
            WeaponProficiency {
                first_class: high,
                second_class: low,
            }
        } else {
            WeaponProficiency {
                first_class: low,
                second_class: high,
            }
        }
    }

    /// Pack first/second-class points back into the engine's byte for
    /// `stat`, inverting [`Self::orient_proficiency`].
    fn pack_proficiency(&self, stat: u8, value: WeaponProficiency) -> u8 {
        let (low, high) = if self.proficiency_slots_swapped(stat) {
            (value.second_class, value.first_class)
        } else {
            (value.first_class, value.second_class)
        };
        (low & 0x07) | ((high & 0x07) << 3)
    }

    /// Raw packed proficiency byte from the V1.0 header (0 elsewhere).
    fn header_proficiency_byte(&self, stat: u8) -> u8 {
        match &self.header {
            CreHeader::V10(h) => v10_proficiency_byte(h, stat),
            _ => 0,
        }
    }

    /// Highest packed proficiency byte set by `op233` effects for `stat`.
    fn effect_proficiency_byte(&self, stat: u8) -> u8 {
        let mut best = 0u8;
        // `param2`'s low word is the stat; the high word is the EE
        // increment flag, which set-mode saves leave at 0.
        self.for_each_proficiency_effect(|prof, points| {
            if prof & 0xFFFF == u32::from(stat) {
                best = best.max((points & 0xFF) as u8);
            }
        });
        best
    }

    /// Whether this creature stores proficiencies as `op233` effects.
    fn has_proficiency_effects(&self) -> bool {
        let mut any = false;
        self.for_each_proficiency_effect(|_, _| any = true);
        any
    }

    fn for_each_proficiency_effect(&self, mut f: impl FnMut(u32, u32)) {
        let effects = match &self.sub_sections {
            SubSections::V1(s) => &s.effects,
            SubSections::V22(s) => &s.effects,
        };
        match effects {
            EffectList::V2(list) => {
                for e in list {
                    if let EffectV2::Proficiency(p) = e {
                        f(p.proficiency, p.points);
                    }
                }
            }
            EffectList::V1(list) => {
                for e in list {
                    if let EffectV1::Proficiency(p) = e {
                        f(p.proficiency, p.points);
                    }
                }
            }
        }
    }

    /// Set the packed `points` of the `op233` effect targeting `stat`,
    /// creating the effect when none exists.
    fn set_or_add_effect_proficiency(&mut self, stat: u8, packed: u8) {
        let new = Proficiency {
            proficiency: u32::from(stat),
            points: u32::from(packed),
        };
        let effects = match &mut self.sub_sections {
            SubSections::V1(s) => &mut s.effects,
            SubSections::V22(s) => &mut s.effects,
        };
        match effects {
            EffectList::V2(list) => {
                let mut found = false;
                for e in list.iter_mut() {
                    if let EffectV2::Proficiency(p) = e
                        && p.proficiency & 0xFFFF == u32::from(stat)
                    {
                        p.points = u32::from(packed);
                        found = true;
                    }
                }
                if !found {
                    list.push(EffectV2::Proficiency(new));
                }
            }
            EffectList::V1(list) => {
                let mut found = false;
                for e in list.iter_mut() {
                    if let EffectV1::Proficiency(p) = e
                        && p.proficiency & 0xFFFF == u32::from(stat)
                    {
                        p.points = u32::from(packed);
                        found = true;
                    }
                }
                if !found {
                    list.push(EffectV1::Proficiency(new));
                }
            }
        }
    }
}

/// `IE_PROFICIENCYBASTARDSWORD` — first stat in the packed V1.0 block.
const PROF_STAT_BASE: u8 = 89;
/// Number of packed proficiency bytes in the V1.0 header (stats 89..=108).
const PROF_STAT_COUNT: u8 = 20;

/// Index into the V1.0 header proficiency block for a stat, if it falls
/// within the packed range.
fn v10_proficiency_index(stat: u8) -> Option<usize> {
    stat.checked_sub(PROF_STAT_BASE)
        .filter(|&i| i < PROF_STAT_COUNT)
        .map(usize::from)
}

/// Read the packed proficiency byte for `stat` from a V1.0 header.
fn v10_proficiency_byte(h: &CreHeaderV10, stat: u8) -> u8 {
    let packed = [
        h.bg1_large_swords_proficiency_other_games,
        h.bg1_small_swords_proficiency,
        h.bg1_bows_proficiency,
        h.bg1_spears_proficiency,
        h.bg1_blunt_proficiency,
        h.bg1_spiked_proficiency,
        h.bg1_axe_proficiency,
        h.bg1_missile_proficiency,
        h.unused_proficiency_proficiencies_are_packed_into,
        h.unused_proficiency_proficiencies_are_packed_into_2,
        h.unused_proficiency_proficiencies_are_packed_into_3,
        h.unused_proficiency_proficiencies_are_packed_into_4,
        h.unused_proficiency_proficiencies_are_packed_into_5,
        h.bg1,
        h.bg1_2,
        h.bg1_3,
        h.bg1_4,
        h.bg1_5,
        h.bg1_6,
        h.bg1_7,
    ];
    v10_proficiency_index(stat).map_or(0, |i| packed[i])
}

/// Mutable reference to the packed proficiency byte for `stat` in a V1.0
/// header, or `None` for stats outside the packed range.
fn v10_proficiency_byte_mut(h: &mut CreHeaderV10, stat: u8) -> Option<&mut u8> {
    Some(match v10_proficiency_index(stat)? {
        0 => &mut h.bg1_large_swords_proficiency_other_games,
        1 => &mut h.bg1_small_swords_proficiency,
        2 => &mut h.bg1_bows_proficiency,
        3 => &mut h.bg1_spears_proficiency,
        4 => &mut h.bg1_blunt_proficiency,
        5 => &mut h.bg1_spiked_proficiency,
        6 => &mut h.bg1_axe_proficiency,
        7 => &mut h.bg1_missile_proficiency,
        8 => &mut h.unused_proficiency_proficiencies_are_packed_into,
        9 => &mut h.unused_proficiency_proficiencies_are_packed_into_2,
        10 => &mut h.unused_proficiency_proficiencies_are_packed_into_3,
        11 => &mut h.unused_proficiency_proficiencies_are_packed_into_4,
        12 => &mut h.unused_proficiency_proficiencies_are_packed_into_5,
        13 => &mut h.bg1,
        14 => &mut h.bg1_2,
        15 => &mut h.bg1_3,
        16 => &mut h.bg1_4,
        17 => &mut h.bg1_5,
        18 => &mut h.bg1_6,
        _ => &mut h.bg1_7,
    })
}

// ── Keeper-facing per-version field accessors ───────────────────────
//
// Friendly getters/setters the save editor needs, with the
// per-version field-name dispatch living here on `Cre` so every
// consumer — and the unit tests below — share one definition of the
// mapping instead of re-deriving it. Reads return `Option<…>` (getter)
// / are no-ops (setter) when a field doesn't exist on the given CRE
// version, so callers can hide/skip the row.

/// Getter + setter for a `u8` field present (under the same name) on
/// every CRE version.
macro_rules! cre_u8_field {
    ($get:ident, $set:ident, $field:ident) => {
        pub fn $get(&self) -> u8 {
            match &self.header {
                CreHeader::V10(h) => h.$field,
                CreHeader::V12(h) => h.$field,
                CreHeader::V90(h) => h.$field,
                CreHeader::V22(h) => h.$field,
            }
        }
        pub fn $set(&mut self, value: u8) {
            match &mut self.header {
                CreHeader::V10(h) => h.$field = value,
                CreHeader::V12(h) => h.$field = value,
                CreHeader::V90(h) => h.$field = value,
                CreHeader::V22(h) => h.$field = value,
            }
        }
    };
}

/// Getter + setter for an AD&D thief-skill `u8` (V1.0 / V1.2 / V9.0);
/// `None` (read) / no-op (write) on V2.2 (IWD2), which uses d20 skills.
macro_rules! cre_adnd_skill {
    ($get:ident, $set:ident, $field:ident) => {
        pub fn $get(&self) -> Option<u8> {
            match &self.header {
                CreHeader::V10(h) => Some(h.$field),
                CreHeader::V12(h) => Some(h.$field),
                CreHeader::V90(h) => Some(h.$field),
                CreHeader::V22(_) => None,
            }
        }
        pub fn $set(&mut self, value: u8) {
            match &mut self.header {
                CreHeader::V10(h) => h.$field = value,
                CreHeader::V12(h) => h.$field = value,
                CreHeader::V90(h) => h.$field = value,
                CreHeader::V22(_) => {}
            }
        }
    };
}

impl Cre {
    /// Set current hit points (getter: [`Self::current_hit_points`]).
    pub fn set_current_hit_points(&mut self, value: u16) {
        match &mut self.header {
            CreHeader::V10(h) => h.current_hit_points = value,
            CreHeader::V12(h) => h.current_hit_points = value,
            CreHeader::V90(h) => h.current_hit_points = value,
            CreHeader::V22(h) => h.current_hit_points = value,
        }
    }

    /// Set maximum hit points (getter: [`Self::maximum_hit_points`]).
    pub fn set_maximum_hit_points(&mut self, value: u16) {
        match &mut self.header {
            CreHeader::V10(h) => h.maximum_hit_points = value,
            CreHeader::V12(h) => h.maximum_hit_points = value,
            CreHeader::V90(h) => h.maximum_hit_points = value,
            CreHeader::V22(h) => h.maximum_hit_points = value,
        }
    }

    /// Armour Class (natural) on AD&D engines; the single Armour Class
    /// on IWD2 (V2.2).
    pub fn ac_natural(&self) -> i16 {
        match &self.header {
            CreHeader::V10(h) => h.armor_class_natural,
            CreHeader::V12(h) => h.armor_class_natural,
            CreHeader::V90(h) => h.armor_class_natural,
            CreHeader::V22(h) => h.armor_class,
        }
    }
    pub fn set_ac_natural(&mut self, value: i16) {
        match &mut self.header {
            CreHeader::V10(h) => h.armor_class_natural = value,
            CreHeader::V12(h) => h.armor_class_natural = value,
            CreHeader::V90(h) => h.armor_class_natural = value,
            CreHeader::V22(h) => h.armor_class = value,
        }
    }

    /// Armour Class (effective) — AD&D headers only; `None` on IWD2.
    pub fn ac_effective(&self) -> Option<i16> {
        match &self.header {
            CreHeader::V10(h) => Some(h.armor_class_effective),
            CreHeader::V12(h) => Some(h.armor_class_effective),
            CreHeader::V90(h) => Some(h.armor_class_effective),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_ac_effective(&mut self, value: i16) {
        match &mut self.header {
            CreHeader::V10(h) => h.armor_class_effective = value,
            CreHeader::V12(h) => h.armor_class_effective = value,
            CreHeader::V90(h) => h.armor_class_effective = value,
            CreHeader::V22(_) => {}
        }
    }

    /// THAC0 on AD&D; Base Attack Bonus on IWD2 (shared signed byte).
    pub fn thac0_or_bab(&self) -> i8 {
        match &self.header {
            CreHeader::V10(h) => h.thac0,
            CreHeader::V12(h) => h.thac0,
            CreHeader::V90(h) => h.thac0,
            CreHeader::V22(h) => h.base_attack_bonus_bab_for_non,
        }
    }
    pub fn set_thac0_or_bab(&mut self, value: i8) {
        match &mut self.header {
            CreHeader::V10(h) => h.thac0 = value,
            CreHeader::V12(h) => h.thac0 = value,
            CreHeader::V90(h) => h.thac0 = value,
            CreHeader::V22(h) => h.base_attack_bonus_bab_for_non = value,
        }
    }

    /// Number-of-attacks byte (see [`NumberOfAttacks`] for the encoding).
    pub fn attacks_byte(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.number_of_attacks.to_u8(),
            CreHeader::V12(h) => h.number_of_attacks.to_u8(),
            CreHeader::V90(h) => h.number_of_attacks.to_u8(),
            CreHeader::V22(h) => h.number_of_attacks.to_u8(),
        }
    }
    pub fn set_attacks_byte(&mut self, value: u8) {
        let n = NumberOfAttacks::from_u8(value);
        match &mut self.header {
            CreHeader::V10(h) => h.number_of_attacks = n,
            CreHeader::V12(h) => h.number_of_attacks = n,
            CreHeader::V90(h) => h.number_of_attacks = n,
            CreHeader::V22(h) => h.number_of_attacks = n,
        }
    }

    cre_u8_field!(fatigue, set_fatigue, fatigue);
    cre_u8_field!(intoxication, set_intoxication, intoxication);
    cre_u8_field!(luck, set_luck, luck);

    /// Character XP — stored in the summoning-power-level field for
    /// party CREs (what the editor surfaces as "Experience").
    pub fn experience(&self) -> u32 {
        match &self.header {
            CreHeader::V10(h) => h.creature_power_level_for_summoning_spells,
            CreHeader::V12(h) => h.creature_power_level_for_summoning_spells,
            CreHeader::V90(h) => h.creature_power_level_for_summoning_spells,
            CreHeader::V22(h) => h.creature_power_level_for_summoning_spells,
        }
    }
    pub fn set_experience(&mut self, value: u32) {
        match &mut self.header {
            CreHeader::V10(h) => h.creature_power_level_for_summoning_spells = value,
            CreHeader::V12(h) => h.creature_power_level_for_summoning_spells = value,
            CreHeader::V90(h) => h.creature_power_level_for_summoning_spells = value,
            CreHeader::V22(h) => h.creature_power_level_for_summoning_spells = value,
        }
    }

    /// XP awarded for killing this creature.
    pub fn xp_for_kill(&self) -> u32 {
        match &self.header {
            CreHeader::V10(h) => h.xp_gained_for_killing_this_creature,
            CreHeader::V12(h) => h.xp_gained_for_killing_this_creature,
            CreHeader::V90(h) => h.xp_gained_for_killing_this_creature,
            CreHeader::V22(h) => h.xp_gained_for_killing_this_creature,
        }
    }
    pub fn set_xp_for_kill(&mut self, value: u32) {
        match &mut self.header {
            CreHeader::V10(h) => h.xp_gained_for_killing_this_creature = value,
            CreHeader::V12(h) => h.xp_gained_for_killing_this_creature = value,
            CreHeader::V90(h) => h.xp_gained_for_killing_this_creature = value,
            CreHeader::V22(h) => h.xp_gained_for_killing_this_creature = value,
        }
    }

    /// Level in the first class. `None` on IWD2 (per-class fields).
    pub fn level_first_class(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.level_first_class_highest_attained_level),
            CreHeader::V12(h) => Some(h.highest_attained_level_in_class),
            CreHeader::V90(h) => Some(h.highest_attained_level_in_class),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_level_first_class(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.level_first_class_highest_attained_level = value,
            CreHeader::V12(h) => h.highest_attained_level_in_class = value,
            CreHeader::V90(h) => h.highest_attained_level_in_class = value,
            CreHeader::V22(_) => {}
        }
    }
    pub fn level_second_class(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.level_second_class_highest_attained_level),
            CreHeader::V12(h) => Some(h.highest_attained_level_in_class_2),
            CreHeader::V90(h) => Some(h.highest_attained_level_in_class_2),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_level_second_class(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.level_second_class_highest_attained_level = value,
            CreHeader::V12(h) => h.highest_attained_level_in_class_2 = value,
            CreHeader::V90(h) => h.highest_attained_level_in_class_2 = value,
            CreHeader::V22(_) => {}
        }
    }
    pub fn level_third_class(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.level_third_class_highest_attained_level),
            CreHeader::V12(h) => Some(h.highest_attained_level_in_class_3),
            CreHeader::V90(h) => Some(h.highest_attained_level_in_class_3),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_level_third_class(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.level_third_class_highest_attained_level = value,
            CreHeader::V12(h) => h.highest_attained_level_in_class_3 = value,
            CreHeader::V90(h) => h.highest_attained_level_in_class_3 = value,
            CreHeader::V22(_) => {}
        }
    }

    /// The three class-level fields as a triplet (`0` = unused slot).
    /// `[0; 3]` on IWD2, which has no AD&D multi-class HP rules.
    pub fn class_levels(&self) -> [u8; 3] {
        match &self.header {
            CreHeader::V10(h) => [
                h.level_first_class_highest_attained_level,
                h.level_second_class_highest_attained_level,
                h.level_third_class_highest_attained_level,
            ],
            CreHeader::V12(h) => [
                h.highest_attained_level_in_class,
                h.highest_attained_level_in_class_2,
                h.highest_attained_level_in_class_3,
            ],
            CreHeader::V90(h) => [
                h.highest_attained_level_in_class,
                h.highest_attained_level_in_class_2,
                h.highest_attained_level_in_class_3,
            ],
            CreHeader::V22(_) => [0, 0, 0],
        }
    }

    /// Highest level among the character's classes — used for
    /// level-scaled derived values. IWD2 (V2.2) reports the engine's
    /// summed `total_levels`.
    pub fn primary_level(&self) -> u8 {
        match &self.header {
            CreHeader::V22(h) => h.total_levels,
            _ => {
                let [a, b, c] = self.class_levels();
                a.max(b).max(c)
            }
        }
    }

    /// Small-portrait resource name (BMP), trailing NULs trimmed.
    pub fn small_portrait_name(&self) -> &str {
        match &self.header {
            CreHeader::V10(h) => &h.small_portrait_bmp,
            CreHeader::V12(h) => &h.small_portrait_bmp,
            CreHeader::V90(h) => &h.small_portrait,
            CreHeader::V22(h) => &h.small_portrait_bmp,
        }
    }

    /// Large-portrait resource name (a BMP everywhere except PSTEE's
    /// V1.0, which stores a BAM here).
    pub fn large_portrait_name(&self) -> &str {
        match &self.header {
            CreHeader::V10(h) => &h.large_portrait_pstee_bam_other_games,
            CreHeader::V12(h) => &h.large_portrait_bmp,
            CreHeader::V90(h) => &h.large_portrait,
            CreHeader::V22(h) => &h.large_portrait_bmp,
        }
    }

    /// Morale (AD&D only; `None` on IWD2, which uses saving throws).
    pub fn morale(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.morale_default_value_is_10_capped),
            CreHeader::V12(h) => Some(h.morale),
            CreHeader::V90(h) => Some(h.morale),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_morale(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.morale_default_value_is_10_capped = value,
            CreHeader::V12(h) => h.morale = value,
            CreHeader::V90(h) => h.morale = value,
            CreHeader::V22(_) => {}
        }
    }
    pub fn morale_break(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.morale_break_see_here_for_further),
            CreHeader::V12(h) => Some(h.morale_break),
            CreHeader::V90(h) => Some(h.morale_break),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_morale_break(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.morale_break_see_here_for_further = value,
            CreHeader::V12(h) => h.morale_break = value,
            CreHeader::V90(h) => h.morale_break = value,
            CreHeader::V22(_) => {}
        }
    }
    pub fn morale_recovery(&self) -> Option<u16> {
        match &self.header {
            CreHeader::V10(h) => Some(h.morale_recovery_time_see_here_for),
            CreHeader::V12(h) => Some(h.morale_recovery_time),
            CreHeader::V90(h) => Some(h.morale_recovery_time),
            CreHeader::V22(_) => None,
        }
    }
    pub fn set_morale_recovery(&mut self, value: u16) {
        match &mut self.header {
            CreHeader::V10(h) => h.morale_recovery_time_see_here_for = value,
            CreHeader::V12(h) => h.morale_recovery_time = value,
            CreHeader::V90(h) => h.morale_recovery_time = value,
            CreHeader::V22(_) => {}
        }
    }

    /// "Hide in Shadows" (V1.0 / V9.0 / V2.2). `None` on V1.2, which
    /// folds hide + silent into one unified stealth field.
    pub fn hide_in_shadows(&self) -> Option<u8> {
        match &self.header {
            CreHeader::V10(h) => Some(h.hide_in_shadows_base),
            CreHeader::V12(_) => None,
            CreHeader::V90(h) => Some(h.hide_in_shadows_base),
            CreHeader::V22(h) => Some(h.hide_in_shadows_base),
        }
    }
    pub fn set_hide_in_shadows(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.hide_in_shadows_base = value,
            CreHeader::V12(_) => {}
            CreHeader::V90(h) => h.hide_in_shadows_base = value,
            CreHeader::V22(h) => h.hide_in_shadows_base = value,
        }
    }

    /// "Move Silently" (V1.0 / V2.2) or the unified "Stealth" value
    /// (V1.2 / V9.0, stored in `stealth`). See
    /// [`Self::move_silently_label`] for the row's display name.
    pub fn move_silently(&self) -> u8 {
        match &self.header {
            CreHeader::V10(h) => h.move_silently,
            CreHeader::V12(h) => h.stealth,
            CreHeader::V90(h) => h.stealth,
            CreHeader::V22(h) => h.move_silently,
        }
    }
    pub fn set_move_silently(&mut self, value: u8) {
        match &mut self.header {
            CreHeader::V10(h) => h.move_silently = value,
            CreHeader::V12(h) => h.stealth = value,
            CreHeader::V90(h) => h.stealth = value,
            CreHeader::V22(h) => h.move_silently = value,
        }
    }
    /// UI label for the move-silently row: "Stealth" on V1.2 / V9.0
    /// (which store a unified value), else "Move Silently".
    pub fn move_silently_label(&self) -> &'static str {
        match &self.header {
            CreHeader::V12(_) | CreHeader::V90(_) => "Stealth",
            _ => "Move Silently",
        }
    }

    cre_adnd_skill!(lockpicking, set_lockpicking, lockpicking);
    cre_adnd_skill!(find_traps, set_find_traps, find_disarm_traps);
    cre_adnd_skill!(set_traps, set_set_traps, set_traps);
    cre_adnd_skill!(pick_pockets, set_pick_pockets, pick_pockets);
    cre_adnd_skill!(detect_illusion, set_detect_illusion, detect_illusion);
    cre_adnd_skill!(lore, set_lore, lore);
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

    use infinitier_common::Game;
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
        CreImporter {
            name: rel_path,
            game: Game::Bgee,
        }
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

    // ── Weapon-proficiency resolution + editing ──────────────────────

    // IE_PROFICIENCY stat numbers used below (GemRB `ie_stats.h`).
    const BASTARD_SWORD: u8 = 89;
    const SHORT_SWORD: u8 = 91;
    const AXE: u8 = 92;
    const DAGGER: u8 = 96;
    const SLING: u8 = 107;
    const SINGLE_WEAPON_STYLE: u8 = 113;
    const CLUB: u8 = 115;

    /// The raw packed `points` of the `op233` effect targeting `stat`,
    /// if any — used to assert byte-for-byte equality with EEKeeper.
    fn op233_points(cre: &Cre, stat: u8) -> Option<u32> {
        let effects = match &cre.sub_sections {
            SubSections::V1(s) => &s.effects,
            SubSections::V22(s) => &s.effects,
        };
        let mut found = None;
        match effects {
            EffectList::V2(l) => {
                for e in l {
                    if let EffectV2::Proficiency(p) = e
                        && p.proficiency & 0xFFFF == u32::from(stat)
                    {
                        found = Some(p.points);
                    }
                }
            }
            EffectList::V1(l) => {
                for e in l {
                    if let EffectV1::Proficiency(p) = e
                        && p.proficiency & 0xFFFF == u32::from(stat)
                    {
                        found = Some(p.points);
                    }
                }
            }
        }
        found
    }

    fn prof(first_class: u8, second_class: u8) -> WeaponProficiency {
        WeaponProficiency {
            first_class,
            second_class,
        }
    }

    fn round_trip(cre: &Cre) -> Cre {
        use infinitier_common::Game;
        use infinitier_datasource::{DataSource, Importer};
        let mut bytes = Vec::new();
        CreExporter.export(cre, &mut bytes).unwrap();
        CreImporter {
            name: "round-trip",
            game: Game::Bgee,
        }
        .import(&DataSource::new(bytes))
        .unwrap()
    }

    #[test]
    fn weapon_proficiency_packs_and_unpacks() {
        assert_eq!(WeaponProficiency::from_packed(9), prof(1, 1)); // 0b001001
        assert_eq!(WeaponProficiency::from_packed(10), prof(2, 1)); // 0b001010
        assert_eq!(prof(3, 2).to_packed(), 19); // 0b010011
        for b in 0u8..64 {
            assert_eq!(WeaponProficiency::from_packed(b).to_packed(), b);
        }
    }

    /// A dual-class save-game member (Imoen, Thief→Mage) carries her
    /// proficiencies as packed `op233` effects with a zeroed header.
    /// For her **weapons** the two slots are swapped vs single-class
    /// (the engine keeps the active class low, the suspended original —
    /// EEKeeper's "First Class" — high), so e.g. Sling `10` reads `1/2`,
    /// not `2/1`. Fighting **styles** (113) are not swapped.
    #[test]
    fn dual_class_proficiency_resolves_from_op233_effects() {
        let cre = crate::test_support::import_fixture("v1_0/IMOEN_DUAL.cre");
        assert!(cre.is_dual_classed());
        assert_eq!(cre.header_proficiency(SHORT_SWORD), prof(0, 0)); // header empty
        assert_eq!(cre.effect_proficiency(SHORT_SWORD), prof(1, 1)); // packed 9
        assert_eq!(cre.proficiency(SHORT_SWORD), prof(1, 1));
        assert_eq!(cre.proficiency(SLING), prof(1, 2)); // packed 10 → swapped
        assert_eq!(cre.proficiency(DAGGER), prof(0, 1)); // packed 1 → swapped
        assert_eq!(cre.proficiency(SINGLE_WEAPON_STYLE), prof(1, 0)); // style: not swapped
        assert_eq!(cre.proficiency(AXE), prof(0, 0)); // none
    }

    /// Editing a proficiency stored as an `op233` effect updates that
    /// effect (EEKeeper's behaviour), survives export/import, and leaves
    /// the header zero — so it doesn't "vanish on save".
    #[test]
    fn set_proficiency_updates_effect_and_round_trips() {
        let mut cre = crate::test_support::import_fixture("v1_0/IMOEN_DUAL.cre");
        cre.set_proficiency(SHORT_SWORD, prof(3, 2));
        assert_eq!(cre.effect_proficiency(SHORT_SWORD), prof(3, 2));
        assert_eq!(cre.header_proficiency(SHORT_SWORD), prof(0, 0)); // header untouched

        let rt = round_trip(&cre);
        assert_eq!(rt.effect_proficiency(SHORT_SWORD), prof(3, 2));
        assert_eq!(rt.proficiency(SHORT_SWORD), prof(3, 2));
    }

    /// On an effect-based creature, setting a stat with no existing
    /// effect **creates** one (not a header write), matching EEKeeper.
    #[test]
    fn set_proficiency_creates_effect_when_effect_based() {
        let mut cre = crate::test_support::import_fixture("v1_0/IMOEN_DUAL.cre");
        assert_eq!(op233_points(&cre, BASTARD_SWORD), None);
        cre.set_proficiency(BASTARD_SWORD, prof(1, 1));
        assert!(
            op233_points(&cre, BASTARD_SWORD).is_some(),
            "effect created"
        );
        assert_eq!(cre.header_proficiency(BASTARD_SWORD), prof(0, 0)); // header untouched
        assert_eq!(round_trip(&cre).proficiency(BASTARD_SWORD), prof(1, 1));
    }

    /// A standalone CRE with no proficiency effects is edited in its
    /// header (single-class IRONGU: header `89=2`, `92=1`, `95=1`).
    #[test]
    fn set_proficiency_writes_header_when_no_effects() {
        let mut cre = crate::test_support::import_fixture("v1_0/IRONGU.cre");
        assert!(!cre.is_dual_classed());
        assert_eq!(cre.header_proficiency(BASTARD_SWORD), prof(2, 0)); // packed 2
        cre.set_proficiency(BASTARD_SWORD, prof(4, 1)); // packed 12
        assert_eq!(cre.header_proficiency(BASTARD_SWORD), prof(4, 1));
        assert_eq!(op233_points(&cre, BASTARD_SWORD), None); // no effect created
        assert_eq!(round_trip(&cre).proficiency(BASTARD_SWORD), prof(4, 1));
    }

    /// Reproduces the exact `op233` bytes EEKeeper writes for the four
    /// reference edits (Xor single-class; Imoen dual-class), proving our
    /// setter is byte-for-byte equal to EEKeeper.
    #[test]
    fn set_proficiency_matches_eekeeper_edits() {
        // Xor — single-class fighter: +1 Bastard Sword First → op233 1.
        let mut xor = crate::test_support::import_fixture("v1_0/FIGHTER_HARBASE.cre");
        assert!(!xor.is_dual_classed());
        xor.set_proficiency(BASTARD_SWORD, prof(1, 0));
        assert_eq!(op233_points(&xor, BASTARD_SWORD), Some(1));

        // Imoen — dual-class: +1 Bastard Sword First → op233 8 (high slot).
        let mut imoen = crate::test_support::import_fixture("v1_0/IMOEN_DUAL.cre");
        imoen.set_proficiency(BASTARD_SWORD, prof(1, 0));
        assert_eq!(op233_points(&imoen, BASTARD_SWORD), Some(8));
        // …then +1 Second → op233 9.
        imoen.set_proficiency(BASTARD_SWORD, prof(1, 1));
        assert_eq!(op233_points(&imoen, BASTARD_SWORD), Some(9));
        // +2 Club in both classes → op233 18.
        imoen.set_proficiency(CLUB, prof(2, 2));
        assert_eq!(op233_points(&imoen, CLUB), Some(18));
    }

    /// The IWD2 (V2.2) exporter recomputes its section table too: growing
    /// the effect list shifts every downstream offset, and re-import must
    /// still find the added record — proving no offset is stored/reused.
    #[test]
    fn v22_layout_recomputes_after_adding_an_effect() {
        let mut cre = crate::test_support::import_fixture("v2_2/52SERSA.cre");
        let SubSections::V22(s) = &mut cre.sub_sections else {
            panic!("expected V2.2 sub-sections");
        };
        let before = s.effects.len();
        let added = Proficiency {
            proficiency: 89,
            points: 1,
        };
        match &mut s.effects {
            EffectList::V2(l) => l.push(EffectV2::Proficiency(added)),
            EffectList::V1(l) => l.push(EffectV1::Proficiency(added)),
        }
        let rt = round_trip(&cre);
        let SubSections::V22(s) = &rt.sub_sections else {
            panic!("expected V2.2 sub-sections");
        };
        assert_eq!(s.effects.len(), before + 1);
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

    /// Same byte-exact coverage proof for the classic 48-byte
    /// [`EffectV1Body`].
    #[test]
    fn effect_v1_full_round_trip_is_byte_exact() {
        let original: Vec<u8> = (0..48u32)
            .map(|i| 0x20 + (i % 0x5E) as u8) // printable ASCII, no NUL
            .collect();
        let body = EffectV1Body::from_record(&original);
        assert_eq!(
            body.to_record(),
            original,
            "EffectV1Body must cover every byte"
        );
    }

    #[test]
    fn proficiency_v1_round_trips_and_is_op233() {
        let original = Proficiency {
            proficiency: 89,
            points: 2,
        };
        let record = original.to_v1_record();
        assert_eq!(record.len(), 48);
        assert_eq!(
            u16::from_le_bytes([record[0x00], record[0x01]]),
            EffectV1::PROFICIENCY_OPCODE as u16,
        );
        assert_eq!(Proficiency::from_v1_record(&record), original);
    }

    #[test]
    fn effect_v1_opcode_for_variants() {
        let prof = EffectV1::Proficiency(Proficiency {
            proficiency: 89,
            points: 2,
        });
        assert_eq!(prof.opcode(), EffectV1::PROFICIENCY_OPCODE);
        assert_eq!(prof.to_record().len(), 48);

        let raw = EffectV1::Raw(vec![233, 0, 0, 0]);
        assert_eq!(raw.opcode(), 233);
    }

    /// Force the V1.0 creature-flags field to `flags` so dual-class
    /// detection can be exercised independently of the fixture's data.
    fn with_flags(cre: &mut Cre, flags: u32) {
        match &mut cre.header {
            CreHeader::V10(h) => h.creature_flags = flags,
            _ => panic!("fixture is not a V1.0 CRE"),
        }
    }

    #[test]
    fn is_dual_classed_requires_exactly_one_mc_was_bit() {
        let mut cre = crate::test_support::import_fixture("v1_0/THIEF3.cre");

        // No former-class bits → single- or multi-class, never dual.
        with_flags(&mut cre, 0x0000);
        assert!(!cre.is_dual_classed());

        // Exactly one MC_WAS_* bit → dual. Check a couple of the bits.
        with_flags(&mut cre, 0x0008); // MC_WAS_FIGHTER
        assert!(cre.is_dual_classed());
        with_flags(&mut cre, 0x0100); // MC_WAS_RANGER (top of the mask)
        assert!(cre.is_dual_classed());

        // Two former-class bits (kuo-toa-style garbage, or a true multi
        // marker) → not a dual.
        with_flags(&mut cre, 0x0008 | 0x0040); // FIGHTER | THIEF
        assert!(!cre.is_dual_classed());

        // Bits outside the MC_WAS mask are ignored: "show longname"
        // (0x01) alone is not dual, but combined with one MC_WAS bit it
        // still is.
        with_flags(&mut cre, 0x0001);
        assert!(!cre.is_dual_classed());
        with_flags(&mut cre, 0x0001 | 0x0010); // show-longname | MC_WAS_MAGE
        assert!(cre.is_dual_classed());
    }

    /// The named creature-flag bits, confirmed empirically against
    /// EEKeeper-edited BG:EE saves (each toggling exactly one flag on
    /// the protagonist's embedded CRE). Guards against the historical
    /// mix-up where `Uninterruptible` was mapped to bit 0 — which is
    /// actually `Identified`.
    #[test]
    fn creature_flag_bits_match_reference_saves() {
        assert_eq!(CreatureFlags::Identified.bits(), 0x0000_0001);
        assert_eq!(CreatureFlags::NoCorpse.bits(), 0x0000_0002);
        assert_eq!(CreatureFlags::PermanentCorpse.bits(), 0x0000_0004);
        assert_eq!(CreatureFlags::Exportable.bits(), 0x0000_0800);
        assert_eq!(CreatureFlags::BeenInParty.bits(), 0x0000_8000);
        assert_eq!(CreatureFlags::Uninterruptible.bits(), 0x8000_0000);
        // The curated "Miscellaneous" list excludes fallen / dual-class
        // markers and the non-persisted EE flags.
        let labels: Vec<&str> = CreatureFlags::MISC.iter().map(|(l, _)| *l).collect();
        assert_eq!(
            labels,
            [
                "Identified",
                "No Corpse",
                "Permanent Corpse",
                "Exportable",
                "Been In Party",
                "Uninterruptible",
            ]
        );
        assert!(
            !labels
                .iter()
                .any(|l| l.contains("Nightmare") || l.contains("Tooltip"))
        );
    }

    #[test]
    fn creature_flags_get_set_round_trips_and_preserves_unmodelled_bits() {
        let mut cre = crate::test_support::import_fixture("v1_0/THIEF3.cre");
        // An MC_WAS_* dual marker (0x40 = THIEF) plus a named flag.
        with_flags(&mut cre, 0x0040 | CreatureFlags::Exportable.bits());

        let mut flags = cre.creature_flags();
        assert!(flags.contains(CreatureFlags::Exportable));
        // Toggle a modelled flag without disturbing the unmodelled 0x40.
        flags.insert(CreatureFlags::BeenInParty);
        flags.remove(CreatureFlags::Exportable);
        cre.set_creature_flags(flags);

        assert!(cre.creature_flags().contains(CreatureFlags::BeenInParty));
        assert!(!cre.creature_flags().contains(CreatureFlags::Exportable));
        // The MC_WAS_THIEF bit rode along untouched, so dual-class
        // detection still sees it.
        assert_eq!(cre.dual_class_original_class(), Some(OriginalClass::Thief));
    }

    #[test]
    fn dual_class_original_class_names_the_former_class() {
        let mut cre = crate::test_support::import_fixture("v1_0/THIEF3.cre");
        with_flags(&mut cre, 0x0010); // MC_WAS_MAGE
        assert_eq!(cre.dual_class_original_class(), Some(OriginalClass::Mage));
        assert_eq!(cre.dual_class_original_class().unwrap().symbol(), "MAGE");
        with_flags(&mut cre, 0x0000);
        assert_eq!(cre.dual_class_original_class(), None);
        with_flags(&mut cre, 0x0008 | 0x0040); // two markers ⇒ ambiguous
        assert_eq!(cre.dual_class_original_class(), None);
    }

    #[test]
    fn is_fallen_reads_paladin_and_ranger_bits() {
        let mut cre = crate::test_support::import_fixture("v1_0/THIEF3.cre");
        with_flags(&mut cre, 0x0000);
        assert!(!cre.is_fallen());
        with_flags(&mut cre, CreatureFlags::FallenPaladin.bits());
        assert!(cre.is_fallen());
        with_flags(&mut cre, CreatureFlags::FallenRanger.bits());
        assert!(cre.is_fallen());
    }

    #[test]
    fn is_dual_classed_is_always_false_for_iwd2() {
        // V2.2 (IWD2) has no dual-classing and reuses the low flag bits,
        // so detection must short-circuit to false even with an
        // MC_WAS-shaped bit pattern.
        let mut cre = crate::test_support::import_fixture("v2_2/52SERSA.cre");
        if let CreHeader::V22(h) = &mut cre.header {
            h.creature_flags = 0x0008;
        } else {
            panic!("fixture is not a V2.2 CRE");
        }
        assert!(!cre.is_dual_classed());
    }

    #[test]
    fn cre_field_accessors_round_trip_on_v10() {
        let mut cre = crate::test_support::import_fixture("v1_0/THIEF3.cre");

        cre.set_current_hit_points(45);
        assert_eq!(cre.current_hit_points(), 45);
        cre.set_maximum_hit_points(123);
        assert_eq!(cre.maximum_hit_points(), 123);
        cre.set_ac_natural(-3);
        assert_eq!(cre.ac_natural(), -3);
        cre.set_ac_effective(-5);
        assert_eq!(cre.ac_effective(), Some(-5));
        cre.set_thac0_or_bab(7);
        assert_eq!(cre.thac0_or_bab(), 7);
        cre.set_attacks_byte(5);
        assert_eq!(cre.attacks_byte(), 5);

        cre.set_fatigue(9);
        assert_eq!(cre.fatigue(), 9);
        cre.set_intoxication(3);
        assert_eq!(cre.intoxication(), 3);
        cre.set_luck(4);
        assert_eq!(cre.luck(), 4);

        cre.set_experience(50_000);
        assert_eq!(cre.experience(), 50_000);
        cre.set_xp_for_kill(64);
        assert_eq!(cre.xp_for_kill(), 64);

        cre.set_level_first_class(9);
        cre.set_level_second_class(7);
        cre.set_level_third_class(0);
        assert_eq!(cre.level_first_class(), Some(9));
        assert_eq!(cre.level_second_class(), Some(7));
        assert_eq!(cre.level_third_class(), Some(0));
        assert_eq!(cre.class_levels(), [9, 7, 0]);
        assert_eq!(cre.primary_level(), 9); // max(9, 7, 0)

        cre.set_morale(10);
        cre.set_morale_break(7);
        cre.set_morale_recovery(60);
        assert_eq!(cre.morale(), Some(10));
        assert_eq!(cre.morale_break(), Some(7));
        assert_eq!(cre.morale_recovery(), Some(60));

        cre.set_lore(40);
        cre.set_lockpicking(55);
        cre.set_find_traps(33);
        cre.set_set_traps(20);
        cre.set_pick_pockets(11);
        cre.set_detect_illusion(2);
        cre.set_hide_in_shadows(66);
        cre.set_move_silently(77);
        assert_eq!(cre.lore(), Some(40));
        assert_eq!(cre.lockpicking(), Some(55));
        assert_eq!(cre.find_traps(), Some(33));
        assert_eq!(cre.set_traps(), Some(20));
        assert_eq!(cre.pick_pockets(), Some(11));
        assert_eq!(cre.detect_illusion(), Some(2));
        assert_eq!(cre.hide_in_shadows(), Some(66));
        assert_eq!(cre.move_silently(), 77);
        assert_eq!(cre.move_silently_label(), "Move Silently");

        // Portrait names resolve to the version-correct field.
        let _ = cre.small_portrait_name();
        let _ = cre.large_portrait_name();
    }

    #[test]
    fn cre_field_accessors_are_version_aware() {
        // V2.2 (IWD2): AD&D-only rows are absent.
        let iwd2 = crate::test_support::import_fixture("v2_2/52SERSA.cre");
        assert_eq!(iwd2.ac_effective(), None);
        assert_eq!(iwd2.morale(), None);
        assert_eq!(iwd2.morale_break(), None);
        assert_eq!(iwd2.morale_recovery(), None);
        assert_eq!(iwd2.lore(), None);
        assert_eq!(iwd2.lockpicking(), None);
        assert_eq!(iwd2.level_first_class(), None);
        assert_eq!(iwd2.class_levels(), [0, 0, 0]);
        // V2.2 reports its summed total level.
        if let CreHeader::V22(h) = &iwd2.header {
            assert_eq!(iwd2.primary_level(), h.total_levels);
        } else {
            panic!("fixture is not V2.2");
        }

        // V1.2 folds hide-in-shadows into the unified stealth field.
        let pst = crate::test_support::import_fixture("v1_2/THIEF3.cre");
        assert_eq!(pst.hide_in_shadows(), None);
        assert_eq!(pst.move_silently_label(), "Stealth");

        // V9.0 also labels the stealth row "Stealth".
        let iwd = crate::test_support::import_fixture("v9_0/BARBWAR2.cre");
        assert_eq!(iwd.move_silently_label(), "Stealth");
    }

    #[test]
    fn cre_setters_are_noops_for_absent_fields() {
        let mut iwd2 = crate::test_support::import_fixture("v2_2/52SERSA.cre");
        iwd2.set_ac_effective(-9);
        iwd2.set_morale(5);
        iwd2.set_lore(5);
        iwd2.set_level_first_class(5);
        assert_eq!(iwd2.ac_effective(), None);
        assert_eq!(iwd2.morale(), None);
        assert_eq!(iwd2.lore(), None);
        assert_eq!(iwd2.level_first_class(), None);

        // V1.2 has no hide-in-shadows field — the setter is a no-op.
        let mut pst = crate::test_support::import_fixture("v1_2/THIEF3.cre");
        pst.set_hide_in_shadows(50);
        assert_eq!(pst.hide_in_shadows(), None);
    }
}
