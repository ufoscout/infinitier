//! GAM file writer.
//!
//! Round-trip semantics: re-importing the exported bytes with the
//! same [`Engine`] yields a [`Gam`] value that is **struct-equal** to
//! the source. Byte-exact equality is *not* guaranteed because the
//! original file may contain reserved or unparsed bytes outside the
//! sections we surface as fields; the exporter zero-fills any such
//! gaps. The importer ignores those gap bytes, so the round-tripped
//! struct is identical.
//!
//! Resref-shaped string fields are encoded back to bytes via
//! WINDOWS-1252 (the IE-wide single-byte encoding) and right-padded
//! with `\0` to their fixed-length on-disk slot. WINDOWS-1252 is
//! bijective for every byte value 0x00..=0xFF, so this round-trip
//! is byte-exact even for resrefs that originally held non-ASCII
//! content.

use std::io::{self, BufWriter, Write};
use std::path::Path;

use encoding_rs::WINDOWS_1252;
use log::debug;

use infinitier_common::Engine;

use crate::{
    Bg2GamData, BgGamData, COMMON_HEADER_LEN, EeGamData, Familiar, GAM_SIGNATURE, Gam,
    GamEngineData, GamHeader, GamNpc, GamVariable, Iwd2GamData, IwdGamData, IwdUnknownTrailer,
    JournalEntry, ModronMaze, PstGamData, StoredLocation, UnknownSection3,
    char_stats_offset_for_engine,
};

const VARIABLE_LEN: usize = 84;
const JOURNAL_ENTRY_LEN: usize = 12;
const STORED_LOCATION_LEN: usize = 12;
const UNKNOWN_SECTION3_LEN: usize = 24;
const FAMILIAR_FIXED_LEN: usize = 400;
const MODRON_MAZE_LEN: usize = 64 * 26 + 14 * 4;
const BESTIARY_LEN: usize = 260;

/// A GAM file writer.
pub struct GamExporter;

impl GamExporter {
    /// Serialises `gam` to the on-disk GAM byte stream.
    pub fn export<W: Write>(&self, gam: &Gam, writer: &mut W) -> io::Result<()> {
        let bytes = serialize(gam)?;
        writer.write_all(&bytes)
    }

    /// Writes `gam` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, gam: &Gam, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(gam, &mut writer)?;
        writer.flush()
    }
}

/// Recomputed offsets/counts for the common-header section table.
/// Every value is derived from the actual data during layout, never
/// reused from the parsed input.
struct CommonLayout {
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

/// Recomputed offsets for the engine-specific sub-sections. The
/// variant-specific offsets (familiar, stored locations, IWD
/// "section 3", PST maze/bestiary, …) are all derived during layout.
enum EngineLayout {
    Bg,
    /// Shared by BG2 and EE (identical sub-section set).
    Bg2Ee {
        familiar_offset: u32,
        familiar_resources_offset: u32,
        stored_locations_offset: u32,
        pocket_plane_locations_offset: u32,
    },
    /// Shared by IWD and IWD2. `end_offset` is the recomputed EOS
    /// pointer (`records_end + 4 + blob.len()`).
    Iwd {
        unknown_offset: u32,
        end_offset: u32,
    },
    Pst {
        modron_maze_offset: u32,
        kill_variables_offset: u32,
        bestiary_offset: u32,
    },
}

fn serialize(gam: &Gam) -> io::Result<Vec<u8>> {
    let engine = gam.engine_data.engine();

    // ── Pass 1: lay everything out contiguously after the engine
    // header. Each section offset and count is recomputed here from the
    // current data, so adding/removing records (or editing an embedded
    // CRE, which changes its blob size) can never desync a stale stored
    // offset.
    let party_npc_bytes: usize = gam.party_npcs.iter().map(|n| n.raw.len()).sum();
    let non_party_npc_bytes: usize = gam.non_party_npcs.iter().map(|n| n.raw.len()).sum();

    let party_npc_offset = engine_header_end(&gam.engine_data);
    // The party-inventory section must immediately follow the party
    // NPCs: V1.1 re-derives the NPC record size from
    // (party_inventory_offset - party_npc_offset) / party_npc_count,
    // and the importer bounds the inventory blob by the next section
    // offset. Keeping inventory adjacent satisfies both.
    let party_inventory_offset = party_npc_offset + party_npc_bytes;
    let non_party_npc_offset = party_inventory_offset + gam.party_inventory.len();
    let globals_offset = non_party_npc_offset + non_party_npc_bytes;
    let journal_offset = globals_offset + gam.variables.len() * VARIABLE_LEN;
    let mut cursor = journal_offset + gam.journal.len() * JOURNAL_ENTRY_LEN;

    let engine_layout = compute_engine_layout(&gam.engine_data, &mut cursor);

    // Embedded CRE blobs are placed last, one per slot that carries
    // one; record each absolute offset to patch into the NPC record.
    let party_cre_offsets = assign_cre_offsets(&gam.party_npcs, &mut cursor);
    let non_party_cre_offsets = assign_cre_offsets(&gam.non_party_npcs, &mut cursor);
    let file_size = cursor;

    let common = CommonLayout {
        party_npc_offset: party_npc_offset as u32,
        party_npc_count: gam.party_npcs.len() as u32,
        party_inventory_offset: party_inventory_offset as u32,
        non_party_npc_offset: non_party_npc_offset as u32,
        non_party_npc_count: gam.non_party_npcs.len() as u32,
        globals_offset: globals_offset as u32,
        globals_count: gam.variables.len() as u32,
        journal_offset: journal_offset as u32,
        journal_count: gam.journal.len() as u32,
    };

    // ── Pass 2: write.
    let mut buf = vec![0u8; file_size];
    buf[0..4].copy_from_slice(GAM_SIGNATURE);
    buf[4..8].copy_from_slice(gam.version.as_bytes());

    write_common_header(&mut buf[..COMMON_HEADER_LEN], &gam.header, &common);
    write_engine_header(&mut buf, &gam.engine_data, &engine_layout);

    write_npcs_at(
        &mut buf,
        party_npc_offset,
        &gam.party_npcs,
        engine,
        &party_cre_offsets,
    );
    write_npcs_at(
        &mut buf,
        non_party_npc_offset,
        &gam.non_party_npcs,
        engine,
        &non_party_cre_offsets,
    );
    if !gam.party_inventory.is_empty() {
        buf[party_inventory_offset..party_inventory_offset + gam.party_inventory.len()]
            .copy_from_slice(&gam.party_inventory);
    }
    write_variables_at(&mut buf, globals_offset, &gam.variables);
    write_journal_at(&mut buf, journal_offset, &gam.journal);

    write_engine_sections(&mut buf, &gam.engine_data, &engine_layout);

    debug!(
        "Serialised GAM ({:?} {:?}): total={} B",
        gam.version, engine, file_size,
    );

    Ok(buf)
}

/// Byte offset just past the engine-specific fixed header. All
/// engines except PST end at 0xB4; PST goes 4 bytes further (the
/// leading u32 at 0x54 is the Modron-Maze offset, not the
/// reputation, so PST has one extra u32 in the header).
fn engine_header_end(data: &GamEngineData) -> usize {
    match data {
        GamEngineData::Pst(_) => 0xB8,
        _ => 0xB4,
    }
}

/// Assign each NPC slot's embedded-CRE blob an absolute file offset
/// (0 when the slot carries no embedded creature), advancing `cursor`
/// past each placed blob.
fn assign_cre_offsets(npcs: &[GamNpc], cursor: &mut usize) -> Vec<u32> {
    npcs.iter()
        .map(|n| {
            if n.cre.is_empty() {
                0
            } else {
                let off = *cursor as u32;
                *cursor += n.cre.len();
                off
            }
        })
        .collect()
}

/// Lay out the engine-specific sub-sections starting at `*cursor`,
/// advancing it past everything placed and returning their recomputed
/// offsets.
fn compute_engine_layout(data: &GamEngineData, cursor: &mut usize) -> EngineLayout {
    match data {
        GamEngineData::Bg(_) => EngineLayout::Bg,
        GamEngineData::Bg2(b) => compute_bg2_ee_layout(
            cursor,
            b.familiar.as_ref(),
            b.stored_locations.len(),
            b.pocket_plane_locations.len(),
        ),
        GamEngineData::Ee(e) => compute_bg2_ee_layout(
            cursor,
            e.familiar.as_ref(),
            e.stored_locations.len(),
            e.pocket_plane_locations.len(),
        ),
        GamEngineData::Iwd(i) => {
            compute_iwd_layout(cursor, i.unknown_section3.len(), i.unknown_trailer.as_ref(), false)
        }
        GamEngineData::Iwd2(i) => {
            compute_iwd_layout(cursor, i.unknown_section3.len(), i.unknown_trailer.as_ref(), true)
        }
        GamEngineData::Pst(p) => {
            let modron_maze_offset = if p.modron_maze.is_some() {
                let off = *cursor as u32;
                *cursor += MODRON_MAZE_LEN;
                off
            } else {
                0
            };
            let kill_variables_offset = if p.kill_variables.is_empty() {
                0
            } else {
                let off = *cursor as u32;
                *cursor += p.kill_variables.len() * VARIABLE_LEN;
                off
            };
            let bestiary_offset = if p.bestiary.is_some() {
                let off = *cursor as u32;
                *cursor += BESTIARY_LEN;
                off
            } else {
                0
            };
            EngineLayout::Pst {
                modron_maze_offset,
                kill_variables_offset,
                bestiary_offset,
            }
        }
    }
}

fn compute_bg2_ee_layout(
    cursor: &mut usize,
    familiar: Option<&Familiar>,
    stored_locations: usize,
    pocket_plane_locations: usize,
) -> EngineLayout {
    let mut familiar_offset = 0;
    let mut familiar_resources_offset = 0;
    if let Some(fam) = familiar {
        familiar_offset = *cursor as u32;
        *cursor += FAMILIAR_FIXED_LEN;
        if !fam.extra_resources.is_empty() {
            familiar_resources_offset = *cursor as u32;
            *cursor += fam.extra_resources.len() * 8;
        }
    }
    let stored_locations_offset = if stored_locations == 0 {
        0
    } else {
        let off = *cursor as u32;
        *cursor += stored_locations * STORED_LOCATION_LEN;
        off
    };
    let pocket_plane_locations_offset = if pocket_plane_locations == 0 {
        0
    } else {
        let off = *cursor as u32;
        *cursor += pocket_plane_locations * STORED_LOCATION_LEN;
        off
    };
    EngineLayout::Bg2Ee {
        familiar_offset,
        familiar_resources_offset,
        stored_locations_offset,
        pocket_plane_locations_offset,
    }
}

fn compute_iwd_layout(
    cursor: &mut usize,
    record_count: usize,
    trailer: Option<&IwdUnknownTrailer>,
    is_iwd2: bool,
) -> EngineLayout {
    if record_count == 0 {
        return EngineLayout::Iwd {
            unknown_offset: 0,
            end_offset: 0,
        };
    }
    let unknown_offset = *cursor as u32;
    let records_end = *cursor + record_count * UNKNOWN_SECTION3_LEN;
    // 4-byte EOS pointer, then the trailing blob.
    let blob_len = trailer.map_or(0, |t| t.blob.len());
    let end_offset = (records_end + 4 + blob_len) as u32;
    *cursor = records_end + 4 + blob_len;
    if is_iwd2 {
        // IWD2 tacks a 4-byte "trailing extra" after the blob.
        *cursor += 4;
    }
    EngineLayout::Iwd {
        unknown_offset,
        end_offset,
    }
}

fn write_common_header(out: &mut [u8], h: &GamHeader, layout: &CommonLayout) {
    debug_assert!(out.len() >= COMMON_HEADER_LEN);
    out[0x08..0x0C].copy_from_slice(&h.game_time.game_seconds().to_le_bytes());
    out[0x0C..0x0E].copy_from_slice(&h.selected_formation.to_le_bytes());
    for (i, v) in h.formation_buttons.iter().enumerate() {
        let off = 0x0E + i * 2;
        out[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    out[0x18..0x1C].copy_from_slice(&h.party_gold.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&h.active_npc_or_party_count.to_le_bytes());
    out[0x1E..0x20].copy_from_slice(&h.weather.to_le_bytes());
    out[0x20..0x24].copy_from_slice(&layout.party_npc_offset.to_le_bytes());
    out[0x24..0x28].copy_from_slice(&layout.party_npc_count.to_le_bytes());
    out[0x28..0x2C].copy_from_slice(&layout.party_inventory_offset.to_le_bytes());
    out[0x2C..0x30].copy_from_slice(&h.party_inventory_count.to_le_bytes());
    out[0x30..0x34].copy_from_slice(&layout.non_party_npc_offset.to_le_bytes());
    out[0x34..0x38].copy_from_slice(&layout.non_party_npc_count.to_le_bytes());
    out[0x38..0x3C].copy_from_slice(&layout.globals_offset.to_le_bytes());
    out[0x3C..0x40].copy_from_slice(&layout.globals_count.to_le_bytes());
    write_string_fixed(&mut out[0x40..0x48], &h.world_area);
    out[0x48..0x4C].copy_from_slice(&h.current_link.to_le_bytes());
    out[0x4C..0x50].copy_from_slice(&layout.journal_count.to_le_bytes());
    out[0x50..0x54].copy_from_slice(&layout.journal_offset.to_le_bytes());
}

fn write_engine_header(buf: &mut [u8], data: &GamEngineData, layout: &EngineLayout) {
    match (data, layout) {
        (GamEngineData::Bg(b), EngineLayout::Bg) => write_bg_header(buf, b),
        (GamEngineData::Bg2(b), EngineLayout::Bg2Ee { familiar_offset, stored_locations_offset, pocket_plane_locations_offset, .. }) => {
            write_bg2_header(buf, b, *familiar_offset, *stored_locations_offset, *pocket_plane_locations_offset);
        }
        (GamEngineData::Ee(e), EngineLayout::Bg2Ee { familiar_offset, stored_locations_offset, pocket_plane_locations_offset, .. }) => {
            write_ee_header(buf, e, *familiar_offset, *stored_locations_offset, *pocket_plane_locations_offset);
        }
        (GamEngineData::Iwd(i), EngineLayout::Iwd { unknown_offset, .. }) => {
            write_iwd_header(buf, i, *unknown_offset);
        }
        (GamEngineData::Iwd2(i), EngineLayout::Iwd { unknown_offset, .. }) => {
            write_iwd2_header(buf, i, *unknown_offset);
        }
        (GamEngineData::Pst(p), EngineLayout::Pst { modron_maze_offset, kill_variables_offset, bestiary_offset }) => {
            write_pst_header(buf, p, *modron_maze_offset, *kill_variables_offset, *bestiary_offset);
        }
        // Engine data and layout variants are produced together by
        // `compute_engine_layout`, so a mismatch is unreachable.
        _ => unreachable!("engine data / layout variant mismatch"),
    }
}

fn write_bg_header(buf: &mut [u8], b: &BgGamData) {
    buf[0x54..0x58].copy_from_slice(&b.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &b.master_area);
    buf[0x60..0x64].copy_from_slice(&b.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&b.save_version.as_u32().to_le_bytes());
    write_bytes_fixed(&mut buf[0x68..0xB4], &b.unknown);
}

fn write_bg2_header(
    buf: &mut [u8],
    b: &Bg2GamData,
    familiar_offset: u32,
    stored_locations_offset: u32,
    pocket_plane_locations_offset: u32,
) {
    buf[0x54..0x58].copy_from_slice(&b.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &b.master_area);
    buf[0x60..0x64].copy_from_slice(&b.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&b.save_version.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&familiar_offset.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&stored_locations_offset.to_le_bytes());
    buf[0x70..0x74].copy_from_slice(&(b.stored_locations.len() as u32).to_le_bytes());
    buf[0x74..0x78].copy_from_slice(&b.real_time.to_le_bytes());
    buf[0x78..0x7C].copy_from_slice(&pocket_plane_locations_offset.to_le_bytes());
    buf[0x7C..0x80].copy_from_slice(&(b.pocket_plane_locations.len() as u32).to_le_bytes());
    write_bytes_fixed(&mut buf[0x80..0xB4], &b.unknown);
}

fn write_ee_header(
    buf: &mut [u8],
    e: &EeGamData,
    familiar_offset: u32,
    stored_locations_offset: u32,
    pocket_plane_locations_offset: u32,
) {
    buf[0x54..0x58].copy_from_slice(&e.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &e.master_area);
    buf[0x60..0x64].copy_from_slice(&e.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&e.save_version.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&familiar_offset.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&stored_locations_offset.to_le_bytes());
    buf[0x70..0x74].copy_from_slice(&(e.stored_locations.len() as u32).to_le_bytes());
    buf[0x74..0x78].copy_from_slice(&e.real_time.to_le_bytes());
    buf[0x78..0x7C].copy_from_slice(&pocket_plane_locations_offset.to_le_bytes());
    buf[0x7C..0x80].copy_from_slice(&(e.pocket_plane_locations.len() as u32).to_le_bytes());
    buf[0x80..0x84].copy_from_slice(&e.zoom_level.to_le_bytes());
    write_string_fixed(&mut buf[0x84..0x8C], &e.random_encounter_area);
    write_string_fixed(&mut buf[0x8C..0x94], &e.worldmap);
    write_string_fixed(&mut buf[0x94..0x9C], &e.campaign);
    buf[0x9C..0xA0].copy_from_slice(&e.familiar_owner.to_le_bytes());
    write_string_fixed(&mut buf[0xA0..0xB4], &e.encounter_entry);
}

fn write_iwd_header(buf: &mut [u8], i: &IwdGamData, unknown_offset: u32) {
    buf[0x54..0x58].copy_from_slice(&i.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &i.master_area);
    buf[0x60..0x64].copy_from_slice(&i.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&(i.unknown_section3.len() as u32).to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&unknown_offset.to_le_bytes());
    write_bytes_fixed(&mut buf[0x6C..0xB4], &i.unknown);
}

fn write_iwd2_header(buf: &mut [u8], i: &Iwd2GamData, unknown_offset: u32) {
    buf[0x54..0x58].copy_from_slice(&i.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &i.master_area);
    buf[0x60..0x64].copy_from_slice(&i.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&(i.unknown_section3.len() as u32).to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&unknown_offset.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&i.nightmare_mode.to_le_bytes());
    write_bytes_fixed(&mut buf[0x70..0xB4], &i.unknown);
}

fn write_pst_header(
    buf: &mut [u8],
    p: &PstGamData,
    modron_maze_offset: u32,
    kill_variables_offset: u32,
    bestiary_offset: u32,
) {
    buf[0x54..0x58].copy_from_slice(&modron_maze_offset.to_le_bytes());
    buf[0x58..0x5C].copy_from_slice(&p.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x5C..0x64], &p.master_area);
    buf[0x64..0x68].copy_from_slice(&kill_variables_offset.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&(p.kill_variables.len() as u32).to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&bestiary_offset.to_le_bytes());
    write_string_fixed(&mut buf[0x70..0x78], &p.master_area_2);
    write_bytes_fixed(&mut buf[0x78..0xB8], &p.unknown);
}

fn write_engine_sections(buf: &mut [u8], data: &GamEngineData, layout: &EngineLayout) {
    match (data, layout) {
        (GamEngineData::Bg(_), _) => {}
        (
            GamEngineData::Bg2(b),
            EngineLayout::Bg2Ee {
                familiar_offset,
                familiar_resources_offset,
                stored_locations_offset,
                pocket_plane_locations_offset,
            },
        ) => {
            if let Some(fam) = &b.familiar {
                write_familiar(buf, *familiar_offset as usize, *familiar_resources_offset, fam);
            }
            write_stored_locations(buf, *stored_locations_offset as usize, &b.stored_locations);
            write_stored_locations(
                buf,
                *pocket_plane_locations_offset as usize,
                &b.pocket_plane_locations,
            );
        }
        (
            GamEngineData::Ee(e),
            EngineLayout::Bg2Ee {
                familiar_offset,
                familiar_resources_offset,
                stored_locations_offset,
                pocket_plane_locations_offset,
            },
        ) => {
            if let Some(fam) = &e.familiar {
                write_familiar(buf, *familiar_offset as usize, *familiar_resources_offset, fam);
            }
            write_stored_locations(buf, *stored_locations_offset as usize, &e.stored_locations);
            write_stored_locations(
                buf,
                *pocket_plane_locations_offset as usize,
                &e.pocket_plane_locations,
            );
        }
        (GamEngineData::Iwd(i), EngineLayout::Iwd { unknown_offset, end_offset }) => {
            write_unknown_section3_block(
                buf,
                *unknown_offset as usize,
                *end_offset,
                &i.unknown_section3,
                i.unknown_trailer.as_ref(),
                None,
            );
        }
        (GamEngineData::Iwd2(i), EngineLayout::Iwd { unknown_offset, end_offset }) => {
            write_unknown_section3_block(
                buf,
                *unknown_offset as usize,
                *end_offset,
                &i.unknown_section3,
                i.unknown_trailer.as_ref(),
                Some(i.trailing_extra),
            );
        }
        (
            GamEngineData::Pst(p),
            EngineLayout::Pst {
                modron_maze_offset,
                kill_variables_offset,
                bestiary_offset,
            },
        ) => {
            if let Some(maze) = &p.modron_maze {
                write_modron_maze(buf, *modron_maze_offset as usize, maze);
            }
            write_variables_at(buf, *kill_variables_offset as usize, &p.kill_variables);
            if let Some(bestiary) = &p.bestiary {
                let off = *bestiary_offset as usize;
                let n = bestiary.len().min(BESTIARY_LEN);
                buf[off..off + n].copy_from_slice(&bestiary[..n]);
            }
        }
        _ => unreachable!("engine data / layout variant mismatch"),
    }
}

fn write_familiar(buf: &mut [u8], offset: usize, resources_offset: u32, fam: &Familiar) {
    for (i, cref) in fam.default_cre_per_alignment.iter().enumerate() {
        let off = offset + i * 8;
        write_string_fixed(&mut buf[off..off + 8], cref);
    }
    buf[offset + 72..offset + 76].copy_from_slice(&resources_offset.to_le_bytes());
    let counts_base = offset + 76;
    for (alignment_idx, row) in fam.counts.iter().enumerate() {
        for (level_idx, &v) in row.iter().enumerate() {
            let off = counts_base + (alignment_idx * 9 + level_idx) * 4;
            buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    if !fam.extra_resources.is_empty() {
        let extras_start = resources_offset as usize;
        for (i, r) in fam.extra_resources.iter().enumerate() {
            let off = extras_start + i * 8;
            write_string_fixed(&mut buf[off..off + 8], r);
        }
    }
}

fn write_stored_locations(buf: &mut [u8], offset: usize, locs: &[StoredLocation]) {
    for (i, l) in locs.iter().enumerate() {
        let off = offset + i * STORED_LOCATION_LEN;
        write_string_fixed(&mut buf[off..off + 8], &l.area);
        buf[off + 8..off + 10].copy_from_slice(&l.x.to_le_bytes());
        buf[off + 10..off + 12].copy_from_slice(&l.y.to_le_bytes());
    }
}

fn write_unknown_section3_block(
    buf: &mut [u8],
    offset: usize,
    end_offset: u32,
    records: &[UnknownSection3],
    trailer: Option<&IwdUnknownTrailer>,
    iwd2_extra: Option<u32>,
) {
    if records.is_empty() {
        return;
    }
    for (i, r) in records.iter().enumerate() {
        let off = offset + i * UNKNOWN_SECTION3_LEN;
        let n = r.raw.len().min(UNKNOWN_SECTION3_LEN);
        buf[off..off + n].copy_from_slice(&r.raw[..n]);
    }
    if let Some(t) = trailer {
        let records_end = offset + records.len() * UNKNOWN_SECTION3_LEN;
        // The EOS pointer is recomputed (records_end + 4 + blob.len()),
        // not reused from the parsed file.
        buf[records_end..records_end + 4].copy_from_slice(&end_offset.to_le_bytes());
        let blob_start = records_end + 4;
        let blob_end = blob_start + t.blob.len();
        buf[blob_start..blob_end].copy_from_slice(&t.blob);
        if let Some(extra) = iwd2_extra {
            buf[blob_end..blob_end + 4].copy_from_slice(&extra.to_le_bytes());
        }
    }
}

fn write_modron_maze(buf: &mut [u8], offset: usize, maze: &ModronMaze) {
    for (i, entry) in maze.entries.iter().enumerate() {
        let off = offset + i * 26;
        buf[off..off + 4].copy_from_slice(&entry.used.to_le_bytes());
        buf[off + 4..off + 8].copy_from_slice(&entry.accessible.to_le_bytes());
        buf[off + 8..off + 12].copy_from_slice(&entry.is_valid.to_le_bytes());
        buf[off + 12..off + 16].copy_from_slice(&entry.is_trapped.to_le_bytes());
        buf[off + 16..off + 20].copy_from_slice(&entry.trap_type.to_le_bytes());
        buf[off + 20..off + 22].copy_from_slice(&entry.exits.to_le_bytes());
        buf[off + 22..off + 26].copy_from_slice(&entry.populated.to_le_bytes());
    }
    let hdr = offset + 64 * 26;
    buf[hdr..hdr + 4].copy_from_slice(&maze.size_x.to_le_bytes());
    buf[hdr + 4..hdr + 8].copy_from_slice(&maze.size_y.to_le_bytes());
    buf[hdr + 8..hdr + 12].copy_from_slice(&maze.wizard_room_x.to_le_bytes());
    buf[hdr + 12..hdr + 16].copy_from_slice(&maze.wizard_room_y.to_le_bytes());
    buf[hdr + 16..hdr + 20].copy_from_slice(&maze.nordom_x.to_le_bytes());
    buf[hdr + 20..hdr + 24].copy_from_slice(&maze.nordom_y.to_le_bytes());
    buf[hdr + 24..hdr + 28].copy_from_slice(&maze.foyer_x.to_le_bytes());
    buf[hdr + 28..hdr + 32].copy_from_slice(&maze.foyer_y.to_le_bytes());
    buf[hdr + 32..hdr + 36].copy_from_slice(&maze.engine_room_x.to_le_bytes());
    buf[hdr + 36..hdr + 40].copy_from_slice(&maze.engine_room_y.to_le_bytes());
    buf[hdr + 40..hdr + 44].copy_from_slice(&maze.num_traps.to_le_bytes());
    buf[hdr + 44..hdr + 48].copy_from_slice(&maze.initialized.to_le_bytes());
    buf[hdr + 48..hdr + 52].copy_from_slice(&maze.maze_blocker_made.to_le_bytes());
    buf[hdr + 52..hdr + 56].copy_from_slice(&maze.engine_blocker_made.to_le_bytes());
}

/// Encode `s` via WINDOWS-1252 and copy the leading bytes into `out`,
/// padding the trailing bytes with `\0` (the buffer is already
/// zero-filled). WINDOWS-1252 is a single-byte encoding whose 256
/// codepoints round-trip bijectively against the decoder in
/// [`Reader::read_string`].
fn write_string_fixed(out: &mut [u8], s: &str) {
    let (encoded, _, _) = WINDOWS_1252.encode(s);
    let n = encoded.len().min(out.len());
    out[..n].copy_from_slice(&encoded[..n]);
}

/// Copies `src` into the first `min(out.len(), src.len())` bytes of
/// `out`. Trailing bytes stay zero (caller's buffer is zero-filled).
fn write_bytes_fixed(out: &mut [u8], src: &[u8]) {
    let n = out.len().min(src.len());
    out[..n].copy_from_slice(&src[..n]);
}

fn write_npcs_at(
    buf: &mut [u8],
    offset: usize,
    npcs: &[GamNpc],
    engine: Engine,
    cre_offsets: &[u32],
) {
    let base = char_stats_offset_for_engine(engine);
    let mut off = offset;
    for (npc, &cre_offset) in npcs.iter().zip(cre_offsets) {
        let start = off;
        buf[off..off + npc.raw.len()].copy_from_slice(&npc.raw);
        off += npc.raw.len();
        // Patch the typed character-stats block back over the raw copy
        // so edits to `char_stats` persist (the rest of the record is
        // written verbatim from `raw`).
        npc.char_stats.write_into(&mut buf[start..off], base);
        // The embedded-CRE offset (raw 0x04) and size (raw 0x08) are
        // recomputed here, not reused from the imported record: editing
        // the creature changes the blob length, which would desync a
        // stale stored offset/size. Slots without an embedded CRE carry
        // an empty `cre` and get a zero offset/size.
        buf[start + 0x04..start + 0x08].copy_from_slice(&cre_offset.to_le_bytes());
        buf[start + 0x08..start + 0x0C].copy_from_slice(&(npc.cre.len() as u32).to_le_bytes());
        if !npc.cre.is_empty() {
            let dest = cre_offset as usize;
            buf[dest..dest + npc.cre.len()].copy_from_slice(&npc.cre);
        }
    }
}

fn write_variables_at(buf: &mut [u8], offset: usize, vars: &[GamVariable]) {
    for (i, v) in vars.iter().enumerate() {
        let off = offset + i * VARIABLE_LEN;
        write_variable(&mut buf[off..off + VARIABLE_LEN], v);
    }
}

fn write_variable(out: &mut [u8], v: &GamVariable) {
    debug_assert_eq!(out.len(), VARIABLE_LEN);
    write_string_fixed(&mut out[0..32], &v.name);
    out[0x20..0x22].copy_from_slice(&v.type_flags.to_le_bytes());
    out[0x22..0x24].copy_from_slice(&v.ref_value.to_le_bytes());
    out[0x24..0x28].copy_from_slice(&v.dword_value.to_le_bytes());
    out[0x28..0x2C].copy_from_slice(&v.int_value.to_le_bytes());
    out[0x2C..0x34].copy_from_slice(&v.double_value.to_le_bytes());
    write_string_fixed(&mut out[0x34..0x54], &v.script_name);
}

fn write_journal_at(buf: &mut [u8], offset: usize, journal: &[JournalEntry]) {
    for (i, j) in journal.iter().enumerate() {
        let off = offset + i * JOURNAL_ENTRY_LEN;
        let c = &mut buf[off..off + JOURNAL_ENTRY_LEN];
        c[0..4].copy_from_slice(&j.strref.to_le_bytes());
        c[4..8].copy_from_slice(&j.time.ticks().to_le_bytes());
        c[8] = j.chapter;
        c[9] = j.read_by_pc;
        c[10] = j.section;
        c[11] = j.location_flag;
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use super::*;
    use crate::GamImporter;
    use crate::test_support::{all_gam_fixtures, engine_for_fixture};

    #[test]
    fn test_corpus_round_trip() {
        let fixtures = all_gam_fixtures();
        assert!(!fixtures.is_empty(), "no GAM fixtures discovered");

        for path in fixtures {
            let engine = engine_for_fixture(&path);
            let original = GamImporter { name: "rt", engine }
                .import(&DataSource::new(path.as_path()))
                .unwrap_or_else(|e| panic!("import {}: {e}", path.display()));

            let mut produced: Vec<u8> = Vec::new();
            GamExporter
                .export(&original, &mut produced)
                .unwrap_or_else(|e| panic!("export {}: {e}", path.display()));

            let re_imported = GamImporter {
                name: "rt2",
                engine,
            }
            .import(&DataSource::new(produced))
            .unwrap_or_else(|e| panic!("re-import {}: {e}", path.display()));

            assert_eq!(
                re_imported,
                original,
                "Gam struct mismatch for {}",
                path.display(),
            );
        }
    }

    #[test]
    fn test_export_to_file_round_trip() {
        let path = get_assets_path().join("SAV_GAM/bg/Save/000000001-Quick-Save/BALDUR.GAM");
        let engine = engine_for_fixture(&path);
        let original = GamImporter {
            name: "rt_file",
            engine,
        }
        .import(&DataSource::new(path.as_path()))
        .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        GamExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = GamImporter {
            name: "rt_file2",
            engine,
        }
        .import(&DataSource::new(tmp.path().to_path_buf()))
        .unwrap();
        assert_eq!(re_imported, original);
    }

    #[test]
    fn test_export_preserves_signature_and_version() {
        let path = get_assets_path().join("SAV_GAM/iwd2/mpsave/default/ICEWIND2.GAM");
        let engine = engine_for_fixture(&path);
        let original = GamImporter {
            name: "sig",
            engine,
        }
        .import(&DataSource::new(path.as_path()))
        .unwrap();
        let mut produced: Vec<u8> = Vec::new();
        GamExporter.export(&original, &mut produced).unwrap();
        assert_eq!(&produced[0..4], GAM_SIGNATURE);
        assert_eq!(&produced[4..8], original.version.as_bytes());
    }

    /// Regression for the save-editor bug this refactor fixes: editing
    /// a party member's embedded CRE changes its blob size, which would
    /// desync every downstream offset if the exporter reused the stored
    /// layout. Grow one party NPC's CRE and confirm the whole save
    /// re-lays-out and round-trips — the grown blob survives and every
    /// other section is still reachable and intact.
    #[test]
    fn growing_an_embedded_cre_relayouts_the_save() {
        let path = get_assets_path().join("SAV_GAM/bg_ee/save/000000000-Auto-Salvataggio/BALDUR.gam");
        let engine = engine_for_fixture(&path);
        let original = GamImporter { name: "grow", engine }
            .import(&DataSource::new(path.as_path()))
            .unwrap();

        let mut edited = original.clone();
        // Pick the first party slot carrying an embedded CRE and append
        // bytes to it (simulating a keeper edit that enlarges the CRE).
        let idx = edited
            .party_npcs
            .iter()
            .position(|n| !n.cre.is_empty())
            .expect("fixture should have an embedded CRE");
        let original_len = edited.party_npcs[idx].cre.len();
        edited.party_npcs[idx].cre.extend_from_slice(&[0xAB; 137]);

        let mut produced = Vec::new();
        GamExporter.export(&edited, &mut produced).unwrap();
        let re_imported = GamImporter { name: "grow2", engine }
            .import(&DataSource::new(produced))
            .unwrap();

        // The grown blob survives at its new size…
        assert_eq!(re_imported.party_npcs[idx].cre.len(), original_len + 137);
        assert_eq!(re_imported.party_npcs[idx].cre, edited.party_npcs[idx].cre);
        // …and the rest of the save is byte-for-byte the edited struct
        // (every other section relaid out without corruption).
        assert_eq!(re_imported, edited);
    }
}
