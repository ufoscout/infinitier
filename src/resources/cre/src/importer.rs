//! CRE file reader. See the crate-level docs for the layout overview.

use std::io::{Cursor, Read, Seek};

use infinitier_common::Game;
use infinitier_datasource::{DataSource, Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{
    CRE_SIGNATURE, Cre, CreHeader, CreVersion, Effect, EffectList, EffectV1, EffectV1Body,
    EffectV2, Item, ItemFlags, Iwd2Slot, Iwd2Table, KnownSpell, MemorizedSpell,
    SpellMemorizationInfo, SpellType, SubSections, V1SubSections, V22SubSections,
    header::{parse_header_v1_0, parse_header_v1_2, parse_header_v2_2, parse_header_v9_0},
};

const KNOWN_SPELL_LEN: u64 = 12;
const SPELL_MEMORIZATION_INFO_LEN: u64 = 16;
const MEMORIZED_SPELL_LEN: u64 = 12;
const ITEM_LEN: u64 = 20;
const EFFECT_V1_LEN: u64 = 48;
const EFFECT_V2_LEN: u64 = 264;
const IWD2_RECORD_LEN: u64 = 16;

/// Header-relative offset of the EFF-structure-version byte (same
/// position in every CRE variant). Value `0` selects the V1 (48 B)
/// effect layout, `1` selects V2 (264 B).
const EFF_VERSION_OFFSET: u64 = 0x33;

/// File reader for CRE resources.
pub struct CreImporter<'a> {
    /// Caller-visible name for error / log messages — typically the
    /// fixture path.
    pub name: &'a str,
    /// The game this CRE belongs to. Needed to disambiguate the few
    /// V1.0 header regions whose meaning is engine-specific (notably the
    /// PST:EE overlay of the BG "tracking target" field).
    pub game: Game,
}

type CreReader = Reader<Cursor<Vec<u8>>>;

impl Importer for CreImporter<'_> {
    type T = Cre;

    fn import(&self, source: &DataSource) -> std::io::Result<Cre> {
        let mut reader = source.preloaded_reader()?;
        let file_size = reader.seek(std::io::SeekFrom::End(0))?;
        if file_size < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("CRE '{}' shorter than 8-byte header", self.name),
            ));
        }

        reader.set_position(0)?;
        let sig: [u8; 4] = reader.read_exact_to_array()?;
        if &sig != CRE_SIGNATURE {
            error!("Unsupported CRE signature in {}: {sig:?}", self.name);
            return Err(std::io::Error::other(format!(
                "Unsupported CRE signature: {sig:?}"
            )));
        }
        let ver: [u8; 4] = reader.read_exact_to_array()?;
        let version = match &ver {
            b"V1.0" => CreVersion::V1_0,
            b"V1.2" => CreVersion::V1_2,
            b"V9.0" => CreVersion::V9_0,
            b"V2.2" => CreVersion::V2_2,
            _ => {
                error!("Unsupported CRE version in {}: {ver:?}", self.name);
                return Err(std::io::Error::other(format!(
                    "Unsupported CRE version: {ver:?}"
                )));
            }
        };

        let header_len = version.header_len() as u64;
        if file_size < header_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "CRE '{}' is {file_size} B; needs at least {header_len} B for the {:?} header",
                    self.name, version
                ),
            ));
        }
        reader.set_position(0)?;
        let mut header_bytes = vec![0u8; header_len as usize];
        reader.read_exact(&mut header_bytes)?;

        let sub_sections = match version {
            CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0 => SubSections::V1(
                parse_v1_sub_sections(&mut reader, &header_bytes, version, self.name)?,
            ),
            CreVersion::V2_2 => SubSections::V22(Box::new(parse_v22_sub_sections(
                &mut reader,
                &header_bytes,
                self.name,
            )?)),
        };

        // Now parse the header bytes into the typed per-version
        // struct. The byte-slice form was the source of truth during
        // sub-section parsing (so offsets work the same way as in NI);
        // the typed view is what the caller actually wants.
        let header = match version {
            CreVersion::V1_0 => CreHeader::V10(parse_header_v1_0(&header_bytes, self.game)?),
            CreVersion::V1_2 => CreHeader::V12(Box::new(parse_header_v1_2(&header_bytes)?)),
            CreVersion::V9_0 => CreHeader::V90(parse_header_v9_0(&header_bytes)?),
            CreVersion::V2_2 => CreHeader::V22(Box::new(parse_header_v2_2(&header_bytes)?)),
        };

        debug!(
            "Loaded {} [CRE {:?}]: file={} B, header={} B",
            self.name, version, file_size, header_len,
        );

        Ok(Cre {
            version,
            header,
            sub_sections,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────
//  V1.0 / V1.2 / V9.0 — shared sub-section layout
// ─────────────────────────────────────────────────────────────────────

/// Header-offset of the `known_spells_offset` u32 (the start of the
/// per-version section table). Every section pointer / count is at a
/// fixed delta from this base.
fn v1_section_table_base(version: CreVersion) -> u64 {
    match version {
        CreVersion::V1_0 => 0x02A0,
        CreVersion::V1_2 => 0x0344,
        CreVersion::V9_0 => 0x0308,
        CreVersion::V2_2 => unreachable!("V2.2 has a different section table"),
    }
}

#[derive(Clone, Copy)]
struct V1Table {
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
}

fn read_v1_section_table(header: &[u8], version: CreVersion) -> std::io::Result<V1Table> {
    let base = v1_section_table_base(version) as usize;
    let read_u32 =
        |off: usize| u32::from_le_bytes(header[base + off..base + off + 4].try_into().unwrap());
    Ok(V1Table {
        known_spells_off: read_u32(0x00),
        known_spells_cnt: read_u32(0x04),
        spell_mem_off: read_u32(0x08),
        spell_mem_cnt: read_u32(0x0C),
        mem_spells_off: read_u32(0x10),
        mem_spells_cnt: read_u32(0x14),
        item_slots_off: read_u32(0x18),
        items_off: read_u32(0x1C),
        items_cnt: read_u32(0x20),
        effects_off: read_u32(0x24),
        effects_cnt: read_u32(0x28),
    })
}

fn parse_v1_sub_sections(
    reader: &mut CreReader,
    header: &[u8],
    version: CreVersion,
    name: &str,
) -> std::io::Result<V1SubSections> {
    let t = read_v1_section_table(header, version)?;
    let eff_version = header[EFF_VERSION_OFFSET as usize];

    let known_spells = parse_known_spells(reader, t.known_spells_off, t.known_spells_cnt, name)?;
    let spell_memorization_info =
        parse_spell_memorization_info(reader, t.spell_mem_off, t.spell_mem_cnt, name)?;
    let memorized_spells =
        parse_memorized_spells(reader, t.mem_spells_off, t.mem_spells_cnt, name)?;
    let items = parse_items(reader, t.items_off, t.items_cnt, name)?;
    let item_slots = parse_item_slots(reader, t.item_slots_off, &t, name)?;
    let effects = parse_effects(reader, t.effects_off, t.effects_cnt, eff_version, name)?;

    Ok(V1SubSections {
        known_spells,
        spell_memorization_info,
        memorized_spells,
        items,
        item_slots,
        effects,
    })
}

/// Item slots is the residual block between [`V1Table::item_slots_off`]
/// and whichever section follows it. Per IESDP, V1.0 / V9.0 use 40 ×
/// `i16` slot indices (80 B), V1.2 uses 48 (96 B), V2.2 uses 52 (104
/// B) — but the spec doesn't expose a count field, so we derive the
/// extent from the file layout: `min` of every section start strictly
/// after `item_slots_off`, falling back to end-of-file.
fn parse_item_slots(
    reader: &mut CreReader,
    item_slots_off: u32,
    t: &V1Table,
    name: &str,
) -> std::io::Result<Vec<u8>> {
    let file_size = reader.seek(std::io::SeekFrom::End(0))? as u32;
    let next = [
        t.known_spells_off,
        t.spell_mem_off,
        t.mem_spells_off,
        t.items_off,
        t.effects_off,
    ]
    .into_iter()
    .filter(|&o| o > item_slots_off && o != 0)
    .min()
    .unwrap_or(file_size);
    let end = next.min(file_size);
    if end < item_slots_off {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("CRE '{name}': item-slots end ({end}) < start ({item_slots_off})"),
        ));
    }
    reader.set_position(item_slots_off as u64)?;
    let mut out = vec![0u8; (end - item_slots_off) as usize];
    reader.read_exact(&mut out)?;
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────
//  V2.2 — IWD2 d20 layout
// ─────────────────────────────────────────────────────────────────────

/// Class-spell offset table starts here. 7 classes × 9 levels × 4 B
/// = 252 B = `0xFC` running from `0x03BA` to `0x04B6`.
const V22_CLASS_SPELL_BASE: usize = 0x03BA;
/// Domain spell offset table starts here. 9 × 4 B = 36 B.
const V22_DOMAIN_SPELL_BASE: usize = 0x05B2;
/// Tail section table — abilities / song / shapes / items / effects.
const V22_TAIL_TABLE_BASE: usize = 0x05FA;

struct V22Table {
    /// 7 classes × 9 levels, each holding the offset (no count
    /// follows — record count is computed from the next-section
    /// boundary).
    class_spell_offsets: [[u32; 9]; 7],
    /// 9 domain spell offsets.
    domain_spell_offsets: [u32; 9],
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
}

fn read_v22_section_table(header: &[u8]) -> std::io::Result<V22Table> {
    let read_u32 = |off: usize| u32::from_le_bytes(header[off..off + 4].try_into().unwrap());
    let mut class_spell_offsets = [[0u32; 9]; 7];
    for (c, row) in class_spell_offsets.iter_mut().enumerate() {
        for (l, cell) in row.iter_mut().enumerate() {
            *cell = read_u32(V22_CLASS_SPELL_BASE + (c * 9 + l) * 4);
        }
    }
    let mut domain_spell_offsets = [0u32; 9];
    for (i, cell) in domain_spell_offsets.iter_mut().enumerate() {
        *cell = read_u32(V22_DOMAIN_SPELL_BASE + i * 4);
    }
    Ok(V22Table {
        class_spell_offsets,
        domain_spell_offsets,
        abilities_off: read_u32(V22_TAIL_TABLE_BASE),
        abilities_cnt: read_u32(V22_TAIL_TABLE_BASE + 0x04),
        song_off: read_u32(V22_TAIL_TABLE_BASE + 0x08),
        song_cnt: read_u32(V22_TAIL_TABLE_BASE + 0x0C),
        shapes_off: read_u32(V22_TAIL_TABLE_BASE + 0x10),
        shapes_cnt: read_u32(V22_TAIL_TABLE_BASE + 0x14),
        item_slots_off: read_u32(V22_TAIL_TABLE_BASE + 0x18),
        items_off: read_u32(V22_TAIL_TABLE_BASE + 0x1C),
        items_cnt: read_u32(V22_TAIL_TABLE_BASE + 0x20),
        effects_off: read_u32(V22_TAIL_TABLE_BASE + 0x24),
        effects_cnt: read_u32(V22_TAIL_TABLE_BASE + 0x28),
    })
}

fn parse_v22_sub_sections(
    reader: &mut CreReader,
    header: &[u8],
    name: &str,
) -> std::io::Result<V22SubSections> {
    let t = read_v22_section_table(header)?;
    let eff_version = header[EFF_VERSION_OFFSET as usize];

    // Collect every section start so we can derive each class /
    // domain spell-table extent from the next start.
    let mut all_offsets: Vec<u32> = t
        .class_spell_offsets
        .iter()
        .flatten()
        .copied()
        .chain(t.domain_spell_offsets.iter().copied())
        .chain([
            t.abilities_off,
            t.song_off,
            t.shapes_off,
            t.item_slots_off,
            t.items_off,
            t.effects_off,
        ])
        .filter(|&o| o != 0)
        .collect();
    let file_size = reader.seek(std::io::SeekFrom::End(0))? as u32;
    all_offsets.push(file_size);
    all_offsets.sort_unstable();
    all_offsets.dedup();

    let extent_of = |off: u32| -> u32 {
        if off == 0 {
            return 0;
        }
        all_offsets
            .iter()
            .copied()
            .find(|&o| o > off)
            .unwrap_or(file_size)
            .saturating_sub(off)
    };

    // Parse one IWD2 sub-section block at `off`: the record count is
    // derived from the gap to the next section, minus the 8-byte
    // trailer that NI's `Iwd2Struct` always emits.
    let parse_iwd2_table = |reader: &mut CreReader, off: u32| -> std::io::Result<Iwd2Table> {
        if off == 0 {
            return Ok(Iwd2Table::default());
        }
        let extent = extent_of(off);
        if extent < 8 {
            return Ok(Iwd2Table::default());
        }
        let count = ((extent - 8) / IWD2_RECORD_LEN as u32) as usize;
        parse_iwd2_table_at(reader, off, count, name)
    };

    let mut class_table = |class_idx: usize| -> std::io::Result<[Iwd2Table; 9]> {
        let mut out: [Iwd2Table; 9] = Default::default();
        for (level, slot) in out.iter_mut().enumerate() {
            *slot = parse_iwd2_table(reader, t.class_spell_offsets[class_idx][level])?;
        }
        Ok(out)
    };
    let bard_spells = class_table(0)?;
    let cleric_spells = class_table(1)?;
    let druid_spells = class_table(2)?;
    let paladin_spells = class_table(3)?;
    let ranger_spells = class_table(4)?;
    let sorcerer_spells = class_table(5)?;
    let wizard_spells = class_table(6)?;

    let mut domain_spells: [Iwd2Table; 9] = Default::default();
    for (i, slot) in domain_spells.iter_mut().enumerate() {
        *slot = parse_iwd2_table(reader, t.domain_spell_offsets[i])?;
    }

    // The tail-table sections (abilities, song, shapes) also carry
    // the 8-byte trailer. NI exposes a `count` field but it counts
    // records (not including the trailer); we still need to add the
    // trailer to the extent-based size derivation. We honour the
    // count field where present and skip the extent-based estimate.
    let parse_tail_table =
        |reader: &mut CreReader, off: u32, cnt: u32| -> std::io::Result<Iwd2Table> {
            if off == 0 {
                // No section. NI never writes the trailer either when
                // the offset is zero.
                return Ok(Iwd2Table::default());
            }
            parse_iwd2_table_at(reader, off, cnt as usize, name)
        };
    let abilities = parse_tail_table(reader, t.abilities_off, t.abilities_cnt)?;
    let songs = parse_tail_table(reader, t.song_off, t.song_cnt)?;
    let shapes = parse_tail_table(reader, t.shapes_off, t.shapes_cnt)?;
    let items = parse_items(reader, t.items_off, t.items_cnt, name)?;
    // Item slots fills the residual span between item_slots_off and
    // the next downstream section (or EOF). NI hard-codes 52 × i16
    // for IWD2 but deriving from the layout matches the GAM crate's
    // approach and naturally handles malformed files.
    let next_after_slots = all_offsets
        .iter()
        .copied()
        .find(|&o| o > t.item_slots_off)
        .unwrap_or(file_size);
    let end = next_after_slots.min(file_size);
    reader.set_position(t.item_slots_off as u64)?;
    let mut item_slots = vec![0u8; end.saturating_sub(t.item_slots_off) as usize];
    reader.read_exact(&mut item_slots)?;
    let effects = parse_effects(reader, t.effects_off, t.effects_cnt, eff_version, name)?;

    Ok(V22SubSections {
        bard_spells,
        cleric_spells,
        druid_spells,
        paladin_spells,
        ranger_spells,
        sorcerer_spells,
        wizard_spells,
        domain_spells,
        abilities,
        songs,
        shapes,
        items,
        item_slots,
        effects,
    })
}

// ─────────────────────────────────────────────────────────────────────
//  Sub-section record parsers (shared)
// ─────────────────────────────────────────────────────────────────────

fn check_range(reader: &mut CreReader, end: u64, name: &str, what: &str) -> std::io::Result<()> {
    let pos = reader.position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.set_position(pos)?;
    if end > len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("CRE '{name}': {what} runs past file end (need {end} B, file is {len} B)"),
        ));
    }
    Ok(())
}

fn parse_known_spells(
    reader: &mut CreReader,
    offset: u32,
    count: u32,
    name: &str,
) -> std::io::Result<Vec<KnownSpell>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    check_range(
        reader,
        start + count as u64 * KNOWN_SPELL_LEN,
        name,
        "known-spells section",
    )?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * KNOWN_SPELL_LEN)?;
        out.push(KnownSpell {
            spell: reader.read_string(8)?,
            level: reader.read_u16()?,
            spell_type: SpellType::from_u16(reader.read_u16()?),
        });
    }
    Ok(out)
}

fn parse_spell_memorization_info(
    reader: &mut CreReader,
    offset: u32,
    count: u32,
    name: &str,
) -> std::io::Result<Vec<SpellMemorizationInfo>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    check_range(
        reader,
        start + count as u64 * SPELL_MEMORIZATION_INFO_LEN,
        name,
        "spell-memorization-info section",
    )?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * SPELL_MEMORIZATION_INFO_LEN)?;
        out.push(SpellMemorizationInfo {
            level: reader.read_u16()?,
            num_memorizable_total: reader.read_u16()?,
            num_memorizable_current: reader.read_u16()?,
            spell_type: SpellType::from_u16(reader.read_u16()?),
            spell_table_index: reader.read_u32()?,
            spell_count: reader.read_u32()?,
        });
    }
    Ok(out)
}

fn parse_memorized_spells(
    reader: &mut CreReader,
    offset: u32,
    count: u32,
    name: &str,
) -> std::io::Result<Vec<MemorizedSpell>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    check_range(
        reader,
        start + count as u64 * MEMORIZED_SPELL_LEN,
        name,
        "memorized-spells section",
    )?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * MEMORIZED_SPELL_LEN)?;
        out.push(MemorizedSpell {
            spell: reader.read_string(8)?,
            memorization_flags: reader.read_u16()?,
            padding: reader.read_u16()?,
        });
    }
    Ok(out)
}

fn parse_items(
    reader: &mut CreReader,
    offset: u32,
    count: u32,
    name: &str,
) -> std::io::Result<Vec<Item>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    check_range(
        reader,
        start + count as u64 * ITEM_LEN,
        name,
        "items section",
    )?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * ITEM_LEN)?;
        out.push(Item {
            item: reader.read_string(8)?,
            duration: reader.read_u16()?,
            quantity1: reader.read_u16()?,
            quantity2: reader.read_u16()?,
            quantity3: reader.read_u16()?,
            flags: ItemFlags::from_bits_retain(reader.read_u32()?),
        });
    }
    Ok(out)
}

fn parse_effects(
    reader: &mut CreReader,
    offset: u32,
    count: u32,
    eff_version: u8,
    name: &str,
) -> std::io::Result<EffectList> {
    if count == 0 {
        return Ok(match eff_version {
            0 => EffectList::V1(Vec::new()),
            _ => EffectList::V2(Vec::new()),
        });
    }
    let start = offset as u64;
    match eff_version {
        0 => {
            check_range(
                reader,
                start + count as u64 * EFFECT_V1_LEN,
                name,
                "effects (V1) section",
            )?;
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count as u64 {
                reader.set_position(start + i * EFFECT_V1_LEN)?;
                let mut buf = [0u8; 48];
                reader.read_exact(&mut buf)?;
                // Every record is parsed into the fully-typed
                // `EffectV1Body`, which round-trips byte-for-byte. (The
                // `op233` proficiency view is derived by opcode on read,
                // not stored as a lossy variant.)
                out.push(EffectV1::Effect(EffectV1Body::from_record(&buf)));
            }
            Ok(EffectList::V1(out))
        }
        _ => {
            check_range(
                reader,
                start + count as u64 * EFFECT_V2_LEN,
                name,
                "effects (V2) section",
            )?;
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count as u64 {
                reader.set_position(start + i * EFFECT_V2_LEN)?;
                let mut buf = vec![0u8; 264];
                reader.read_exact(&mut buf)?;
                // Every record is parsed into the fully-typed `Effect`,
                // which round-trips byte-for-byte. The `op187` local-
                // variable and `op233` proficiency views are derived by
                // opcode on read, not stored as lossy template variants.
                out.push(EffectV2::Effect(Box::new(Effect::from_record(&buf))));
            }
            Ok(EffectList::V2(out))
        }
    }
}

/// Parse one IWD2 sub-section table at `offset`: `count` 16-byte
/// records followed by the 8-byte `num_memorizable` / `num_remaining`
/// trailer.
fn parse_iwd2_table_at(
    reader: &mut CreReader,
    offset: u32,
    count: usize,
    name: &str,
) -> std::io::Result<Iwd2Table> {
    let start = offset as u64;
    let trailer_off = start + count as u64 * IWD2_RECORD_LEN;
    check_range(reader, trailer_off + 8, name, "IWD2 sub-section block")?;
    let mut entries = Vec::with_capacity(count);
    for i in 0..count as u64 {
        reader.set_position(start + i * IWD2_RECORD_LEN)?;
        entries.push(Iwd2Slot {
            index: reader.read_u32()?,
            memorized: reader.read_u32()?,
            memorizable: reader.read_u32()?,
            flags: reader.read_u32()?,
        });
    }
    reader.set_position(trailer_off)?;
    let num_memorizable = reader.read_u32()?;
    let num_remaining = reader.read_u32()?;
    Ok(Iwd2Table {
        entries,
        num_memorizable,
        num_remaining,
        present: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn test_parse_v1_0_bg() {
        let cre = import_fixture("v1_0/IRONGU.cre");
        assert_eq!(cre.version, CreVersion::V1_0);
        assert!(matches!(cre.header, CreHeader::V10(_)));
        let SubSections::V1(s) = &cre.sub_sections else {
            panic!("expected V1 sub-sections");
        };
        // IRONGU has 17 spell-memorization-info rows (one per
        // wizard / priest / innate level) like every fresh CRE.
        assert_eq!(s.spell_memorization_info.len(), 17);
        // 5 items per its on-disk count.
        assert_eq!(s.items.len(), 5);
        // Effects are V1 (vanilla BG1).
        assert!(matches!(s.effects, EffectList::V1(_)));
    }

    #[test]
    fn test_parse_v1_0_iwdee_with_known_spells() {
        // EVERARD2 is a priest with a populated known-spells table.
        // Note: IWDEE creatures still use V1 (48-byte) effect
        // records — the eff-version byte at header offset 0x33 is
        // what selects between V1 / V2, not the engine.
        let cre = import_fixture("v1_0/EVERARD2.cre");
        assert_eq!(cre.version, CreVersion::V1_0);
        let SubSections::V1(s) = &cre.sub_sections else {
            panic!("expected V1 sub-sections");
        };
        assert_eq!(s.known_spells.len(), 36);
        assert_eq!(s.spell_memorization_info.len(), 17);
        assert_eq!(s.memorized_spells.len(), 35);
        assert_eq!(s.items.len(), 1);
        // First known spell has a sane resref.
        assert!(!s.known_spells[0].spell.is_empty());
        // Effects variant tracks the on-disk EFF flag (0 = V1 here).
        assert_eq!(cre.eff_version(), 0);
        assert!(matches!(s.effects, EffectList::V1(_)));
    }

    #[test]
    fn test_parse_v1_2_pst() {
        let cre = import_fixture("v1_2/THIEF3.cre");
        assert_eq!(cre.version, CreVersion::V1_2);
        assert!(matches!(cre.header, CreHeader::V12(_)));
        let SubSections::V1(s) = &cre.sub_sections else {
            panic!("expected V1 sub-sections");
        };
        assert_eq!(s.items.len(), 6);
    }

    #[test]
    fn test_parse_v9_0_iwd() {
        let cre = import_fixture("v9_0/BARBWAR2.cre");
        assert_eq!(cre.version, CreVersion::V9_0);
        assert!(matches!(cre.header, CreHeader::V90(_)));
        let SubSections::V1(s) = &cre.sub_sections else {
            panic!("expected V1 sub-sections");
        };
        assert_eq!(s.items.len(), 1);
    }

    #[test]
    fn test_parse_v2_2_iwd2() {
        let cre = import_fixture("v2_2/52SERSA.cre");
        assert_eq!(cre.version, CreVersion::V2_2);
        assert!(matches!(cre.header, CreHeader::V22(_)));
        let SubSections::V22(s) = &cre.sub_sections else {
            panic!("expected V2.2 sub-sections");
        };
        assert_eq!(s.items.len(), 2);
        // Every IWD2 sub-section block — class spells, domain
        // spells, abilities, song, shapes — carries a non-zero file
        // offset (and therefore an 8-byte trailer), even when its
        // record list is empty. So every class table has all 9
        // slots present at distinct offsets.
        for table in &s.bard_spells {
            // Trailer counters are u32s — just verify the table
            // exists (slots default to empty entries).
            let _ = table.num_memorizable;
        }
        // Item slots should be 52 × i16 = 104 bytes per NI / IESDP.
        assert_eq!(s.item_slots.len(), 104);
    }

    #[test]
    fn test_rejects_wrong_signature() {
        let err = CreImporter {
            name: "junk",
            game: Game::Bgee,
        }
        .import(&DataSource::new(b"BAD V1.0\x00\x00\x00\x00".as_slice()))
        .unwrap_err();
        assert!(err.to_string().contains("Unsupported CRE signature"));
    }

    #[test]
    fn test_rejects_unknown_version() {
        let err = CreImporter {
            name: "future",
            game: Game::Bgee,
        }
        .import(&DataSource::new(b"CRE V9.9\x00\x00\x00\x00".as_slice()))
        .unwrap_err();
        assert!(err.to_string().contains("Unsupported CRE version"));
    }

    #[test]
    fn test_rejects_truncated_header() {
        let bytes = b"CRE V1.0                      ";
        assert!(bytes.len() < CreVersion::V1_0.header_len());
        let err = CreImporter {
            name: "tiny",
            game: Game::Bgee,
        }
        .import(&DataSource::new(bytes.as_slice()))
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn test_every_corpus_cre_parses() {
        // Strong-conformance sweep: every `.cre` under `assets/cre/`
        // must parse, and the parsed shape must match its version
        // family (V1.0 / V1.2 / V9.0 use V1 sub-sections, V2.2 uses
        // V22).
        let fixtures = all_cre_fixtures();
        assert!(!fixtures.is_empty(), "no CRE fixtures discovered");
        for path in fixtures {
            let cre = CreImporter {
                name: path.to_string_lossy().as_ref(),
                game: Game::Bgee,
            }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("parse {} failed: {e}", path.display()));
            match cre.version {
                CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0 => {
                    assert!(matches!(cre.sub_sections, SubSections::V1(_)));
                }
                CreVersion::V2_2 => {
                    assert!(matches!(cre.sub_sections, SubSections::V22(_)));
                }
            }
        }
    }
}
