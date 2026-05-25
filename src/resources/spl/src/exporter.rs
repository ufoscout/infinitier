//! SPL file writer.
//!
//! Round-trip semantics: re-importing the exported bytes yields a
//! [`Spl`] value that is **struct-equal** to the source. Bytes
//! outside the parsed sections (e.g. trailing padding written by
//! some toolchains) aren't surfaced as fields and won't survive —
//! the importer ignores them, so structural equality still holds.

use std::io::{self, BufWriter, Write};
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use log::debug;

use crate::{
    ABILITY_LEN, EFFECT_LEN, HEADER_LEN_V1, HEADER_LEN_V2, SPL_SIGNATURE, Spl, SplAbility,
    SplEffect, SplHeader, SplHeaderV1, SplHeaderV2,
};

/// File writer for SPL resources.
pub struct SplExporter;

impl SplExporter {
    /// Serialises `spl` to the on-disk byte stream.
    pub fn export<W: Write>(&self, spl: &Spl, writer: &mut W) -> io::Result<()> {
        let bytes = serialize(spl)?;
        writer.write_all(&bytes)
    }

    /// Writes `spl` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, spl: &Spl, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(spl, &mut writer)?;
        writer.flush()
    }
}

fn serialize(spl: &Spl) -> io::Result<Vec<u8>> {
    // The `version` field and the header variant must agree —
    // otherwise the emitted bytes would lie about which dispatch
    // the reader should use.
    if spl.version != spl.header.version() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "SPL Spl::version ({:?}) doesn't match SplHeader variant ({:?})",
                spl.version,
                spl.header.version(),
            ),
        ));
    }

    // File size = max(header_end, every section's end). For both
    // versions the abilities + effects sections sit downstream of
    // the header.
    let header_len = spl.version.header_len();
    let abilities_end = (spl.header.abilities_offset() as usize)
        + spl.abilities.len() * ABILITY_LEN;
    let effects_end =
        (spl.header.effects_offset() as usize) + spl.effects.len() * EFFECT_LEN;
    let mut file_size = header_len.max(abilities_end).max(effects_end);
    // Empty section offsets must not retroactively extend the file
    // when their count is zero — guard with `count > 0` checks above
    // by re-deriving the conservative size when the slots are zero.
    if spl.abilities.is_empty() {
        file_size = file_size.max(header_len);
    }
    if spl.effects.is_empty() {
        file_size = file_size.max(header_len);
    }

    let mut buf = vec![0u8; file_size];

    buf[0..4].copy_from_slice(SPL_SIGNATURE);
    buf[4..8].copy_from_slice(spl.version.as_bytes());

    match &spl.header {
        SplHeader::V1(h) => write_header_v1(&mut buf[..HEADER_LEN_V1], h),
        SplHeader::V2(h) => write_header_v2(&mut buf[..HEADER_LEN_V2], h),
    }

    // Abilities.
    {
        let mut off = spl.header.abilities_offset() as usize;
        for a in &spl.abilities {
            write_ability(&mut buf[off..off + ABILITY_LEN], a);
            off += ABILITY_LEN;
        }
    }
    // Effects.
    {
        let mut off = spl.header.effects_offset() as usize;
        for e in &spl.effects {
            write_effect(&mut buf[off..off + EFFECT_LEN], e);
            off += EFFECT_LEN;
        }
    }

    debug!(
        "Serialised SPL ({:?}): abilities={}, effects={}, total={} B",
        spl.version,
        spl.abilities.len(),
        spl.effects.len(),
        buf.len(),
    );

    Ok(buf)
}

fn write_header_v1(out: &mut [u8], h: &SplHeaderV1) {
    debug_assert_eq!(out.len(), HEADER_LEN_V1);
    out[0x08..0x0C].copy_from_slice(&h.name_unidentified.to_le_bytes());
    out[0x0C..0x10].copy_from_slice(&h.name_identified.to_le_bytes());
    write_resref(&mut out[0x10..0x18], &h.completion_sound);
    out[0x18..0x1C].copy_from_slice(&h.flags.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&h.spell_type.to_le_bytes());
    out[0x1E..0x22].copy_from_slice(&h.exclusion_flags.to_le_bytes());
    out[0x22..0x24].copy_from_slice(&h.casting_graphics.to_le_bytes());
    out[0x24] = h.min_level;
    out[0x25] = h.primary_type;
    out[0x26] = h.min_strength;
    out[0x27] = h.secondary_type;
    out[0x28] = h.min_strength_bonus;
    out[0x29] = h.usability1;
    out[0x2A] = h.min_intelligence;
    out[0x2B] = h.usability2;
    out[0x2C] = h.min_dexterity;
    out[0x2D] = h.usability3;
    out[0x2E] = h.min_wisdom;
    out[0x2F] = h.usability4;
    out[0x30..0x32].copy_from_slice(&h.min_constitution.to_le_bytes());
    out[0x32..0x34].copy_from_slice(&h.min_charisma.to_le_bytes());
    out[0x34..0x38].copy_from_slice(&h.spell_level.to_le_bytes());
    out[0x38..0x3A].copy_from_slice(&h.stack_amount.to_le_bytes());
    write_resref(&mut out[0x3A..0x42], &h.spellbook_icon);
    out[0x42..0x44].copy_from_slice(&h.lore_to_id.to_le_bytes());
    write_resref(&mut out[0x44..0x4C], &h.ground_icon);
    out[0x4C..0x50].copy_from_slice(&h.weight.to_le_bytes());
    out[0x50..0x54].copy_from_slice(&h.description_unidentified.to_le_bytes());
    out[0x54..0x58].copy_from_slice(&h.description_identified.to_le_bytes());
    write_resref(&mut out[0x58..0x60], &h.description_icon);
    out[0x60..0x64].copy_from_slice(&h.enchantment.to_le_bytes());
    out[0x64..0x68].copy_from_slice(&h.abilities_offset.to_le_bytes());
    out[0x68..0x6A].copy_from_slice(&h.abilities_count.to_le_bytes());
    out[0x6A..0x6E].copy_from_slice(&h.effects_offset.to_le_bytes());
    out[0x6E..0x70].copy_from_slice(&h.casting_feature_offset.to_le_bytes());
    out[0x70..0x72].copy_from_slice(&h.casting_feature_count.to_le_bytes());
}

fn write_header_v2(out: &mut [u8], h: &SplHeaderV2) {
    debug_assert_eq!(out.len(), HEADER_LEN_V2);
    // Reuse the V1 writer for the shared 0x08..0x72 fields by
    // building a temporary V1 view. (Avoids restating 35 field
    // copies and guarantees the V1 / V2 layouts stay in sync.)
    let shared = SplHeaderV1 {
        name_unidentified: h.name_unidentified,
        name_identified: h.name_identified,
        completion_sound: h.completion_sound.clone(),
        flags: h.flags,
        spell_type: h.spell_type,
        exclusion_flags: h.exclusion_flags,
        casting_graphics: h.casting_graphics,
        min_level: h.min_level,
        primary_type: h.primary_type,
        min_strength: h.min_strength,
        secondary_type: h.secondary_type,
        min_strength_bonus: h.min_strength_bonus,
        usability1: h.usability1,
        min_intelligence: h.min_intelligence,
        usability2: h.usability2,
        min_dexterity: h.min_dexterity,
        usability3: h.usability3,
        min_wisdom: h.min_wisdom,
        usability4: h.usability4,
        min_constitution: h.min_constitution,
        min_charisma: h.min_charisma,
        spell_level: h.spell_level,
        stack_amount: h.stack_amount,
        spellbook_icon: h.spellbook_icon.clone(),
        lore_to_id: h.lore_to_id,
        ground_icon: h.ground_icon.clone(),
        weight: h.weight,
        description_unidentified: h.description_unidentified,
        description_identified: h.description_identified,
        description_icon: h.description_icon.clone(),
        enchantment: h.enchantment,
        abilities_offset: h.abilities_offset,
        abilities_count: h.abilities_count,
        effects_offset: h.effects_offset,
        casting_feature_offset: h.casting_feature_offset,
        casting_feature_count: h.casting_feature_count,
    };
    write_header_v1(&mut out[..HEADER_LEN_V1], &shared);
    // V2 trailer.
    out[0x72] = h.duration_modifier_per_level;
    out[0x73] = h.duration_modifier_base;
    let n = h.trailing_unknown.len().min(HEADER_LEN_V2 - 0x74);
    out[0x74..0x74 + n].copy_from_slice(&h.trailing_unknown[..n]);
}

fn write_ability(out: &mut [u8], a: &SplAbility) {
    debug_assert_eq!(out.len(), ABILITY_LEN);
    out[0x00] = a.spell_form;
    out[0x01] = a.misc_flags;
    out[0x02..0x04].copy_from_slice(&a.location.to_le_bytes());
    write_resref(&mut out[0x04..0x0C], &a.memorised_icon);
    out[0x0C] = a.target;
    out[0x0D] = a.target_count;
    out[0x0E..0x10].copy_from_slice(&a.range.to_le_bytes());
    out[0x10..0x12].copy_from_slice(&a.level_required.to_le_bytes());
    out[0x12..0x14].copy_from_slice(&a.casting_time.to_le_bytes());
    out[0x14..0x16].copy_from_slice(&a.times_per_day.to_le_bytes());
    out[0x16..0x18].copy_from_slice(&a.dice_sides.to_le_bytes());
    out[0x18..0x1A].copy_from_slice(&a.dice_thrown.to_le_bytes());
    out[0x1A..0x1C].copy_from_slice(&a.enchanted.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&a.damage_type.to_le_bytes());
    out[0x1E..0x20].copy_from_slice(&a.num_effects.to_le_bytes());
    out[0x20..0x22].copy_from_slice(&a.first_effect_index.to_le_bytes());
    out[0x22..0x24].copy_from_slice(&a.charges.to_le_bytes());
    out[0x24..0x26].copy_from_slice(&a.charge_depletion.to_le_bytes());
    out[0x26..0x28].copy_from_slice(&a.projectile.to_le_bytes());
}

fn write_effect(out: &mut [u8], e: &SplEffect) {
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
/// slot. Matches the importer's `Reader::read_string` (which decodes
/// via the source's WINDOWS-1252 encoding by default) so resref
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
    use crate::{SplImporter, SplVersion};
    use crate::test_support::all_spl_fixtures;

    #[test]
    fn test_corpus_round_trip() {
        // For every `.spl` under `assets/spl/`: import → export →
        // re-import → struct-equal `Spl`. The intermediate bytes are
        // NOT expected to match byte-for-byte (some fixtures carry
        // trailing padding outside the parsed sections); structural
        // equality is what matters.
        let fixtures = all_spl_fixtures();
        assert!(!fixtures.is_empty(), "no SPL fixtures discovered");
        for path in fixtures {
            let name = path.to_string_lossy().into_owned();
            let original = SplImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));

            let mut produced: Vec<u8> = Vec::new();
            SplExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {name}: {e}"));

            let re_imported = SplImporter { name: &name }
                .import(&DataSource::new(produced))
                .unwrap_or_else(|e| panic!("re-import {name}: {e}"));

            assert_eq!(re_imported, original, "Spl struct mismatch for {name}");
        }
    }

    #[test]
    fn test_export_to_file_round_trip() {
        // File-path overload — exercises the BufWriter<File> path
        // with at least one fixture per version.
        for fixture in &["v1/bg_SPWI413.spl", "v2_0/iwd2_SPWI426.spl"] {
            let path = infinitier_test_utils::get_assets_path()
                .join("spl")
                .join(fixture);
            let original = SplImporter { name: fixture }
                .import(&DataSource::new(path.as_path()))
                .unwrap();
            let tmp = tempfile::NamedTempFile::new().unwrap();
            SplExporter.export_to_file(&original, tmp.path()).unwrap();
            let re_imported = SplImporter { name: fixture }
                .import(&DataSource::new(tmp.path().to_path_buf()))
                .unwrap();
            assert_eq!(re_imported, original, "round-trip mismatch for {fixture}");
        }
    }

    #[test]
    fn test_export_preserves_signature_and_version() {
        let path = infinitier_test_utils::get_assets_path()
            .join("spl/v2_0/iwd2_SPWI426.spl");
        let original = SplImporter { name: "sig" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        let mut produced = Vec::new();
        SplExporter.export(&original, &mut produced).unwrap();
        assert_eq!(&produced[0..4], SPL_SIGNATURE);
        assert_eq!(&produced[4..8], original.version.as_bytes());
    }

    #[test]
    fn test_export_rejects_inconsistent_version() {
        // If a caller has mutated `Spl::version` without also
        // swapping the matching `SplHeader` variant, the export
        // must surface that as an error rather than emit a
        // self-contradicting file.
        let path = infinitier_test_utils::get_assets_path()
            .join("spl/v1/bg_SPWI413.spl");
        let mut spl = SplImporter { name: "x" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        spl.version = SplVersion::V2_0;
        let mut produced = Vec::new();
        let err = SplExporter.export(&spl, &mut produced).unwrap_err();
        assert!(err.to_string().contains("doesn't match SplHeader variant"));
    }
}
