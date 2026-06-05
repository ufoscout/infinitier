//! Read-only extraction of a creature's resistances, saving throws
//! and per-damage-type armor-class modifiers for the Resistances tab.
//!
//! All three groups live in the CRE header. The combat resistances
//! (percent, 0..=100) and the AC modifiers are present on every CRE
//! version; the five AD&D saving throws exist only on V1.0 / V1.2 /
//! V9.0 — IWD2 (V2.2) replaces them with Fortitude / Reflex / Will, so
//! [`ResistData::saving_throws`] is `None` there.

use infinitier_core::resource::cre::{Cre, CreHeader};

/// Combat resistances (percent) shown in the "Resistances" group box.
/// The three EEKeeper "Magic" fields are [`magic`](Self::magic),
/// [`magic_fire`](Self::magic_fire) and [`magic_cold`](Self::magic_cold).
pub struct Resistances {
    pub acid: i8,
    pub cold: i8,
    pub electricity: i8,
    pub fire: i8,
    pub crushing: i8,
    pub piercing: i8,
    pub slashing: i8,
    pub missile: i8,
    pub magic: i8,
    pub magic_fire: i8,
    pub magic_cold: i8,
}

/// The five AD&D saving throws (lower is better). Absent on IWD2.
pub struct SavingThrows {
    pub paralyze_poison_death: u8,
    pub rod_staff_wand: u8,
    pub petrify_polymorph: u8,
    pub breath: u8,
    pub spells: u8,
}

/// Per-damage-type armor-class modifiers (signed; lower AC is better).
pub struct AcModifiers {
    pub slashing: i16,
    pub missile: i16,
    pub crushing: i16,
    pub piercing: i16,
}

pub struct ResistData {
    pub resistances: Resistances,
    pub saving_throws: Option<SavingThrows>,
    pub ac_modifiers: AcModifiers,
}

/// Pull the resistances / saves / AC modifiers out of a creature's
/// header. V1.0 / V1.2 / V9.0 share an identical layout for all three
/// groups; V2.2 shares the resistances + AC modifiers but has no AD&D
/// saving throws.
pub fn resist_data(cre: &Cre) -> ResistData {
    // The resistance + AC-modifier field names are identical across
    // every header struct, so a small macro extracts them without
    // repeating the field list once per version.
    macro_rules! resist_and_ac {
        ($h:expr) => {
            (
                Resistances {
                    acid: $h.resist_acid,
                    cold: $h.resist_cold,
                    electricity: $h.resist_electricity,
                    fire: $h.resist_fire,
                    crushing: $h.resist_crushing,
                    piercing: $h.resist_piercing,
                    slashing: $h.resist_slashing,
                    missile: $h.resist_missile,
                    magic: $h.resist_magic,
                    magic_fire: $h.resist_magic_fire,
                    magic_cold: $h.resist_magic_cold,
                },
                AcModifiers {
                    slashing: $h.armor_class_slashing_attacks_modifier,
                    missile: $h.armor_class_missile_attacks_modifier,
                    crushing: $h.armor_class_crushing_attacks_modifier,
                    piercing: $h.armor_class_piercing_attacks_modifier,
                },
            )
        };
    }
    macro_rules! saves {
        ($h:expr) => {
            SavingThrows {
                paralyze_poison_death: $h.save_versus_death,
                rod_staff_wand: $h.save_versus_wands,
                petrify_polymorph: $h.save_versus_polymorph,
                breath: $h.save_versus_breath_attacks,
                spells: $h.save_versus_spells,
            }
        };
    }

    let (resistances, ac_modifiers, saving_throws) = match &cre.header {
        CreHeader::V10(h) => {
            let (r, a) = resist_and_ac!(h);
            (r, a, Some(saves!(h)))
        }
        CreHeader::V12(h) => {
            let (r, a) = resist_and_ac!(h);
            (r, a, Some(saves!(h)))
        }
        CreHeader::V90(h) => {
            let (r, a) = resist_and_ac!(h);
            (r, a, Some(saves!(h)))
        }
        CreHeader::V22(h) => {
            let (r, a) = resist_and_ac!(h);
            (r, a, None)
        }
    };

    ResistData {
        resistances,
        saving_throws,
        ac_modifiers,
    }
}
