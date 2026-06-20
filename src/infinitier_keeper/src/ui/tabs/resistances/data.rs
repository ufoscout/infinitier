//! Read-only extraction of a creature's resistances, saving throws and
//! per-damage-type armor-class modifiers for the Resistances tab.
//!
//! The two engine families lay this out differently:
//!
//! * **AD&D** (V1.0 / V1.2 / V9.0): combat resistances, the five AD&D
//!   saving throws, and the per-damage-type AC modifiers.
//! * **IWD2** (V2.2, d20): the same combat resistances plus a separate
//!   "Magic Damage" resistance, and the three d20 saves (Fortitude /
//!   Reflex / Will). The `0x59` resistance byte is *spell resistance*
//!   ("Spells"), not the AD&D generic "Magic"; there are no AC
//!   modifiers shown.
//!
//! All values live in the CRE header.

use infinitier_core::resource::cre::{Cre, CreHeader};

/// Combat resistances (percent). [`magic`](Self::magic) is the `0x59`
/// byte — shown as "Magic" on AD&D creatures and "Spells" on IWD2.
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

/// The five AD&D saving throws (lower is better).
pub struct SavingThrows {
    pub paralyze_poison_death: u8,
    pub rod_staff_wand: u8,
    pub petrify_polymorph: u8,
    pub breath: u8,
    pub spells: u8,
}

/// The three IWD2 (d20) saving throws.
pub struct Iwd2Saves {
    pub fortitude: u8,
    pub reflex: u8,
    pub will: u8,
}

/// Per-damage-type armor-class modifiers (signed; lower AC is better).
pub struct AcModifiers {
    pub slashing: i16,
    pub missile: i16,
    pub crushing: i16,
    pub piercing: i16,
}

/// Resistances tab payload, shaped per engine family.
pub enum ResistData {
    /// V1.0 / V1.2 / V9.0 — AD&D saves + AC modifiers.
    Adnd {
        resistances: Resistances,
        saving_throws: SavingThrows,
        ac_modifiers: AcModifiers,
    },
    /// V2.2 (IWD2) — d20 saves and a separate Magic Damage resistance.
    Iwd2 {
        resistances: Resistances,
        /// `0x60` — "Magic Damage" resistance (IWD2 only).
        magic_damage: i8,
        saves: Iwd2Saves,
    },
}

/// Pull the resistances / saves / AC modifiers out of a creature's
/// header, in the shape its engine family uses.
pub fn resist_data(cre: &Cre) -> ResistData {
    // The resistance field names are identical across every header
    // struct, so a small macro extracts them without repeating the list.
    macro_rules! resistances {
        ($h:expr) => {
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
            }
        };
    }
    macro_rules! ac_modifiers {
        ($h:expr) => {
            AcModifiers {
                slashing: $h.armor_class_slashing_attacks_modifier,
                missile: $h.armor_class_missile_attacks_modifier,
                crushing: $h.armor_class_crushing_attacks_modifier,
                piercing: $h.armor_class_piercing_attacks_modifier,
            }
        };
    }
    macro_rules! adnd {
        ($h:expr) => {
            ResistData::Adnd {
                resistances: resistances!($h),
                saving_throws: SavingThrows {
                    paralyze_poison_death: $h.save_versus_death,
                    rod_staff_wand: $h.save_versus_wands,
                    petrify_polymorph: $h.save_versus_polymorph,
                    breath: $h.save_versus_breath_attacks,
                    spells: $h.save_versus_spells,
                },
                ac_modifiers: ac_modifiers!($h),
            }
        };
    }

    match &cre.header {
        CreHeader::V10(h) => adnd!(h),
        CreHeader::V12(h) => adnd!(h),
        CreHeader::V90(h) => adnd!(h),
        CreHeader::V22(h) => ResistData::Iwd2 {
            resistances: resistances!(h),
            magic_damage: h.resist_magic_damage,
            saves: Iwd2Saves {
                fortitude: h.save_versus_fortitude,
                reflex: h.save_versus_reflex,
                will: h.save_versus_will,
            },
        },
    }
}
