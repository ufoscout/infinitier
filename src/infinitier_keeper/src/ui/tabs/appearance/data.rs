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

/// Pull the appearance fields out of the CRE header. Only the V1.0 (BG /
/// BG2 / EE) header layout is read — the engine this tab targets; other
/// header versions return `None` and the tab shows nothing.
pub fn appearance_data(cre: &Cre) -> Option<AppearanceData> {
    let CreHeader::V10(h) = &cre.header else {
        return None;
    };
    Some(AppearanceData {
        animation_id: h.animation_id,
        colors: [
            ColorSlot {
                label: "Hair",
                index: h.hair_colour_index,
            },
            ColorSlot {
                label: "Skin",
                index: h.skin_colour_index,
            },
            ColorSlot {
                label: "Clothing Major",
                index: h.major_colour_index,
            },
            ColorSlot {
                label: "Clothing Minor",
                index: h.minor_colour_index,
            },
            ColorSlot {
                label: "Armor",
                index: h.armor_colour_index,
            },
            ColorSlot {
                label: "Leather",
                index: h.leather_colour_index,
            },
            ColorSlot {
                label: "Metal",
                index: h.metal_colour_index,
            },
        ],
    })
}
