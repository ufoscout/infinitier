//! CRE file writer.
//!
//! Round-trip semantics: re-importing the exported bytes yields a
//! [`Cre`] that is **struct-equal** to the source. Bytes outside the
//! parsed sections (e.g. trailing padding written by some toolchains)
//! aren't surfaced as fields and won't survive — the importer
//! ignores them, so struct-equality still holds.
//!
//! The exporter writes the header (preserved bytes) at offset 0,
//! then writes each sub-section at the offset recorded in the
//! header. Callers who mutate sub-section counts must keep the
//! header's offset/count fields in sync; the writer trusts them.

use std::io::{self, BufWriter, Write};
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use log::debug;

use crate::{
    Cre, CreHeader, CreVersion, EffectList, Item, Iwd2Table, KnownSpell, MemorizedSpell,
    SpellMemorizationInfo, SubSections, V1SubSections, V22SubSections,
    header_generated::{
        serialize_header_v1_0, serialize_header_v1_2, serialize_header_v2_2, serialize_header_v9_0,
    },
};

const KNOWN_SPELL_LEN: usize = 12;
const SPELL_MEMORIZATION_INFO_LEN: usize = 16;
const MEMORIZED_SPELL_LEN: usize = 12;
const ITEM_LEN: usize = 20;
const IWD2_RECORD_LEN: usize = 16;

const V22_CLASS_SPELL_BASE: usize = 0x03BA;
const V22_DOMAIN_SPELL_BASE: usize = 0x05B2;
const V22_TAIL_TABLE_BASE: usize = 0x05FA;

/// File writer for CRE resources.
pub struct CreExporter;

impl CreExporter {
    /// Serialises `cre` to the on-disk byte stream.
    pub fn export<W: Write>(&self, cre: &Cre, writer: &mut W) -> io::Result<()> {
        let bytes = serialize(cre)?;
        writer.write_all(&bytes)
    }

    /// Writes `cre` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, cre: &Cre, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(cre, &mut writer)?;
        writer.flush()
    }
}

fn serialize(cre: &Cre) -> io::Result<Vec<u8>> {
    // The `version` field and the `header` enum variant must agree,
    // otherwise the emitted bytes would lie about which dispatch the
    // reader should use.
    if cre.version != cre.header.version() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CRE Cre::version ({:?}) doesn't match CreHeader variant ({:?})",
                cre.version,
                cre.header.version(),
            ),
        ));
    }

    // Serialise the typed header into its fixed-width byte form
    // first — the section pointers we need to lay out sub-sections
    // live inside it.
    let header_bytes = serialize_header(&cre.header);

    let file_size = compute_file_size(cre, &header_bytes)?;
    let mut buf = vec![0u8; file_size];

    buf[..header_bytes.len()].copy_from_slice(&header_bytes);

    match (cre.version, &cre.sub_sections) {
        (CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0, SubSections::V1(s)) => {
            write_v1_sub_sections(&mut buf, &header_bytes, cre.version, s)?;
        }
        (CreVersion::V2_2, SubSections::V22(s)) => {
            write_v22_sub_sections(&mut buf, &header_bytes, s)?;
        }
        (v, _) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CRE sub-sections variant doesn't match version {v:?}"),
            ));
        }
    }

    debug!("Serialised CRE ({:?}): total={} B", cre.version, buf.len());
    Ok(buf)
}

/// Dispatch the typed header back to its byte form.
fn serialize_header(h: &CreHeader) -> Vec<u8> {
    match h {
        CreHeader::V10(h) => serialize_header_v1_0(h),
        CreHeader::V12(h) => serialize_header_v1_2(h),
        CreHeader::V90(h) => serialize_header_v9_0(h),
        CreHeader::V22(h) => serialize_header_v2_2(h),
    }
}

fn compute_file_size(cre: &Cre, header_bytes: &[u8]) -> io::Result<usize> {
    let header_end = header_bytes.len();
    let mut file_size = header_end;
    let extend = |fs: &mut usize, offset: u32, len: usize| {
        if len > 0 {
            *fs = (*fs).max(offset as usize + len);
        }
    };
    match (cre.version, &cre.sub_sections) {
        (CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0, SubSections::V1(s)) => {
            let base = v1_section_table_base(cre.version);
            let read_u32 = |off: usize| {
                u32::from_le_bytes(header_bytes[base + off..base + off + 4].try_into().unwrap())
            };
            extend(
                &mut file_size,
                read_u32(0x00),
                s.known_spells.len() * KNOWN_SPELL_LEN,
            );
            extend(
                &mut file_size,
                read_u32(0x08),
                s.spell_memorization_info.len() * SPELL_MEMORIZATION_INFO_LEN,
            );
            extend(
                &mut file_size,
                read_u32(0x10),
                s.memorized_spells.len() * MEMORIZED_SPELL_LEN,
            );
            extend(&mut file_size, read_u32(0x18), s.item_slots.len());
            extend(&mut file_size, read_u32(0x1C), s.items.len() * ITEM_LEN);
            extend(
                &mut file_size,
                read_u32(0x24),
                s.effects.len() * s.effects.record_size(),
            );
        }
        (CreVersion::V2_2, SubSections::V22(s)) => {
            let read_u32 =
                |off: usize| u32::from_le_bytes(header_bytes[off..off + 4].try_into().unwrap());
            // IWD2 sub-section block = `entries.len() * 16` record
            // bytes + 8-byte trailer. We only emit the trailer when
            // the table has actually been parsed (non-zero offset);
            // empty tables at offset zero contribute nothing.
            let table_bytes = |t: &Iwd2Table, off: u32| {
                if off == 0 {
                    0
                } else {
                    t.entries.len() * IWD2_RECORD_LEN + 8
                }
            };
            // Class spell tables (7 classes × 9 levels).
            let class_lists: [&[Iwd2Table; 9]; 7] = [
                &s.bard_spells,
                &s.cleric_spells,
                &s.druid_spells,
                &s.paladin_spells,
                &s.ranger_spells,
                &s.sorcerer_spells,
                &s.wizard_spells,
            ];
            for (c, list) in class_lists.iter().enumerate() {
                for (l, table) in list.iter().enumerate() {
                    let off = read_u32(V22_CLASS_SPELL_BASE + (c * 9 + l) * 4);
                    extend(&mut file_size, off, table_bytes(table, off));
                }
            }
            for (i, table) in s.domain_spells.iter().enumerate() {
                let off = read_u32(V22_DOMAIN_SPELL_BASE + i * 4);
                extend(&mut file_size, off, table_bytes(table, off));
            }
            // Tail table.
            let abilities_off = read_u32(V22_TAIL_TABLE_BASE);
            extend(
                &mut file_size,
                abilities_off,
                table_bytes(&s.abilities, abilities_off),
            );
            let songs_off = read_u32(V22_TAIL_TABLE_BASE + 0x08);
            extend(&mut file_size, songs_off, table_bytes(&s.songs, songs_off));
            let shapes_off = read_u32(V22_TAIL_TABLE_BASE + 0x10);
            extend(
                &mut file_size,
                shapes_off,
                table_bytes(&s.shapes, shapes_off),
            );
            extend(
                &mut file_size,
                read_u32(V22_TAIL_TABLE_BASE + 0x18),
                s.item_slots.len(),
            );
            extend(
                &mut file_size,
                read_u32(V22_TAIL_TABLE_BASE + 0x1C),
                s.items.len() * ITEM_LEN,
            );
            extend(
                &mut file_size,
                read_u32(V22_TAIL_TABLE_BASE + 0x24),
                s.effects.len() * s.effects.record_size(),
            );
        }
        _ => unreachable!("variant mismatch caught earlier"),
    }
    Ok(file_size)
}

// ─────────────────────────────────────────────────────────────────────
//  V1.0 / V1.2 / V9.0 writer
// ─────────────────────────────────────────────────────────────────────

fn v1_section_table_base(version: CreVersion) -> usize {
    match version {
        CreVersion::V1_0 => 0x02A0,
        CreVersion::V1_2 => 0x0344,
        CreVersion::V9_0 => 0x0308,
        CreVersion::V2_2 => unreachable!(),
    }
}

fn write_v1_sub_sections(
    buf: &mut [u8],
    header: &[u8],
    version: CreVersion,
    s: &V1SubSections,
) -> io::Result<()> {
    let base = v1_section_table_base(version);
    let read_u32 =
        |off: usize| u32::from_le_bytes(header[base + off..base + off + 4].try_into().unwrap());

    write_known_spells(buf, read_u32(0x00) as usize, &s.known_spells);
    write_spell_memorization_info(buf, read_u32(0x08) as usize, &s.spell_memorization_info);
    write_memorized_spells(buf, read_u32(0x10) as usize, &s.memorized_spells);
    // Item-slots are an opaque indices buffer.
    let item_slots_off = read_u32(0x18) as usize;
    let n = s.item_slots.len();
    if n > 0 {
        buf[item_slots_off..item_slots_off + n].copy_from_slice(&s.item_slots);
    }
    write_items(buf, read_u32(0x1C) as usize, &s.items);
    write_effects(buf, read_u32(0x24) as usize, &s.effects);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────
//  V2.2 writer
// ─────────────────────────────────────────────────────────────────────

fn write_v22_sub_sections(buf: &mut [u8], header: &[u8], s: &V22SubSections) -> io::Result<()> {
    let read_u32 = |off: usize| u32::from_le_bytes(header[off..off + 4].try_into().unwrap());

    let class_lists: [&[Iwd2Table; 9]; 7] = [
        &s.bard_spells,
        &s.cleric_spells,
        &s.druid_spells,
        &s.paladin_spells,
        &s.ranger_spells,
        &s.sorcerer_spells,
        &s.wizard_spells,
    ];
    for (c, list) in class_lists.iter().enumerate() {
        for (l, table) in list.iter().enumerate() {
            let off = read_u32(V22_CLASS_SPELL_BASE + (c * 9 + l) * 4);
            write_iwd2_table(buf, off, table);
        }
    }
    for (i, table) in s.domain_spells.iter().enumerate() {
        let off = read_u32(V22_DOMAIN_SPELL_BASE + i * 4);
        write_iwd2_table(buf, off, table);
    }

    write_iwd2_table(buf, read_u32(V22_TAIL_TABLE_BASE), &s.abilities);
    write_iwd2_table(buf, read_u32(V22_TAIL_TABLE_BASE + 0x08), &s.songs);
    write_iwd2_table(buf, read_u32(V22_TAIL_TABLE_BASE + 0x10), &s.shapes);
    let item_slots_off = read_u32(V22_TAIL_TABLE_BASE + 0x18) as usize;
    let n = s.item_slots.len();
    if n > 0 {
        buf[item_slots_off..item_slots_off + n].copy_from_slice(&s.item_slots);
    }
    write_items(buf, read_u32(V22_TAIL_TABLE_BASE + 0x1C) as usize, &s.items);
    write_effects(
        buf,
        read_u32(V22_TAIL_TABLE_BASE + 0x24) as usize,
        &s.effects,
    );
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────
//  Sub-section record writers
// ─────────────────────────────────────────────────────────────────────

fn write_known_spells(buf: &mut [u8], offset: usize, list: &[KnownSpell]) {
    for (i, k) in list.iter().enumerate() {
        let off = offset + i * KNOWN_SPELL_LEN;
        write_resref(&mut buf[off..off + 8], &k.spell);
        buf[off + 8..off + 10].copy_from_slice(&k.level.to_le_bytes());
        buf[off + 10..off + 12].copy_from_slice(&k.spell_type.to_u16().to_le_bytes());
    }
}

fn write_spell_memorization_info(buf: &mut [u8], offset: usize, list: &[SpellMemorizationInfo]) {
    for (i, m) in list.iter().enumerate() {
        let off = offset + i * SPELL_MEMORIZATION_INFO_LEN;
        buf[off..off + 2].copy_from_slice(&m.level.to_le_bytes());
        buf[off + 2..off + 4].copy_from_slice(&m.num_memorizable_total.to_le_bytes());
        buf[off + 4..off + 6].copy_from_slice(&m.num_memorizable_current.to_le_bytes());
        buf[off + 6..off + 8].copy_from_slice(&m.spell_type.to_u16().to_le_bytes());
        buf[off + 8..off + 12].copy_from_slice(&m.spell_table_index.to_le_bytes());
        buf[off + 12..off + 16].copy_from_slice(&m.spell_count.to_le_bytes());
    }
}

fn write_memorized_spells(buf: &mut [u8], offset: usize, list: &[MemorizedSpell]) {
    for (i, m) in list.iter().enumerate() {
        let off = offset + i * MEMORIZED_SPELL_LEN;
        write_resref(&mut buf[off..off + 8], &m.spell);
        buf[off + 8..off + 10].copy_from_slice(&m.memorization_flags.to_le_bytes());
        buf[off + 10..off + 12].copy_from_slice(&m.padding.to_le_bytes());
    }
}

fn write_items(buf: &mut [u8], offset: usize, list: &[Item]) {
    for (i, it) in list.iter().enumerate() {
        let off = offset + i * ITEM_LEN;
        write_resref(&mut buf[off..off + 8], &it.item);
        buf[off + 8..off + 10].copy_from_slice(&it.duration.to_le_bytes());
        buf[off + 10..off + 12].copy_from_slice(&it.quantity1.to_le_bytes());
        buf[off + 12..off + 14].copy_from_slice(&it.quantity2.to_le_bytes());
        buf[off + 14..off + 16].copy_from_slice(&it.quantity3.to_le_bytes());
        buf[off + 16..off + 20].copy_from_slice(&it.flags.bits().to_le_bytes());
    }
}

fn write_effects(buf: &mut [u8], offset: usize, effects: &EffectList) {
    match effects {
        EffectList::V1(list) => {
            for (i, e) in list.iter().enumerate() {
                let off = offset + i * 48;
                buf[off..off + 48].copy_from_slice(&e.raw);
            }
        }
        EffectList::V2(list) => {
            for (i, e) in list.iter().enumerate() {
                let off = offset + i * 264;
                // `Raw` effects are emitted verbatim; a `LocalVariable`
                // is rebuilt from its parsed name/value via a canonical
                // op187 template.
                let r = e.to_record();
                let n = r.len().min(264);
                buf[off..off + n].copy_from_slice(&r[..n]);
            }
        }
    }
}

/// Writes one IWD2 sub-section block: `entries.len()` × 16-byte
/// records followed by the 8-byte trailer (`num_memorizable` /
/// `num_remaining`). `offset == 0` means the block was absent on
/// import and we don't emit a trailer either.
fn write_iwd2_table(buf: &mut [u8], offset: u32, table: &Iwd2Table) {
    if offset == 0 {
        return;
    }
    let base = offset as usize;
    for (i, slot) in table.entries.iter().enumerate() {
        let off = base + i * IWD2_RECORD_LEN;
        write_resref(&mut buf[off..off + 8], &slot.spell);
        buf[off + 8..off + 12].copy_from_slice(&slot.memorized.to_le_bytes());
        buf[off + 12..off + 16].copy_from_slice(&slot.remaining.to_le_bytes());
    }
    let trailer_off = base + table.entries.len() * IWD2_RECORD_LEN;
    buf[trailer_off..trailer_off + 4].copy_from_slice(&table.num_memorizable.to_le_bytes());
    buf[trailer_off + 4..trailer_off + 8].copy_from_slice(&table.num_remaining.to_le_bytes());
}

/// Write a [`String`] (decoded via WINDOWS-1252 on import) back into
/// the fixed-width `out` slot, padding the trailing bytes with `\0`.
/// WINDOWS-1252 is a bijective 8-bit encoding so this round-trips
/// any byte the importer accepted, including non-ASCII garbage in
/// otherwise-zero resref slots.
fn write_resref(out: &mut [u8], s: &str) {
    let (encoded, _, _) = WINDOWS_1252.encode(s);
    let n = encoded.len().min(out.len());
    out[..n].copy_from_slice(&encoded[..n]);
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::{DataSource, Importer};

    use super::*;
    use crate::CreImporter;
    use crate::test_support::all_cre_fixtures;

    #[test]
    fn test_corpus_round_trip() {
        // For every `.cre` under `assets/cre/`: import → export →
        // re-import → struct-equal `Cre`. Bytes outside parsed
        // sections aren't surfaced as fields, so byte-equality is
        // not asserted — only structural equality (matches the GAM
        // crate's round-trip semantics).
        let fixtures = all_cre_fixtures();
        assert!(!fixtures.is_empty(), "no CRE fixtures discovered");
        for path in fixtures {
            let name = path.to_string_lossy().into_owned();
            let original = CreImporter { name: &name }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {name}: {e}"));

            let mut produced: Vec<u8> = Vec::new();
            CreExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {name}: {e}"));

            let re_imported = CreImporter { name: &name }
                .import(&DataSource::new(produced))
                .unwrap_or_else(|e| panic!("re-import {name}: {e}"));

            assert_eq!(re_imported, original, "Cre struct mismatch for {name}");
        }
    }

    #[test]
    fn test_export_to_file_round_trip() {
        // File-path overload — exercises the BufWriter<File> path
        // with at least one fixture per version family.
        for fixture in &[
            "v1_0/IRONGU.cre",
            "v1_2/THIEF3.cre",
            "v9_0/BARBWAR2.cre",
            "v2_2/52SERSA.cre",
        ] {
            let path = infinitier_test_utils::get_assets_path()
                .join("cre")
                .join(fixture);
            let original = CreImporter { name: fixture }
                .import(&DataSource::new(path.as_path()))
                .unwrap();
            let tmp = tempfile::NamedTempFile::new().unwrap();
            CreExporter.export_to_file(&original, tmp.path()).unwrap();
            let re_imported = CreImporter { name: fixture }
                .import(&DataSource::new(tmp.path().to_path_buf()))
                .unwrap();
            assert_eq!(re_imported, original, "round-trip mismatch for {fixture}");
        }
    }

    #[test]
    fn test_export_preserves_signature_and_version() {
        // Header bytes are written verbatim — the first 8 bytes must
        // therefore match the canonical signature + version tag.
        let path = infinitier_test_utils::get_assets_path().join("cre/v2_2/52SERSA.cre");
        let original = CreImporter { name: "sig" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        let mut produced = Vec::new();
        CreExporter.export(&original, &mut produced).unwrap();
        assert_eq!(&produced[0..4], crate::CRE_SIGNATURE);
        assert_eq!(&produced[4..8], original.version.as_bytes());
    }

    #[test]
    fn test_export_rejects_inconsistent_version() {
        // If a caller has mutated `Cre::version` without also
        // swapping the matching `CreHeader::V*` variant, the export
        // must surface that as an error rather than emit a
        // self-contradicting file.
        let path = infinitier_test_utils::get_assets_path().join("cre/v1_0/IRONGU.cre");
        let mut cre = CreImporter { name: "x" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        cre.version = CreVersion::V2_2;
        let mut produced = Vec::new();
        let err = CreExporter.export(&cre, &mut produced).unwrap_err();
        assert!(err.to_string().contains("doesn't match CreHeader variant"));
    }
}
