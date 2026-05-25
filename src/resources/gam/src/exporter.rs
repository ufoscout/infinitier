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

use crate::{
    Bg2GamData, BgGamData, COMMON_HEADER_LEN, EeGamData, Familiar, GAM_SIGNATURE, Gam,
    GamEngineData, GamHeader, GamNpc, GamVariable, Iwd2GamData, IwdGamData, IwdUnknownTrailer,
    JournalEntry, ModronMaze, PstGamData, StoredLocation, UnknownSection3,
};

const VARIABLE_LEN: usize = 60;
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

fn serialize(gam: &Gam) -> io::Result<Vec<u8>> {
    let mut file_size: u32 = engine_header_end(&gam.engine_data) as u32;
    let h = &gam.header;
    let extend = |fs: &mut u32, offset: u32, len: usize| {
        if len > 0 {
            *fs = (*fs).max(offset.saturating_add(len as u32));
        }
    };
    let party_npc_bytes: usize = gam.party_npcs.iter().map(|n| n.raw.len()).sum();
    let non_party_npc_bytes: usize = gam.non_party_npcs.iter().map(|n| n.raw.len()).sum();
    extend(&mut file_size, h.party_npc_offset, party_npc_bytes);
    extend(&mut file_size, h.non_party_npc_offset, non_party_npc_bytes);
    extend(
        &mut file_size,
        h.globals_offset,
        gam.variables.len() * VARIABLE_LEN,
    );
    extend(
        &mut file_size,
        h.journal_offset,
        gam.journal.len() * JOURNAL_ENTRY_LEN,
    );
    extend(
        &mut file_size,
        h.party_inventory_offset,
        gam.party_inventory.len(),
    );
    extend_for_engine(&mut file_size, &gam.engine_data);

    let mut buf = vec![0u8; file_size as usize];

    buf[0..4].copy_from_slice(GAM_SIGNATURE);
    buf[4..8].copy_from_slice(gam.version.as_bytes());

    write_common_header(&mut buf[..COMMON_HEADER_LEN], h);
    write_engine_header(&mut buf, &gam.engine_data);

    write_npcs_at(&mut buf, h.party_npc_offset as usize, &gam.party_npcs);
    write_npcs_at(
        &mut buf,
        h.non_party_npc_offset as usize,
        &gam.non_party_npcs,
    );
    write_variables_at(&mut buf, h.globals_offset as usize, &gam.variables);
    write_journal_at(&mut buf, h.journal_offset as usize, &gam.journal);
    if !gam.party_inventory.is_empty() {
        let off = h.party_inventory_offset as usize;
        buf[off..off + gam.party_inventory.len()].copy_from_slice(&gam.party_inventory);
    }

    write_engine_sections(&mut buf, &gam.engine_data);

    debug!(
        "Serialised GAM ({:?} {:?}): total={} B",
        gam.version,
        gam.engine_data.engine(),
        buf.len(),
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

fn extend_for_engine(file_size: &mut u32, data: &GamEngineData) {
    let extend = |fs: &mut u32, offset: u32, len: usize| {
        if len > 0 && offset > 0 {
            *fs = (*fs).max(offset.saturating_add(len as u32));
        }
    };
    match data {
        GamEngineData::Bg(_) => {}
        GamEngineData::Bg2(b) => {
            if b.familiar.is_some() {
                extend(file_size, b.familiar_offset, FAMILIAR_FIXED_LEN);
            }
            if let Some(fam) = &b.familiar
                && !fam.extra_resources.is_empty()
            {
                extend(
                    file_size,
                    fam.resources_offset,
                    fam.extra_resources.len() * 8,
                );
            }
            extend(
                file_size,
                b.stored_locations_offset,
                b.stored_locations.len() * STORED_LOCATION_LEN,
            );
            extend(
                file_size,
                b.pocket_plane_locations_offset,
                b.pocket_plane_locations.len() * STORED_LOCATION_LEN,
            );
        }
        GamEngineData::Ee(e) => {
            if e.familiar.is_some() {
                extend(file_size, e.familiar_offset, FAMILIAR_FIXED_LEN);
            }
            if let Some(fam) = &e.familiar
                && !fam.extra_resources.is_empty()
            {
                extend(
                    file_size,
                    fam.resources_offset,
                    fam.extra_resources.len() * 8,
                );
            }
            extend(
                file_size,
                e.stored_locations_offset,
                e.stored_locations.len() * STORED_LOCATION_LEN,
            );
            extend(
                file_size,
                e.pocket_plane_locations_offset,
                e.pocket_plane_locations.len() * STORED_LOCATION_LEN,
            );
        }
        GamEngineData::Iwd(i) => {
            extend_iwd_unknown(
                file_size,
                i.unknown_offset,
                i.unknown_count,
                &i.unknown_trailer,
                false,
            );
        }
        GamEngineData::Iwd2(i) => {
            extend_iwd_unknown(
                file_size,
                i.unknown_offset,
                i.unknown_count,
                &i.unknown_trailer,
                true,
            );
        }
        GamEngineData::Pst(p) => {
            if p.modron_maze.is_some() && p.modron_maze_offset > 0 {
                *file_size = (*file_size).max(p.modron_maze_offset + MODRON_MAZE_LEN as u32);
            }
            extend(
                file_size,
                p.kill_variables_offset,
                p.kill_variables.len() * VARIABLE_LEN,
            );
            if p.bestiary.is_some() && p.bestiary_offset > 0 {
                *file_size = (*file_size).max(p.bestiary_offset + BESTIARY_LEN as u32);
            }
        }
    }
}

fn extend_iwd_unknown(
    file_size: &mut u32,
    offset: u32,
    count: u32,
    trailer: &Option<IwdUnknownTrailer>,
    is_iwd2: bool,
) {
    if count == 0 {
        return;
    }
    let records_end = offset.saturating_add(count * UNKNOWN_SECTION3_LEN as u32);
    *file_size = (*file_size).max(records_end + 4);
    if let Some(t) = trailer {
        let blob_end = records_end + 4 + t.blob.len() as u32;
        *file_size = (*file_size).max(blob_end);
        if is_iwd2 {
            *file_size = (*file_size).max(blob_end + 4);
        }
    }
}

fn write_common_header(out: &mut [u8], h: &GamHeader) {
    debug_assert!(out.len() >= COMMON_HEADER_LEN);
    out[0x08..0x0C].copy_from_slice(&h.game_time.to_le_bytes());
    out[0x0C..0x0E].copy_from_slice(&h.selected_formation.to_le_bytes());
    for (i, v) in h.formation_buttons.iter().enumerate() {
        let off = 0x0E + i * 2;
        out[off..off + 2].copy_from_slice(&v.to_le_bytes());
    }
    out[0x18..0x1C].copy_from_slice(&h.party_gold.to_le_bytes());
    out[0x1C..0x1E].copy_from_slice(&h.active_npc_or_party_count.to_le_bytes());
    out[0x1E..0x20].copy_from_slice(&h.weather.to_le_bytes());
    out[0x20..0x24].copy_from_slice(&h.party_npc_offset.to_le_bytes());
    out[0x24..0x28].copy_from_slice(&h.party_npc_count.to_le_bytes());
    out[0x28..0x2C].copy_from_slice(&h.party_inventory_offset.to_le_bytes());
    out[0x2C..0x30].copy_from_slice(&h.party_inventory_count.to_le_bytes());
    out[0x30..0x34].copy_from_slice(&h.non_party_npc_offset.to_le_bytes());
    out[0x34..0x38].copy_from_slice(&h.non_party_npc_count.to_le_bytes());
    out[0x38..0x3C].copy_from_slice(&h.globals_offset.to_le_bytes());
    out[0x3C..0x40].copy_from_slice(&h.globals_count.to_le_bytes());
    write_string_fixed(&mut out[0x40..0x48], &h.world_area);
    out[0x48..0x4C].copy_from_slice(&h.current_link.to_le_bytes());
    out[0x4C..0x50].copy_from_slice(&h.journal_count.to_le_bytes());
    out[0x50..0x54].copy_from_slice(&h.journal_offset.to_le_bytes());
}

fn write_engine_header(buf: &mut [u8], data: &GamEngineData) {
    match data {
        GamEngineData::Bg(b) => write_bg_header(buf, b),
        GamEngineData::Bg2(b) => write_bg2_header(buf, b),
        GamEngineData::Ee(e) => write_ee_header(buf, e),
        GamEngineData::Iwd(i) => write_iwd_header(buf, i),
        GamEngineData::Iwd2(i) => write_iwd2_header(buf, i),
        GamEngineData::Pst(p) => write_pst_header(buf, p),
    }
}

fn write_bg_header(buf: &mut [u8], b: &BgGamData) {
    buf[0x54..0x58].copy_from_slice(&b.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &b.master_area);
    buf[0x60..0x64].copy_from_slice(&b.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&b.save_version.as_u32().to_le_bytes());
    write_bytes_fixed(&mut buf[0x68..0xB4], &b.unknown);
}

fn write_bg2_header(buf: &mut [u8], b: &Bg2GamData) {
    buf[0x54..0x58].copy_from_slice(&b.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &b.master_area);
    buf[0x60..0x64].copy_from_slice(&b.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&b.save_version.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&b.familiar_offset.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&b.stored_locations_offset.to_le_bytes());
    buf[0x70..0x74].copy_from_slice(&b.stored_locations_count.to_le_bytes());
    buf[0x74..0x78].copy_from_slice(&b.real_time.to_le_bytes());
    buf[0x78..0x7C].copy_from_slice(&b.pocket_plane_locations_offset.to_le_bytes());
    buf[0x7C..0x80].copy_from_slice(&b.pocket_plane_locations_count.to_le_bytes());
    write_bytes_fixed(&mut buf[0x80..0xB4], &b.unknown);
}

fn write_ee_header(buf: &mut [u8], e: &EeGamData) {
    buf[0x54..0x58].copy_from_slice(&e.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &e.master_area);
    buf[0x60..0x64].copy_from_slice(&e.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&e.save_version.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&e.familiar_offset.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&e.stored_locations_offset.to_le_bytes());
    buf[0x70..0x74].copy_from_slice(&e.stored_locations_count.to_le_bytes());
    buf[0x74..0x78].copy_from_slice(&e.real_time.to_le_bytes());
    buf[0x78..0x7C].copy_from_slice(&e.pocket_plane_locations_offset.to_le_bytes());
    buf[0x7C..0x80].copy_from_slice(&e.pocket_plane_locations_count.to_le_bytes());
    buf[0x80..0x84].copy_from_slice(&e.zoom_level.to_le_bytes());
    write_string_fixed(&mut buf[0x84..0x8C], &e.random_encounter_area);
    write_string_fixed(&mut buf[0x8C..0x94], &e.worldmap);
    write_string_fixed(&mut buf[0x94..0x9C], &e.campaign);
    buf[0x9C..0xA0].copy_from_slice(&e.familiar_owner.to_le_bytes());
    write_string_fixed(&mut buf[0xA0..0xB4], &e.encounter_entry);
}

fn write_iwd_header(buf: &mut [u8], i: &IwdGamData) {
    buf[0x54..0x58].copy_from_slice(&i.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &i.master_area);
    buf[0x60..0x64].copy_from_slice(&i.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&i.unknown_count.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&i.unknown_offset.to_le_bytes());
    write_bytes_fixed(&mut buf[0x6C..0xB4], &i.unknown);
}

fn write_iwd2_header(buf: &mut [u8], i: &Iwd2GamData) {
    buf[0x54..0x58].copy_from_slice(&i.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x58..0x60], &i.master_area);
    buf[0x60..0x64].copy_from_slice(&i.configuration.to_le_bytes());
    buf[0x64..0x68].copy_from_slice(&i.unknown_count.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&i.unknown_offset.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&i.nightmare_mode.to_le_bytes());
    write_bytes_fixed(&mut buf[0x70..0xB4], &i.unknown);
}

fn write_pst_header(buf: &mut [u8], p: &PstGamData) {
    buf[0x54..0x58].copy_from_slice(&p.modron_maze_offset.to_le_bytes());
    buf[0x58..0x5C].copy_from_slice(&p.reputation.to_le_bytes());
    write_string_fixed(&mut buf[0x5C..0x64], &p.master_area);
    buf[0x64..0x68].copy_from_slice(&p.kill_variables_offset.to_le_bytes());
    buf[0x68..0x6C].copy_from_slice(&p.kill_variables_count.to_le_bytes());
    buf[0x6C..0x70].copy_from_slice(&p.bestiary_offset.to_le_bytes());
    write_string_fixed(&mut buf[0x70..0x78], &p.master_area_2);
    write_bytes_fixed(&mut buf[0x78..0xB8], &p.unknown);
}

fn write_engine_sections(buf: &mut [u8], data: &GamEngineData) {
    match data {
        GamEngineData::Bg(_) => {}
        GamEngineData::Bg2(b) => {
            if let Some(fam) = &b.familiar {
                write_familiar(buf, b.familiar_offset as usize, fam);
            }
            write_stored_locations(buf, b.stored_locations_offset as usize, &b.stored_locations);
            write_stored_locations(
                buf,
                b.pocket_plane_locations_offset as usize,
                &b.pocket_plane_locations,
            );
        }
        GamEngineData::Ee(e) => {
            if let Some(fam) = &e.familiar {
                write_familiar(buf, e.familiar_offset as usize, fam);
            }
            write_stored_locations(buf, e.stored_locations_offset as usize, &e.stored_locations);
            write_stored_locations(
                buf,
                e.pocket_plane_locations_offset as usize,
                &e.pocket_plane_locations,
            );
        }
        GamEngineData::Iwd(i) => {
            write_unknown_section3_block(
                buf,
                i.unknown_offset as usize,
                &i.unknown_section3,
                i.unknown_trailer.as_ref(),
                None,
            );
        }
        GamEngineData::Iwd2(i) => {
            write_unknown_section3_block(
                buf,
                i.unknown_offset as usize,
                &i.unknown_section3,
                i.unknown_trailer.as_ref(),
                Some(i.trailing_extra),
            );
        }
        GamEngineData::Pst(p) => {
            if let Some(maze) = &p.modron_maze {
                write_modron_maze(buf, p.modron_maze_offset as usize, maze);
            }
            write_variables_at(buf, p.kill_variables_offset as usize, &p.kill_variables);
            if let Some(bestiary) = &p.bestiary {
                let off = p.bestiary_offset as usize;
                let n = bestiary.len().min(BESTIARY_LEN);
                buf[off..off + n].copy_from_slice(&bestiary[..n]);
            }
        }
    }
}

fn write_familiar(buf: &mut [u8], offset: usize, fam: &Familiar) {
    for (i, cref) in fam.default_cre_per_alignment.iter().enumerate() {
        let off = offset + i * 8;
        write_string_fixed(&mut buf[off..off + 8], cref);
    }
    buf[offset + 72..offset + 76].copy_from_slice(&fam.resources_offset.to_le_bytes());
    let counts_base = offset + 76;
    for (alignment_idx, row) in fam.counts.iter().enumerate() {
        for (level_idx, &v) in row.iter().enumerate() {
            let off = counts_base + (alignment_idx * 9 + level_idx) * 4;
            buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    if !fam.extra_resources.is_empty() {
        let extras_start = fam.resources_offset as usize;
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
        buf[records_end..records_end + 4].copy_from_slice(&t.end_offset.to_le_bytes());
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

fn write_npcs_at(buf: &mut [u8], offset: usize, npcs: &[GamNpc]) {
    let mut off = offset;
    for npc in npcs {
        buf[off..off + npc.raw.len()].copy_from_slice(&npc.raw);
        off += npc.raw.len();
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
    write_string_fixed(&mut out[0x34..0x3C], &v.script_name);
}

fn write_journal_at(buf: &mut [u8], offset: usize, journal: &[JournalEntry]) {
    for (i, j) in journal.iter().enumerate() {
        let off = offset + i * JOURNAL_ENTRY_LEN;
        let c = &mut buf[off..off + JOURNAL_ENTRY_LEN];
        c[0..4].copy_from_slice(&j.strref.to_le_bytes());
        c[4..8].copy_from_slice(&j.time_seconds.to_le_bytes());
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
}
