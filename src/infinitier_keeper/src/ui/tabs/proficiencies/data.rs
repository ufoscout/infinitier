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
//!    so these must be folded in or the table reads all-zero.
//!
//! `param2` of an op233 effect is the `IE_PROFICIENCY*` stat number
//! (89 = Bastard Sword … 114 = Two-Weapon Style — GemRB `ie_stats.h`);
//! `param1` is the **same packed byte** as the header (low 3 bits =
//! first class, next 3 = second), NOT a plain point count. So a
//! dual-class Imoen's Short Sword effect of `9` (`0b001001`) is 1 point
//! in each class, not 9 — it must be unpacked, exactly like the header.
//! op233 set-sets (max), so header and effect combine via `max`.

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

/// Build the table rows for a creature. The packed proficiency byte for
/// each stat is the larger of the CRE header byte (via
/// [`Cre::header_proficiency_byte`]) and any `op233` effect value (which
/// is the same packed byte — save-game party members carry their
/// proficiencies there). The byte is then split into first/second-class
/// points, exactly like EEKeeper's two columns.
pub fn proficiency_rows(cre: &Cre) -> Vec<ProfRow> {
    let effects = effect_proficiency_bytes(cre);

    PROFICIENCIES
        .iter()
        .map(|&(stat, name)| {
            let effect = effects.get(&u32::from(stat)).copied().unwrap_or(0);
            // op233 is a set-if-greater (GemRB `fx_set_proficiency`), so
            // the effective packed byte is the max of header and effect.
            let packed = cre.header_proficiency_byte(stat).max(effect);
            ProfRow {
                name,
                first: u32::from(packed & 0x07),
                second: u32::from((packed >> 3) & 0x07),
            }
        })
        .collect()
}

/// The packed proficiency byte granted by `op233` ("set proficiency")
/// effects, keyed by the targeted `IE_PROFICIENCY*` stat (`param2`).
/// `param1` is the packed byte; op233 keeps the largest, so we reduce
/// duplicates with `max` rather than summing.
fn effect_proficiency_bytes(cre: &Cre) -> HashMap<u32, u8> {
    let mut bytes: HashMap<u32, u8> = HashMap::new();
    let SubSections::V1(sub) = &cre.sub_sections else {
        return bytes;
    };
    let mut record = |stat: u32, packed: u32| {
        let packed = packed.min(u32::from(u8::MAX)) as u8;
        let entry = bytes.entry(stat).or_default();
        *entry = (*entry).max(packed);
    };
    // `op233` parses to a typed `Proficiency` variant in both effect
    // versions (`proficiency` = stat, `points` = the packed byte).
    match &sub.effects {
        EffectList::V2(effects) => {
            for e in effects {
                if let EffectV2::Proficiency(p) = e {
                    record(p.proficiency, p.points);
                }
            }
        }
        EffectList::V1(effects) => {
            for e in effects {
                if let EffectV1::Proficiency(p) = e {
                    record(p.proficiency, p.points);
                }
            }
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_core::fs::{DataSource, Importer};
    use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
    use infinitier_core::resource::Game;
    use infinitier_core::resource::gam::GamImporter;

    /// A shipped BGEE save whose pre-built party includes the dual-class
    /// thief/mage Imoen (`*MOEN1`) and the single-class fighter PC
    /// (`*HARBASE`) — both carry their proficiencies as `op233` effects.
    const SAVE: &str = "SAV_GAM/bg_ee/save/000000001-Salvataggio Rapido/BALDUR.gam";

    fn party_cre(name: &str) -> Box<Cre> {
        let path = infinitier_test_utils::get_assets_path().join(SAVE);
        let gam = GamImporter {
            name: "test",
            engine: Game::Bgee.engine(),
        }
        .import(&DataSource::new(path.as_path()))
        .unwrap();
        let imported = ImportedGam::load_with_tlk(gam, Game::Bgee, None).unwrap();
        imported
            .party_npcs
            .iter()
            .find_map(|n| match &n.cre {
                Some(NpcCre::Cre(c)) if n.character_name.eq_ignore_ascii_case(name) => {
                    Some(c.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("party member {name} not found"))
    }

    fn fs(rows: &[ProfRow], name: &str) -> (u32, u32) {
        let r = rows.iter().find(|r| r.name == name).unwrap();
        (r.first, r.second)
    }

    #[test]
    fn dual_class_proficiencies_unpack_into_first_and_second() {
        // Imoen (dual Thief→Mage) stores packed proficiency bytes in
        // op233 effects: Short Sword 9 (0b001001) = 1/1, Sling 10
        // (0b001010) = 2/1. Earlier the table showed the raw 9/10 in the
        // First Class column; they must be unpacked like the header.
        let rows = proficiency_rows(&party_cre("*MOEN1"));
        assert_eq!(fs(&rows, "Dagger"), (1, 0));
        assert_eq!(fs(&rows, "Short Sword"), (1, 1));
        assert_eq!(fs(&rows, "Shortbow"), (1, 1));
        assert_eq!(fs(&rows, "Single-Weapon Style"), (1, 0));
        assert_eq!(fs(&rows, "Sling"), (2, 1));
        assert_eq!(fs(&rows, "Axe"), (0, 0));
    }

    #[test]
    fn single_class_proficiencies_are_first_class_only() {
        // The fighter PC's pips all land in First Class (low 3 bits),
        // none in Second — calibrating the bit order.
        let rows = proficiency_rows(&party_cre("*HARBASE"));
        assert_eq!(fs(&rows, "Axe"), (5, 0));
        assert_eq!(fs(&rows, "Long Sword"), (5, 0));
        assert_eq!(fs(&rows, "Two-Weapon Style"), (3, 0));
    }
}
