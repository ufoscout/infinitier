//! CRE file writer.
//!
//! Round-trip semantics: re-importing the exported bytes yields a
//! [`Cre`] that is **struct-equal** to the source. Bytes outside the
//! parsed sections (e.g. trailing padding written by some toolchains)
//! aren't surfaced as fields and won't survive — the importer
//! ignores them, so struct-equality still holds.
//!
//! Section offsets and counts are NOT stored on the header struct — the
//! V1-family writer recomputes them from the sub-section sizes and lays
//! the sections out contiguously after the fixed-width header, so adding
//! or removing records (e.g. an op233 effect) can never desync a stored
//! offset. (V2.2 / IWD2 still places sections at the offsets read back
//! from the header bytes.)

use std::io::{self, BufWriter, Write};
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use log::debug;

use crate::{
    Cre, CreHeader, CreVersion, EffectList, Item, Iwd2Table, KnownSpell, MemorizedSpell,
    SpellMemorizationInfo, SubSections, V1SubSections, V22SubSections,
    header::{
        serialize_header_v1_0, serialize_header_v1_2, serialize_header_v2_2, serialize_header_v9_0,
    },
};

const KNOWN_SPELL_LEN: usize = 12;
const SPELL_MEMORIZATION_INFO_LEN: usize = 16;
const MEMORIZED_SPELL_LEN: usize = 12;
const ITEM_LEN: usize = 20;
const IWD2_RECORD_LEN: usize = 16;

const V22_CLASS_SPELL_BASE: usize = 0x03BA;
const V22_CLASS_SPELL_COUNT_BASE: usize = 0x04B6;
const V22_DOMAIN_SPELL_BASE: usize = 0x05B2;
const V22_DOMAIN_SPELL_COUNT_BASE: usize = 0x05D6;
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

    // Serialise the typed header into its fixed-width byte form. The
    // section pointers/counts are NOT in the header struct — they are
    // recomputed here from the actual sub-section sizes and patched in,
    // so the layout always matches the data (adding/removing records
    // can never desync a stored offset).
    let header_bytes = serialize_header(&cre.header);

    let buf = match (cre.version, &cre.sub_sections) {
        (CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0, SubSections::V1(s)) => {
            serialize_v1(cre.version, &header_bytes, s)
        }
        (CreVersion::V2_2, SubSections::V22(s)) => serialize_v22(&header_bytes, s),
        (v, _) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CRE sub-sections variant doesn't match version {v:?}"),
            ));
        }
    };

    debug!("Serialised CRE ({:?}): total={} B", cre.version, buf.len());
    Ok(buf)
}

// ─────────────────────────────────────────────────────────────────────
//  V1.0 / V1.2 / V9.0 — recomputed contiguous layout
// ─────────────────────────────────────────────────────────────────────

/// Computed section table for a V1-family CRE: every offset/count is
/// derived from the sub-section sizes, never read from the header.
struct V1Layout {
    known_spells_off: u32,
    known_spells_cnt: u32,
    spell_mem_off: u32,
    spell_mem_cnt: u32,
    mem_spells_off: u32,
    mem_spells_cnt: u32,
    item_slots_off: u32,
    items_off: u32,
    items_cnt: u32,
    effects_off: u32,
    effects_cnt: u32,
    file_size: usize,
}

/// Lay the sub-sections out contiguously after the fixed-width header
/// (order: known spells, spell-memorization info, memorized spells, item
/// slots, items, effects), computing each offset and count. Item slots
/// is followed by items so the importer's gap-derived slot length comes
/// out as `item_slots.len()`.
fn compute_v1_layout(version: CreVersion, s: &V1SubSections) -> V1Layout {
    let mut pos = version.header_len() as u32;
    // An empty section stores offset 0 (the engine/NI convention), not the
    // current write position — matching it is required for a byte-faithful
    // round-trip. A present section advances `pos` past its bytes.
    let mut place = |len: usize| -> u32 {
        if len == 0 {
            return 0;
        }
        let off = pos;
        pos += len as u32;
        off
    };
    let known_spells_off = place(s.known_spells.len() * KNOWN_SPELL_LEN);
    let spell_mem_off = place(s.spell_memorization_info.len() * SPELL_MEMORIZATION_INFO_LEN);
    let mem_spells_off = place(s.memorized_spells.len() * MEMORIZED_SPELL_LEN);
    let item_slots_off = place(s.item_slots.len());
    let items_off = place(s.items.len() * ITEM_LEN);
    let effects_off = place(s.effects.len() * s.effects.record_size());
    let file_size = pos as usize;
    V1Layout {
        known_spells_off,
        known_spells_cnt: s.known_spells.len() as u32,
        spell_mem_off,
        spell_mem_cnt: s.spell_memorization_info.len() as u32,
        mem_spells_off,
        mem_spells_cnt: s.memorized_spells.len() as u32,
        item_slots_off,
        items_off,
        items_cnt: s.items.len() as u32,
        effects_off,
        effects_cnt: s.effects.len() as u32,
        file_size,
    }
}

fn serialize_v1(version: CreVersion, header_bytes: &[u8], s: &V1SubSections) -> Vec<u8> {
    let layout = compute_v1_layout(version, s);
    let mut buf = vec![0u8; layout.file_size];
    buf[..header_bytes.len()].copy_from_slice(header_bytes);
    write_v1_section_table(&mut buf, version, &layout);
    write_v1_sub_sections(&mut buf, &layout, s);
    buf
}

/// Patch the computed offsets/counts into the header's section table.
fn write_v1_section_table(buf: &mut [u8], version: CreVersion, l: &V1Layout) {
    let base = v1_section_table_base(version);
    let mut put = |delta: usize, val: u32| {
        buf[base + delta..base + delta + 4].copy_from_slice(&val.to_le_bytes());
    };
    put(0x00, l.known_spells_off);
    put(0x04, l.known_spells_cnt);
    put(0x08, l.spell_mem_off);
    put(0x0C, l.spell_mem_cnt);
    put(0x10, l.mem_spells_off);
    put(0x14, l.mem_spells_cnt);
    put(0x18, l.item_slots_off);
    put(0x1C, l.items_off);
    put(0x20, l.items_cnt);
    put(0x24, l.effects_off);
    put(0x28, l.effects_cnt);
}

// ─────────────────────────────────────────────────────────────────────
//  V2.2 — IWD2 recomputed contiguous layout
// ─────────────────────────────────────────────────────────────────────

/// Computed IWD2 section table — every offset/count derived from the
/// sub-section sizes. A spell/tail table is "present" (gets a non-zero
/// offset and its 8-byte trailer) exactly when it isn't the default
/// (empty) table, so absent and present-but-empty tables both round-trip.
struct V22Layout {
    class_spell_offsets: [[u32; 9]; 7],
    class_spell_counts: [[u32; 9]; 7],
    domain_spell_offsets: [u32; 9],
    domain_spell_counts: [u32; 9],
    abilities_off: u32,
    abilities_cnt: u32,
    song_off: u32,
    song_cnt: u32,
    shapes_off: u32,
    shapes_cnt: u32,
    item_slots_off: u32,
    items_off: u32,
    items_cnt: u32,
    effects_off: u32,
    effects_cnt: u32,
    file_size: usize,
}

/// Place one IWD2 spell/tail table if it was present in the source file,
/// advancing `pos`. Returns `(offset, record_count)`.
///
/// A table is "present" when it was stored at a non-zero offset in the
/// original CRE — even when it carries zero entries (IWD2 always emits
/// 8-byte trailers for all 63 class-spell and 9 domain-spell slots).
/// Absent tables (`present == false`) get `(0, 0)` — no bytes written.
fn place_iwd2_table(pos: &mut u32, t: &Iwd2Table) -> (u32, u32) {
    if !t.present {
        return (0, 0);
    }
    let off = *pos;
    *pos += (t.entries.len() * IWD2_RECORD_LEN + 8) as u32;
    (off, t.entries.len() as u32)
}

fn compute_v22_layout(s: &V22SubSections) -> V22Layout {
    let mut pos = CreVersion::V2_2.header_len() as u32;
    let class_lists: [&[Iwd2Table; 9]; 7] = [
        &s.bard_spells,
        &s.cleric_spells,
        &s.druid_spells,
        &s.paladin_spells,
        &s.ranger_spells,
        &s.sorcerer_spells,
        &s.wizard_spells,
    ];
    let mut class_spell_offsets = [[0u32; 9]; 7];
    let mut class_spell_counts = [[0u32; 9]; 7];
    for (c, list) in class_lists.iter().enumerate() {
        for (l, table) in list.iter().enumerate() {
            let (off, cnt) = place_iwd2_table(&mut pos, table);
            class_spell_offsets[c][l] = off;
            class_spell_counts[c][l] = cnt;
        }
    }
    let mut domain_spell_offsets = [0u32; 9];
    let mut domain_spell_counts = [0u32; 9];
    for (i, table) in s.domain_spells.iter().enumerate() {
        let (off, cnt) = place_iwd2_table(&mut pos, table);
        domain_spell_offsets[i] = off;
        domain_spell_counts[i] = cnt;
    }
    let (abilities_off, abilities_cnt) = place_iwd2_table(&mut pos, &s.abilities);
    let (song_off, song_cnt) = place_iwd2_table(&mut pos, &s.songs);
    let (shapes_off, shapes_cnt) = place_iwd2_table(&mut pos, &s.shapes);
    // Empty sections store offset 0 (engine/NI convention), not the write
    // position — needed for a byte-faithful round-trip.
    let item_slots_off = if s.item_slots.is_empty() { 0 } else { pos };
    pos += s.item_slots.len() as u32;
    let items_off = if s.items.is_empty() { 0 } else { pos };
    pos += (s.items.len() * ITEM_LEN) as u32;
    let effects_off = if s.effects.is_empty() { 0 } else { pos };
    pos += (s.effects.len() * s.effects.record_size()) as u32;
    V22Layout {
        class_spell_offsets,
        class_spell_counts,
        domain_spell_offsets,
        domain_spell_counts,
        abilities_off,
        abilities_cnt,
        song_off,
        song_cnt,
        shapes_off,
        shapes_cnt,
        item_slots_off,
        items_off,
        items_cnt: s.items.len() as u32,
        effects_off,
        effects_cnt: s.effects.len() as u32,
        file_size: pos as usize,
    }
}

fn serialize_v22(header_bytes: &[u8], s: &V22SubSections) -> Vec<u8> {
    let layout = compute_v22_layout(s);
    let mut buf = vec![0u8; layout.file_size];
    buf[..header_bytes.len()].copy_from_slice(header_bytes);
    write_v22_section_table(&mut buf, &layout);
    write_v22_sub_sections(&mut buf, &layout, s);
    buf
}

/// Patch the computed IWD2 offsets/counts into the header section tables.
fn write_v22_section_table(buf: &mut [u8], l: &V22Layout) {
    let mut put = |off: usize, val: u32| {
        buf[off..off + 4].copy_from_slice(&val.to_le_bytes());
    };
    for c in 0..7 {
        for lvl in 0..9 {
            let idx = c * 9 + lvl;
            put(
                V22_CLASS_SPELL_BASE + idx * 4,
                l.class_spell_offsets[c][lvl],
            );
            put(
                V22_CLASS_SPELL_COUNT_BASE + idx * 4,
                l.class_spell_counts[c][lvl],
            );
        }
    }
    for i in 0..9 {
        put(V22_DOMAIN_SPELL_BASE + i * 4, l.domain_spell_offsets[i]);
        put(
            V22_DOMAIN_SPELL_COUNT_BASE + i * 4,
            l.domain_spell_counts[i],
        );
    }
    put(V22_TAIL_TABLE_BASE, l.abilities_off);
    put(V22_TAIL_TABLE_BASE + 0x04, l.abilities_cnt);
    put(V22_TAIL_TABLE_BASE + 0x08, l.song_off);
    put(V22_TAIL_TABLE_BASE + 0x0C, l.song_cnt);
    put(V22_TAIL_TABLE_BASE + 0x10, l.shapes_off);
    put(V22_TAIL_TABLE_BASE + 0x14, l.shapes_cnt);
    put(V22_TAIL_TABLE_BASE + 0x18, l.item_slots_off);
    put(V22_TAIL_TABLE_BASE + 0x1C, l.items_off);
    put(V22_TAIL_TABLE_BASE + 0x20, l.items_cnt);
    put(V22_TAIL_TABLE_BASE + 0x24, l.effects_off);
    put(V22_TAIL_TABLE_BASE + 0x28, l.effects_cnt);
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

fn write_v1_sub_sections(buf: &mut [u8], l: &V1Layout, s: &V1SubSections) {
    write_known_spells(buf, l.known_spells_off as usize, &s.known_spells);
    write_spell_memorization_info(buf, l.spell_mem_off as usize, &s.spell_memorization_info);
    write_memorized_spells(buf, l.mem_spells_off as usize, &s.memorized_spells);
    // Item-slots are an opaque indices buffer.
    let item_slots_off = l.item_slots_off as usize;
    let n = s.item_slots.len();
    if n > 0 {
        buf[item_slots_off..item_slots_off + n].copy_from_slice(&s.item_slots);
    }
    write_items(buf, l.items_off as usize, &s.items);
    write_effects(buf, l.effects_off as usize, &s.effects);
}

// ─────────────────────────────────────────────────────────────────────
//  V2.2 writer
// ─────────────────────────────────────────────────────────────────────

fn write_v22_sub_sections(buf: &mut [u8], l: &V22Layout, s: &V22SubSections) {
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
        for (lvl, table) in list.iter().enumerate() {
            write_iwd2_table(buf, l.class_spell_offsets[c][lvl], table);
        }
    }
    for (i, table) in s.domain_spells.iter().enumerate() {
        write_iwd2_table(buf, l.domain_spell_offsets[i], table);
    }
    write_iwd2_table(buf, l.abilities_off, &s.abilities);
    write_iwd2_table(buf, l.song_off, &s.songs);
    write_iwd2_table(buf, l.shapes_off, &s.shapes);
    let item_slots_off = l.item_slots_off as usize;
    let n = s.item_slots.len();
    if n > 0 {
        buf[item_slots_off..item_slots_off + n].copy_from_slice(&s.item_slots);
    }
    write_items(buf, l.items_off as usize, &s.items);
    write_effects(buf, l.effects_off as usize, &s.effects);
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
                let r = e.to_record();
                let n = r.len().min(48);
                buf[off..off + n].copy_from_slice(&r[..n]);
            }
        }
        EffectList::V2(list) => {
            for (i, e) in list.iter().enumerate() {
                let off = offset + i * 264;
                // Each variant rebuilds its 264-byte record: typed
                // effects serialise their fields, template variants
                // (proficiency / local variable) rebuild from a
                // canonical template, `Raw` is emitted verbatim.
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
        buf[off..off + 4].copy_from_slice(&slot.index.to_le_bytes());
        buf[off + 4..off + 8].copy_from_slice(&slot.memorized.to_le_bytes());
        buf[off + 8..off + 12].copy_from_slice(&slot.memorizable.to_le_bytes());
        buf[off + 12..off + 16].copy_from_slice(&slot.flags.to_le_bytes());
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

    use infinitier_common::Game;

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
            let original = CreImporter {
                name: &name,
                game: Game::Bgee,
            }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("import {name}: {e}"));

            let mut produced: Vec<u8> = Vec::new();
            CreExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {name}: {e}"));

            let re_imported = CreImporter {
                name: &name,
                game: Game::Bgee,
            }
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
            let original = CreImporter {
                name: fixture,
                game: Game::Bgee,
            }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
            let tmp = tempfile::NamedTempFile::new().unwrap();
            CreExporter.export_to_file(&original, tmp.path()).unwrap();
            let re_imported = CreImporter {
                name: fixture,
                game: Game::Bgee,
            }
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
        let original = CreImporter {
            name: "sig",
            game: Game::Bgee,
        }
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
        let mut cre = CreImporter {
            name: "x",
            game: Game::Bgee,
        }
        .import(&DataSource::new(path.as_path()))
        .unwrap();
        cre.version = CreVersion::V2_2;
        let mut produced = Vec::new();
        let err = CreExporter.export(&cre, &mut produced).unwrap_err();
        assert!(err.to_string().contains("doesn't match CreHeader variant"));
    }
}
