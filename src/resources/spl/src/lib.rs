#![doc = include_str!("../readme.md")]
//!
//! ## On-disk layout (per version)
//!
//! Both versions share the same first 114 bytes (`0x00..0x72`). V2.0
//! adds a 16-byte trailer (`0x72..0x82`) holding two duration
//! modifiers plus 14 bytes of "unknown" padding the engine reserves.
//! After the header come variable-length sub-section bodies
//! referenced by the offsets / counts stored inside it.
//!
//! ```text
//!  0x00  4   'SPL '
//!  0x04  4   'V1  ' or 'V2.0'
//!  0x08  4   spell-name strref (unidentified)
//!  ...        (38 documented primitive fields)
//!  0x64  4   abilities offset
//!  0x68  2   abilities count
//!  0x6A  4   effects offset
//!  0x6E  2   "casting" feature-block index (into the effects array)
//!  0x70  2   "casting" feature-block count
//! [V2 only]
//!  0x72  1   duration modifier — rounds per caster level
//!  0x73  1   duration modifier — base
//!  0x74  14  unknown
//! ```
//!
//! Abilities (a.k.a. "Extended Headers") are 40 B each, effects
//! ("Feature Blocks") are 48 B each. Both have identical on-disk
//! layouts across V1 and V2.

mod exporter;
mod importer;

pub use exporter::SplExporter;
pub use importer::SplImporter;

/// 4-byte signature at offset 0 of every SPL file. Note the
/// trailing space — every IE signature is exactly 4 bytes.
pub const SPL_SIGNATURE: &[u8; 4] = b"SPL ";

/// Total fixed-header length per version (sub-section bodies start
/// at or after this offset).
pub(crate) const HEADER_LEN_V1: usize = 0x72;
pub(crate) const HEADER_LEN_V2: usize = 0x82;

/// On-disk record size — see [`SplAbility`] / [`SplEffect`].
pub(crate) const ABILITY_LEN: usize = 40;
pub(crate) const EFFECT_LEN: usize = 48;

/// Known on-disk version tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplVersion {
    /// `V1  ` — BG, BG2, BG:EE, BG2:EE, IWD, IWD:EE, PST, PST:EE,
    /// EET. 114-byte header.
    V1,
    /// `V2.0` — IWD2. 130-byte header (V1 + 16-byte trailer).
    V2_0,
}

impl SplVersion {
    /// 4-byte tag stored at offset 0x04 of the file.
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            SplVersion::V1 => b"V1  ",
            SplVersion::V2_0 => b"V2.0",
        }
    }

    /// Total length of the fixed-width header for this version.
    pub fn header_len(&self) -> usize {
        match self {
            SplVersion::V1 => HEADER_LEN_V1,
            SplVersion::V2_0 => HEADER_LEN_V2,
        }
    }
}

/// A parsed SPL file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spl {
    /// On-disk version tag.
    pub version: SplVersion,
    /// Typed per-version header — every IESDP-documented primitive
    /// field is surfaced as a struct field.
    pub header: SplHeader,
    /// Extended-header records ("abilities"). 40 bytes each on
    /// disk. Each entry's `(first_effect_index, num_effects)` pair
    /// addresses a window of [`Self::effects`].
    pub abilities: Vec<SplAbility>,
    /// Flat list of feature-block records ("effects"). 48 bytes
    /// each. The "casting" feature blocks pointed at by
    /// [`SplHeader::casting_feature_offset`] /
    /// [`SplHeader::casting_feature_count`] share this same vector.
    pub effects: Vec<SplEffect>,
}

/// Per-version typed header. We use a discriminated enum so the
/// parser / writer can dispatch statically on the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplHeader {
    V1(SplHeaderV1),
    V2(SplHeaderV2),
}

impl SplHeader {
    pub fn version(&self) -> SplVersion {
        match self {
            SplHeader::V1(_) => SplVersion::V1,
            SplHeader::V2(_) => SplVersion::V2_0,
        }
    }

    // The handful of fields downstream consumers actually need are
    // all in the shared portion. Expose them as cross-version
    // helpers so callers don't have to match on the variant.

    /// Strref of the spell's unidentified name.
    pub fn name_strref(&self) -> u32 {
        match self {
            SplHeader::V1(h) => h.name_unidentified,
            SplHeader::V2(h) => h.name_unidentified,
        }
    }
    /// Strref of the spell's identified name (usually `0xFFFFFFFF`).
    pub fn name_identified_strref(&self) -> u32 {
        match self {
            SplHeader::V1(h) => h.name_identified,
            SplHeader::V2(h) => h.name_identified,
        }
    }
    /// Strref of the spell's unidentified description.
    pub fn description_strref(&self) -> u32 {
        match self {
            SplHeader::V1(h) => h.description_unidentified,
            SplHeader::V2(h) => h.description_unidentified,
        }
    }
    /// Strref of the spell's identified description (usually `0xFFFFFFFF`).
    pub fn description_identified_strref(&self) -> u32 {
        match self {
            SplHeader::V1(h) => h.description_identified,
            SplHeader::V2(h) => h.description_identified,
        }
    }
    /// BAM resref for the spellbook icon (same offset in every version).
    pub fn spellbook_icon(&self) -> &str {
        match self {
            SplHeader::V1(h) => &h.spellbook_icon,
            SplHeader::V2(h) => &h.spellbook_icon,
        }
    }
    /// Spell-school level (1..=9 in standard rule sets; spells with
    /// no level use 0).
    pub fn spell_level(&self) -> u32 {
        match self {
            SplHeader::V1(h) => h.spell_level,
            SplHeader::V2(h) => h.spell_level,
        }
    }
    /// Spell type — `0` Special, `1` Wizard, `2` Priest, `3`
    /// Psionic, `4` Innate, `5` Bard song.
    pub fn spell_type(&self) -> u16 {
        match self {
            SplHeader::V1(h) => h.spell_type,
            SplHeader::V2(h) => h.spell_type,
        }
    }
    /// Index into [`Spl::effects`] of the first "casting"
    /// feature-block (effects emitted *while the caster is reading
    /// the spell*, before the projectile is in the air).
    pub fn casting_feature_offset(&self) -> u16 {
        match self {
            SplHeader::V1(h) => h.casting_feature_offset,
            SplHeader::V2(h) => h.casting_feature_offset,
        }
    }
    /// Number of casting feature-blocks.
    pub fn casting_feature_count(&self) -> u16 {
        match self {
            SplHeader::V1(h) => h.casting_feature_count,
            SplHeader::V2(h) => h.casting_feature_count,
        }
    }
}

/// SPL V1 header — 114 bytes on disk. Field offsets are documented
/// in IESDP `spl_v1.htm` and verified against NearInfinity's
/// `SplResource.read`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplHeaderV1 {
    /// 0x08: strref for the unidentified spell name.
    pub name_unidentified: u32,
    /// 0x0C: strref for the identified spell name (usually
    /// `0xFFFFFFFF`).
    pub name_identified: u32,
    /// 0x10: 8-byte ASCIIZ WAV resref played at the end of the
    /// casting animation.
    pub completion_sound: String,
    /// 0x18: spell flags bitfield (see IESDP `Header_Flags`).
    pub flags: u32,
    /// 0x1C: spell type — `0` Special, `1` Wizard, `2` Priest,
    /// `3` Psionic, `4` Innate, `5` Bard song.
    pub spell_type: u16,
    /// 0x1E: exclusion flags (which other spells this one suppresses).
    pub exclusion_flags: u32,
    /// 0x22: casting-graphics enum.
    pub casting_graphics: u16,
    /// 0x24: minimum caster level (unused on most engines).
    pub min_level: u8,
    /// 0x25: primary type (spell school index into `school.2da` /
    /// `mschool.2da`).
    pub primary_type: u8,
    /// 0x26: minimum Strength (unused for spells).
    pub min_strength: u8,
    /// 0x27: secondary type (sub-school index into `msectype.2da`).
    pub secondary_type: u8,
    /// 0x28: minimum Strength bonus (unused).
    pub min_strength_bonus: u8,
    /// 0x29: usability flags 1 (unused).
    pub usability1: u8,
    /// 0x2A: minimum Intelligence (unused).
    pub min_intelligence: u8,
    /// 0x2B: usability flags 2 (unused).
    pub usability2: u8,
    /// 0x2C: minimum Dexterity (unused).
    pub min_dexterity: u8,
    /// 0x2D: usability flags 3 (unused).
    pub usability3: u8,
    /// 0x2E: minimum Wisdom (unused).
    pub min_wisdom: u8,
    /// 0x2F: usability flags 4 (unused).
    pub usability4: u8,
    /// 0x30: minimum Constitution (unused).
    pub min_constitution: u16,
    /// 0x32: minimum Charisma (unused).
    pub min_charisma: u16,
    /// 0x34: spell level (1..=9 for standard rules).
    pub spell_level: u32,
    /// 0x38: stack amount (unused for spells).
    pub stack_amount: u16,
    /// 0x3A: 8-byte ASCIIZ BAM resref for the spellbook icon.
    pub spellbook_icon: String,
    /// 0x42: Lore-to-ID skill required (unused).
    pub lore_to_id: u16,
    /// 0x44: 8-byte ASCIIZ BAM resref for the ground-icon (unused).
    pub ground_icon: String,
    /// 0x4C: weight (unused).
    pub weight: u32,
    /// 0x50: strref for the unidentified description.
    pub description_unidentified: u32,
    /// 0x54: strref for the identified description (usually
    /// `0xFFFFFFFF`).
    pub description_identified: u32,
    /// 0x58: 8-byte ASCIIZ BAM resref for the description icon.
    pub description_icon: String,
    /// 0x60: enchantment level (unused for spells).
    pub enchantment: u32,
    // 0x64 abilities offset, 0x68 count, 0x6A effects offset are file
    // layout — recomputed by the exporter, not stored.
    /// 0x6E: index into the effects array of the first "casting"
    /// feature-block.
    pub casting_feature_offset: u16,
    /// 0x70: number of casting feature-blocks.
    pub casting_feature_count: u16,
}

/// SPL V2.0 header — 130 bytes on disk. Strict superset of V1: same
/// 114 bytes plus two duration-modifier bytes and a 14-byte
/// reserved trailer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplHeaderV2 {
    /// V1-shared fields (mirror of [`SplHeaderV1`]).
    pub name_unidentified: u32,
    pub name_identified: u32,
    pub completion_sound: String,
    pub flags: u32,
    pub spell_type: u16,
    pub exclusion_flags: u32,
    pub casting_graphics: u16,
    pub min_level: u8,
    pub primary_type: u8,
    pub min_strength: u8,
    pub secondary_type: u8,
    pub min_strength_bonus: u8,
    pub usability1: u8,
    pub min_intelligence: u8,
    pub usability2: u8,
    pub min_dexterity: u8,
    pub usability3: u8,
    pub min_wisdom: u8,
    pub usability4: u8,
    pub min_constitution: u16,
    pub min_charisma: u16,
    pub spell_level: u32,
    pub stack_amount: u16,
    pub spellbook_icon: String,
    pub lore_to_id: u16,
    pub ground_icon: String,
    pub weight: u32,
    pub description_unidentified: u32,
    pub description_identified: u32,
    pub description_icon: String,
    pub enchantment: u32,
    // 0x64/0x68/0x6A section table recomputed by the exporter, not stored.
    pub casting_feature_offset: u16,
    pub casting_feature_count: u16,
    /// 0x72 (V2 only): duration modifier — rounds per caster level.
    pub duration_modifier_per_level: u8,
    /// 0x73 (V2 only): duration modifier — base.
    pub duration_modifier_base: u8,
    /// 0x74..0x82 (V2 only): 14 bytes reserved by the engine.
    pub trailing_unknown: Vec<u8>,
}

/// One ability ("Extended Header"). 40 bytes on disk, layout
/// identical between V1 and V2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplAbility {
    /// 0x00: spell-form — `1` standard, `2` projectile.
    pub spell_form: u8,
    /// 0x01: misc flags (PST uses bit 2 for "friendly").
    pub misc_flags: u8,
    /// 0x02: usable-location — `0` none, `2` spell book, `4`
    /// innate, …
    pub location: u16,
    /// 0x04: 8-byte ASCIIZ BAM resref for the memorised icon.
    pub memorised_icon: String,
    /// 0x0C: target type.
    pub target: u8,
    /// 0x0D: target count (for area / multi-target spells).
    pub target_count: u8,
    /// 0x0E: targeting range (in feet).
    pub range: u16,
    /// 0x10: caster-level threshold required for this ability.
    pub level_required: u16,
    /// 0x12: casting time in tenths of a round.
    pub casting_time: u16,
    /// 0x14: times-per-day cap.
    pub times_per_day: u16,
    /// 0x16: damage-dice sides (unused for spells).
    pub dice_sides: u16,
    /// 0x18: damage-dice count (unused for spells).
    pub dice_thrown: u16,
    /// 0x1A: enchanted level (unused).
    pub enchanted: u16,
    /// 0x1C: damage type (unused).
    pub damage_type: u16,
    /// 0x1E: number of feature blocks this ability owns.
    pub num_effects: u16,
    /// 0x20: index of the first feature-block (relative to
    /// [`Spl::effects`]).
    pub first_effect_index: u16,
    /// 0x22: charges (unused).
    pub charges: u16,
    /// 0x24: charge-depletion behaviour (unused).
    pub charge_depletion: u16,
    /// 0x26: projectile index into `projectl.ids` / `missile.ids`.
    pub projectile: u16,
}

/// One feature block ("Effect"). 48 bytes on disk, layout identical
/// between V1 and V2 (the V2 SPL doesn't use the 264-byte CRE V2
/// effect record).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplEffect {
    /// 0x00: opcode number (effects.ids).
    pub opcode: u16,
    /// 0x02: target type.
    pub target_type: u8,
    /// 0x03: power level (used by dispel-magic etc).
    pub power: u8,
    /// 0x04: opcode-specific parameter 1.
    pub parameter1: u32,
    /// 0x08: opcode-specific parameter 2.
    pub parameter2: u32,
    /// 0x0C: timing mode (0 = duration, 1 = permanent, …).
    pub timing_mode: u8,
    /// 0x0D: resistance flags (dispel / magic resistance gating).
    pub resistance: u8,
    /// 0x0E: duration in seconds (for duration timing modes).
    pub duration: u32,
    /// 0x12: probability 1.
    pub probability1: u8,
    /// 0x13: probability 2.
    pub probability2: u8,
    /// 0x14: 8-byte ASCIIZ resref of the resource the opcode applies.
    pub resource: String,
    /// 0x1C: dice thrown (or max level for some opcodes).
    pub dice_thrown: u32,
    /// 0x20: dice sides (or min level for some opcodes).
    pub dice_sides: u32,
    /// 0x24: saving-throw type bitfield.
    pub save_type: u32,
    /// 0x28: saving-throw bonus (signed).
    pub save_bonus: i32,
    /// 0x2C: trailing 4-byte field — TobEx labels this "stacking
    /// ID", IWD2 SPL V2 labels it "Unknown".
    pub trailing_extra: u32,
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::{Spl, SplImporter};

    /// Recursively collect every `.spl` file under `assets/spl/`.
    pub fn all_spl_fixtures() -> Vec<PathBuf> {
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.eq_ignore_ascii_case("spl"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        visit(&get_assets_path().join("spl"), &mut out);
        out.sort();
        out
    }

    /// Imports a fixture path relative to `assets/spl/`.
    pub fn import_fixture(rel_path: &str) -> Spl {
        let path = get_assets_path().join("spl").join(rel_path);
        SplImporter { name: rel_path }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("import {rel_path}: {e}"))
    }
}
