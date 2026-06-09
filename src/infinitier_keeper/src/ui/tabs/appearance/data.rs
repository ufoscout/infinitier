//! Read-only extraction for the Appearance tab.
//!
//! The creature header carries an `animation_id` (resolved elsewhere to
//! a name like "Fighter Male Human" via ANIMATE.IDS) and seven colour
//! indices. Each colour index selects a gradient ramp in the game's
//! palette; the view renders them as swatches. The display order and
//! labels mirror EEKeeper's "Colors" box.

use infinitier_core::resource::cre::{Cre, CreHeader};

/// One labelled colour swatch: a slot name and its palette ramp index.
pub struct ColorSlot {
    pub label: &'static str,
    pub index: u8,
}

/// Everything the Appearance tab paints.
pub struct AppearanceData {
    /// Creature animation id, resolved to a name in the view.
    pub animation_id: u32,
    /// The seven colours, in EEKeeper's display order (row-major over a
    /// two-column grid): Hair / Skin, Clothing Major / Clothing Minor,
    /// Armor / Leather, Metal.
    pub colors: [ColorSlot; 7],
}

/// Pull the appearance fields out of the CRE header. V1.0 (BG / BG2 / EE),
/// V9.0 (classic IWD / HoW) and V1.2 (classic PST) all carry an animation id
/// plus the seven colour indices; only the generated field names differ. V2.2
/// (IWD2) isn't decoded here and returns `None`, so the tab shows nothing.
pub fn appearance_data(cre: &Cre) -> Option<AppearanceData> {
    // Build the seven colour slots in EEKeeper's display order (row-major
    // over a two-column grid). The colour-field idents differ per header
    // version, so the slot fields are passed explicitly.
    macro_rules! colour_slots {
        ($h:expr, $hair:ident, $skin:ident, $major:ident, $minor:ident,
         $armor:ident, $leather:ident, $metal:ident) => {
            [
                ColorSlot {
                    label: "Hair",
                    index: $h.$hair,
                },
                ColorSlot {
                    label: "Skin",
                    index: $h.$skin,
                },
                ColorSlot {
                    label: "Clothing Major",
                    index: $h.$major,
                },
                ColorSlot {
                    label: "Clothing Minor",
                    index: $h.$minor,
                },
                ColorSlot {
                    label: "Armor",
                    index: $h.$armor,
                },
                ColorSlot {
                    label: "Leather",
                    index: $h.$leather,
                },
                ColorSlot {
                    label: "Metal",
                    index: $h.$metal,
                },
            ]
        };
    }
    let (animation_id, colors) = match &cre.header {
        CreHeader::V10(h) => (
            h.animation_id,
            colour_slots!(
                h,
                hair_colour_index,
                skin_colour_index,
                major_colour_index,
                minor_colour_index,
                armor_colour_index,
                leather_colour_index,
                metal_colour_index
            ),
        ),
        CreHeader::V90(h) => (
            h.animation_id_animate_ids,
            colour_slots!(
                h,
                hair_colour_index,
                skin_colour_index,
                major_colour_index,
                minor_colour_index,
                armor_colour_index,
                leather_colour_index,
                metal_colour_index
            ),
        ),
        CreHeader::V12(h) => (
            h.animation_id_animate_ids,
            colour_slots!(
                h,
                hair_colour_index_bg1_animations,
                skin_colour_index_bg1_animations,
                major_colour_index_bg1_animations,
                minor_colour_index_bg1_animations,
                armor_colour_index_bg1_animations,
                leather_colour_index_bg1_animations,
                metal_colour_index_bg1_animations
            ),
        ),
        CreHeader::V22(_) => return None,
    };
    Some(AppearanceData {
        animation_id,
        colors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_core::fs::{DataSource, Importer};
    use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
    use infinitier_core::resource::Game;
    use infinitier_core::resource::cre::CreImporter;
    use infinitier_core::resource::gam::GamImporter;

    /// The colour index for a slot by its display label.
    fn colour(appearance: &AppearanceData, label: &str) -> u8 {
        appearance
            .colors
            .iter()
            .find(|c| c.label == label)
            .unwrap_or_else(|| panic!("missing colour slot {label}"))
            .index
    }

    /// Classic IWD creatures are CRE V9.0; the tab must read their animation
    /// id and seven colour indices (previously it returned `None` → "Appearance
    /// is unavailable").
    #[test]
    fn appearance_data_reads_v9_0_iwd_cre() {
        let path = infinitier_test_utils::get_assets_path().join("cre/v9_0/BARBWAR2.cre");
        let cre = CreImporter {
            name: "BARBWAR2",
            game: Game::Iwd {
                heart_of_winter: false,
                totl: false,
            },
        }
        .import(&DataSource::new(path.as_path()))
        .expect("import V9.0 CRE fixture");

        let appearance = appearance_data(&cre).expect("V9.0 appearance must be readable");
        assert_eq!(appearance.animation_id, 0xF788);
        assert_eq!(colour(&appearance, "Hair"), 0);
        assert_eq!(colour(&appearance, "Skin"), 12);
        assert_eq!(colour(&appearance, "Clothing Major"), 57);
        assert_eq!(colour(&appearance, "Clothing Minor"), 37);
        assert_eq!(colour(&appearance, "Armor"), 28);
        assert_eq!(colour(&appearance, "Leather"), 23);
        assert_eq!(colour(&appearance, "Metal"), 30);
    }

    /// Classic PST creatures are CRE V1.2, whose colour fields carry the
    /// `_bg1_animations` suffix; the tab must read those too. Nameless One
    /// (party slot 0 of the Modron Foyer save) animates as 0x6032 with a
    /// fleshed-out colour set.
    #[test]
    fn appearance_data_reads_v1_2_pst_cre() {
        let path = infinitier_test_utils::get_assets_path()
            .join("SAV_GAM/pst/save/000000029-Modron-Foyer/TORMENT.GAM");
        let gam = GamImporter {
            name: "pst",
            engine: Game::Pst.engine(),
        }
        .import(&DataSource::new(path.as_path()))
        .expect("import PST GAM fixture");
        let imported =
            ImportedGam::load_with_tlk(gam, Game::Pst, None).expect("ImportedGam::load_with_tlk");
        let Some(NpcCre::Cre(cre)) = &imported.party_npcs[0].cre else {
            panic!("party slot 0 must carry an embedded CRE");
        };

        let appearance = appearance_data(cre).expect("V1.2 appearance must be readable");
        assert_eq!(appearance.animation_id, 0x6032); // Nameless One, Fist
        assert_eq!(colour(&appearance, "Hair"), 0);
        assert_eq!(colour(&appearance, "Skin"), 7);
        assert_eq!(colour(&appearance, "Clothing Major"), 31);
        assert_eq!(colour(&appearance, "Clothing Minor"), 31);
        assert_eq!(colour(&appearance, "Armor"), 20);
        assert_eq!(colour(&appearance, "Leather"), 14);
        assert_eq!(colour(&appearance, "Metal"), 20);
    }
}
