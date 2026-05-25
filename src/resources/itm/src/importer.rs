//! ITM file reader.

use std::io::{Cursor, Read, Seek};

use infinitier_datasource::{DataSource, Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{
    ABILITY_LEN, EFFECT_LEN, HEADER_LEN_V1, HEADER_LEN_V1_1, HEADER_LEN_V2, ITM_SIGNATURE, Itm,
    ItmAbility, ItmEffect, ItmHeader, ItmHeaderV1, ItmHeaderV1_1, ItmHeaderV2, ItmVersion,
};

/// File reader for ITM resources.
pub struct ItmImporter<'a> {
    /// Caller-visible name for error / log messages.
    pub name: &'a str,
}

type ItmReader = Reader<Cursor<Vec<u8>>>;

impl Importer for ItmImporter<'_> {
    type T = Itm;

    fn import(&self, source: &DataSource) -> std::io::Result<Itm> {
        let mut reader = source.preloaded_reader()?;
        let file_size = reader.seek(std::io::SeekFrom::End(0))?;
        if file_size < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("ITM '{}' shorter than 8-byte header", self.name),
            ));
        }

        reader.set_position(0)?;
        let sig: [u8; 4] = reader.read_exact_to_array()?;
        if &sig != ITM_SIGNATURE {
            error!("Unsupported ITM signature in {}: {sig:?}", self.name);
            return Err(std::io::Error::other(format!(
                "Unsupported ITM signature: {sig:?}"
            )));
        }
        let ver: [u8; 4] = reader.read_exact_to_array()?;
        let version = match &ver {
            b"V1  " => ItmVersion::V1,
            b"V1.1" => ItmVersion::V1_1,
            b"V2.0" => ItmVersion::V2_0,
            _ => {
                error!("Unsupported ITM version in {}: {ver:?}", self.name);
                return Err(std::io::Error::other(format!(
                    "Unsupported ITM version: {ver:?}"
                )));
            }
        };

        let header_len = version.header_len() as u64;
        if file_size < header_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "ITM '{}' is {file_size} B; needs at least {header_len} B for the {:?} header",
                    self.name, version,
                ),
            ));
        }

        let header = match version {
            ItmVersion::V1 => ItmHeader::V1(parse_header_v1(&mut reader)?),
            ItmVersion::V1_1 => ItmHeader::V1_1(Box::new(parse_header_v1_1(&mut reader)?)),
            ItmVersion::V2_0 => ItmHeader::V2(parse_header_v2(&mut reader)?),
        };

        let abilities = parse_abilities(
            &mut reader,
            header.extended_headers_offset(),
            header.extended_headers_count(),
            self.name,
        )?;
        // Effects count isn't carried in the header — it's the max
        // (first_effect_index + num_effects) across every ability,
        // plus the equipping-feature window. Mirrors what NI does.
        let effects_count = compute_effects_count(&abilities, &header);
        let effects = parse_effects(
            &mut reader,
            header.feature_blocks_offset(),
            effects_count,
            self.name,
        )?;

        debug!(
            "Loaded {} [ITM {:?}]: file={} B, abilities={}, effects={}",
            self.name,
            version,
            file_size,
            abilities.len(),
            effects.len(),
        );

        Ok(Itm {
            version,
            header,
            abilities,
            effects,
        })
    }
}

fn compute_effects_count(abilities: &[ItmAbility], header: &ItmHeader) -> usize {
    let mut max = (header.equipping_feature_offset() as usize)
        .saturating_add(header.equipping_feature_count() as usize);
    for a in abilities {
        let end = (a.first_effect_index as usize).saturating_add(a.num_effects as usize);
        if end > max {
            max = end;
        }
    }
    max
}

fn check_range(reader: &mut ItmReader, end: u64, name: &str, what: &str) -> std::io::Result<()> {
    let pos = reader.position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.set_position(pos)?;
    if end > len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("ITM '{name}': {what} runs past file end (need {end} B, file is {len} B)"),
        ));
    }
    Ok(())
}

fn parse_header_v1(reader: &mut ItmReader) -> std::io::Result<ItmHeaderV1> {
    reader.set_position(0x08)?;
    let name_unidentified = reader.read_u32()?;
    let name_identified = reader.read_u32()?;
    let replacement_item = reader.read_string(8)?;
    let flags = reader.read_u32()?;
    let item_type = reader.read_u16()?;
    let usability = reader.read_exact_to_array::<4>()?;
    let item_animation = reader.read_exact_to_array::<2>()?;
    let min_level = reader.read_u16()?;
    let min_strength = reader.read_u16()?;
    let min_strength_bonus = reader.read_u8()?;
    let kit_usability_1 = reader.read_u8()?;
    let min_intelligence = reader.read_u8()?;
    let kit_usability_2 = reader.read_u8()?;
    let min_dexterity = reader.read_u8()?;
    let kit_usability_3 = reader.read_u8()?;
    let min_wisdom = reader.read_u8()?;
    let kit_usability_4 = reader.read_u8()?;
    let min_constitution = reader.read_u8()?;
    let weapon_proficiency = reader.read_u8()?;
    let min_charisma = reader.read_u16()?;
    let price = reader.read_u32()?;
    let stack_amount = reader.read_u16()?;
    let inventory_icon = reader.read_string(8)?;
    let lore_to_id = reader.read_u16()?;
    let ground_icon = reader.read_string(8)?;
    let weight = reader.read_u32()?;
    let description_unidentified = reader.read_u32()?;
    let description_identified = reader.read_u32()?;
    let description_icon = reader.read_string(8)?;
    let enchantment = reader.read_u32()?;
    let extended_headers_offset = reader.read_u32()?;
    let extended_headers_count = reader.read_u16()?;
    let feature_blocks_offset = reader.read_u32()?;
    let equipping_feature_offset = reader.read_u16()?;
    let equipping_feature_count = reader.read_u16()?;
    debug_assert_eq!(reader.position()?, HEADER_LEN_V1 as u64);
    Ok(ItmHeaderV1 {
        name_unidentified,
        name_identified,
        replacement_item,
        flags,
        item_type,
        usability,
        item_animation,
        min_level,
        min_strength,
        min_strength_bonus,
        kit_usability_1,
        min_intelligence,
        kit_usability_2,
        min_dexterity,
        kit_usability_3,
        min_wisdom,
        kit_usability_4,
        min_constitution,
        weapon_proficiency,
        min_charisma,
        price,
        stack_amount,
        inventory_icon,
        lore_to_id,
        ground_icon,
        weight,
        description_unidentified,
        description_identified,
        description_icon,
        enchantment,
        extended_headers_offset,
        extended_headers_count,
        feature_blocks_offset,
        equipping_feature_offset,
        equipping_feature_count,
    })
}

fn parse_header_v1_1(reader: &mut ItmReader) -> std::io::Result<ItmHeaderV1_1> {
    reader.set_position(0x08)?;
    let name_unidentified = reader.read_u32()?;
    let name_identified = reader.read_u32()?;
    let drop_sound = reader.read_string(8)?;
    let flags = reader.read_u32()?;
    let item_type = reader.read_u16()?;
    let usability = reader.read_exact_to_array::<4>()?;
    let item_animation = reader.read_exact_to_array::<2>()?;
    let min_level = reader.read_u16()?;
    // 0x26..0x34: 7 × u16 "unused" PST stat slots.
    let mut unused_stats = [0u16; 7];
    for slot in unused_stats.iter_mut() {
        *slot = reader.read_u16()?;
    }
    let price = reader.read_u32()?;
    let stack_amount = reader.read_u16()?;
    let inventory_icon = reader.read_string(8)?;
    let lore_to_id = reader.read_u16()?;
    let ground_icon = reader.read_string(8)?;
    let weight = reader.read_u32()?;
    let description_unidentified = reader.read_u32()?;
    let description_identified = reader.read_u32()?;
    let pickup_sound = reader.read_string(8)?;
    let enchantment = reader.read_u32()?;
    let extended_headers_offset = reader.read_u32()?;
    let extended_headers_count = reader.read_u16()?;
    let feature_blocks_offset = reader.read_u32()?;
    let equipping_feature_offset = reader.read_u16()?;
    let equipping_feature_count = reader.read_u16()?;
    debug_assert_eq!(reader.position()?, 0x72);
    let dialog = reader.read_string(8)?;
    let conversable_strref = reader.read_u32()?;
    let paperdoll_colour = reader.read_u16()?;
    debug_assert_eq!(reader.position()?, 0x80);
    let mut trailing_unknown = vec![0u8; HEADER_LEN_V1_1 - 0x80];
    reader.read_exact(&mut trailing_unknown)?;
    debug_assert_eq!(reader.position()?, HEADER_LEN_V1_1 as u64);
    Ok(ItmHeaderV1_1 {
        name_unidentified,
        name_identified,
        drop_sound,
        flags,
        item_type,
        usability,
        item_animation,
        min_level,
        unused_stats,
        price,
        stack_amount,
        inventory_icon,
        lore_to_id,
        ground_icon,
        weight,
        description_unidentified,
        description_identified,
        pickup_sound,
        enchantment,
        extended_headers_offset,
        extended_headers_count,
        feature_blocks_offset,
        equipping_feature_offset,
        equipping_feature_count,
        dialog,
        conversable_strref,
        paperdoll_colour,
        trailing_unknown,
    })
}

fn parse_header_v2(reader: &mut ItmReader) -> std::io::Result<ItmHeaderV2> {
    // V2 = V1 + 16-byte trailer; reuse the V1 parser.
    let v1 = parse_header_v1(reader)?;
    debug_assert_eq!(reader.position()?, HEADER_LEN_V1 as u64);
    let mut trailing_unknown = vec![0u8; HEADER_LEN_V2 - HEADER_LEN_V1];
    reader.read_exact(&mut trailing_unknown)?;
    debug_assert_eq!(reader.position()?, HEADER_LEN_V2 as u64);
    Ok(ItmHeaderV2 {
        name_unidentified: v1.name_unidentified,
        name_identified: v1.name_identified,
        replacement_item: v1.replacement_item,
        flags: v1.flags,
        item_type: v1.item_type,
        usability: v1.usability,
        item_animation: v1.item_animation,
        min_level: v1.min_level,
        min_strength: v1.min_strength,
        min_strength_bonus: v1.min_strength_bonus,
        kit_usability_1: v1.kit_usability_1,
        min_intelligence: v1.min_intelligence,
        kit_usability_2: v1.kit_usability_2,
        min_dexterity: v1.min_dexterity,
        kit_usability_3: v1.kit_usability_3,
        min_wisdom: v1.min_wisdom,
        kit_usability_4: v1.kit_usability_4,
        min_constitution: v1.min_constitution,
        weapon_proficiency: v1.weapon_proficiency,
        min_charisma: v1.min_charisma,
        price: v1.price,
        stack_amount: v1.stack_amount,
        inventory_icon: v1.inventory_icon,
        lore_to_id: v1.lore_to_id,
        ground_icon: v1.ground_icon,
        weight: v1.weight,
        description_unidentified: v1.description_unidentified,
        description_identified: v1.description_identified,
        description_icon: v1.description_icon,
        enchantment: v1.enchantment,
        extended_headers_offset: v1.extended_headers_offset,
        extended_headers_count: v1.extended_headers_count,
        feature_blocks_offset: v1.feature_blocks_offset,
        equipping_feature_offset: v1.equipping_feature_offset,
        equipping_feature_count: v1.equipping_feature_count,
        trailing_unknown,
    })
}

fn parse_abilities(
    reader: &mut ItmReader,
    offset: u32,
    count: u16,
    name: &str,
) -> std::io::Result<Vec<ItmAbility>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    let end = start + (count as u64) * ABILITY_LEN as u64;
    check_range(reader, end, name, "abilities section")?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * ABILITY_LEN as u64)?;
        out.push(parse_ability(reader)?);
    }
    Ok(out)
}

fn parse_ability(reader: &mut ItmReader) -> std::io::Result<ItmAbility> {
    let attack_type = reader.read_u8()?;
    let id_required = reader.read_u8()?;
    let location = reader.read_u8()?;
    let alt_dice_sides = reader.read_u8()?;
    let use_icon = reader.read_string(8)?;
    let target = reader.read_u8()?;
    let target_count = reader.read_u8()?;
    let range = reader.read_u16()?;
    let projectile_type = reader.read_u8()?;
    let alt_dice_thrown = reader.read_u8()?;
    let speed_factor = reader.read_u8()?;
    let alt_damage_bonus = reader.read_u8()?;
    let thaco_bonus = reader.read_u16()?;
    let dice_sides = reader.read_u8()?;
    let primary_type = reader.read_u8()?;
    let dice_thrown = reader.read_u8()?;
    let secondary_type = reader.read_u8()?;
    let damage_bonus = reader.read_u16()?;
    let damage_type = reader.read_u16()?;
    let num_effects = reader.read_u16()?;
    let first_effect_index = reader.read_u16()?;
    let max_charges = reader.read_u16()?;
    let charge_depletion = reader.read_u16()?;
    let flags = reader.read_u32()?;
    let projectile_animation = reader.read_u16()?;
    let melee_animation = [reader.read_u16()?, reader.read_u16()?, reader.read_u16()?];
    let qualifier_arrow = reader.read_u16()?;
    let qualifier_bolt = reader.read_u16()?;
    let qualifier_bullet = reader.read_u16()?;
    Ok(ItmAbility {
        attack_type,
        id_required,
        location,
        alt_dice_sides,
        use_icon,
        target,
        target_count,
        range,
        projectile_type,
        alt_dice_thrown,
        speed_factor,
        alt_damage_bonus,
        thaco_bonus,
        dice_sides,
        primary_type,
        dice_thrown,
        secondary_type,
        damage_bonus,
        damage_type,
        num_effects,
        first_effect_index,
        max_charges,
        charge_depletion,
        flags,
        projectile_animation,
        melee_animation,
        qualifier_arrow,
        qualifier_bolt,
        qualifier_bullet,
    })
}

fn parse_effects(
    reader: &mut ItmReader,
    offset: u32,
    count: usize,
    name: &str,
) -> std::io::Result<Vec<ItmEffect>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    let end = start + (count as u64) * EFFECT_LEN as u64;
    check_range(reader, end, name, "effects section")?;
    let mut out = Vec::with_capacity(count);
    for i in 0..count as u64 {
        reader.set_position(start + i * EFFECT_LEN as u64)?;
        out.push(parse_effect(reader)?);
    }
    Ok(out)
}

fn parse_effect(reader: &mut ItmReader) -> std::io::Result<ItmEffect> {
    let opcode = reader.read_u16()?;
    let target_type = reader.read_u8()?;
    let power = reader.read_u8()?;
    let parameter1 = reader.read_u32()?;
    let parameter2 = reader.read_u32()?;
    let timing_mode = reader.read_u8()?;
    let resistance = reader.read_u8()?;
    let duration = reader.read_u32()?;
    let probability1 = reader.read_u8()?;
    let probability2 = reader.read_u8()?;
    let resource = reader.read_string(8)?;
    let dice_thrown = reader.read_u32()?;
    let dice_sides = reader.read_u32()?;
    let save_type = reader.read_u32()?;
    let save_bonus = reader.read_i32()?;
    let trailing_extra = reader.read_u32()?;
    Ok(ItmEffect {
        opcode,
        target_type,
        power,
        parameter1,
        parameter2,
        timing_mode,
        resistance,
        duration,
        probability1,
        probability2,
        resource,
        dice_thrown,
        dice_sides,
        save_type,
        save_bonus,
        trailing_extra,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn test_parse_v1_bg1_weapon() {
        // AX1H02 is a BG1 hand axe — has equipping effects and a
        // melee ability. Good sanity check for the typed fields.
        let itm = import_fixture("v1/bg_AX1H02.itm");
        assert_eq!(itm.version, ItmVersion::V1);
        assert!(matches!(itm.header, ItmHeader::V1(_)));
        let ItmHeader::V1(h) = &itm.header else {
            unreachable!()
        };
        // BG1 weapon — should have at least one ability.
        assert!(!itm.abilities.is_empty());
        // Inventory icon resref must look sane.
        assert!(!h.inventory_icon.is_empty());
        assert_eq!(
            itm.header.extended_headers_count() as usize,
            itm.abilities.len()
        );
    }

    #[test]
    fn test_parse_v1_iwdee_with_charges() {
        // IWDEE SHAMME3 — a more complex item; tests EE-specific
        // V1 layout (still 114-byte header).
        let itm = import_fixture("v1/iwdee_SHAMME3.itm");
        assert_eq!(itm.version, ItmVersion::V1);
        assert!(!itm.abilities.is_empty());
        // Effects must be at or past the header end.
        assert!(
            itm.header.feature_blocks_offset() as usize >= itm.version.header_len(),
            "feature_blocks_offset {:#x} overlaps header",
            itm.header.feature_blocks_offset()
        );
    }

    #[test]
    fn test_parse_v1_1_pst() {
        // PST LIMLIM — only 154 bytes total (header-only,
        // 0 abilities, 0 effects).
        let itm = import_fixture("v1_1/pst_LIMLIM.itm");
        assert_eq!(itm.version, ItmVersion::V1_1);
        let ItmHeader::V1_1(h) = &itm.header else {
            panic!("expected V1.1 header");
        };
        assert_eq!(h.trailing_unknown.len(), HEADER_LEN_V1_1 - 0x80);
    }

    #[test]
    fn test_parse_v2_iwd2() {
        // IWD2 ISW11 — V2.0 weapon. Header should be 130 bytes
        // with a 16-byte trailing reserved blob.
        let itm = import_fixture("v2_0/iwd2_ISW11.itm");
        assert_eq!(itm.version, ItmVersion::V2_0);
        let ItmHeader::V2(h) = &itm.header else {
            panic!("expected V2 header");
        };
        assert_eq!(h.trailing_unknown.len(), 16);
        assert!(!itm.abilities.is_empty());
    }

    #[test]
    fn test_every_corpus_itm_parses() {
        // Strong-conformance sweep: every `.itm` under
        // `assets/itm/` must parse and the header's offset/count
        // fields must match what we actually pulled out.
        let fixtures = all_itm_fixtures();
        assert!(!fixtures.is_empty(), "no ITM fixtures discovered");
        for path in fixtures {
            let itm = ItmImporter {
                name: path.to_string_lossy().as_ref(),
            }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("parse {} failed: {e}", path.display()));
            assert_eq!(
                itm.header.extended_headers_count() as usize,
                itm.abilities.len(),
                "abilities count mismatch in {}",
                path.display(),
            );
            assert_eq!(itm.header.version(), itm.version);
        }
    }

    #[test]
    fn test_rejects_wrong_signature() {
        let err = ItmImporter { name: "junk" }
            .import(&DataSource::new(b"BAD V1  \0\0\0\0".as_slice()))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported ITM signature"));
    }

    #[test]
    fn test_rejects_unknown_version() {
        let err = ItmImporter { name: "future" }
            .import(&DataSource::new(b"ITM V9.9\0\0\0\0".as_slice()))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported ITM version"));
    }

    #[test]
    fn test_rejects_truncated_header() {
        let bytes = b"ITM V1  \0\0\0\0\0\0\0\0\0\0\0\0";
        assert!(bytes.len() < HEADER_LEN_V1);
        let err = ItmImporter { name: "tiny" }
            .import(&DataSource::new(bytes.as_slice()))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }
}
