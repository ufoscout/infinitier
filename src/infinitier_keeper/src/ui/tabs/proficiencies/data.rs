//! Read-only extraction of weapon-proficiency points for the
//! Proficiencies tab.
//!
//! The *list* of proficiencies — which `IE_PROFICIENCY*` stats to show
//! and their display names — is resolved once at startup by
//! [`EngineCaps`](infinitier_core::engine_caps::EngineCaps) from the
//! game's `WEAPPROF.2DA` (PST:EE thus shows its own six categories,
//! BG/BG2/IWD(:EE) the 24 AD&D ones). This module only pairs each entry
//! with the creature's effective first/second-class points, resolved by
//! the [`infinitier_cre_resource`] crate via [`Cre::proficiency`].

use infinitier_core::engine_caps::Proficiency;
use infinitier_core::resource::cre::Cre;

/// One table row: a proficiency and its first/second-class points.
pub struct ProfRow {
    pub name: String,
    pub first: u32,
    pub second: u32,
}

/// Build the table rows: each proficiency from `proficiencies` (the
/// engine's `WEAPPROF.2DA`-derived list) paired with `cre`'s effective
/// points for that stat. The list order is preserved so the table
/// renders in EEKeeper's alphabetical order.
pub fn proficiency_rows(cre: &Cre, proficiencies: &[Proficiency]) -> Vec<ProfRow> {
    proficiencies
        .iter()
        .map(|p| {
            let points = cre.proficiency(p.stat);
            ProfRow {
                name: p.name.clone(),
                first: u32::from(points.first_class),
                second: u32::from(points.second_class),
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

    /// A shipped BGEE save whose pre-built party includes the single-class
    /// fighter PC (`*HARBASE`) — carrying its proficiencies as `op233`
    /// effects.
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

    /// `proficiency_rows` keeps the engine-caps list (order + names) and
    /// fills each row from `Cre::proficiency` — including untrained
    /// proficiencies, which stay in the table at 0/0.
    #[test]
    fn rows_pair_engine_caps_list_with_cre_values() {
        let cre = party_cre("*HARBASE"); // single-class fighter
        let profs = vec![
            Proficiency {
                stat: 92,
                name: "Axe".into(),
            },
            Proficiency {
                stat: 90,
                name: "Long Sword".into(),
            },
            Proficiency {
                stat: 103,
                name: "Crossbow".into(),
            },
        ];
        let rows = proficiency_rows(&cre, &profs);
        assert_eq!(rows.len(), 3);
        // List order + names preserved; values come from the CRE.
        assert_eq!(rows[0].name, "Axe");
        assert_eq!((rows[0].first, rows[0].second), (5, 0));
        assert_eq!(rows[1].name, "Long Sword");
        assert_eq!((rows[1].first, rows[1].second), (5, 0));
        // Untrained, but still shown (the list is engine-driven).
        assert_eq!(rows[2].name, "Crossbow");
        assert_eq!((rows[2].first, rows[2].second), (0, 0));
    }
}
