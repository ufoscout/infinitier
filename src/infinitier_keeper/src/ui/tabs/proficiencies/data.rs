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

use infinitier_core::resource::cre::{Cre, EffectList, EffectV1, EffectV2, SubSections};

/// One table row: a proficiency and its first/second-class points.
pub struct ProfRow {
    pub name: &'static str,
    pub first: u32,
    pub second: u32,
}

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

/// Build the table rows for a creature. The first/second-class points
/// come from the CRE header's packed proficiency block (via
/// [`Cre::header_proficiency`]); `op233` effects add to the first‑class
/// total (save-game party members carry their proficiencies there).
pub fn proficiency_rows(cre: &Cre) -> Vec<ProfRow> {
    let effects = effect_proficiency_points(cre);

    PROFICIENCIES
        .iter()
        .map(|&(stat, name)| {
            let (first, second) = cre.header_proficiency(stat);
            let effect = effects.get(&u32::from(stat)).copied().unwrap_or(0);
            ProfRow {
                name,
                first: u32::from(first) + effect,
                second: u32::from(second),
            }
        })
        .collect()
}

/// Sum the points granted by `op233` ("set proficiency") effects,
/// keyed by the targeted `IE_PROFICIENCY*` stat (`param2`).
fn effect_proficiency_points(cre: &Cre) -> HashMap<u32, u32> {
    let mut points: HashMap<u32, u32> = HashMap::new();
    let SubSections::V1(sub) = &cre.sub_sections else {
        return points;
    };
    // `op233` parses to a typed `Proficiency` variant in both effect
    // versions (`proficiency` = stat, `points` = points).
    match &sub.effects {
        EffectList::V2(effects) => {
            for e in effects {
                if let EffectV2::Proficiency(p) = e {
                    *points.entry(p.proficiency).or_default() += p.points;
                }
            }
        }
        EffectList::V1(effects) => {
            for e in effects {
                if let EffectV1::Proficiency(p) = e {
                    *points.entry(p.proficiency).or_default() += p.points;
                }
            }
        }
    }
    points
}
