#![doc = include_str!("../readme.md")]
//!
//! ## Format
//!
//! GAM holds the per-savegame mutable game state: the party, NPCs in
//! the world, global variables, journal entries, weather, gold,
//! reputation, the current and main area, and engine-specific extras
//! (PST's Modron Maze, IWD2's Heart-of-Fury flag, EE campaigns…).
//!
//! Four wire-format versions exist in the corpus:
//!
//! | Version | Games | Notes |
//! |---------|------------------|-------|
//! | `V1.1`  | BG1, IWD, PST    | Three different post-0x54 tails (BG1/IWD/PST). |
//! | `V2.0`  | BG2, BG:EE, BG2:EE, PST:EE | Familiar / stored locations / pocket plane locations sections. |
//! | `V2.1`  | BG2 ToB, BG2:EE, IWD:EE | Undocumented in IESDP. Same backbone as V2.0; extra EE-only trailing fields. |
//! | `V2.2`  | IWD2             | Larger NPC struct (≈ 832 bytes), Heart-of-Fury flag. |
//!
//! Spec references:
//! <https://gibberlings3.github.io/iesdp/file_formats/ie_formats/gam_v1.1.htm>,
//! <https://gibberlings3.github.io/iesdp/file_formats/ie_formats/gam_v2.0.htm>,
//! <https://gibberlings3.github.io/iesdp/file_formats/ie_formats/gam_v2.2.htm>
//!
//! ## Parsing scope
//!
//! The 0x00..0x54 prefix is **universally shared** across every
//! version. After 0x54 the layout differs by engine — the file's
//! version string alone can't disambiguate BG1 / IWD / PST (all
//! `V1.1`) or BG2 / EE (both `V2.0`/`V2.1`). To handle that, both
//! [`GamImporter`] and [`GamExporter`] take an [`Engine`] selector
//! which drives the engine-specific dispatch. The parsed result
//! carries the engine-specific extension in [`Gam::engine_data`].

use infinitier_common::Engine;

mod exporter;
mod importer;

pub use exporter::GamExporter;
pub use importer::GamImporter;

/// 4-byte file signature — present at offset 0 of every GAM.
pub const GAM_SIGNATURE: &[u8; 4] = b"GAME";

/// Total byte length of the common 0x00..0x54 header.
pub const COMMON_HEADER_LEN: usize = 0x54;

/// Known on-disk version tags. The full 8-byte file prefix is
/// `<GAM_SIGNATURE><version.as_bytes()>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamVersion {
    /// `V1.1` — BG1 / IWD / PST. Post-0x54 layout is engine-specific
    /// (which the parser can't disambiguate from the version string
    /// alone) — see [`Gam::engine_data`].
    V1_1,
    /// `V2.0` — BG2 / BG:EE / BG2:EE / PST:EE.
    V2_0,
    /// `V2.1` — BG2 ToB / BG2:EE / IWD:EE. Undocumented in IESDP;
    /// structurally a V2.0 variant.
    V2_1,
    /// `V2.2` — IWD2. Larger NPC struct, Heart-of-Fury flag.
    V2_2,
}

impl GamVersion {
    /// The 4-byte tag stored at offset 0x04 of the file.
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            GamVersion::V1_1 => b"V1.1",
            GamVersion::V2_0 => b"V2.0",
            GamVersion::V2_1 => b"V2.1",
            GamVersion::V2_2 => b"V2.2",
        }
    }
}

/// A game-world time broken into day / hour / minute, the way the
/// in-game clock and the keeper display it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dhm {
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

/// A game-world time, measured in **game-seconds**.
///
/// The engine calendar runs at 1 game-hour = 300 game-seconds, hence
/// 1 game-day = 7200 game-seconds and 1 game-minute = 5 game-seconds.
/// This is the unit the GAM stores its `game_time` in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameTime {
    game_seconds: u32,
}

impl GameTime {
    /// Wrap a raw game-second count.
    pub fn from_game_seconds(game_seconds: u32) -> Self {
        Self { game_seconds }
    }

    /// The underlying game-second count.
    pub fn game_seconds(self) -> u32 {
        self.game_seconds
    }

    /// The same instant expressed in ticks (×15). Use this when
    /// combining with tick-based values so no precision is lost.
    pub fn to_ticks(self) -> GameTicks {
        GameTicks::from_ticks(self.game_seconds.saturating_mul(GameTicks::PER_GAME_SECOND))
    }

    /// Break the time into day / hour / minute.
    pub fn dhm(self) -> Dhm {
        self.to_ticks().dhm()
    }
}

/// A game-world time, measured in **engine ticks** (15 ticks per
/// game-second).
///
/// Some on-disk fields (e.g. a party member's join time) are stored in
/// ticks rather than game-seconds, so this keeps the raw tick value
/// for byte-exact round-trip while still converting to game-seconds /
/// [`GameTime`] / [`Dhm`] on demand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameTicks {
    ticks: u32,
}

impl GameTicks {
    /// Ticks per game-second.
    pub const PER_GAME_SECOND: u32 = 15;
    /// Ticks per game-minute (5 game-seconds).
    pub const PER_MINUTE: u32 = 75;
    /// Ticks per game-hour (300 game-seconds).
    pub const PER_HOUR: u32 = 4500;
    /// Ticks per game-day (24 hours).
    pub const PER_DAY: u32 = 108_000;

    /// Wrap a raw tick count.
    pub fn from_ticks(ticks: u32) -> Self {
        Self { ticks }
    }

    /// The underlying tick count.
    pub fn ticks(self) -> u32 {
        self.ticks
    }

    /// The time in game-seconds (ticks ÷ 15, truncated).
    pub fn game_seconds(self) -> u32 {
        self.ticks / Self::PER_GAME_SECOND
    }

    /// The time as a [`GameTime`] (game-second precision).
    pub fn game_time(self) -> GameTime {
        GameTime::from_game_seconds(self.game_seconds())
    }

    /// Ticks elapsed between two instants (saturating at zero).
    pub fn saturating_sub(self, earlier: GameTicks) -> GameTicks {
        GameTicks {
            ticks: self.ticks.saturating_sub(earlier.ticks),
        }
    }

    /// Break the time into day / hour / minute (full tick precision).
    pub fn dhm(self) -> Dhm {
        Dhm {
            day: self.ticks / Self::PER_DAY,
            hour: (self.ticks % Self::PER_DAY) / Self::PER_HOUR,
            minute: (self.ticks % Self::PER_HOUR) / Self::PER_MINUTE,
        }
    }
}

/// A parsed GAM file.
#[derive(Debug, Clone, PartialEq)]
pub struct Gam {
    /// On-disk version, recognised by the `"V1.1"`/`"V2.0"`/`"V2.1"`/`"V2.2"`
    /// 4-byte tag at offset 4.
    pub version: GamVersion,
    /// Fixed 0x00..0x54 header. Universally shared across all
    /// versions — covers game time, formation, gold, weather, the
    /// offset / count pairs for every variable-length section, the
    /// main area resref, and the journal section.
    pub header: GamHeader,
    /// Engine-specific extension parsed from 0x54 onwards. The
    /// variant matches the [`Engine`] passed to the importer.
    pub engine_data: GamEngineData,
    /// Party-member NPCs in file order. Every entry includes the
    /// common 0x14-byte sub-header plus its full original byte slice
    /// (see [`GamNpc::raw`]) for engine-specific drill-down.
    pub party_npcs: Vec<GamNpc>,
    /// Non-party (world / spawn) NPCs in file order.
    pub non_party_npcs: Vec<GamNpc>,
    /// Global variables in file order. Layout is the same across
    /// every version (60 bytes each).
    pub variables: Vec<GamVariable>,
    /// Journal entries in file order. 12 bytes each, identical
    /// layout across versions.
    pub journal: Vec<JournalEntry>,
    /// Raw bytes of the party-inventory section (20-byte item
    /// records, layout not parsed here).
    pub party_inventory: Vec<u8>,
}

/// 0x54-byte common header at the start of every GAM, regardless of
/// version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamHeader {
    /// Total elapsed game time (game-seconds; 1 hour = 300).
    pub game_time: GameTime,
    /// Index of the currently-selected formation preset.
    pub selected_formation: u16,
    /// The five formation hot-buttons (1..=5) at the bottom of the UI.
    pub formation_buttons: [u16; 5],
    /// Party gold piece count.
    pub party_gold: u32,
    /// V1.1: count of party NPC structs excluding the protagonist.
    /// V2.x: "active area override" (party member index, or `0xFFFF`).
    ///
    /// Exposed raw because the meaning depends on the version.
    pub active_npc_or_party_count: u16,
    /// Weather bitfield (rain, snow, wind, lightning, …).
    pub weather: u16,
    /// Party-inventory record count. Kept (not derivable) because the
    /// inventory section is an opaque byte blob whose record size is
    /// version/engine-specific — the count can't be recovered from the
    /// blob length alone. All section *offsets* and the other section
    /// *counts* (party / non-party NPC, globals, journal) are layout
    /// details: they're not stored here, and the exporter recomputes
    /// them from the actual data so edits can never desync a stale
    /// offset.
    pub party_inventory_count: u32,
    /// 8-byte ASCIIZ "world area" resref..
    pub world_area: String,
    /// 4-byte "current link" u32 at offset 0x48 (NearInfinity's
    /// `GAM_CURRENT_LINK`). Stored verbatim for round-trip work.
    pub current_link: u32,
}

/// One NPC slot — party or non-party.
///
/// The first 0x14 bytes of every NPC slot are the same across every
/// game / version, so they're parsed here. The remainder is
/// engine-specific (BG1: 352 bytes total, IWD: 384, PST: 360,
/// BG2/EE: 352, IWD2: ~832) and kept in [`Self::raw`] for callers
/// that want to decode it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamNpc {
    /// 0x00: selection state (`0`=unselected, `1`=selected,
    /// `0x8000`=dead).
    pub selection_state: u16,
    /// 0x02: party slot (`0..=5`) or `0xFFFF` for "not in party".
    pub party_order: u16,
    // 0x04 (embedded-CRE absolute file offset) and 0x08 (its byte
    // length) are layout details, not stored here: the offset depends
    // on where the blob lands in the file and the length is just
    // `cre.len()`. Both are recomputed by the exporter and patched
    // into `raw[0x04..0x0C]`, so editing the embedded creature (which
    // changes the blob size) can never desync a stale offset/size — a
    // real save-editor bug class this layout avoids.
    /// 0x0C: 8-byte resref-shaped "character name" field (often the
    /// short name; the longer name lives deeper inside the engine-
    /// specific tail). Decoded via WINDOWS-1252 with trailing NULs
    /// stripped. Names that start with `*` carry an embedded CRE
    /// pointed at by [`Self::cre_offset`]; everything else is a
    /// CRE-resref into the resource system.
    pub character_name: String,
    /// The party-member "character statistics" block (kill counts,
    /// time in party, favourite spells/weapons, …), parsed from its
    /// engine-specific position inside [`Self::raw`]. Editing these
    /// fields and re-exporting patches them back into the record.
    pub char_stats: NpcCharStats,
    /// The NPC struct's full byte slice (length depends on engine
    /// variant — BG1 = 352, IWD = 384, PST = 360, BG2/EE = 352,
    /// IWD2 = 832). Includes the parsed-out 0x14-byte sub-header so
    /// the entry is self-contained. Fields surfaced as typed members
    /// (the sub-header, [`Self::char_stats`]) are patched back into
    /// this on export; the remainder round-trips verbatim.
    pub raw: Vec<u8>,
    /// Embedded CRE bytes lifted from `gam_file[cre_offset ..
    /// cre_offset + cre_size]` during import. Empty when
    /// `cre_size == 0`. The exporter writes these back at
    /// [`Self::cre_offset`] so save games round-trip correctly.
    pub cre: Vec<u8>,
}

impl GamNpc {
    /// Embedded CRE bytes — the canonical accessor used by
    /// downstream parsers (e.g. `infinitier_cre_resource`). Returns
    /// an empty slice when the slot doesn't carry an embedded CRE.
    pub fn cre_data(&self) -> &[u8] {
        &self.cre
    }

    /// Localized 32-byte display name stored deep inside the NPC
    /// struct (the field NearInfinity calls `GAM_NPC_NAME`, distinct
    /// from the 8-byte engine script-name in [`Self::character_name`]).
    /// Decoded via WINDOWS-1252 with trailing NULs stripped.
    ///
    /// The offset of this field depends on the engine — BG / BG2 /
    /// EE / IWD store it at `+0xC0`, PST at `+0xC8`, IWD2 at `+0x1BE`
    /// — so the caller must pass the engine in. Returns an empty
    /// string for slots whose `raw` is too short to contain the
    /// field (malformed saves).
    pub fn long_name(&self, engine: infinitier_common::Engine) -> String {
        let off = long_name_offset_for_engine(engine);
        let end = off + 32;
        if end > self.raw.len() {
            return String::new();
        }
        let bytes = &self.raw[off..end];
        let trimmed_end = bytes.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
        let (decoded, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes[..trimmed_end]);
        decoded.into_owned()
    }
}

/// Offset (within an NPC's `raw` byte slice) of the 32-byte localized
/// display-name field — engine-specific, per NearInfinity's
/// `PartyNPC.read`.
fn long_name_offset_for_engine(engine: infinitier_common::Engine) -> usize {
    use infinitier_common::Engine::*;
    match engine {
        // BG1, BG2 (vanilla or Tutu), every EE flavour, and IWD
        // vanilla all put the name at offset 0xC0 within the NPC
        // struct.
        Bg | Bg2 | Ee | Iwd => 0xC0,
        // PST shifts the field by 8 bytes because of its different
        // quick-item slot layout.
        Pst => 0xC8,
        // IWD2's NPC struct is much larger (the d20 quick-spell /
        // ability / song / button tables sit between the common
        // header and the name) — the name ends up at 0x1BE.
        Iwd2 => 0x1BE,
    }
}

/// Offset of the 116-byte "character statistics" block within an NPC
/// struct, per engine (cf. NearInfinity's `PartyNPC.readCharStats`).
fn char_stats_offset_for_engine(engine: Engine) -> usize {
    match engine {
        // BG1, BG2, every EE flavour, and IWD vanilla.
        Engine::Bg | Engine::Bg2 | Engine::Ee | Engine::Iwd => 228,
        // PST vanilla shifts the quick-item layout by 8 bytes.
        Engine::Pst => 236,
        // IWD2's much larger NPC struct.
        Engine::Iwd2 => 482,
    }
}

/// The party-member "character statistics" block (116 bytes on disk).
///
/// Located inside the NPC record at an engine-specific offset (see
/// [`char_stats_offset_for_engine`]). Holds the kill counters the
/// Characteristics tab shows plus the party-timing and favourite
/// spell/weapon fields — all typed for easy editing. Re-exporting
/// patches them back into [`GamNpc::raw`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NpcCharStats {
    /// 0x00: TLK strref of the most powerful creature vanquished
    /// (`0xFFFFFFFF` / `0` when none).
    pub most_powerful_vanquished_name: u32,
    /// 0x04: XP of the most powerful creature vanquished.
    pub most_powerful_vanquished_xp: u32,
    /// 0x08: time spent in the party (1/15-second "ticks").
    pub time_in_party: u32,
    /// 0x0C: game time the member joined the party (in ticks).
    pub join_time: GameTicks,
    /// 0x10: currently in the party (`0`/`1`).
    pub in_party: u8,
    /// 0x11: unknown / preserved verbatim.
    pub unknown_0x11: u16,
    /// 0x13: first letter of the member's CRE resref.
    pub initial_character: u8,
    /// 0x14: kill XP this chapter.
    pub kills_xp_chapter: u32,
    /// 0x18: number of kills this chapter.
    pub kills_number_chapter: u32,
    /// 0x1C: kill XP this game.
    pub kills_xp_game: u32,
    /// 0x20: number of kills this game.
    pub kills_number_game: u32,
    /// 0x24: four favourite-spell resrefs.
    pub favourite_spells: [String; 4],
    /// 0x44: usage counts for the four favourite spells.
    pub favourite_spell_counts: [u16; 4],
    /// 0x4C: four favourite-weapon resrefs.
    pub favourite_weapons: [String; 4],
    /// 0x6C: usage counts for the four favourite weapons.
    pub favourite_weapon_counts: [u16; 4],
}

impl NpcCharStats {
    /// On-disk size of the block.
    pub const LEN: usize = 0x74;

    /// Parse the block out of an NPC `record` at `base`. Returns the
    /// default (all-zero) block when the record is too short to contain
    /// it (malformed / foreign saves).
    fn parse(record: &[u8], base: usize) -> Self {
        let Some(b) = record.get(base..base + Self::LEN) else {
            return Self::default();
        };
        let resref = |o: usize| -> String {
            let (s, _, _) = encoding_rs::WINDOWS_1252.decode(&b[o..o + 8]);
            s.trim_end_matches('\0').to_owned()
        };
        NpcCharStats {
            most_powerful_vanquished_name: rd_u32(b, 0x00),
            most_powerful_vanquished_xp: rd_u32(b, 0x04),
            time_in_party: rd_u32(b, 0x08),
            join_time: GameTicks::from_ticks(rd_u32(b, 0x0C)),
            in_party: b[0x10],
            unknown_0x11: rd_u16(b, 0x11),
            initial_character: b[0x13],
            kills_xp_chapter: rd_u32(b, 0x14),
            kills_number_chapter: rd_u32(b, 0x18),
            kills_xp_game: rd_u32(b, 0x1C),
            kills_number_game: rd_u32(b, 0x20),
            favourite_spells: std::array::from_fn(|i| resref(0x24 + i * 8)),
            favourite_spell_counts: std::array::from_fn(|i| rd_u16(b, 0x44 + i * 2)),
            favourite_weapons: std::array::from_fn(|i| resref(0x4C + i * 8)),
            favourite_weapon_counts: std::array::from_fn(|i| rd_u16(b, 0x6C + i * 2)),
        }
    }

    /// Write the block back into an NPC `record` at `base`. No-op when
    /// the record is too short.
    fn write_into(&self, record: &mut [u8], base: usize) {
        let Some(b) = record.get_mut(base..base + Self::LEN) else {
            return;
        };
        let mut resref = |o: usize, s: &str| {
            let (enc, _, _) = encoding_rs::WINDOWS_1252.encode(s);
            let n = enc.len().min(8);
            b[o..o + 8].fill(0);
            b[o..o + n].copy_from_slice(&enc[..n]);
        };
        for (i, s) in self.favourite_spells.iter().enumerate() {
            resref(0x24 + i * 8, s);
        }
        for (i, s) in self.favourite_weapons.iter().enumerate() {
            resref(0x4C + i * 8, s);
        }
        b[0x00..0x04].copy_from_slice(&self.most_powerful_vanquished_name.to_le_bytes());
        b[0x04..0x08].copy_from_slice(&self.most_powerful_vanquished_xp.to_le_bytes());
        b[0x08..0x0C].copy_from_slice(&self.time_in_party.to_le_bytes());
        b[0x0C..0x10].copy_from_slice(&self.join_time.ticks().to_le_bytes());
        b[0x10] = self.in_party;
        b[0x11..0x13].copy_from_slice(&self.unknown_0x11.to_le_bytes());
        b[0x13] = self.initial_character;
        b[0x14..0x18].copy_from_slice(&self.kills_xp_chapter.to_le_bytes());
        b[0x18..0x1C].copy_from_slice(&self.kills_number_chapter.to_le_bytes());
        b[0x1C..0x20].copy_from_slice(&self.kills_xp_game.to_le_bytes());
        b[0x20..0x24].copy_from_slice(&self.kills_number_game.to_le_bytes());
        for (i, c) in self.favourite_spell_counts.iter().enumerate() {
            b[0x44 + i * 2..0x46 + i * 2].copy_from_slice(&c.to_le_bytes());
        }
        for (i, c) in self.favourite_weapon_counts.iter().enumerate() {
            b[0x6C + i * 2..0x6E + i * 2].copy_from_slice(&c.to_le_bytes());
        }
    }
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// One GLOBAL / Kill variable record. 84 bytes on disk, identical
/// layout in every GAM version.
#[derive(Debug, Clone, PartialEq)]
pub struct GamVariable {
    /// 0x00: variable name (32-byte buffer, trailing NUL bytes
    /// stripped). Decoded via WINDOWS-1252; round-trips bijectively
    /// because the IE-wide encoding is a single-byte mapping that
    /// covers every byte value 0x00..=0xFF.
    pub name: String,
    /// 0x20: bitfield indicating which of the following value slots
    /// is meaningful — int / float / script-name / resref / strref /
    /// dword.
    pub type_flags: u16,
    /// 0x22: secondary "reference" field, often zero.
    pub ref_value: u16,
    /// 0x24: u32 value slot.
    pub dword_value: u32,
    /// 0x28: i32 value slot. Most "GLOBAL" int variables live here.
    pub int_value: i32,
    /// 0x2C: f64 value slot. Almost never used in vanilla saves.
    pub double_value: f64,
    /// 0x34: 32-byte script-name buffer. WINDOWS-1252 decode with
    /// trailing NULs stripped.
    pub script_name: String,
}

/// One journal-log entry. 12 bytes on disk, identical layout in
/// every GAM version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalEntry {
    /// 0x00: TLK string-reference for the entry's display text.
    pub strref: u32,
    /// 0x04: game time the entry was logged (in ticks).
    pub time: GameTicks,
    /// 0x08: chapter number at the time of the entry.
    pub chapter: u8,
    /// 0x09: index of the party member who read this (`0xFF` if
    /// nobody / user note).
    pub read_by_pc: u8,
    /// 0x0A: journal section bitfield — bits flag Quests /
    /// Completed / Info; `0` means user note.
    pub section: u8,
    /// 0x0B: location flag (`0x1F` = external dialog.tlk, `0xFF` =
    /// internal).
    pub location_flag: u8,
}

// ─────────────────────────────────────────────────────────────────────
//  Engine-specific extensions
// ─────────────────────────────────────────────────────────────────────

/// Engine-specific GAM extension. Carries the parsed contents of the
/// 0x54-onwards region — both the fixed header bytes and any
/// variable-length sub-sections referenced by the engine-specific
/// header offsets. The variant is fixed by the [`Engine`] passed to
/// [`GamImporter`].
#[derive(Debug, Clone, PartialEq)]
pub enum GamEngineData {
    /// BG1 (V1.1). No variable-length sub-sections.
    Bg(BgGamData),
    /// BG2 (V2.0 / V2.1). Familiar info + stored locations + pocket
    /// plane locations.
    Bg2(Bg2GamData),
    /// EE family (BG:EE, BG2:EE, IWD:EE, PST:EE, EET, BgeeSod).
    /// V2.0/V2.1 layout with the EE-only trailing fields (zoom,
    /// random encounter area, worldmap, campaign, familiar owner).
    Ee(EeGamData),
    /// IWD vanilla (V1.1). Includes a count + offset of opaque
    /// "section 3" records plus an EOS-pointed trailing blob.
    Iwd(IwdGamData),
    /// IWD2 (V2.2). Heart-of-Fury flag + IWD-style trailing
    /// section 3 records + trailing 4-byte field.
    Iwd2(Iwd2GamData),
    /// PST vanilla (V1.1). Modron Maze + kill variables + bestiary.
    Pst(PstGamData),
}

impl GamEngineData {
    /// The [`Engine`] this extension belongs to.
    pub fn engine(&self) -> Engine {
        match self {
            GamEngineData::Bg(_) => Engine::Bg,
            GamEngineData::Bg2(_) => Engine::Bg2,
            GamEngineData::Ee(_) => Engine::Ee,
            GamEngineData::Iwd(_) => Engine::Iwd,
            GamEngineData::Iwd2(_) => Engine::Iwd2,
            GamEngineData::Pst(_) => Engine::Pst,
        }
    }

    /// Party-wide reputation lives on the GAM, in `reputation × 10`
    /// units. Returns the player-facing value (0..=20 typically).
    pub fn reputation(&self) -> u32 {
        let raw = match self {
            GamEngineData::Bg(d) => d.reputation,
            GamEngineData::Bg2(d) => d.reputation,
            GamEngineData::Ee(d) => d.reputation,
            GamEngineData::Iwd(d) => d.reputation,
            GamEngineData::Iwd2(d) => d.reputation,
            GamEngineData::Pst(d) => d.reputation,
        };
        raw / 10
    }
}

/// BG1 (V1.1) engine extension. Layout follows IESDP `gam_v1.1.htm`
/// for the BG1 engine specifically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BgGamData {
    /// 0x54: party reputation × 10.
    pub reputation: u32,
    /// 0x58: 8-byte ASCIIZ master area resref.
    pub master_area: String,
    /// 0x60: configuration / GUI flags bitfield.
    pub configuration: u32,
    /// 0x64: which XP cap applies to this save. NearInfinity's
    /// `VERSION_BG1_ARRAY` bitmap — see [`BgSaveVersion`].
    pub save_version: BgSaveVersion,
    /// 0x68..0xB4: 76 bytes of unknown / reserved data, preserved
    /// verbatim for round-trip.
    pub unknown: Vec<u8>,
}

/// BG1 `save_version` field. Tells the engine which XP cap to apply
/// to the save — base BG1 or the TotSC expansion. Round-trip-safe
/// thanks to the [`Self::Unknown`] catch-all for unrecognised values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgSaveVersion {
    /// `0` — `"Restrict XP to BG1 limit"`. Base BG1 saves.
    Bg1,
    /// `1` — `"Restrict XP to TotSC limit"`. Saves created with the
    /// Tales of the Sword Coast expansion installed.
    TotSC,
    /// Any other on-disk value, preserved verbatim.
    Unknown(u32),
}

impl BgSaveVersion {
    /// Decode the on-disk u32 into the enum.
    pub fn from_u32(v: u32) -> Self {
        match v {
            0 => BgSaveVersion::Bg1,
            1 => BgSaveVersion::TotSC,
            other => BgSaveVersion::Unknown(other),
        }
    }

    /// The u32 representation written back to disk. `Unknown(v)`
    /// round-trips its raw `v`.
    pub fn as_u32(self) -> u32 {
        match self {
            BgSaveVersion::Bg1 => 0,
            BgSaveVersion::TotSC => 1,
            BgSaveVersion::Unknown(v) => v,
        }
    }
}

/// BG2 vanilla (V2.0 / V2.1) engine extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bg2GamData {
    /// 0x54: party reputation × 10.
    pub reputation: u32,
    /// 0x58: 8-byte master area resref.
    pub master_area: String,
    /// 0x60: configuration flags.
    pub configuration: u32,
    /// 0x64: save version.
    pub save_version: u32,
    // 0x68 familiar offset, 0x6C/0x70 stored-locations offset/count,
    // 0x78/0x7C pocket-plane-locations offset/count are layout
    // details: recomputed on export from the sub-sections below.
    /// 0x74: elapsed real time (ticks).
    pub real_time: u32,
    /// 0x80..0xB4: 52 bytes of reserved data (zeroed in vanilla
    /// saves), preserved verbatim.
    pub unknown: Vec<u8>,
    /// Variable-length sub-section: familiar-info struct at
    /// `familiar_offset` (`None` when the offset is zero).
    pub familiar: Option<Familiar>,
    /// Variable-length sub-section: stored-location records.
    pub stored_locations: Vec<StoredLocation>,
    /// Variable-length sub-section: pocket-plane location records.
    pub pocket_plane_locations: Vec<StoredLocation>,
}

/// Enhanced Edition (BG:EE / BG2:EE / IWD:EE / PST:EE / EET /
/// BgeeSod) extension. Same backbone as [`Bg2GamData`] but with the
/// EE-only trailing fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EeGamData {
    /// 0x54: party reputation × 10.
    pub reputation: u32,
    /// 0x58: 8-byte master area resref.
    pub master_area: String,
    /// 0x60: configuration flags.
    pub configuration: u32,
    /// 0x64: save version.
    pub save_version: u32,
    // 0x68 familiar offset, 0x6C/0x70 stored-locations offset/count,
    // 0x78/0x7C pocket-plane-locations offset/count are layout
    // details: recomputed on export from the sub-sections below.
    /// 0x74: elapsed real time (ticks).
    pub real_time: u32,
    /// 0x80: zoom level (EE-only).
    pub zoom_level: u32,
    /// 0x84: 8-byte random-encounter-area resref (EE-only).
    pub random_encounter_area: String,
    /// 0x8C: 8-byte worldmap resref (EE-only).
    pub worldmap: String,
    /// 0x94: 8-byte campaign tag (EE-only).
    pub campaign: String,
    /// 0x9C: party-slot index of the familiar owner (EE-only).
    pub familiar_owner: u32,
    /// 0xA0..0xB4: 20-byte encounter-entry resref (EE-only).
    pub encounter_entry: String,
    /// Familiar-info struct at `familiar_offset` (when non-zero).
    pub familiar: Option<Familiar>,
    /// Stored-location records.
    pub stored_locations: Vec<StoredLocation>,
    /// Pocket-plane location records.
    pub pocket_plane_locations: Vec<StoredLocation>,
}

/// IWD vanilla (V1.1) extension. Adds a "section 3" count/offset and
/// an EOS-pointed trailing blob — both verbatim because IESDP
/// doesn't document the inner layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IwdGamData {
    /// 0x54: reputation × 10.
    pub reputation: u32,
    /// 0x58: 8-byte master area resref.
    pub master_area: String,
    /// 0x60: configuration flags.
    pub configuration: u32,
    // 0x64 count / 0x68 offset of the opaque "section 3" records are
    // layout details: recomputed on export from `unknown_section3`.
    /// 0x6C..0xB4: 72 bytes of reserved data, preserved verbatim.
    pub unknown: Vec<u8>,
    /// The "section 3" records (24 bytes each, raw).
    pub unknown_section3: Vec<UnknownSection3>,
    /// EOS pointer + trailing blob — only present when
    /// `unknown_count > 0`. The pointer (a 4-byte absolute offset)
    /// follows the last section-3 record and marks the end of the
    /// blob that comes after it.
    pub unknown_trailer: Option<IwdUnknownTrailer>,
}

/// Trailing structure that follows the "section 3" records in
/// IWD/IWD2 saves when `unknown_count > 0`. Round-tripped verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IwdUnknownTrailer {
    /// Raw bytes of the trailing blob that follows the 4-byte
    /// "end-of-unknown-structure" pointer. That pointer is an absolute
    /// file offset equal to `records_end + 4 + blob.len()` for every
    /// well-formed save, so it's a layout detail recomputed by the
    /// exporter rather than stored here.
    pub blob: Vec<u8>,
}

/// IWD2 (V2.2) extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iwd2GamData {
    /// 0x54: reputation × 10.
    pub reputation: u32,
    /// 0x58: 8-byte master area resref.
    pub master_area: String,
    /// 0x60: configuration flags.
    pub configuration: u32,
    // 0x64 count / 0x68 offset of the "section 3" records are layout
    // details: recomputed on export from `unknown_section3`.
    /// 0x6C: nightmare-mode (a.k.a. Heart of Fury) flag.
    pub nightmare_mode: u32,
    /// 0x70..0xB4: 68 bytes of reserved data, preserved verbatim.
    pub unknown: Vec<u8>,
    /// The "section 3" records (24 bytes each, raw).
    pub unknown_section3: Vec<UnknownSection3>,
    /// EOS pointer + trailing blob (same shape as IWD).
    pub unknown_trailer: Option<IwdUnknownTrailer>,
    /// IWD2-only: 4 extra trailing bytes that follow the
    /// unknown-trailer blob.
    pub trailing_extra: u32,
}

/// PST vanilla (V1.1) extension.
#[derive(Debug, Clone, PartialEq)]
pub struct PstGamData {
    // 0x54 Modron-Maze offset, 0x64/0x68 kill-variables offset/count,
    // and 0x6C bestiary offset are layout details: recomputed on
    // export from the sub-sections below.
    /// 0x58: reputation × 10.
    pub reputation: u32,
    /// 0x5C: 8-byte master area resref.
    pub master_area: String,
    /// 0x70: 8-byte "master area 2" resref (still undocumented).
    pub master_area_2: String,
    /// 0x78..0xB8: 64 bytes reserved, preserved verbatim.
    pub unknown: Vec<u8>,
    /// Modron Maze state (when `modron_maze_offset > 0`).
    pub modron_maze: Option<ModronMaze>,
    /// Kill-variables records (same 60-byte layout as
    /// [`GamVariable`]).
    pub kill_variables: Vec<GamVariable>,
    /// 260-byte Bestiary blob (when `bestiary_offset > 0`).
    pub bestiary: Option<Vec<u8>>,
}

/// 12-byte stored-location record (BG2 / EE pocket-plane and
/// stored-location sections).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredLocation {
    /// 0x00: 8-byte ASCIIZ area resref (WINDOWS-1252 decoded,
    /// trailing NULs stripped).
    pub area: String,
    /// 0x08: stored x coordinate.
    pub x: i16,
    /// 0x0A: stored y coordinate.
    pub y: i16,
}

/// 24-byte opaque "section 3" record (IWD / IWD2). IESDP doesn't
/// document the internal layout, so the bytes are kept verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSection3 {
    /// Raw 24 bytes.
    pub raw: Vec<u8>,
}

/// Familiar-info struct (BG2 / EE). 400 fixed bytes + optionally a
/// trailing list of CRE resrefs pointed to by [`Self::resources_offset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Familiar {
    /// 0x00: 8-byte CRE resrefs, one per D&D alignment (LG, LN, LE,
    /// NG, TN, NE, CG, CN, CE — in that on-disk order). WINDOWS-1252
    /// decoded, trailing NULs stripped.
    pub default_cre_per_alignment: [String; 9],
    // 0x48 "extra familiar resources" list offset is a layout detail:
    // recomputed on export (it sits right after the 400-byte fixed
    // block when `extra_resources` is non-empty, else zero).
    /// 0x4C..0x190: 9 alignments × 9 character levels of u32 count
    /// fields (NearInfinity exposes these per row).
    pub counts: [[u32; 9]; 9],
    /// Optional list of 8-byte CRE resrefs sitting at
    /// `resources_offset`. Length is the sum of all values in
    /// [`Self::counts`].
    pub extra_resources: Vec<String>,
}

/// Modron-Maze state (PST). 1720 bytes on disk: 64 × 26-byte room
/// entries followed by a 56-byte fixed header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModronMaze {
    /// 64 room entries — NearInfinity lays them out FIRST, before
    /// the fixed header.
    pub entries: Box<[ModronMazeEntry; 64]>,
    /// Maze grid size in x.
    pub size_x: i32,
    /// Maze grid size in y.
    pub size_y: i32,
    /// Wizard room x position.
    pub wizard_room_x: i32,
    /// Wizard room y position.
    pub wizard_room_y: i32,
    /// Nordom x position.
    pub nordom_x: i32,
    /// Nordom y position.
    pub nordom_y: i32,
    /// Foyer x position.
    pub foyer_x: i32,
    /// Foyer y position.
    pub foyer_y: i32,
    /// Engine room x position.
    pub engine_room_x: i32,
    /// Engine room y position.
    pub engine_room_y: i32,
    /// Number of traps.
    pub num_traps: i32,
    /// "Maze initialized" flag.
    pub initialized: u32,
    /// Foyer maze blocker made.
    pub maze_blocker_made: u32,
    /// Foyer engine blocker made.
    pub engine_blocker_made: u32,
}

/// One 26-byte Modron Maze room entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModronMazeEntry {
    /// 0x00: used flag.
    pub used: u32,
    /// 0x04: accessible flag.
    pub accessible: u32,
    /// 0x08: is-valid flag.
    pub is_valid: u32,
    /// 0x0C: is-trapped flag.
    pub is_trapped: u32,
    /// 0x10: trap type (0=A, 1=B, 2=C).
    pub trap_type: u32,
    /// 0x14: exit-walls bitfield (None / East / West / North /
    /// South).
    pub exits: u16,
    /// 0x16: populated flag (4 bytes — overlaps the field-width of
    /// the previous u16; treat as `u32` per NearInfinity).
    pub populated: u32,
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};

    use infinitier_common::Engine;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::{Gam, GamImporter};

    /// Recursively collect every `.gam` file under `assets/SAV_GAM/`.
    pub fn all_gam_fixtures() -> Vec<PathBuf> {
        fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("read_dir") {
                let entry = entry.expect("dir entry");
                let path = entry.path();
                if path.is_dir() {
                    visit(&path, out);
                } else if path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.eq_ignore_ascii_case("gam"))
                    .unwrap_or(false)
                {
                    out.push(path);
                }
            }
        }
        let mut out = Vec::new();
        visit(&get_assets_path().join("SAV_GAM"), &mut out);
        out.sort();
        out
    }

    /// Maps a fixture path's top-level corpus directory to the
    /// [`Engine`] needed to parse it. The corpus uses one top-level
    /// directory per game release; `bg` → BG1, `bg2` → BG2 vanilla,
    /// the various EE flavours all share `Engine::Ee`, etc.
    pub fn engine_for_fixture(path: &Path) -> Engine {
        let root = get_assets_path().join("SAV_GAM");
        let rel = path.strip_prefix(&root).expect("fixture under SAV_GAM");
        let first = rel
            .iter()
            .next()
            .and_then(|c| c.to_str())
            .expect("first path component");
        match first {
            "bg" => Engine::Bg,
            "bg2" => Engine::Bg2,
            "bg_ee" | "bg2_ee" | "iwdee" | "pst_ee" => Engine::Ee,
            "iwd" => Engine::Iwd,
            "iwd2" => Engine::Iwd2,
            "pst" => Engine::Pst,
            other => panic!("unrecognised fixture engine directory: {other}"),
        }
    }

    /// Import a specific fixture given its path under
    /// `assets/SAV_GAM/`. Engine is inferred from the path.
    pub fn import_fixture(rel_path: &str) -> Gam {
        let path = get_assets_path().join("SAV_GAM").join(rel_path);
        let engine = engine_for_fixture(&path);
        GamImporter {
            name: rel_path,
            engine,
        }
        .import(&DataSource::new(path.as_path()))
        .unwrap_or_else(|e| panic!("import {rel_path}: {e}"))
    }

    /// `DataSource` shim used by negative tests.
    pub fn ds(bytes: &'static [u8]) -> DataSource {
        DataSource::new(bytes)
    }
}

#[cfg(test)]
mod char_stats_tests {
    use super::*;

    /// Every byte of the 116-byte character-stats block is read and
    /// written back by [`NpcCharStats`]: fill a record with a NUL-free
    /// pattern (so the resref fields survive intact) and assert a parse
    /// → write round-trip reproduces it byte-for-byte.
    #[test]
    fn char_stats_round_trip_is_byte_exact() {
        const BASE: usize = 8; // arbitrary non-zero base within the record
        let mut record: Vec<u8> = (0..(BASE + NpcCharStats::LEN) as u32)
            .map(|i| 0x21 + (i % 0x5D) as u8) // printable ASCII, no NUL
            .collect();
        let original = record.clone();
        let stats = NpcCharStats::parse(&record, BASE);
        // Wipe the block, then write it back — must reproduce the input.
        record[BASE..BASE + NpcCharStats::LEN].fill(0);
        stats.write_into(&mut record, BASE);
        assert_eq!(record, original, "NpcCharStats must cover every byte");
    }

    #[test]
    fn char_stats_offsets_match_known_engines() {
        assert_eq!(char_stats_offset_for_engine(Engine::Bg), 228);
        assert_eq!(char_stats_offset_for_engine(Engine::Ee), 228);
        assert_eq!(char_stats_offset_for_engine(Engine::Pst), 236);
        assert_eq!(char_stats_offset_for_engine(Engine::Iwd2), 482);
    }
}

#[cfg(test)]
mod game_time_tests {
    use super::*;

    #[test]
    fn game_time_to_dhm() {
        // From the BG2EE reference save: game_time 1219219 game-seconds
        // is day 169, hour 8, minute 3.
        assert_eq!(
            GameTime::from_game_seconds(1219219).dhm(),
            Dhm {
                day: 169,
                hour: 8,
                minute: 3
            }
        );
    }

    #[test]
    fn ticks_round_trip_and_game_seconds() {
        let t = GameTicks::from_ticks(6214308);
        assert_eq!(t.ticks(), 6214308);
        assert_eq!(t.game_seconds(), 414287); // 6214308 / 15, truncated
        assert_eq!(GameTime::from_game_seconds(300).to_ticks().ticks(), 4500);
    }

    #[test]
    fn elapsed_since_join_uses_tick_precision() {
        // "Joined Party" = now (game-seconds → ticks) − join (ticks).
        // Keldorn join 6214308 ticks → 111d 19h 6m; Aerie 13987044 →
        // 39d 19h 49m. Computing in ticks (not truncated game-seconds)
        // is what yields Aerie's minute 49 rather than 50.
        let now = GameTime::from_game_seconds(1219219).to_ticks();
        assert_eq!(
            now.saturating_sub(GameTicks::from_ticks(6214308)).dhm(),
            Dhm {
                day: 111,
                hour: 19,
                minute: 6
            }
        );
        assert_eq!(
            now.saturating_sub(GameTicks::from_ticks(13987044)).dhm(),
            Dhm {
                day: 39,
                hour: 19,
                minute: 49
            }
        );
    }
}
