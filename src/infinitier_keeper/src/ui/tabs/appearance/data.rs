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

/// Pull the appearance fields out of the CRE header. V1.0 (BG / BG2 / EE)
/// and V9.0 (classic IWD / HoW) share the animation-id + seven colour-index
/// layout; only the animation field's generated name differs. V1.2 (PST) and
/// V2.2 (IWD2) aren't decoded here and return `None`, so the tab shows
/// nothing.
pub fn appearance_data(cre: &Cre) -> Option<AppearanceData> {
    // The seven colour fields share the same names on V1.0 and V9.0, in
    // EEKeeper's display order (row-major over a two-column grid).
    macro_rules! colour_slots {
        ($h:expr) => {
            [
                ColorSlot {
                    label: "Hair",
                    index: $h.hair_colour_index,
                },
                ColorSlot {
                    label: "Skin",
                    index: $h.skin_colour_index,
                },
                ColorSlot {
                    label: "Clothing Major",
                    index: $h.major_colour_index,
                },
                ColorSlot {
                    label: "Clothing Minor",
                    index: $h.minor_colour_index,
                },
                ColorSlot {
                    label: "Armor",
                    index: $h.armor_colour_index,
                },
                ColorSlot {
                    label: "Leather",
                    index: $h.leather_colour_index,
                },
                ColorSlot {
                    label: "Metal",
                    index: $h.metal_colour_index,
                },
            ]
        };
    }
    let (animation_id, colors) = match &cre.header {
        CreHeader::V10(h) => (h.animation_id, colour_slots!(h)),
        CreHeader::V90(h) => (h.animation_id_animate_ids, colour_slots!(h)),
        CreHeader::V12(_) | CreHeader::V22(_) => return None,
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
    use infinitier_core::resource::Game;
    use infinitier_core::resource::cre::CreImporter;

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
        let by_label = |label: &str| {
            appearance
                .colors
                .iter()
                .find(|c| c.label == label)
                .unwrap_or_else(|| panic!("missing colour slot {label}"))
                .index
        };
        assert_eq!(by_label("Hair"), 0);
        assert_eq!(by_label("Skin"), 12);
        assert_eq!(by_label("Clothing Major"), 57);
        assert_eq!(by_label("Clothing Minor"), 37);
        assert_eq!(by_label("Armor"), 28);
        assert_eq!(by_label("Leather"), 23);
        assert_eq!(by_label("Metal"), 30);
    }
}
