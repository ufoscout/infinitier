//! GAM file reader.
//!
//! Uses [`DataSource::preloaded_reader`] to read the whole file
//! once, then drives parsing with the encoding-aware [`Reader`]
//! (random-access via `set_position`, NUL-trimmed string reads via
//! `read_string`, little-endian integer reads via [`ReadExt`]).

use std::io::{Cursor, Read, Seek};

use infinitier_common::Engine;
use infinitier_datasource::{DataSource, Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{
    Bg2GamData, BgGamData, BgSaveVersion, COMMON_HEADER_LEN, EeGamData, Familiar, GAM_SIGNATURE,
    Gam, GamEngineData, GamHeader, GamNpc, GamVariable, GamVersion, GameTicks, GameTime,
    Iwd2GamData, IwdGamData, IwdUnknownTrailer, JournalEntry, ModronMaze, ModronMazeEntry,
    NpcCharStats, PstGamData, StoredLocation, UnknownSection3, char_stats_offset_for_engine,
};

/// On-disk size of one [`GamVariable`] record: a 32-byte name, the
/// type/value slots through the 8-byte double at 0x2C, then a 32-byte
/// script-name field at 0x34 (0x34 + 32 = 0x54).
const VARIABLE_LEN: u64 = 84;

/// On-disk size of one [`JournalEntry`] record.
const JOURNAL_ENTRY_LEN: u64 = 12;

/// First 0x14 bytes of every NPC slot — the version-independent
/// sub-header that we always parse.
const NPC_HEADER_LEN: u64 = 0x14;

/// On-disk size of one [`StoredLocation`] record.
const STORED_LOCATION_LEN: u64 = 12;

/// On-disk size of one [`UnknownSection3`] record (IWD/IWD2).
const UNKNOWN_SECTION3_LEN: u64 = 24;

/// Fixed on-disk size of a [`Familiar`] struct's header + count
/// table: 9 resrefs (72 B) + 1 EOS pointer (4 B) + 9×9 counts (324 B).
const FAMILIAR_FIXED_LEN: u64 = 400;

/// Fixed on-disk size of a [`ModronMaze`]: 64 × 26-byte entries +
/// 14 × 4-byte trailing header fields.
const MODRON_MAZE_LEN: u64 = 64 * 26 + 14 * 4;

/// Fixed on-disk size of the Bestiary blob (PST).
const BESTIARY_LEN: u64 = 260;

/// Convenience alias for the preloaded reader the importer uses
/// throughout (random-access cursor over an in-memory `Vec<u8>`).
type GamReader = Reader<Cursor<Vec<u8>>>;

/// A GAM file importer.
pub struct GamImporter<'a> {
    /// Caller-visible name for error/log messages — usually the
    /// fixture path.
    pub name: &'a str,
    /// Which engine produced the file. Drives the post-0x54 layout
    /// dispatch — see [`GamEngineData`].
    pub engine: Engine,
}

impl Importer for GamImporter<'_> {
    type T = Gam;

    fn import(&self, source: &DataSource) -> std::io::Result<Gam> {
        let mut reader = source.preloaded_reader()?;
        let file_size = reader.seek(std::io::SeekFrom::End(0))?;

        if file_size < 8 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!("GAM '{}' shorter than 8-byte header", self.name),
            ));
        }
        // Validate signature.
        reader.set_position(0)?;
        let sig: [u8; 4] = reader.read_exact_to_array()?;
        if &sig != GAM_SIGNATURE {
            error!("Unsupported GAM signature in {}: {sig:?}", self.name);
            return Err(std::io::Error::other(format!(
                "Unsupported GAM signature: {sig:?}"
            )));
        }
        // Validate version.
        let ver: [u8; 4] = reader.read_exact_to_array()?;
        let version = match &ver {
            b"V1.1" => GamVersion::V1_1,
            b"V2.0" => GamVersion::V2_0,
            b"V2.1" => GamVersion::V2_1,
            b"V2.2" => GamVersion::V2_2,
            _ => {
                error!("Unsupported GAM version in {}: {ver:?}", self.name);
                return Err(std::io::Error::other(format!(
                    "Unsupported GAM version: {ver:?}"
                )));
            }
        };

        if file_size < COMMON_HEADER_LEN as u64 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                format!(
                    "GAM '{}' is {file_size} bytes; needs at least {COMMON_HEADER_LEN} for the common header",
                    self.name,
                ),
            ));
        }
        // The common header's section *offsets* and *counts* (party /
        // non-party NPC, globals, journal, party-inventory) are file-
        // layout details we read transiently to drive parsing but do
        // not persist on `GamHeader` — the exporter recomputes them.
        let (header, table) = parse_header(&mut reader)?;
        let engine_data = parse_engine_data(&mut reader, self.engine, file_size, self.name)?;

        let variables = parse_variables(
            &mut reader,
            table.globals_offset,
            table.globals_count,
            self.name,
            "variable",
        )?;
        let journal = parse_journal(
            &mut reader,
            table.journal_offset,
            table.journal_count,
            self.name,
        )?;
        let party_inventory = extract_party_inventory(
            &mut reader,
            &table,
            header.party_inventory_count,
            file_size,
            self.name,
        )?;
        let npc_size = npc_record_size(&table, version, self.engine);
        let party_npcs = parse_npcs(
            &mut reader,
            table.party_npc_offset,
            table.party_npc_count,
            npc_size,
            "party NPC",
            self.name,
            self.engine,
        )?;
        let non_party_npcs = parse_npcs(
            &mut reader,
            table.non_party_npc_offset,
            table.non_party_npc_count,
            npc_size,
            "non-party NPC",
            self.name,
            self.engine,
        )?;

        debug!(
            "Loaded {} [GAM {:?} / {:?}]: {} party NPCs, {} non-party NPCs, {} variables, {} journal entries",
            self.name,
            version,
            self.engine,
            party_npcs.len(),
            non_party_npcs.len(),
            variables.len(),
            journal.len(),
        );

        Ok(Gam {
            version,
            header,
            engine_data,
            party_npcs,
            non_party_npcs,
            variables,
            journal,
            party_inventory,
        })
    }
}

/// Transient view of the common header's section table — the
/// offset/count pairs that locate the variable-length sections. Read
/// during import to drive parsing, then discarded: they are layout
/// details the exporter recomputes, so they are not kept on
/// [`GamHeader`].
struct SectionTable {
    party_npc_offset: u32,
    party_npc_count: u32,
    party_inventory_offset: u32,
    non_party_npc_offset: u32,
    non_party_npc_count: u32,
    globals_offset: u32,
    globals_count: u32,
    journal_offset: u32,
    journal_count: u32,
}

/// Parse the universally-shared 0x00..0x54 header. Assumes the
/// reader's cursor is positioned at offset 0x08 (right after
/// signature + version). Returns the persisted [`GamHeader`] fields
/// alongside the transient [`SectionTable`] used only during parsing.
fn parse_header(reader: &mut GamReader) -> std::io::Result<(GamHeader, SectionTable)> {
    reader.set_position(0x08)?;
    let game_time = GameTime::from_game_seconds(reader.read_u32()?);
    let selected_formation = reader.read_u16()?;
    let formation_buttons = [
        reader.read_u16()?,
        reader.read_u16()?,
        reader.read_u16()?,
        reader.read_u16()?,
        reader.read_u16()?,
    ];
    let party_gold = reader.read_u32()?;
    let active_npc_or_party_count = reader.read_u16()?;
    let weather = reader.read_u16()?;
    let party_npc_offset = reader.read_u32()?;
    let party_npc_count = reader.read_u32()?;
    let party_inventory_offset = reader.read_u32()?;
    let party_inventory_count = reader.read_u32()?;
    let non_party_npc_offset = reader.read_u32()?;
    let non_party_npc_count = reader.read_u32()?;
    let globals_offset = reader.read_u32()?;
    let globals_count = reader.read_u32()?;
    let world_area = reader.read_string(8)?;
    let current_link = reader.read_u32()?;
    let journal_count = reader.read_u32()?;
    let journal_offset = reader.read_u32()?;
    Ok((
        GamHeader {
            game_time,
            selected_formation,
            formation_buttons,
            party_gold,
            active_npc_or_party_count,
            weather,
            party_inventory_count,
            world_area,
            current_link,
        },
        SectionTable {
            party_npc_offset,
            party_npc_count,
            party_inventory_offset,
            non_party_npc_offset,
            non_party_npc_count,
            globals_offset,
            globals_count,
            journal_offset,
            journal_count,
        },
    ))
}

/// Per-NPC record size, in bytes.
///
/// The `V1.1` version string is shared by BG1, IWD and Planescape: Torment,
/// which do *not* agree on the NPC record size — BG1/IWD use 352 (`0x160`),
/// PST uses 360 (`0x168`, an 8-byte-larger struct). The version alone can't
/// tell them apart, so:
/// - PST is identified by [`Engine::Pst`] and pinned to 360. (Its derivation
///   below can't work anyway: PST has no shared party-inventory section, so
///   `party_inventory_offset` is 0 and the span underflows.)
/// - For BG1/IWD we derive the stride from the gap between the party-NPC and
///   party-inventory sections when possible, falling back to 352.
///
/// Getting this wrong silently misreads every NPC after the first (their CRE
/// pointers land mid-record), so they show up as empty/external slots.
fn npc_record_size(t: &SectionTable, version: GamVersion, engine: Engine) -> u32 {
    match version {
        GamVersion::V2_2 => 832,
        GamVersion::V2_0 | GamVersion::V2_1 => 352,
        GamVersion::V1_1 if engine == Engine::Pst => 360,
        GamVersion::V1_1 => {
            if t.party_npc_count > 0 && t.party_inventory_offset > t.party_npc_offset {
                let span = t.party_inventory_offset - t.party_npc_offset;
                let derived = span / t.party_npc_count;
                if derived >= NPC_HEADER_LEN as u32 {
                    return derived;
                }
            }
            352
        }
    }
}

fn parse_variables(
    reader: &mut GamReader,
    offset: u32,
    count: u32,
    name: &str,
    label: &str,
) -> std::io::Result<Vec<GamVariable>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    let end = start + (count as u64) * VARIABLE_LEN;
    check_range(reader, end, name, &format!("{label} section"))?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * VARIABLE_LEN)?;
        out.push(parse_variable(reader)?);
    }
    Ok(out)
}

fn parse_variable(reader: &mut GamReader) -> std::io::Result<GamVariable> {
    let pos = reader.position()?;
    let name = reader.read_string(32)?;
    reader.set_position(pos + 0x20)?;
    let type_flags = reader.read_u16()?;
    let ref_value = reader.read_u16()?;
    let dword_value = reader.read_u32()?;
    let int_value = reader.read_i32()?;
    let mut double_bytes = [0u8; 8];
    reader.set_position(pos + 0x2C)?;
    reader.read_exact(&mut double_bytes)?;
    let double_value = f64::from_le_bytes(double_bytes);
    reader.set_position(pos + 0x34)?;
    let script_name = reader.read_string(32)?;
    Ok(GamVariable {
        name,
        type_flags,
        ref_value,
        dword_value,
        int_value,
        double_value,
        script_name,
    })
}

fn parse_journal(
    reader: &mut GamReader,
    offset: u32,
    count: u32,
    name: &str,
) -> std::io::Result<Vec<JournalEntry>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    let end = start + (count as u64) * JOURNAL_ENTRY_LEN;
    check_range(reader, end, name, "journal section")?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * JOURNAL_ENTRY_LEN)?;
        out.push(JournalEntry {
            strref: reader.read_u32()?,
            time: GameTicks::from_ticks(reader.read_u32()?),
            chapter: reader.read_u8()?,
            read_by_pc: reader.read_u8()?,
            section: reader.read_u8()?,
            location_flag: reader.read_u8()?,
        });
    }
    Ok(out)
}

fn parse_npcs(
    reader: &mut GamReader,
    offset: u32,
    count: u32,
    record_size: u32,
    what: &str,
    name: &str,
    engine: Engine,
) -> std::io::Result<Vec<GamNpc>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if record_size < NPC_HEADER_LEN as u32 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "GAM '{name}': derived {what} record size {record_size} \
                 is smaller than the 0x14-byte sub-header"
            ),
        ));
    }
    let start = offset as u64;
    let end = start + (count as u64) * (record_size as u64);
    check_range(reader, end, name, &format!("{what} section"))?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        let entry_off = start + i * record_size as u64;
        out.push(parse_npc(reader, entry_off, record_size as u64, engine)?);
    }
    Ok(out)
}

fn parse_npc(
    reader: &mut GamReader,
    offset: u64,
    record_size: u64,
    engine: Engine,
) -> std::io::Result<GamNpc> {
    reader.set_position(offset)?;
    let selection_state = reader.read_u16()?;
    let party_order = reader.read_u16()?;
    let cre_offset = reader.read_u32()?;
    let cre_size = reader.read_u32()?;
    let character_name = reader.read_string(8)?;
    // Re-read the entry verbatim for round-trip preservation of the
    // engine-specific tail.
    reader.set_position(offset)?;
    let mut raw = vec![0u8; record_size as usize];
    reader.read_exact(&mut raw)?;
    // The embedded-CRE pointer (0x04) and size (0x08) are file-layout
    // details: the exporter recomputes them and patches them back into
    // `raw`. Normalise them to zero here so a round-tripped record
    // (whose blob may have moved or been resized) still compares equal
    // — the live values live in the transient `cre_offset`/`cre_size`
    // locals and the `cre` blob below, not in `raw`.
    raw[0x04..0x0C].fill(0);
    // Pull the embedded CRE blob from its **absolute** position in
    // the GAM file (cre_offset is into the whole file, not into this
    // NPC's bytes — see `GamNpc::cre_offset` for the NI cross-ref).
    // Out-of-range pointers silently become an empty slice so that
    // malformed saves still load.
    let cre = if cre_size > 0 && cre_offset > 0 {
        let file_size = reader.seek(std::io::SeekFrom::End(0))?;
        let end = (cre_offset as u64).saturating_add(cre_size as u64);
        if end <= file_size {
            reader.set_position(cre_offset as u64)?;
            let mut buf = vec![0u8; cre_size as usize];
            reader.read_exact(&mut buf)?;
            buf
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let char_stats = NpcCharStats::parse(&raw, char_stats_offset_for_engine(engine));
    Ok(GamNpc {
        selection_state,
        party_order,
        character_name,
        char_stats,
        raw,
        cre,
    })
}

fn extract_party_inventory(
    reader: &mut GamReader,
    table: &SectionTable,
    party_inventory_count: u32,
    file_size: u64,
    name: &str,
) -> std::io::Result<Vec<u8>> {
    if party_inventory_count == 0 || table.party_inventory_offset == 0 {
        return Ok(Vec::new());
    }
    let start = table.party_inventory_offset as u64;
    let end = following_offset(table.party_inventory_offset, table, file_size as u32) as u64;
    if end > file_size || start > end {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "GAM '{name}': party inventory range [{start}..{end}] invalid (file is {file_size} bytes)"
            ),
        ));
    }
    reader.set_position(start)?;
    let mut out = vec![0u8; (end - start) as usize];
    reader.read_exact(&mut out)?;
    Ok(out)
}

fn following_offset(from: u32, t: &SectionTable, file_size: u32) -> u32 {
    let mut next = file_size;
    for o in [
        t.party_npc_offset,
        t.party_inventory_offset,
        t.non_party_npc_offset,
        t.globals_offset,
        t.journal_offset,
    ] {
        if o > from && o < next {
            next = o;
        }
    }
    next
}

// ─────────────────────────────────────────────────────────────────────
//  Engine-specific extension parsers
// ─────────────────────────────────────────────────────────────────────

fn parse_engine_data(
    reader: &mut GamReader,
    engine: Engine,
    file_size: u64,
    name: &str,
) -> std::io::Result<GamEngineData> {
    match engine {
        Engine::Bg => parse_bg(reader, name).map(GamEngineData::Bg),
        Engine::Bg2 => parse_bg2(reader, name).map(GamEngineData::Bg2),
        Engine::Ee => parse_ee(reader, name).map(GamEngineData::Ee),
        Engine::Iwd => parse_iwd(reader, file_size, name).map(GamEngineData::Iwd),
        Engine::Iwd2 => parse_iwd2(reader, file_size, name).map(GamEngineData::Iwd2),
        Engine::Pst => parse_pst(reader, name).map(GamEngineData::Pst),
    }
}

fn check_range(reader: &mut GamReader, end: u64, name: &str, what: &str) -> std::io::Result<()> {
    let pos = reader.position()?;
    let len = reader.seek(std::io::SeekFrom::End(0))?;
    reader.set_position(pos)?;
    if end > len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            format!("GAM '{name}': {what} runs past file end (need {end} bytes, file is {len})"),
        ));
    }
    Ok(())
}

fn read_fixed_bytes(reader: &mut GamReader, n: u64) -> std::io::Result<Vec<u8>> {
    let mut out = vec![0u8; n as usize];
    reader.read_exact(&mut out)?;
    Ok(out)
}

fn parse_bg(reader: &mut GamReader, name: &str) -> std::io::Result<BgGamData> {
    check_range(reader, 0xB4, name, "BG1 engine header")?;
    reader.set_position(0x54)?;
    let reputation = reader.read_u32()?;
    let master_area = reader.read_string(8)?;
    let configuration = reader.read_u32()?;
    let save_version = BgSaveVersion::from_u32(reader.read_u32()?);
    let unknown = read_fixed_bytes(reader, 0xB4 - 0x68)?;
    Ok(BgGamData {
        reputation,
        master_area,
        configuration,
        save_version,
        unknown,
    })
}

fn parse_bg2(reader: &mut GamReader, name: &str) -> std::io::Result<Bg2GamData> {
    check_range(reader, 0xB4, name, "BG2 engine header")?;
    reader.set_position(0x54)?;
    let reputation = reader.read_u32()?;
    let master_area = reader.read_string(8)?;
    let configuration = reader.read_u32()?;
    let save_version = reader.read_u32()?;
    let familiar_offset = reader.read_u32()?;
    let stored_locations_offset = reader.read_u32()?;
    let stored_locations_count = reader.read_u32()?;
    let real_time = reader.read_u32()?;
    let pocket_plane_locations_offset = reader.read_u32()?;
    let pocket_plane_locations_count = reader.read_u32()?;
    let unknown = read_fixed_bytes(reader, 0xB4 - 0x80)?;

    let familiar = parse_familiar(reader, familiar_offset, name)?;
    let stored_locations = parse_stored_locations(
        reader,
        stored_locations_offset,
        stored_locations_count,
        name,
    )?;
    let pocket_plane_locations = parse_stored_locations(
        reader,
        pocket_plane_locations_offset,
        pocket_plane_locations_count,
        name,
    )?;

    Ok(Bg2GamData {
        reputation,
        master_area,
        configuration,
        save_version,
        real_time,
        unknown,
        familiar,
        stored_locations,
        pocket_plane_locations,
    })
}

fn parse_ee(reader: &mut GamReader, name: &str) -> std::io::Result<EeGamData> {
    check_range(reader, 0xB4, name, "EE engine header")?;
    reader.set_position(0x54)?;
    let reputation = reader.read_u32()?;
    let master_area = reader.read_string(8)?;
    let configuration = reader.read_u32()?;
    let save_version = reader.read_u32()?;
    let familiar_offset = reader.read_u32()?;
    let stored_locations_offset = reader.read_u32()?;
    let stored_locations_count = reader.read_u32()?;
    let real_time = reader.read_u32()?;
    let pocket_plane_locations_offset = reader.read_u32()?;
    let pocket_plane_locations_count = reader.read_u32()?;
    let zoom_level = reader.read_u32()?;
    let random_encounter_area = reader.read_string(8)?;
    let worldmap = reader.read_string(8)?;
    let campaign = reader.read_string(8)?;
    let familiar_owner = reader.read_u32()?;
    let encounter_entry = reader.read_string(20)?;

    let familiar = parse_familiar(reader, familiar_offset, name)?;
    let stored_locations = parse_stored_locations(
        reader,
        stored_locations_offset,
        stored_locations_count,
        name,
    )?;
    let pocket_plane_locations = parse_stored_locations(
        reader,
        pocket_plane_locations_offset,
        pocket_plane_locations_count,
        name,
    )?;

    Ok(EeGamData {
        reputation,
        master_area,
        configuration,
        save_version,
        real_time,
        zoom_level,
        random_encounter_area,
        worldmap,
        campaign,
        familiar_owner,
        encounter_entry,
        familiar,
        stored_locations,
        pocket_plane_locations,
    })
}

fn parse_iwd(reader: &mut GamReader, file_size: u64, name: &str) -> std::io::Result<IwdGamData> {
    check_range(reader, 0xB4, name, "IWD engine header")?;
    reader.set_position(0x54)?;
    let reputation = reader.read_u32()?;
    let master_area = reader.read_string(8)?;
    let configuration = reader.read_u32()?;
    let unknown_count = reader.read_u32()?;
    let unknown_offset = reader.read_u32()?;
    let unknown = read_fixed_bytes(reader, 0xB4 - 0x6C)?;
    let (unknown_section3, unknown_trailer) = parse_unknown_section3_block(
        reader,
        unknown_offset,
        unknown_count,
        file_size,
        name,
        false,
    )?;
    Ok(IwdGamData {
        reputation,
        master_area,
        configuration,
        unknown,
        unknown_section3,
        unknown_trailer,
    })
}

fn parse_iwd2(reader: &mut GamReader, file_size: u64, name: &str) -> std::io::Result<Iwd2GamData> {
    check_range(reader, 0xB4, name, "IWD2 engine header")?;
    reader.set_position(0x54)?;
    let reputation = reader.read_u32()?;
    let master_area = reader.read_string(8)?;
    let configuration = reader.read_u32()?;
    let unknown_count = reader.read_u32()?;
    let unknown_offset = reader.read_u32()?;
    let nightmare_mode = reader.read_u32()?;
    let unknown = read_fixed_bytes(reader, 0xB4 - 0x70)?;
    let (unknown_section3, mut unknown_trailer) =
        parse_unknown_section3_block(reader, unknown_offset, unknown_count, file_size, name, true)?;
    // IWD2 sticks an extra 4-byte tail after the trailer blob; pull
    // it back off so we can round-trip it independently.
    let trailing_extra = if let Some(trailer) = &mut unknown_trailer {
        let len = trailer.blob.len();
        if len < 4 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "GAM '{name}': IWD2 unknown trailer blob ({len} B) too small \
                     for the 4-byte trailing-extra field"
                ),
            ));
        }
        let extra = u32::from_le_bytes(trailer.blob[len - 4..].try_into().unwrap());
        trailer.blob.truncate(len - 4);
        extra
    } else {
        0
    };
    Ok(Iwd2GamData {
        reputation,
        master_area,
        configuration,
        nightmare_mode,
        unknown,
        unknown_section3,
        unknown_trailer,
        trailing_extra,
    })
}

fn parse_pst(reader: &mut GamReader, name: &str) -> std::io::Result<PstGamData> {
    // PST's engine block is 4 bytes longer than the others (ends at
    // 0xB8 rather than 0xB4 because the leading u32 at 0x54 is the
    // ModronMaze offset, not the reputation).
    check_range(reader, 0xB8, name, "PST engine header")?;
    reader.set_position(0x54)?;
    let modron_maze_offset = reader.read_u32()?;
    let reputation = reader.read_u32()?;
    let master_area = reader.read_string(8)?;
    let kill_variables_offset = reader.read_u32()?;
    let kill_variables_count = reader.read_u32()?;
    let bestiary_offset = reader.read_u32()?;
    let master_area_2 = reader.read_string(8)?;
    let unknown = read_fixed_bytes(reader, 0xB8 - 0x78)?;

    let modron_maze = parse_modron_maze(reader, modron_maze_offset, name)?;
    let kill_variables = parse_variables(
        reader,
        kill_variables_offset,
        kill_variables_count,
        name,
        "kill variables",
    )?;
    let bestiary = parse_bestiary(reader, bestiary_offset, name)?;

    Ok(PstGamData {
        reputation,
        master_area,
        master_area_2,
        unknown,
        modron_maze,
        kill_variables,
        bestiary,
    })
}

fn parse_stored_locations(
    reader: &mut GamReader,
    offset: u32,
    count: u32,
    name: &str,
) -> std::io::Result<Vec<StoredLocation>> {
    if count == 0 || offset == 0 {
        return Ok(Vec::new());
    }
    let start = offset as u64;
    let end = start + (count as u64) * STORED_LOCATION_LEN;
    check_range(reader, end, name, "stored-locations section")?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * STORED_LOCATION_LEN)?;
        let area = reader.read_string(8)?;
        let x = reader.read_i16()?;
        let y = reader.read_i16()?;
        out.push(StoredLocation { area, x, y });
    }
    Ok(out)
}

fn parse_familiar(
    reader: &mut GamReader,
    offset: u32,
    name: &str,
) -> std::io::Result<Option<Familiar>> {
    if offset == 0 {
        return Ok(None);
    }
    let start = offset as u64;
    check_range(reader, start + FAMILIAR_FIXED_LEN, name, "familiar block")?;
    let mut default_cre_per_alignment: [String; 9] = Default::default();
    for (i, slot) in default_cre_per_alignment.iter_mut().enumerate() {
        reader.set_position(start + (i as u64) * 8)?;
        *slot = reader.read_string(8)?;
    }
    reader.set_position(start + 72)?;
    let resources_offset = reader.read_u32()?;
    let mut counts = [[0u32; 9]; 9];
    let mut total: u64 = 0;
    let counts_base = start + 76;
    for (alignment_idx, row) in counts.iter_mut().enumerate() {
        for (level_idx, cell) in row.iter_mut().enumerate() {
            reader.set_position(counts_base + ((alignment_idx * 9 + level_idx) as u64) * 4)?;
            let v = reader.read_u32()?;
            *cell = v;
            total = total.saturating_add(v as u64);
        }
    }
    let extra_resources = if total > 0 && resources_offset != 0 {
        let extras_start = resources_offset as u64;
        check_range(
            reader,
            extras_start + total * 8,
            name,
            "familiar extra resources",
        )?;
        let mut extras = Vec::with_capacity(total as usize);
        for i in 0..total {
            reader.set_position(extras_start + i * 8)?;
            extras.push(reader.read_string(8)?);
        }
        extras
    } else {
        Vec::new()
    };
    Ok(Some(Familiar {
        default_cre_per_alignment,
        counts,
        extra_resources,
    }))
}

fn parse_unknown_section3_block(
    reader: &mut GamReader,
    offset: u32,
    count: u32,
    file_size: u64,
    name: &str,
    is_iwd2: bool,
) -> std::io::Result<(Vec<UnknownSection3>, Option<IwdUnknownTrailer>)> {
    if count == 0 {
        return Ok((Vec::new(), None));
    }
    let start = offset as u64;
    let records_end = start + (count as u64) * UNKNOWN_SECTION3_LEN;
    check_range(
        reader,
        records_end + 4,
        name,
        "unknown-section-3 records + EOS",
    )?;
    let mut records = Vec::with_capacity(count as usize);
    for i in 0..count as u64 {
        reader.set_position(start + i * UNKNOWN_SECTION3_LEN)?;
        records.push(UnknownSection3 {
            raw: read_fixed_bytes(reader, UNKNOWN_SECTION3_LEN)?,
        });
    }
    reader.set_position(records_end)?;
    let end_offset = reader.read_u32()?;
    let blob_start = records_end + 4;
    let raw_blob_end = if is_iwd2 {
        (end_offset as u64).min(file_size.saturating_sub(4))
    } else {
        (end_offset as u64).min(file_size)
    };
    let blob_end = raw_blob_end.max(blob_start);
    reader.set_position(blob_start)?;
    let mut blob = read_fixed_bytes(reader, blob_end - blob_start)?;
    if is_iwd2 {
        // Append the 4 trailing IWD2-only bytes so the caller can
        // split them back off.
        let extra_start = blob_end;
        let extra_end = extra_start + 4;
        check_range(reader, extra_end, name, "IWD2 trailing 4 bytes")?;
        reader.set_position(extra_start)?;
        let extra = read_fixed_bytes(reader, 4)?;
        blob.extend_from_slice(&extra);
    }
    Ok((records, Some(IwdUnknownTrailer { blob })))
}

fn parse_modron_maze(
    reader: &mut GamReader,
    offset: u32,
    name: &str,
) -> std::io::Result<Option<ModronMaze>> {
    if offset == 0 {
        return Ok(None);
    }
    let start = offset as u64;
    check_range(reader, start + MODRON_MAZE_LEN, name, "modron maze block")?;
    let mut entries: [ModronMazeEntry; 64] = [ModronMazeEntry::default(); 64];
    for (i, entry) in entries.iter_mut().enumerate() {
        reader.set_position(start + (i as u64) * 26)?;
        *entry = ModronMazeEntry {
            used: reader.read_u32()?,
            accessible: reader.read_u32()?,
            is_valid: reader.read_u32()?,
            is_trapped: reader.read_u32()?,
            trap_type: reader.read_u32()?,
            exits: reader.read_u16()?,
            populated: reader.read_u32()?,
        };
    }
    reader.set_position(start + 64 * 26)?;
    Ok(Some(ModronMaze {
        entries: Box::new(entries),
        size_x: reader.read_i32()?,
        size_y: reader.read_i32()?,
        wizard_room_x: reader.read_i32()?,
        wizard_room_y: reader.read_i32()?,
        nordom_x: reader.read_i32()?,
        nordom_y: reader.read_i32()?,
        foyer_x: reader.read_i32()?,
        foyer_y: reader.read_i32()?,
        engine_room_x: reader.read_i32()?,
        engine_room_y: reader.read_i32()?,
        num_traps: reader.read_i32()?,
        initialized: reader.read_u32()?,
        maze_blocker_made: reader.read_u32()?,
        engine_blocker_made: reader.read_u32()?,
    }))
}

fn parse_bestiary(
    reader: &mut GamReader,
    offset: u32,
    name: &str,
) -> std::io::Result<Option<Vec<u8>>> {
    if offset == 0 {
        return Ok(None);
    }
    let start = offset as u64;
    check_range(reader, start + BESTIARY_LEN, name, "bestiary block")?;
    reader.set_position(start)?;
    Ok(Some(read_fixed_bytes(reader, BESTIARY_LEN)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    // ── per-engine smoke tests ────────────────────────────────────────

    #[test]
    fn test_parse_v1_1_bg1() {
        let gam = import_fixture("bg/Save/000000001-Quick-Save/BALDUR.GAM");
        assert_eq!(gam.version, GamVersion::V1_1);
        assert!(matches!(gam.engine_data, GamEngineData::Bg(_)));
        assert!(!gam.party_npcs.is_empty());
        assert!(!gam.variables.is_empty());
    }

    #[test]
    fn test_parse_v1_1_pst() {
        let gam = import_fixture("pst/save/000000001-Quick-Save/TORMENT.GAM");
        assert_eq!(gam.version, GamVersion::V1_1);
        let pst = match &gam.engine_data {
            GamEngineData::Pst(p) => p,
            other => panic!("expected PstGamData, got {other:?}"),
        };
        assert!(!pst.master_area.is_empty());
        assert!(!gam.variables.is_empty());
    }

    #[test]
    fn test_pst_party_npcs_have_embedded_cres() {
        // Regression: PST shares the "V1.1" version string with BG1/IWD but
        // its NPC record is 360 bytes (0x168), not 352, and it has no shared
        // party-inventory section (so the size can't be derived from the
        // section gap). With the wrong 352 stride, only the first NPC reads
        // correctly; every later slot's CRE pointer lands mid-record and the
        // embedded creature is lost (shown as an "external CRE"). Each party
        // member must carry an embedded CRE blob beginning with "CRE ".
        let gam = import_fixture("pst/save/000000001-Quick-Save/TORMENT.GAM");
        assert!(
            gam.party_npcs.len() >= 2,
            "fixture should have a real party"
        );
        for (i, npc) in gam.party_npcs.iter().enumerate() {
            assert!(
                npc.cre.len() >= 4 && &npc.cre[0..4] == b"CRE ",
                "party NPC {i} is missing its embedded CRE blob",
            );
        }
    }

    #[test]
    fn test_npc_record_size_pst_is_360() {
        // PST's missing party-inventory section makes the derivation
        // impossible (`party_inventory_offset == 0`), so it must be pinned
        // by engine, not derived/fallen-back to the BG 352.
        let table = SectionTable {
            party_npc_offset: 0xB8,
            party_npc_count: 5,
            party_inventory_offset: 0, // PST has no shared party inventory
            non_party_npc_offset: 0,
            non_party_npc_count: 0,
            globals_offset: 0,
            globals_count: 0,
            journal_offset: 0,
            journal_count: 0,
        };
        assert_eq!(npc_record_size(&table, GamVersion::V1_1, Engine::Pst), 360);
        assert_eq!(npc_record_size(&table, GamVersion::V1_1, Engine::Bg), 352);
    }

    #[test]
    fn test_parse_v2_0_bg2_vanilla() {
        let gam = import_fixture("bg2/save/000000000-Auto-Save/BALDUR.GAM");
        assert_eq!(gam.version, GamVersion::V2_0);
        assert!(matches!(gam.engine_data, GamEngineData::Bg2(_)));
    }

    #[test]
    fn test_parse_v2_1_bg2_tob() {
        let gam = import_fixture("bg2/save/000000003-Auto-Save-TOB/BALDUR.GAM");
        assert_eq!(gam.version, GamVersion::V2_1);
        assert!(matches!(gam.engine_data, GamEngineData::Bg2(_)));
    }

    #[test]
    fn test_party_npcs_have_embedded_cre_blobs() {
        // Regression for a real-world bug: the embedded-CRE pointer in
        // an NPC slot is an **absolute** GAM-file offset, not relative
        // to the NPC struct. A BG:EE save's party NPCs each carry a
        // sizeable embedded CRE (typically ~1.5–30 KB); when the offset
        // was (wrongly) treated as NPC-relative, `cre_data()` returned
        // empty for every slot and downstream parsers thought the
        // creature record was missing. The fix lifts the bytes from the
        // file at the absolute offset on import.
        let gam = import_fixture("bg_ee/save/000000000-Auto-Salvataggio/BALDUR.gam");
        let main_pc = gam
            .party_npcs
            .iter()
            .find(|n| !n.cre.is_empty())
            .expect("expected at least one party NPC with an embedded CRE");
        // The blob should start with the CRE file signature.
        assert_eq!(&main_pc.cre[0..4], b"CRE ", "first 4 bytes weren't 'CRE '");
    }

    #[test]
    fn test_parse_v2_2_iwd2() {
        let gam = import_fixture("iwd2/mpsave/default/ICEWIND2.GAM");
        assert_eq!(gam.version, GamVersion::V2_2);
        let iwd2 = match &gam.engine_data {
            GamEngineData::Iwd2(i) => i,
            other => panic!("expected Iwd2GamData, got {other:?}"),
        };
        // A multiplayer default IWD2 save carries no "section 3"
        // records; just confirm the variant parsed.
        let _ = &iwd2.unknown_section3;
    }

    // ── corpus walk ───────────────────────────────────────────────────

    #[test]
    fn test_every_corpus_gam_parses_and_self_describes() {
        let fixtures = all_gam_fixtures();
        assert!(!fixtures.is_empty(), "no GAM fixtures discovered");

        let mut by_version = [0usize; 4];
        for path in &fixtures {
            let engine = engine_for_fixture(path);
            let gam = GamImporter {
                name: path.file_name().and_then(|s| s.to_str()).unwrap_or("?"),
                engine,
            }
            .import(&DataSource::new(path.as_path()))
            .unwrap_or_else(|e| panic!("parse {} failed: {e}", path.display()));

            assert_eq!(
                gam.engine_data.engine(),
                engine,
                "engine_data variant mismatch for {}",
                path.display(),
            );
            let idx = match gam.version {
                GamVersion::V1_1 => 0,
                GamVersion::V2_0 => 1,
                GamVersion::V2_1 => 2,
                GamVersion::V2_2 => 3,
            };
            by_version[idx] += 1;
        }
        assert!(by_version[0] > 0, "no V1.1 fixtures parsed");
        assert!(by_version[1] > 0, "no V2.0 fixtures parsed");
        assert!(by_version[2] > 0, "no V2.1 fixtures parsed");
        assert!(by_version[3] > 0, "no V2.2 fixtures parsed");
    }

    // ── negative cases ────────────────────────────────────────────────

    #[test]
    fn test_rejects_wrong_signature() {
        let err = GamImporter {
            name: "junk",
            engine: Engine::Bg,
        }
        .import(&ds(b"JUNKV1.1\0\0\0\0"))
        .unwrap_err();
        assert!(err.to_string().contains("Unsupported GAM signature"));
    }

    #[test]
    fn test_rejects_unknown_version() {
        let err = GamImporter {
            name: "future",
            engine: Engine::Bg,
        }
        .import(&ds(b"GAMEV9.9\0\0\0\0"))
        .unwrap_err();
        assert!(err.to_string().contains("Unsupported GAM version"));
    }

    #[test]
    fn test_rejects_truncated_header() {
        let bytes = b"GAMEV1.1                      ";
        assert!(bytes.len() < COMMON_HEADER_LEN);
        let err = GamImporter {
            name: "tiny",
            engine: Engine::Bg,
        }
        .import(&DataSource::new(bytes.as_slice()))
        .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
    }
}
