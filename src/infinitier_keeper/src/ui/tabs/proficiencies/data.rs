//! Read-only extraction of weapon-proficiency points for the
//! Proficiencies tab.
//!
//! The resolution (CRE header block + `op233` "set proficiency" effects,
//! unpacked into first/second-class points) lives in the
//! [`infinitier_cre_resource`] crate via [`Cre::proficiency`]; this
//! module just maps each `IE_PROFICIENCY*` stat to its display name and
//! reads the resolved value.

use infinitier_core::resource::cre::Cre;

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

/// Build the table rows for a creature: each `IE_PROFICIENCY*` stat's
/// effective first/second-class points, resolved by [`Cre::proficiency`].
pub fn proficiency_rows(cre: &Cre) -> Vec<ProfRow> {
    PROFICIENCIES
        .iter()
        .map(|&(stat, name)| {
            let p = cre.proficiency(stat);
            ProfRow {
                name,
                first: u32::from(p.first_class),
                second: u32::from(p.second_class),
            }
        })
        .collect()
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
