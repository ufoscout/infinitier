//! ITM file writer.
//!
//! Round-trip semantics: re-importing the exported bytes yields an
//! [`Itm`] value that is **struct-equal** to the source. Bytes
//! outside the parsed sections aren't surfaced as fields and won't
//! survive — the importer ignores them, so structural equality
//! still holds.

use std::io::{self, BufWriter, Write};
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use log::debug;

use crate::{
    ABILITY_LEN, EFFECT_LEN, HEADER_LEN_V1, HEADER_LEN_V1_1, HEADER_LEN_V2, ITM_SIGNATURE, Itm,
    ItmAbility, ItmEffect, ItmHeader, ItmHeaderV1, ItmHeaderV1_1, ItmHeaderV2,
};

/// File writer for ITM resources.
pub struct ItmExporter;

impl ItmExporter {
    /// Serialises `itm` to the on-disk byte stream.
    pub fn export<W: Write>(&self, itm: &Itm, writer: &mut W) -> io::Result<()> {
        let bytes = serialize(itm)?;
        writer.write_all(&bytes)
    }

    /// Writes `itm` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, itm: &Itm, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(itm, &mut writer)?;
        writer.flush()
    }
}

fn serialize(itm: &Itm) -> io::Result<Vec<u8>> {
    if itm.version != itm.header.version() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ITM Itm::version ({:?}) doesn't match ItmHeader variant ({:?})",
                itm.version,
                itm.header.version(),
            ),
        ));
    }

    let header_len = itm.version.header_len();
    let abilities_end =
        (itm.header.extended_headers_offset() as usize) + itm.abilities.len() * ABILITY_LEN;
    let effects_end =
        (itm.header.feature_blocks_offset() as usize) + itm.effects.len() * EFFECT_LEN;
    let file_size = header_len.max(abilities_end).max(effects_end);

    let mut buf = vec![0u8; file_size];

    buf[0..4].copy_from_slice(ITM_SIGNATURE);
    buf[4..8].copy_from_slice(itm.version.as_bytes());

    match &itm.header {
        ItmHeader::V1(h) => write_header_v1(&mut buf[..HEADER_LEN_V1], h),
        ItmHeader::V1_1(h) => write_header_v1_1(&mut buf[..HEADER_LEN_V1_1], h),
        ItmHeader::V2(h) => write_header_v2(&mut buf[..HEADER_LEN_V2], h),
    }

    // Abilities.
    {
        let mut off = itm.header.extended_headers_offset() as usize;
        for a in &itm.abilities {
            write_ability(&mut buf[off..off + ABILITY_LEN], a);
            off += ABILITY_LEN;
        }
    }
    // Effects.
    {
        let mut off = itm.header.feature_blocks_offset() as usize;
        for e in &itm.effects {
            write_effect(&mut buf[off..off + EFFECT_LEN], e);
            off += EFFECT_LEN;
        }
    }

    debug!(
        "Serialised ITM ({:?}): abilities={}, effects={}, total={} B",
        itm.version,
        itm.abilities.len(),
        itm.effects.len(),
        buf.len(),
    );

    Ok(buf)
}

fn write_header_v1(out: &mut [u8], h: &ItmHeaderV1) {
    debug_assert_eq!(out.len(), HEADER_LEN_V1);
    out[0x08..0x0C].copy_from_slice(&h.name_unidentified.to_le_bytes());
    out[0x0C..0x10].copy_from_slice(&h.name_identified.to_le_bytes());
    write_resref(&mut out[0x10..0x18], &h.replacement_item);
    out[0x18..0x1C].copy_from_slice(&h.flags.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&h.item_type.to_le_bytes());
    out[0x1E..0x22].copy_from_slice(&h.usability);
    out[0x22..0x24].copy_from_slice(&h.item_animation);
    out[0x24..0x26].copy_from_slice(&h.min_level.to_le_bytes());
    out[0x26..0x28].copy_from_slice(&h.min_strength.to_le_bytes());
    out[0x28] = h.min_strength_bonus;
    out[0x29] = h.kit_usability_1;
    out[0x2A] = h.min_intelligence;
    out[0x2B] = h.kit_usability_2;
    out[0x2C] = h.min_dexterity;
    out[0x2D] = h.kit_usability_3;
    out[0x2E] = h.min_wisdom;
    out[0x2F] = h.kit_usability_4;
    out[0x30] = h.min_constitution;
    out[0x31] = h.weapon_proficiency;
    out[0x32..0x34].copy_from_slice(&h.min_charisma.to_le_bytes());
    out[0x34..0x38].copy_from_slice(&h.price.to_le_bytes());
    out[0x38..0x3A].copy_from_slice(&h.stack_amount.to_le_bytes());
    write_resref(&mut out[0x3A..0x42], &h.inventory_icon);
    out[0x42..0x44].copy_from_slice(&h.lore_to_id.to_le_bytes());
    write_resref(&mut out[0x44..0x4C], &h.ground_icon);
    out[0x4C..0x50].copy_from_slice(&h.weight.to_le_bytes());
    out[0x50..0x54].copy_from_slice(&h.description_unidentified.to_le_bytes());
    out[0x54..0x58].copy_from_slice(&h.description_identified.to_le_bytes());
    write_resref(&mut out[0x58..0x60], &h.description_icon);
    out[0x60..0x64].copy_from_slice(&h.enchantment.to_le_bytes());
    out[0x64..0x68].copy_from_slice(&h.extended_headers_offset.to_le_bytes());
    out[0x68..0x6A].copy_from_slice(&h.extended_headers_count.to_le_bytes());
    out[0x6A..0x6E].copy_from_slice(&h.feature_blocks_offset.to_le_bytes());
    out[0x6E..0x70].copy_from_slice(&h.equipping_feature_offset.to_le_bytes());
    out[0x70..0x72].copy_from_slice(&h.equipping_feature_count.to_le_bytes());
}

fn write_header_v1_1(out: &mut [u8], h: &ItmHeaderV1_1) {
    debug_assert_eq!(out.len(), HEADER_LEN_V1_1);
    // Mostly mirrors V1, with the resref-name + stat-slot
    // substitutions PST does.
    out[0x08..0x0C].copy_from_slice(&h.name_unidentified.to_le_bytes());
    out[0x0C..0x10].copy_from_slice(&h.name_identified.to_le_bytes());
    write_resref(&mut out[0x10..0x18], &h.drop_sound);
    out[0x18..0x1C].copy_from_slice(&h.flags.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&h.item_type.to_le_bytes());
    out[0x1E..0x22].copy_from_slice(&h.usability);
    out[0x22..0x24].copy_from_slice(&h.item_animation);
    out[0x24..0x26].copy_from_slice(&h.min_level.to_le_bytes());
    for (i, v) in h.unused_stats.iter().enumerate() {
        let off = 0x26 + i * 2;
        out[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    out[0x34..0x38].copy_from_slice(&h.price.to_le_bytes());
    out[0x38..0x3A].copy_from_slice(&h.stack_amount.to_le_bytes());
    write_resref(&mut out[0x3A..0x42], &h.inventory_icon);
    out[0x42..0x44].copy_from_slice(&h.lore_to_id.to_le_bytes());
    write_resref(&mut out[0x44..0x4C], &h.ground_icon);
    out[0x4C..0x50].copy_from_slice(&h.weight.to_le_bytes());
    out[0x50..0x54].copy_from_slice(&h.description_unidentified.to_le_bytes());
    out[0x54..0x58].copy_from_slice(&h.description_identified.to_le_bytes());
    write_resref(&mut out[0x58..0x60], &h.pickup_sound);
    out[0x60..0x64].copy_from_slice(&h.enchantment.to_le_bytes());
    out[0x64..0x68].copy_from_slice(&h.extended_headers_offset.to_le_bytes());
    out[0x68..0x6A].copy_from_slice(&h.extended_headers_count.to_le_bytes());
    out[0x6A..0x6E].copy_from_slice(&h.feature_blocks_offset.to_le_bytes());
    out[0x6E..0x70].copy_from_slice(&h.equipping_feature_offset.to_le_bytes());
    out[0x70..0x72].copy_from_slice(&h.equipping_feature_count.to_le_bytes());
    write_resref(&mut out[0x72..0x7A], &h.dialog);
    out[0x7A..0x7E].copy_from_slice(&h.conversable_strref.to_le_bytes());
    out[0x7E..0x80].copy_from_slice(&h.paperdoll_colour.to_le_bytes());
    let n = h.trailing_unknown.len().min(HEADER_LEN_V1_1 - 0x80);
    out[0x80..0x80 + n].copy_from_slice(&h.trailing_unknown[..n]);
}

fn write_header_v2(out: &mut [u8], h: &ItmHeaderV2) {
    debug_assert_eq!(out.len(), HEADER_LEN_V2);
    // V2 reuses the V1 layout for the first 114 bytes — clone the
    // shared fields into a V1 view and reuse the V1 writer.
    let shared = ItmHeaderV1 {
        name_unidentified: h.name_unidentified,
        name_identified: h.name_identified,
        replacement_item: h.replacement_item.clone(),
        flags: h.flags,
        item_type: h.item_type,
        usability: h.usability,
        item_animation: h.item_animation,
        min_level: h.min_level,
        min_strength: h.min_strength,
        min_strength_bonus: h.min_strength_bonus,
        kit_usability_1: h.kit_usability_1,
        min_intelligence: h.min_intelligence,
        kit_usability_2: h.kit_usability_2,
        min_dexterity: h.min_dexterity,
        kit_usability_3: h.kit_usability_3,
        min_wisdom: h.min_wisdom,
        kit_usability_4: h.kit_usability_4,
        min_constitution: h.min_constitution,
        weapon_proficiency: h.weapon_proficiency,
        min_charisma: h.min_charisma,
        price: h.price,
        stack_amount: h.stack_amount,
        inventory_icon: h.inventory_icon.clone(),
        lore_to_id: h.lore_to_id,
        ground_icon: h.ground_icon.clone(),
        weight: h.weight,
        description_unidentified: h.description_unidentified,
        description_identified: h.description_identified,
        description_icon: h.description_icon.clone(),
        enchantment: h.enchantment,
        extended_headers_offset: h.extended_headers_offset,
        extended_headers_count: h.extended_headers_count,
        feature_blocks_offset: h.feature_blocks_offset,
        equipping_feature_offset: h.equipping_feature_offset,
        equipping_feature_count: h.equipping_feature_count,
    };
    write_header_v1(&mut out[..HEADER_LEN_V1], &shared);
    let n = h.trailing_unknown.len().min(HEADER_LEN_V2 - HEADER_LEN_V1);
    out[HEADER_LEN_V1..HEADER_LEN_V1 + n].copy_from_slice(&h.trailing_unknown[..n]);
}

fn write_ability(out: &mut [u8], a: &ItmAbility) {
    debug_assert_eq!(out.len(), ABILITY_LEN);
    out[0x00] = a.attack_type;
    out[0x01] = a.id_required;
    out[0x02] = a.location;
    out[0x03] = a.alt_dice_sides;
    write_resref(&mut out[0x04..0x0C], &a.use_icon);
    out[0x0C] = a.target;
    out[0x0D] = a.target_count;
    out[0x0E..0x10].copy_from_slice(&a.range.to_le_bytes());
    out[0x10] = a.projectile_type;
    out[0x11] = a.alt_dice_thrown;
    out[0x12] = a.speed_factor;
    out[0x13] = a.alt_damage_bonus;
    out[0x14..0x16].copy_from_slice(&a.thaco_bonus.to_le_bytes());
    out[0x16] = a.dice_sides;
    out[0x17] = a.primary_type;
    out[0x18] = a.dice_thrown;
    out[0x19] = a.secondary_type;
    out[0x1A..0x1C].copy_from_slice(&a.damage_bonus.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&a.damage_type.to_le_bytes());
    out[0x1E..0x20].copy_from_slice(&a.num_effects.to_le_bytes());
    out[0x20..0x22].copy_from_slice(&a.first_effect_index.to_le_bytes());
    out[0x22..0x24].copy_from_slice(&a.max_charges.to_le_bytes());
    out[0x24..0x26].copy_from_slice(&a.charge_depletion.to_le_bytes());
    out[0x26..0x2A].copy_from_slice(&a.flags.to_le_bytes());
    out[0x2A..0x2C].copy_from_slice(&a.projectile_animation.to_le_bytes());
    for (i, v) in a.melee_animation.iter().enumerate() {
        let off = 0x2C + i * 2;
        out[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    out[0x32..0x34].copy_from_slice(&a.qualifier_arrow.to_le_bytes());
    out[0x34..0x36].copy_from_slice(&a.qualifier_bolt.to_le_bytes());
    out[0x36..0x38].copy_from_slice(&a.qualifier_bullet.to_le_bytes());
}

fn write_effect(out: &mut [u8], e: &ItmEffect) {
    debug_assert_eq!(out.len(), EFFECT_LEN);
    out[0x00..0x02].copy_from_slice(&e.opcode.to_le_bytes());
    out[0x02] = e.target_type;
    out[0x03] = e.power;
    out[0x04..0x08].copy_from_slice(&e.parameter1.to_le_bytes());
    out[0x08..0x0C].copy_from_slice(&e.parameter2.to_le_bytes());
    out[0x0C] = e.timing_mode;
    out[0x0D] = e.resistance;
    out[0x0E..0x12].copy_from_slice(&e.duration.to_le_bytes());
    out[0x12] = e.probability1;
    out[0x13] = e.probability2;
    write_resref(&mut out[0x14..0x1C], &e.resource);
    out[0x1C..0x20].copy_from_slice(&e.dice_thrown.to_le_bytes());
    out[0x20..0x24].copy_from_slice(&e.dice_sides.to_le_bytes());
    out[0x24..0x28].copy_from_slice(&e.save_type.to_le_bytes());
    out[0x28..0x2C].copy_from_slice(&e.save_bonus.to_le_bytes());
    out[0x2C..0x30].copy_from_slice(&e.trailing_extra.to_le_bytes());
}

/// Encode a String via WINDOWS-1252 into a fixed-width zero-padded
/// slot. Matches the importer's `Reader::read_string` so resref
/// round-trip is byte-exact even for slots holding non-ASCII bytes.
fn write_resref(out: &mut [u8], s: &str) {
    let (encoded, _, _) = WINDOWS_1252.encode(s);
    let n = encoded.len().min(out.len());
    out[..n].copy_from_slice(&encoded[..n]);
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::{DataSource, Importer};

    use super::*;
    use crate::test_support::all_itm_fixtures;
    use crate::{ItmImporter, ItmVersion};

    #[test]
    fn test_corpus_round_trip() {
        // For every `.itm` under `assets/itm/`: import → export →
        // re-import → struct-equal `Itm`.
        let fixtures = all_itm_fixtures();
        assert!(!fixtures.is_empty(), "no ITM fixtures discovered");
        for path in fixtures {
            let name = path.to_string_lossy().into_owned();
            let original = ItmImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));

            let mut produced: Vec<u8> = Vec::new();
            ItmExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {name}: {e}"));

            let re_imported = ItmImporter { name: &name }
                .import(&DataSource::new(produced))
                .unwrap_or_else(|e| panic!("re-import {name}: {e}"));

            assert_eq!(re_imported, original, "Itm struct mismatch for {name}");
        }
    }

    #[test]
    fn test_export_to_file_round_trip() {
        // File-path overload — exercises the BufWriter<File> path
        // with at least one fixture per version.
        for fixture in &[
            "v1/bg_AX1H02.itm",
            "v1_1/pst_LIMLIM.itm",
            "v2_0/iwd2_ISW11.itm",
        ] {
            let path = infinitier_test_utils::get_assets_path()
                .join("itm")
                .join(fixture);
            let original = ItmImporter { name: fixture }
                .import(&DataSource::new(path.as_path()))
                .unwrap();
            let tmp = tempfile::NamedTempFile::new().unwrap();
            ItmExporter.export_to_file(&original, tmp.path()).unwrap();
            let re_imported = ItmImporter { name: fixture }
                .import(&DataSource::new(tmp.path().to_path_buf()))
                .unwrap();
            assert_eq!(re_imported, original, "round-trip mismatch for {fixture}");
        }
    }

    #[test]
    fn test_export_preserves_signature_and_version() {
        let path = infinitier_test_utils::get_assets_path().join("itm/v2_0/iwd2_ISW11.itm");
        let original = ItmImporter { name: "sig" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        let mut produced = Vec::new();
        ItmExporter.export(&original, &mut produced).unwrap();
        assert_eq!(&produced[0..4], ITM_SIGNATURE);
        assert_eq!(&produced[4..8], original.version.as_bytes());
    }

    #[test]
    fn test_export_rejects_inconsistent_version() {
        let path = infinitier_test_utils::get_assets_path().join("itm/v1/bg_AX1H02.itm");
        let mut itm = ItmImporter { name: "x" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        itm.version = ItmVersion::V2_0;
        let mut produced = Vec::new();
        let err = ItmExporter.export(&itm, &mut produced).unwrap_err();
        assert!(err.to_string().contains("doesn't match ItmHeader variant"));
    }
}
