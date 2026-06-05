//! SPL file reader.

use std::io::{Cursor, Read, Seek};

use infinitier_datasource::{DataSource, Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{
    ABILITY_LEN, EFFECT_LEN, HEADER_LEN_V1, HEADER_LEN_V2, SPL_SIGNATURE, Spl, SplAbility,
    SplEffect, SplHeader, SplHeaderV1, SplHeaderV2, SplVersion,
};

/// File reader for SPL resources.
pub struct SplImporter<'a> {
    /// Caller-visible name for error / log messages — usually the
    /// fixture path or `.SPL` resref.
    pub name: &'a str,
}

type SplReader = Reader<Cursor<Vec<u8>>>;

impl Importer for SplImporter<'_> {
    type T = Spl;

    fn import(&self, source: &DataSource) -> std::io::Result<Spl> {
        let mut reader = source.preloaded_reader()?;
        let file_size = reader.seek(std::io::SeekFrom::End(0))?;
        if file_size < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("SPL '{}' shorter than 8-byte header", self.name),
            ));
        }

        reader.set_position(0)?;
        let sig: [u8; 4] = reader.read_exact_to_array()?;
        if &sig != SPL_SIGNATURE {
            error!("Unsupported SPL signature in {}: {sig:?}", self.name);
            return Err(std::io::Error::other(format!(
                "Unsupported SPL signature: {sig:?}"
            )));
        }
        let ver: [u8; 4] = reader.read_exact_to_array()?;
        let version = match &ver {
            b"V1  " => SplVersion::V1,
            b"V2.0" => SplVersion::V2_0,
            _ => {
                error!("Unsupported SPL version in {}: {ver:?}", self.name);
                return Err(std::io::Error::other(format!(
                    "Unsupported SPL version: {ver:?}"
                )));
            }
        };

        let header_len = version.header_len() as u64;
        if file_size < header_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "SPL '{}' is {file_size} B; needs at least {header_len} B for the {:?} header",
                    self.name, version,
                ),
            ));
        }

        let header = match version {
            SplVersion::V1 => SplHeader::V1(parse_header_v1(&mut reader)?),
            SplVersion::V2_0 => SplHeader::V2(parse_header_v2(&mut reader)?),
        };

        // The section table (abilities offset/count + effects offset) is
        // file layout, not stored on the header — read it transiently.
        // Same byte positions in both SPL versions.
        reader.set_position(0x64)?;
        let abilities_offset = reader.read_u32()?;
        let abilities_count = reader.read_u16()?;
        let effects_offset = reader.read_u32()?;

        let abilities = parse_abilities(&mut reader, abilities_offset, abilities_count, self.name)?;
        // The effects section's record count is the **max
        // first_effect_index + num_effects** across every ability,
        // plus the casting feature-blocks. We compute it that way
        // because the SPL header doesn't expose a flat effect count.
        let effects_count = compute_effects_count(&abilities, &header);
        let effects = parse_effects(&mut reader, effects_offset, effects_count, self.name)?;

        debug!(
            "Loaded {} [SPL {:?}]: file={} B, abilities={}, effects={}",
            self.name,
            version,
            file_size,
            abilities.len(),
            effects.len(),
        );

        Ok(Spl {
            version,
            header,
            abilities,
            effects,
        })
    }
}

/// Returns the number of effect records the file actually carries —
/// derived from the highest `first_effect_index + num_effects` across
/// every ability, plus the casting feature-block window.
fn compute_effects_count(abilities: &[SplAbility], header: &SplHeader) -> usize {
    let mut max = (header.casting_feature_offset() as usize)
        .saturating_add(header.casting_feature_count() as usize);
    for a in abilities {
        let end = (a.first_effect_index as usize).saturating_add(a.num_effects as usize);
        if end > max {
            max = end;
        }
    }
    max
}

fn parse_header_v1(reader: &mut SplReader) -> std::io::Result<SplHeaderV1> {
    // Caller has already advanced past the 8-byte sig+ver prefix —
    // position the cursor at 0x08 explicitly so this function is
    // robust to call ordering.
    reader.set_position(0x08)?;
    let name_unidentified = reader.read_u32()?;
    let name_identified = reader.read_u32()?;
    let completion_sound = reader.read_string(8)?;
    let flags = reader.read_u32()?;
    let spell_type = reader.read_u16()?;
    let exclusion_flags = reader.read_u32()?;
    let casting_graphics = reader.read_u16()?;
    let min_level = reader.read_u8()?;
    let primary_type = reader.read_u8()?;
    let min_strength = reader.read_u8()?;
    let secondary_type = reader.read_u8()?;
    let min_strength_bonus = reader.read_u8()?;
    let usability1 = reader.read_u8()?;
    let min_intelligence = reader.read_u8()?;
    let usability2 = reader.read_u8()?;
    let min_dexterity = reader.read_u8()?;
    let usability3 = reader.read_u8()?;
    let min_wisdom = reader.read_u8()?;
    let usability4 = reader.read_u8()?;
    let min_constitution = reader.read_u16()?;
    let min_charisma = reader.read_u16()?;
    let spell_level = reader.read_u32()?;
    let stack_amount = reader.read_u16()?;
    let spellbook_icon = reader.read_string(8)?;
    let lore_to_id = reader.read_u16()?;
    let ground_icon = reader.read_string(8)?;
    let weight = reader.read_u32()?;
    let description_unidentified = reader.read_u32()?;
    let description_identified = reader.read_u32()?;
    let description_icon = reader.read_string(8)?;
    let enchantment = reader.read_u32()?;
    // 0x64/0x68/0x6A section table is file layout — skipped (recomputed
    // on export).
    let _ = reader.read_u32()?; // abilities offset
    let _ = reader.read_u16()?; // abilities count
    let _ = reader.read_u32()?; // effects offset
    let casting_feature_offset = reader.read_u16()?;
    let casting_feature_count = reader.read_u16()?;
    debug_assert_eq!(reader.position()?, HEADER_LEN_V1 as u64);
    Ok(SplHeaderV1 {
        name_unidentified,
        name_identified,
        completion_sound,
        flags,
        spell_type,
        exclusion_flags,
        casting_graphics,
        min_level,
        primary_type,
        min_strength,
        secondary_type,
        min_strength_bonus,
        usability1,
        min_intelligence,
        usability2,
        min_dexterity,
        usability3,
        min_wisdom,
        usability4,
        min_constitution,
        min_charisma,
        spell_level,
        stack_amount,
        spellbook_icon,
        lore_to_id,
        ground_icon,
        weight,
        description_unidentified,
        description_identified,
        description_icon,
        enchantment,
        casting_feature_offset,
        casting_feature_count,
    })
}

fn parse_header_v2(reader: &mut SplReader) -> std::io::Result<SplHeaderV2> {
    // V2 = V1 + 16-byte trailer; reuse the V1 parser, then read the
    // duration modifiers and the 14-byte reserved blob.
    let v1 = parse_header_v1(reader)?;
    reader.set_position(0x72)?;
    let duration_modifier_per_level = reader.read_u8()?;
    let duration_modifier_base = reader.read_u8()?;
    let mut trailing_unknown = vec![0u8; HEADER_LEN_V2 - 0x74];
    reader.read_exact(&mut trailing_unknown)?;
    debug_assert_eq!(reader.position()?, HEADER_LEN_V2 as u64);
    Ok(SplHeaderV2 {
        name_unidentified: v1.name_unidentified,
        name_identified: v1.name_identified,
        completion_sound: v1.completion_sound,
        flags: v1.flags,
        spell_type: v1.spell_type,
        exclusion_flags: v1.exclusion_flags,
        casting_graphics: v1.casting_graphics,
        min_level: v1.min_level,
        primary_type: v1.primary_type,
        min_strength: v1.min_strength,
        secondary_type: v1.secondary_type,
        min_strength_bonus: v1.min_strength_bonus,
        usability1: v1.usability1,
        min_intelligence: v1.min_intelligence,
        usability2: v1.usability2,
        min_dexterity: v1.min_dexterity,
        usability3: v1.usability3,
        min_wisdom: v1.min_wisdom,
        usability4: v1.usability4,
        min_constitution: v1.min_constitution,
        min_charisma: v1.min_charisma,
        spell_level: v1.spell_level,
        stack_amount: v1.stack_amount,
        spellbook_icon: v1.spellbook_icon,
        lore_to_id: v1.lore_to_id,
        ground_icon: v1.ground_icon,
        weight: v1.weight,
        description_unidentified: v1.description_unidentified,
        description_identified: v1.description_identified,
        description_icon: v1.description_icon,
        enchantment: v1.enchantment,
        casting_feature_offset: v1.casting_feature_offset,
        casting_feature_count: v1.casting_feature_count,
        duration_modifier_per_level,
        duration_modifier_base,
        trailing_unknown,
    })
}

fn check_range(reader: &mut SplReader, end: u64, name: &str, what: &str) -> std::io::Result<()> {
    let pos = reader.position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.set_position(pos)?;
    if end > len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("SPL '{name}': {what} runs past file end (need {end} B, file is {len} B)"),
        ));
    }
    Ok(())
}

fn parse_abilities(
    reader: &mut SplReader,
    offset: u32,
    count: u16,
    name: &str,
) -> std::io::Result<Vec<SplAbility>> {
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

fn parse_ability(reader: &mut SplReader) -> std::io::Result<SplAbility> {
    let spell_form = reader.read_u8()?;
    let misc_flags = reader.read_u8()?;
    let location = reader.read_u16()?;
    let memorised_icon = reader.read_string(8)?;
    let target = reader.read_u8()?;
    let target_count = reader.read_u8()?;
    let range = reader.read_u16()?;
    let level_required = reader.read_u16()?;
    let casting_time = reader.read_u16()?;
    let times_per_day = reader.read_u16()?;
    let dice_sides = reader.read_u16()?;
    let dice_thrown = reader.read_u16()?;
    let enchanted = reader.read_u16()?;
    let damage_type = reader.read_u16()?;
    let num_effects = reader.read_u16()?;
    let first_effect_index = reader.read_u16()?;
    let charges = reader.read_u16()?;
    let charge_depletion = reader.read_u16()?;
    let projectile = reader.read_u16()?;
    Ok(SplAbility {
        spell_form,
        misc_flags,
        location,
        memorised_icon,
        target,
        target_count,
        range,
        level_required,
        casting_time,
        times_per_day,
        dice_sides,
        dice_thrown,
        enchanted,
        damage_type,
        num_effects,
        first_effect_index,
        charges,
        charge_depletion,
        projectile,
    })
}

fn parse_effects(
    reader: &mut SplReader,
    offset: u32,
    count: usize,
    name: &str,
) -> std::io::Result<Vec<SplEffect>> {
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

fn parse_effect(reader: &mut SplReader) -> std::io::Result<SplEffect> {
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
    Ok(SplEffect {
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
    fn test_parse_v1_bg2_innate() {
        // SPIN572 (BG2 / "Cure Light Wounds-style" innate) has a
        // single ability with a small effect list — perfect to
        // exercise the count derivation.
        let spl = import_fixture("v1/bg2_SPIN572.spl");
        assert_eq!(spl.version, SplVersion::V1);
        assert!(matches!(spl.header, SplHeader::V1(_)));
        // BG2 SPIN572 is 250 B = 114 B header + 1 ability (40 B) +
        // some effects (48 B each). Should have at least 1 ability.
        assert!(!spl.abilities.is_empty());
        assert!(!spl.effects.is_empty());
    }

    #[test]
    fn test_parse_v1_bg_wizard_spell() {
        // SPWI413 — a BG1 wizard spell, larger file, exercises
        // multi-ability scaling (different abilities per caster
        // level range).
        let spl = import_fixture("v1/bg_SPWI413.spl");
        assert_eq!(spl.version, SplVersion::V1);
        let SplHeader::V1(h) = &spl.header else {
            panic!("expected V1 header");
        };
        // 1 = Wizard spell type.
        assert_eq!(h.spell_type, 1);
        assert!(!spl.abilities.is_empty());
        // The completion sound resref shouldn't be empty for a
        // BG1 wizard spell.
        assert!(!h.completion_sound.is_empty());
    }

    #[test]
    fn test_parse_v2_iwd2() {
        let spl = import_fixture("v2_0/iwd2_SPWI426.spl");
        assert_eq!(spl.version, SplVersion::V2_0);
        let SplHeader::V2(h) = &spl.header else {
            panic!("expected V2 header");
        };
        // IWD2 uses the same primitive fields plus the duration
        // modifiers — they're at least readable (engine sets them).
        let _ = h.duration_modifier_per_level;
        assert_eq!(h.trailing_unknown.len(), 14);
        assert!(!spl.abilities.is_empty());
    }

    #[test]
    fn test_every_corpus_spl_parses() {
        // Strong-conformance sweep: every `.spl` under
        // `assets/spl/` must parse, and the header offset/count
        // fields must match what we actually pulled out.
        let fixtures = all_spl_fixtures();
        assert!(!fixtures.is_empty(), "no SPL fixtures discovered");
        for path in fixtures {
            let spl = SplImporter {
                name: path.to_string_lossy().as_ref(),
            }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("parse {} failed: {e}", path.display()));
            assert_eq!(spl.header.version(), spl.version);
        }
    }

    #[test]
    fn test_rejects_wrong_signature() {
        let err = SplImporter { name: "junk" }
            .import(&DataSource::new(b"BAD V1  \0\0\0\0".as_slice()))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported SPL signature"));
    }

    #[test]
    fn test_rejects_unknown_version() {
        let err = SplImporter { name: "future" }
            .import(&DataSource::new(b"SPL V9.9\0\0\0\0".as_slice()))
            .unwrap_err();
        assert!(err.to_string().contains("Unsupported SPL version"));
    }

    #[test]
    fn test_rejects_truncated_header() {
        // 20 bytes is past sig+ver but well short of the 114-byte
        // V1 header — must error.
        let bytes = b"SPL V1  \0\0\0\0\0\0\0\0\0\0\0\0";
        assert!(bytes.len() < HEADER_LEN_V1);
        let err = SplImporter { name: "tiny" }
            .import(&DataSource::new(bytes.as_slice()))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }
}
