#![doc = include_str!("../readme.md")]
//!
//! ## On-disk layout (per version)
//!
//! Every ITM file opens with the 8-byte file prefix
//! `<ITM_SIGNATURE><version.as_bytes()>` followed by a fixed-width
//! header. The header carries every primitive field (name strrefs,
//! type / flags / usability bitmasks, stat requirements, price /
//! stack / weight, icon resrefs, description strrefs / icon,
//! enchantment) plus a small **section table** — two `(offset,
//! count)` pairs pointing at the variable-length ability
//! ("extended header") and effect ("feature block") bodies.
//!
//! | Version | Header size | Section-table base | Per-version extras |
//! |---------|-------------|--------------------|--------------------|
//! | `V1  `  | 0x72 (114 B) | 0x0064 (extended-header offset) | — |
//! | `V1.1`  | 0x9A (154 B) | 0x0064            | Dialog + Conversable label + Paperdoll colour + 6 reserved dwords (PST) |
//! | `V2.0`  | 0x82 (130 B) | 0x0064            | 16-byte trailing reserved blob; extended-header `flags` split into two `u16`s |

mod exporter;
mod importer;

pub use exporter::ItmExporter;
pub use importer::ItmImporter;

/// 4-byte file signature — present at offset 0 of every ITM file.
/// Note the trailing space.
pub const ITM_SIGNATURE: &[u8; 4] = b"ITM ";

/// Fixed header lengths per version.
pub(crate) const HEADER_LEN_V1: usize = 0x72;
pub(crate) const HEADER_LEN_V1_1: usize = 0x9A;
pub(crate) const HEADER_LEN_V2: usize = 0x82;

/// On-disk record sizes — same across every ITM version.
pub(crate) const ABILITY_LEN: usize = 56;
pub(crate) const EFFECT_LEN: usize = 48;

/// Known on-disk version tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItmVersion {
    /// `V1  ` — BG, BG2, BG:EE, BG2:EE, IWD, IWD:EE, PST:EE, EET.
    /// 114-byte header.
    V1,
    /// `V1.1` — PST vanilla. 154-byte header with PST-only Dialog,
    /// Conversable-label, Paperdoll-colour and reserved fields.
    V1_1,
    /// `V2.0` — IWD2. 130-byte header with a 16-byte reserved
    /// trailer.
    V2_0,
}

impl ItmVersion {
    /// 4-byte tag stored at offset 0x04 of the file.
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            ItmVersion::V1 => b"V1  ",
            ItmVersion::V1_1 => b"V1.1",
            ItmVersion::V2_0 => b"V2.0",
        }
    }

    /// Total length of the fixed-width header for this version.
    pub fn header_len(&self) -> usize {
        match self {
            ItmVersion::V1 => HEADER_LEN_V1,
            ItmVersion::V1_1 => HEADER_LEN_V1_1,
            ItmVersion::V2_0 => HEADER_LEN_V2,
        }
    }
}

/// A parsed ITM file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Itm {
    /// On-disk version tag.
    pub version: ItmVersion,
    /// Typed per-version header.
    pub header: ItmHeader,
    /// Extended-header records ("abilities") — 56 bytes each, same
    /// byte layout in every version. Each entry's
    /// `(first_effect_index, num_effects)` pair addresses a window
    /// of [`Self::effects`].
    pub abilities: Vec<ItmAbility>,
    /// Flat list of feature-block records ("effects"). 48 bytes
    /// each. The "equipping" feature blocks pointed at by
    /// [`ItmHeader::equipping_feature_offset`] /
    /// [`ItmHeader::equipping_feature_count`] share this same
    /// vector.
    pub effects: Vec<ItmEffect>,
}

/// Per-version typed header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItmHeader {
    V1(ItmHeaderV1),
    V1_1(Box<ItmHeaderV1_1>),
    V2(ItmHeaderV2),
}

impl ItmHeader {
    pub fn version(&self) -> ItmVersion {
        match self {
            ItmHeader::V1(_) => ItmVersion::V1,
            ItmHeader::V1_1(_) => ItmVersion::V1_1,
            ItmHeader::V2(_) => ItmVersion::V2_0,
        }
    }

    // Cross-version accessors for the shared 0x08..0x72 prefix so
    // callers don't have to match on the variant for every common
    // field.

    pub fn name_strref(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.name_unidentified,
            ItmHeader::V1_1(h) => h.name_unidentified,
            ItmHeader::V2(h) => h.name_unidentified,
        }
    }
    pub fn name_identified_strref(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.name_identified,
            ItmHeader::V1_1(h) => h.name_identified,
            ItmHeader::V2(h) => h.name_identified,
        }
    }
    pub fn description_strref(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.description_unidentified,
            ItmHeader::V1_1(h) => h.description_unidentified,
            ItmHeader::V2(h) => h.description_unidentified,
        }
    }
    pub fn price(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.price,
            ItmHeader::V1_1(h) => h.price,
            ItmHeader::V2(h) => h.price,
        }
    }
    pub fn weight(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.weight,
            ItmHeader::V1_1(h) => h.weight,
            ItmHeader::V2(h) => h.weight,
        }
    }
    pub fn enchantment(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.enchantment,
            ItmHeader::V1_1(h) => h.enchantment,
            ItmHeader::V2(h) => h.enchantment,
        }
    }
    pub fn extended_headers_offset(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.extended_headers_offset,
            ItmHeader::V1_1(h) => h.extended_headers_offset,
            ItmHeader::V2(h) => h.extended_headers_offset,
        }
    }
    pub fn extended_headers_count(&self) -> u16 {
        match self {
            ItmHeader::V1(h) => h.extended_headers_count,
            ItmHeader::V1_1(h) => h.extended_headers_count,
            ItmHeader::V2(h) => h.extended_headers_count,
        }
    }
    pub fn feature_blocks_offset(&self) -> u32 {
        match self {
            ItmHeader::V1(h) => h.feature_blocks_offset,
            ItmHeader::V1_1(h) => h.feature_blocks_offset,
            ItmHeader::V2(h) => h.feature_blocks_offset,
        }
    }
    pub fn equipping_feature_offset(&self) -> u16 {
        match self {
            ItmHeader::V1(h) => h.equipping_feature_offset,
            ItmHeader::V1_1(h) => h.equipping_feature_offset,
            ItmHeader::V2(h) => h.equipping_feature_offset,
        }
    }
    pub fn equipping_feature_count(&self) -> u16 {
        match self {
            ItmHeader::V1(h) => h.equipping_feature_count,
            ItmHeader::V1_1(h) => h.equipping_feature_count,
            ItmHeader::V2(h) => h.equipping_feature_count,
        }
    }
}

/// V1 (and EE-V1) header — 114 bytes on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItmHeaderV1 {
    /// 0x08: strref for the unidentified item name.
    pub name_unidentified: u32,
    /// 0x0C: strref for the identified item name.
    pub name_identified: u32,
    /// 0x10: 8-byte ASCIIZ replacement-item resref (BG / IWD) /
    /// drop-sound resref (PSTEE).
    pub replacement_item: String,
    /// 0x18: item flags bitfield.
    pub flags: u32,
    /// 0x1C: item type (sword / staff / shield / armour / …).
    pub item_type: u16,
    /// 0x1E..0x22: 4-byte usability bitmask (per IESDP
    /// `Header_Usability`).
    pub usability: [u8; 4],
    /// 0x22..0x24: 2-byte "Item animation" string (e.g. `S1`, `BW`)
    /// indexing into the engine's animation tables.
    pub item_animation: [u8; 2],
    /// 0x24: minimum caster level.
    pub min_level: u16,
    /// 0x26: minimum Strength.
    pub min_strength: u16,
    /// 0x28: minimum Strength bonus (the 18/01-style mod).
    pub min_strength_bonus: u8,
    /// 0x29: kit usability bitmask 1/4.
    pub kit_usability_1: u8,
    /// 0x2A: minimum Intelligence.
    pub min_intelligence: u8,
    /// 0x2B: kit usability bitmask 2/4.
    pub kit_usability_2: u8,
    /// 0x2C: minimum Dexterity.
    pub min_dexterity: u8,
    /// 0x2D: kit usability bitmask 3/4.
    pub kit_usability_3: u8,
    /// 0x2E: minimum Wisdom.
    pub min_wisdom: u8,
    /// 0x2F: kit usability bitmask 4/4.
    pub kit_usability_4: u8,
    /// 0x30: minimum Constitution.
    pub min_constitution: u8,
    /// 0x31: weapon proficiency (IESDP `Header_Proficiency`).
    pub weapon_proficiency: u8,
    /// 0x32: minimum Charisma.
    pub min_charisma: u16,
    /// 0x34: item price (GP).
    pub price: u32,
    /// 0x38: stack amount.
    pub stack_amount: u16,
    /// 0x3A: 8-byte BAM resref for the inventory icon.
    pub inventory_icon: String,
    /// 0x42: lore-to-ID skill required.
    pub lore_to_id: u16,
    /// 0x44: 8-byte BAM resref for the ground icon.
    pub ground_icon: String,
    /// 0x4C: item weight (lb / kg).
    pub weight: u32,
    /// 0x50: strref for the unidentified description.
    pub description_unidentified: u32,
    /// 0x54: strref for the identified description.
    pub description_identified: u32,
    /// 0x58: 8-byte BAM resref for the description icon.
    pub description_icon: String,
    /// 0x60: enchantment level.
    pub enchantment: u32,
    /// 0x64: file offset of the first extended-header record.
    pub extended_headers_offset: u32,
    /// 0x68: number of extended-header records.
    pub extended_headers_count: u16,
    /// 0x6A: file offset of the first feature-block record.
    pub feature_blocks_offset: u32,
    /// 0x6E: index into the effects array of the first "equipping"
    /// feature-block.
    pub equipping_feature_offset: u16,
    /// 0x70: number of equipping feature-blocks.
    pub equipping_feature_count: u16,
}

/// V1.1 (PST vanilla) header — 154 bytes on disk. Same first 114
/// bytes as V1 except:
/// - The 0x10..0x18 resref is the **drop sound** (not the
///   replacement item).
/// - Min Strength..Min Charisma at 0x26..0x34 are 8 "unused" words
///   in PST.
/// - PST adds a 40-byte trailer carrying the Dialog / Conversable
///   label / Paperdoll colour / 6 reserved dwords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItmHeaderV1_1 {
    pub name_unidentified: u32,
    pub name_identified: u32,
    /// 0x10..0x18: PST drop-sound resref.
    pub drop_sound: String,
    pub flags: u32,
    pub item_type: u16,
    pub usability: [u8; 4],
    pub item_animation: [u8; 2],
    pub min_level: u16,
    /// 0x26..0x36: 8 "unused" 2-byte words PST reserved for future
    /// stat requirements. Preserved verbatim for round-trip.
    pub unused_stats: [u16; 7],
    pub price: u32,
    pub stack_amount: u16,
    pub inventory_icon: String,
    pub lore_to_id: u16,
    pub ground_icon: String,
    pub weight: u32,
    pub description_unidentified: u32,
    pub description_identified: u32,
    /// 0x58..0x60: PST replaces the "description icon" with a
    /// **pickup sound** resref.
    pub pickup_sound: String,
    pub enchantment: u32,
    pub extended_headers_offset: u32,
    pub extended_headers_count: u16,
    pub feature_blocks_offset: u32,
    pub equipping_feature_offset: u16,
    pub equipping_feature_count: u16,
    /// 0x72: 8-byte DLG resref for the conversable item's dialog.
    pub dialog: String,
    /// 0x7A: strref for the conversable-label display name.
    pub conversable_strref: u32,
    /// 0x7E: paperdoll-animation colour index.
    pub paperdoll_colour: u16,
    /// 0x80..0x9A: 26 bytes of reserved fields (one unknown word
    /// then six unknown dwords in IESDP). Preserved verbatim.
    pub trailing_unknown: Vec<u8>,
}

/// V2.0 (IWD2) header — 130 bytes on disk. Same as V1 plus a 16-byte
/// reserved trailer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItmHeaderV2 {
    pub name_unidentified: u32,
    pub name_identified: u32,
    pub replacement_item: String,
    pub flags: u32,
    pub item_type: u16,
    pub usability: [u8; 4],
    pub item_animation: [u8; 2],
    pub min_level: u16,
    pub min_strength: u16,
    pub min_strength_bonus: u8,
    pub kit_usability_1: u8,
    pub min_intelligence: u8,
    pub kit_usability_2: u8,
    pub min_dexterity: u8,
    pub kit_usability_3: u8,
    pub min_wisdom: u8,
    pub kit_usability_4: u8,
    pub min_constitution: u8,
    pub weapon_proficiency: u8,
    pub min_charisma: u16,
    pub price: u32,
    pub stack_amount: u16,
    pub inventory_icon: String,
    pub lore_to_id: u16,
    pub ground_icon: String,
    pub weight: u32,
    pub description_unidentified: u32,
    pub description_identified: u32,
    pub description_icon: String,
    pub enchantment: u32,
    pub extended_headers_offset: u32,
    pub extended_headers_count: u16,
    pub feature_blocks_offset: u32,
    pub equipping_feature_offset: u16,
    pub equipping_feature_count: u16,
    /// 0x72..0x82: 16 bytes IWD2 reserves for unknown use.
    pub trailing_unknown: Vec<u8>,
}

/// One extended-header ("ability") record — 56 bytes on disk, same
/// byte layout in every ITM version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItmAbility {
    /// 0x00: attack type — `0` None, `1` Melee, `2` Ranged,
    /// `3` Magical, `4` Launcher.
    pub attack_type: u8,
    /// 0x01: identification-required bitfield.
    pub id_required: u8,
    /// 0x02: ability location — `0` None, `1` Weapon, `2` Spell,
    /// `3` Item, `4` Innate.
    pub location: u8,
    /// 0x03: alternate dice-sides (vs. the engine default).
    pub alt_dice_sides: u8,
    /// 0x04: 8-byte BAM resref for the use icon.
    pub use_icon: String,
    /// 0x0C: target type.
    pub target: u8,
    /// 0x0D: target count for multi-target abilities.
    pub target_count: u8,
    /// 0x0E: range in feet.
    pub range: u16,
    /// 0x10: launcher required (V1) / projectile type (V1.1, V2.0).
    /// `0` None, `1` Arrow / Bow, `2` Bolt, `3` Bullet.
    pub projectile_type: u8,
    /// 0x11: alternate dice-thrown count.
    pub alt_dice_thrown: u8,
    /// 0x12: speed factor (NI `Speed`).
    pub speed_factor: u8,
    /// 0x13: alternate damage bonus.
    pub alt_damage_bonus: u8,
    /// 0x14: THAC0 bonus (signed for some opcodes).
    pub thaco_bonus: u16,
    /// 0x16: damage-dice sides.
    pub dice_sides: u8,
    /// 0x17: primary type (school index into `mschool.2da`).
    pub primary_type: u8,
    /// 0x18: damage-dice thrown.
    pub dice_thrown: u8,
    /// 0x19: secondary type (sub-school index into `msectype.2da`).
    pub secondary_type: u8,
    /// 0x1A: damage bonus.
    pub damage_bonus: u16,
    /// 0x1C: damage type bitfield.
    pub damage_type: u16,
    /// 0x1E: number of feature blocks this ability owns.
    pub num_effects: u16,
    /// 0x20: index of the first feature-block (relative to
    /// [`Itm::effects`]).
    pub first_effect_index: u16,
    /// 0x22: maximum charges.
    pub max_charges: u16,
    /// 0x24: charge-depletion behaviour.
    pub charge_depletion: u16,
    /// 0x26..0x2A: 4 bytes of flags. V1 / V1.1 interpret this as a
    /// single `u32` flags field; V2.0 splits it into `flags` (low
    /// 16 bits) + `attack_type_v2` (high 16 bits — Normal / Bypass
    /// armor / Keen). Stored as `u32` so byte-layout round-trips
    /// regardless of which interpretation the file uses.
    pub flags: u32,
    /// 0x2A: projectile animation index (`projectl.ids` /
    /// `missile.ids`).
    pub projectile_animation: u16,
    /// 0x2C..0x32: three 2-byte melee-animation values (per IESDP
    /// — meant to total 100).
    pub melee_animation: [u16; 3],
    /// 0x32: V1: arrow qualifier ("is arrow?"); V1.1: unused; V2.0:
    /// bow / arrow qualifier.
    pub qualifier_arrow: u16,
    /// 0x34: bolt qualifier / unused / crossbow-bolt qualifier
    /// (same scheme as `qualifier_arrow`).
    pub qualifier_bolt: u16,
    /// 0x36: bullet qualifier / unused / misc-projectile qualifier.
    pub qualifier_bullet: u16,
}

/// One feature-block ("effect") record — 48 bytes on disk, same
/// layout in every ITM version (and identical to the SPL
/// feature-block).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItmEffect {
    /// 0x00: opcode number (`effects.ids`).
    pub opcode: u16,
    /// 0x02: target type.
    pub target_type: u8,
    /// 0x03: power level.
    pub power: u8,
    /// 0x04: opcode-specific parameter 1.
    pub parameter1: u32,
    /// 0x08: opcode-specific parameter 2.
    pub parameter2: u32,
    /// 0x0C: timing mode.
    pub timing_mode: u8,
    /// 0x0D: resistance flags.
    pub resistance: u8,
    /// 0x0E: duration in seconds.
    pub duration: u32,
    /// 0x12: probability 1.
    pub probability1: u8,
    /// 0x13: probability 2.
    pub probability2: u8,
    /// 0x14: 8-byte ASCIIZ resref of the resource the opcode
    /// applies.
    pub resource: String,
    /// 0x1C: dice thrown (or max level for some opcodes).
    pub dice_thrown: u32,
    /// 0x20: dice sides (or min level for some opcodes).
    pub dice_sides: u32,
    /// 0x24: saving-throw type bitfield.
    pub save_type: u32,
    /// 0x28: saving-throw bonus (signed).
    pub save_bonus: i32,
    /// 0x2C: trailing 4-byte field — TobEx "stacking ID" /
    /// IWD2 "Unknown" / V2 "Special".
    pub trailing_extra: u32,
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::{Itm, ItmImporter};

    /// Recursively collect every `.itm` file under `assets/itm/`.
    pub fn all_itm_fixtures() -> Vec<PathBuf> {
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.eq_ignore_ascii_case("itm"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        visit(&get_assets_path().join("itm"), &mut out);
        out.sort();
        out
    }

    /// Imports a fixture path relative to `assets/itm/`.
    pub fn import_fixture(rel_path: &str) -> Itm {
        let path = get_assets_path().join("itm").join(rel_path);
        ItmImporter { name: rel_path }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("import {rel_path}: {e}"))
    }
}
