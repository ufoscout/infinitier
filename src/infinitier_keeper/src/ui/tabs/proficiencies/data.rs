//! Read-only extraction of weapon-proficiency points for the
//! Proficiencies tab.
//!
//! Two sources are combined, mirroring how the engine resolves a
//! creature's proficiencies:
//!
//! 1. The CRE header proficiency block (0x6E..0x81 on V1.0), one byte
//!    per weapon stat. Each byte packs the *first-class* points in the
//!    low 3 bits and the *second-class* points (dual/multi) in the
//!    next 3 — exactly EEKeeper's two columns.
//! 2. Permanent `op233` ("set proficiency") effects on the creature.
//!    Save-game party members frequently carry their proficiencies
//!    here with a zero header block (the BG2EE reference save does),
//!    so these must be added in or the table reads all-zero.
//!
//! `param2` of an op233 effect is the `IE_PROFICIENCY*` stat number
//! (89 = Bastard Sword … 114 = Two-Weapon Style — GemRB `ie_stats.h`);
//! `param1` is the points granted.

use std::collections::HashMap;

use infinitier_core::resource::cre::{Cre, CreHeader, EffectList, EffectV2, SubSections};

/// One table row: a proficiency and its first/second-class points.
pub struct ProfRow {
    pub name: &'static str,
    pub first: u32,
    pub second: u32,
}

/// `IE_PROFICIENCYBASTARDSWORD` — the stat number of the first
/// weapon proficiency; header byte `n` carries stat `BASE + n`.
const PROF_STAT_BASE: u8 = 89;
/// Effect opcode that sets a weapon proficiency.
const SET_PROFICIENCY_OPCODE: u32 = 233;

/// The proficiencies EEKeeper lists, in its display order, paired with
/// their `IE_PROFICIENCY*` stat number. This set (24 entries) is fixed
/// for BG2/EE — there is no stock 2DA that maps stat → display name,
/// so the names are spelled out here (as NearInfinity / EEKeeper do).
/// Blackjack (108), Gun (109) and Martial Arts (110) are intentionally
/// omitted — they aren't part of the BG2 weapon-proficiency UI.
const PROFICIENCIES: &[(u8, &str)] = &[
    (92, "Axe"),
    (89, "Bastard Sword"),
    (115, "Club"),
    (103, "Crossbow"),
    (96, "Dagger"),
    (106, "Dart"),
    (100, "Flail/Morning Star"),
    (99, "Halberd"),
    (94, "Katana"),
    (90, "Long Sword"),
    (104, "Longbow"),
    (101, "Mace"),
    (102, "Quarterstaff"),
    (95, "Scimitar / Wakizashi / Ninjato"),
    (91, "Short Sword"),
    (105, "Shortbow"),
    (113, "Single-Weapon Style"),
    (107, "Sling"),
    (98, "Spear"),
    (112, "Sword and Shield Style"),
    (93, "Two-Handed Sword"),
    (111, "Two-Handed Weapon Style"),
    (114, "Two-Weapon Style"),
    (97, "War Hammer"),
];

/// Build the table rows for a creature.
pub fn proficiency_rows(cre: &Cre) -> Vec<ProfRow> {
    let header = header_proficiency_bytes(cre);
    let effects = effect_proficiency_points(cre);

    PROFICIENCIES
        .iter()
        .map(|&(stat, name)| {
            // Header byte only exists for the contiguous weapon stats
            // (89..=108); styles / club live in effects only.
            let base = stat
                .checked_sub(PROF_STAT_BASE)
                .and_then(|i| header.and_then(|h| h.get(i as usize).copied()))
                .unwrap_or(0);
            let effect = effects.get(&u32::from(stat)).copied().unwrap_or(0);
            ProfRow {
                name,
                first: u32::from(base & 0x07) + effect,
                second: u32::from((base >> 3) & 0x07),
            }
        })
        .collect()
}

/// The 20 packed proficiency bytes (stats 89..=108) from a V1.0
/// header. `None` for other engines (PST / IWD / IWD2 use different
/// proficiency models) — those fall back to effects only.
fn header_proficiency_bytes(cre: &Cre) -> Option<[u8; 20]> {
    match &cre.header {
        CreHeader::V10(h) => Some([
            h.bg1_large_swords_proficiency_other_games,
            h.bg1_small_swords_proficiency,
            h.bg1_bows_proficiency,
            h.bg1_spears_proficiency,
            h.bg1_blunt_proficiency,
            h.bg1_spiked_proficiency,
            h.bg1_axe_proficiency,
            h.bg1_missile_proficiency,
            h.unused_proficiency_proficiencies_are_packed_into,
            h.unused_proficiency_proficiencies_are_packed_into_2,
            h.unused_proficiency_proficiencies_are_packed_into_3,
            h.unused_proficiency_proficiencies_are_packed_into_4,
            h.unused_proficiency_proficiencies_are_packed_into_5,
            h.bg1,
            h.bg1_2,
            h.bg1_3,
            h.bg1_4,
            h.bg1_5,
            h.bg1_6,
            h.bg1_7,
        ]),
        _ => None,
    }
}

/// Sum the points granted by `op233` ("set proficiency") effects,
/// keyed by the targeted `IE_PROFICIENCY*` stat (`param2`).
fn effect_proficiency_points(cre: &Cre) -> HashMap<u32, u32> {
    let mut points: HashMap<u32, u32> = HashMap::new();
    let SubSections::V1(sub) = &cre.sub_sections else {
        return points;
    };
    match &sub.effects {
        // EE / BG2 264-byte records: `op233` parses to a typed
        // `Proficiency` (`proficiency` = stat, `points` = points).
        EffectList::V2(effects) => {
            for e in effects {
                let EffectV2::Proficiency(p) = e else {
                    continue;
                };
                *points.entry(p.proficiency).or_default() += p.points;
            }
        }
        // Classic 48-byte records: word opcode at 0x00, dword param1 /
        // param2 at 0x04 / 0x08.
        EffectList::V1(effects) => {
            for e in effects {
                let r = &e.raw;
                if u32::from(u16::from_le_bytes([r[0], r[1]])) == SET_PROFICIENCY_OPCODE {
                    *points.entry(rd_u32(r, 0x08)).or_default() += rd_u32(r, 0x04);
                }
            }
        }
    }
    points
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
