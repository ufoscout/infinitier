//! Copied verbatim from infinitier_keeper. Framework-agnostic CRE/GAM
//! field accessors (read getters are used; setters are kept for parity
//! but unused in the read-only port).
#![allow(dead_code)]

//! Per-version accessors for the CRE header fields the keeper
//! edits. We deliberately keep the per-version pattern-matching in
//! the keeper instead of polluting `infinitier_cre_resource` with
//! 50+ tiny getter/setter pairs — only one consumer needs them.
//!
//! Every function follows the same shape: read returns the current
//! value (with `Option<…>` when the field is version-specific so the
//! UI can hide the row); write is a no-op when the field doesn't
//! exist on the given CRE version. Type-specific clamping lives in
//! [`crate::editable_fields`], not here.

use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::cre::{Cre, CreHeader};
use infinitier_core::resource::gam::{
    Bg2GamData, BgGamData, EeGamData, GamEngineData, Iwd2GamData, IwdGamData, PstGamData,
};

// ─── Combat & status (CRE) ───────────────────────────────────────────

pub fn current_hit_points(cre: &Cre) -> u16 {
    cre.current_hit_points()
}

pub fn set_current_hit_points(cre: &mut Cre, value: u16) {
    match &mut cre.header {
        CreHeader::V10(h) => h.current_hit_points = value,
        CreHeader::V12(h) => h.current_hit_points = value,
        CreHeader::V90(h) => h.current_hit_points = value,
        CreHeader::V22(h) => h.current_hit_points = value,
    }
}

pub fn max_hit_points(cre: &Cre) -> u16 {
    cre.maximum_hit_points()
}

pub fn set_max_hit_points(cre: &mut Cre, value: u16) {
    match &mut cre.header {
        CreHeader::V10(h) => h.maximum_hit_points = value,
        CreHeader::V12(h) => h.maximum_hit_points = value,
        CreHeader::V90(h) => h.maximum_hit_points = value,
        CreHeader::V22(h) => h.maximum_hit_points = value,
    }
}

/// AC (natural) on AD&D, single AC on IWD2.
pub fn ac_natural(cre: &Cre) -> i16 {
    match &cre.header {
        CreHeader::V10(h) => h.armor_class_natural,
        CreHeader::V12(h) => h.armor_class_natural,
        CreHeader::V90(h) => h.armor_class_natural,
        CreHeader::V22(h) => h.armor_class,
    }
}

pub fn set_ac_natural(cre: &mut Cre, value: i16) {
    match &mut cre.header {
        CreHeader::V10(h) => h.armor_class_natural = value,
        CreHeader::V12(h) => h.armor_class_natural = value,
        CreHeader::V90(h) => h.armor_class_natural = value,
        CreHeader::V22(h) => h.armor_class = value,
    }
}

/// AC (effective) — only AD&D headers carry the separate effective
/// value. `None` on IWD2 (V22) so the UI can hide the row.
pub fn ac_effective(cre: &Cre) -> Option<i16> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.armor_class_effective),
        CreHeader::V12(h) => Some(h.armor_class_effective),
        CreHeader::V90(h) => Some(h.armor_class_effective),
        CreHeader::V22(_) => None,
    }
}

pub fn set_ac_effective(cre: &mut Cre, value: i16) {
    match &mut cre.header {
        CreHeader::V10(h) => h.armor_class_effective = value,
        CreHeader::V12(h) => h.armor_class_effective = value,
        CreHeader::V90(h) => h.armor_class_effective = value,
        CreHeader::V22(_) => {}
    }
}

/// THAC0 on AD&D, Base Attack Bonus on IWD2 (same storage byte,
/// signed `i8`). AD&D THAC0 is conceptually signed — high-level
/// characters carry small/negative values. IWD2 BAB is non-negative
/// in practice; sharing the signed type is fine because gameplay
/// values stay in `0..=127`.
pub fn thac0_or_bab(cre: &Cre) -> i8 {
    match &cre.header {
        CreHeader::V10(h) => h.thac0,
        CreHeader::V12(h) => h.thac0,
        CreHeader::V90(h) => h.thac0,
        CreHeader::V22(h) => h.base_attack_bonus_bab_for_non,
    }
}

pub fn set_thac0_or_bab(cre: &mut Cre, value: i8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.thac0 = value,
        CreHeader::V12(h) => h.thac0 = value,
        CreHeader::V90(h) => h.thac0 = value,
        CreHeader::V22(h) => h.base_attack_bonus_bab_for_non = value,
    }
}

pub fn attacks_byte(cre: &Cre) -> u8 {
    match &cre.header {
        CreHeader::V10(h) => h.number_of_attacks.to_u8(),
        CreHeader::V12(h) => h.number_of_attacks.to_u8(),
        CreHeader::V90(h) => h.number_of_attacks.to_u8(),
        CreHeader::V22(h) => h.number_of_attacks.to_u8(),
    }
}

pub fn set_attacks_byte(cre: &mut Cre, value: u8) {
    use infinitier_core::resource::cre::NumberOfAttacks;
    let n = NumberOfAttacks::from_u8(value);
    match &mut cre.header {
        CreHeader::V10(h) => h.number_of_attacks = n,
        CreHeader::V12(h) => h.number_of_attacks = n,
        CreHeader::V90(h) => h.number_of_attacks = n,
        CreHeader::V22(h) => h.number_of_attacks = n,
    }
}

// Simple u8 fields present on every version: fatigue / intoxication /
// luck. One macro condenses the repetition.
macro_rules! cre_u8_field {
    ($read:ident, $write:ident, $field:ident) => {
        pub fn $read(cre: &Cre) -> u8 {
            match &cre.header {
                CreHeader::V10(h) => h.$field,
                CreHeader::V12(h) => h.$field,
                CreHeader::V90(h) => h.$field,
                CreHeader::V22(h) => h.$field,
            }
        }
        pub fn $write(cre: &mut Cre, value: u8) {
            match &mut cre.header {
                CreHeader::V10(h) => h.$field = value,
                CreHeader::V12(h) => h.$field = value,
                CreHeader::V90(h) => h.$field = value,
                CreHeader::V22(h) => h.$field = value,
            }
        }
    };
}

cre_u8_field!(fatigue, set_fatigue, fatigue);
cre_u8_field!(intoxication, set_intoxication, intoxication);
cre_u8_field!(luck, set_luck, luck);

// ─── Experience & levels (CRE) ───────────────────────────────────────

/// Character XP. Stored in `creature_power_level_for_summoning_spells`
/// for party-member CREs, which is the field the abilities tab
/// already surfaces as "Experience" / "Experience (primary)".
pub fn experience(cre: &Cre) -> u32 {
    match &cre.header {
        CreHeader::V10(h) => h.creature_power_level_for_summoning_spells,
        CreHeader::V12(h) => h.creature_power_level_for_summoning_spells,
        CreHeader::V90(h) => h.creature_power_level_for_summoning_spells,
        CreHeader::V22(h) => h.creature_power_level_for_summoning_spells,
    }
}

pub fn set_experience(cre: &mut Cre, value: u32) {
    match &mut cre.header {
        CreHeader::V10(h) => h.creature_power_level_for_summoning_spells = value,
        CreHeader::V12(h) => h.creature_power_level_for_summoning_spells = value,
        CreHeader::V90(h) => h.creature_power_level_for_summoning_spells = value,
        CreHeader::V22(h) => h.creature_power_level_for_summoning_spells = value,
    }
}

pub fn xp_for_kill(cre: &Cre) -> u32 {
    match &cre.header {
        CreHeader::V10(h) => h.xp_gained_for_killing_this_creature,
        CreHeader::V12(h) => h.xp_gained_for_killing_this_creature,
        CreHeader::V90(h) => h.xp_gained_for_killing_this_creature,
        CreHeader::V22(h) => h.xp_gained_for_killing_this_creature,
    }
}

pub fn set_xp_for_kill(cre: &mut Cre, value: u32) {
    match &mut cre.header {
        CreHeader::V10(h) => h.xp_gained_for_killing_this_creature = value,
        CreHeader::V12(h) => h.xp_gained_for_killing_this_creature = value,
        CreHeader::V90(h) => h.xp_gained_for_killing_this_creature = value,
        CreHeader::V22(h) => h.xp_gained_for_killing_this_creature = value,
    }
}

/// "Level (1st class)". V10 stores it under
/// `level_first_class_highest_attained_level`; V12 / V90 renamed it
/// to `highest_attained_level_in_class`. V22 uses per-class fields
/// — `None` there so the UI hides the row.
pub fn level_first_class(cre: &Cre) -> Option<u8> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.level_first_class_highest_attained_level),
        CreHeader::V12(h) => Some(h.highest_attained_level_in_class),
        CreHeader::V90(h) => Some(h.highest_attained_level_in_class),
        CreHeader::V22(_) => None,
    }
}

pub fn set_level_first_class(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.level_first_class_highest_attained_level = value,
        CreHeader::V12(h) => h.highest_attained_level_in_class = value,
        CreHeader::V90(h) => h.highest_attained_level_in_class = value,
        CreHeader::V22(_) => {}
    }
}

pub fn level_second_class(cre: &Cre) -> Option<u8> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.level_second_class_highest_attained_level),
        CreHeader::V12(h) => Some(h.highest_attained_level_in_class_2),
        CreHeader::V90(h) => Some(h.highest_attained_level_in_class_2),
        CreHeader::V22(_) => None,
    }
}

pub fn set_level_second_class(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.level_second_class_highest_attained_level = value,
        CreHeader::V12(h) => h.highest_attained_level_in_class_2 = value,
        CreHeader::V90(h) => h.highest_attained_level_in_class_2 = value,
        CreHeader::V22(_) => {}
    }
}

/// Name of the small-portrait resource (BMP). Stripped of trailing
/// NULs by the importer. Per-version field naming dispatch.
pub fn small_portrait_name(cre: &Cre) -> &str {
    match &cre.header {
        CreHeader::V10(h) => &h.small_portrait_bmp,
        CreHeader::V12(h) => &h.small_portrait_bmp,
        CreHeader::V90(h) => &h.small_portrait,
        CreHeader::V22(h) => &h.small_portrait_bmp,
    }
}

/// Name of the large-portrait resource. Usually a BMP. V10 names
/// the field `large_portrait_pstee_bam_other_games` because PSTEE
/// uses a BAM here while every other engine uses a BMP — the
/// keeper loads it as a BMP and falls back to the small portrait
/// when that fails.
pub fn large_portrait_name(cre: &Cre) -> &str {
    match &cre.header {
        CreHeader::V10(h) => &h.large_portrait_pstee_bam_other_games,
        CreHeader::V12(h) => &h.large_portrait_bmp,
        CreHeader::V90(h) => &h.large_portrait,
        CreHeader::V22(h) => &h.large_portrait_bmp,
    }
}

/// Highest level among any class the character holds. Used to
/// estimate derived values whose magnitude scales with level — e.g.
/// the cumulative HP bonus from Constitution (`level * con_bonus`).
/// V22 (IWD2) reports the summed level count via `total_levels`.
pub fn primary_level(cre: &Cre) -> u8 {
    match &cre.header {
        CreHeader::V10(h) => [
            h.level_first_class_highest_attained_level,
            h.level_second_class_highest_attained_level,
            h.level_third_class_highest_attained_level,
        ]
        .into_iter()
        .max()
        .unwrap_or(0),
        CreHeader::V12(h) => [
            h.highest_attained_level_in_class,
            h.highest_attained_level_in_class_2,
            h.highest_attained_level_in_class_3,
        ]
        .into_iter()
        .max()
        .unwrap_or(0),
        CreHeader::V90(h) => [
            h.highest_attained_level_in_class,
            h.highest_attained_level_in_class_2,
            h.highest_attained_level_in_class_3,
        ]
        .into_iter()
        .max()
        .unwrap_or(0),
        CreHeader::V22(h) => h.total_levels,
    }
}

pub fn level_third_class(cre: &Cre) -> Option<u8> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.level_third_class_highest_attained_level),
        CreHeader::V12(h) => Some(h.highest_attained_level_in_class_3),
        CreHeader::V90(h) => Some(h.highest_attained_level_in_class_3),
        CreHeader::V22(_) => None,
    }
}

pub fn set_level_third_class(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.level_third_class_highest_attained_level = value,
        CreHeader::V12(h) => h.highest_attained_level_in_class_3 = value,
        CreHeader::V90(h) => h.highest_attained_level_in_class_3 = value,
        CreHeader::V22(_) => {}
    }
}

// ─── Morale (CRE) — AD&D only ────────────────────────────────────────

/// V10's morale field has a verbose name; V12 / V90 use just
/// `morale`. V22 has no morale (d20 uses save throws instead).
pub fn morale(cre: &Cre) -> Option<u8> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.morale_default_value_is_10_capped),
        CreHeader::V12(h) => Some(h.morale),
        CreHeader::V90(h) => Some(h.morale),
        CreHeader::V22(_) => None,
    }
}

pub fn set_morale(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.morale_default_value_is_10_capped = value,
        CreHeader::V12(h) => h.morale = value,
        CreHeader::V90(h) => h.morale = value,
        CreHeader::V22(_) => {}
    }
}

pub fn morale_break(cre: &Cre) -> Option<u8> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.morale_break_see_here_for_further),
        CreHeader::V12(h) => Some(h.morale_break),
        CreHeader::V90(h) => Some(h.morale_break),
        CreHeader::V22(_) => None,
    }
}

pub fn set_morale_break(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.morale_break_see_here_for_further = value,
        CreHeader::V12(h) => h.morale_break = value,
        CreHeader::V90(h) => h.morale_break = value,
        CreHeader::V22(_) => {}
    }
}

pub fn morale_recovery(cre: &Cre) -> Option<u16> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.morale_recovery_time_see_here_for),
        CreHeader::V12(h) => Some(h.morale_recovery_time),
        CreHeader::V90(h) => Some(h.morale_recovery_time),
        CreHeader::V22(_) => None,
    }
}

pub fn set_morale_recovery(cre: &mut Cre, value: u16) {
    match &mut cre.header {
        CreHeader::V10(h) => h.morale_recovery_time_see_here_for = value,
        CreHeader::V12(h) => h.morale_recovery_time = value,
        CreHeader::V90(h) => h.morale_recovery_time = value,
        CreHeader::V22(_) => {}
    }
}

// ─── Thief skills (CRE) — AD&D ───────────────────────────────────────

/// "Hide in Shadows". V10/V90/V22 store `hide_in_shadows_base`. V12
/// folded hide+silent into the unified `stealth` field, so V12
/// returns `None` for this row (the UI hides it on V12).
pub fn hide_in_shadows(cre: &Cre) -> Option<u8> {
    match &cre.header {
        CreHeader::V10(h) => Some(h.hide_in_shadows_base),
        CreHeader::V12(_) => None,
        CreHeader::V90(h) => Some(h.hide_in_shadows_base),
        CreHeader::V22(h) => Some(h.hide_in_shadows_base),
    }
}

pub fn set_hide_in_shadows(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.hide_in_shadows_base = value,
        CreHeader::V12(_) => {}
        CreHeader::V90(h) => h.hide_in_shadows_base = value,
        CreHeader::V22(h) => h.hide_in_shadows_base = value,
    }
}

/// "Move Silently". V10 / V22 have a `move_silently` field; V12 and
/// V90 instead store the value in `stealth` (V12 has only stealth,
/// V90 has stealth alongside a separate hide-in-shadows). The UI
/// labels the row accordingly via [`move_silently_label`].
pub fn move_silently(cre: &Cre) -> u8 {
    match &cre.header {
        CreHeader::V10(h) => h.move_silently,
        CreHeader::V12(h) => h.stealth,
        CreHeader::V90(h) => h.stealth,
        CreHeader::V22(h) => h.move_silently,
    }
}

pub fn set_move_silently(cre: &mut Cre, value: u8) {
    match &mut cre.header {
        CreHeader::V10(h) => h.move_silently = value,
        CreHeader::V12(h) => h.stealth = value,
        CreHeader::V90(h) => h.stealth = value,
        CreHeader::V22(h) => h.move_silently = value,
    }
}

/// Per-version label for the move-silently / stealth row. V12 and
/// V90 store the value under `stealth`, so the UI labels it
/// "Stealth" there. V10 / V22 keep the AD&D "Move Silently" name.
pub fn move_silently_label(cre: &Cre) -> &'static str {
    match &cre.header {
        CreHeader::V12(_) | CreHeader::V90(_) => "Stealth",
        _ => "Move Silently",
    }
}

// AD&D u8 skill fields present on V10/V12/V90; V22 uses d20 skills
// and returns `None` here so the rows hide.
macro_rules! adnd_skill {
    ($read:ident, $write:ident, $field:ident) => {
        pub fn $read(cre: &Cre) -> Option<u8> {
            match &cre.header {
                CreHeader::V10(h) => Some(h.$field),
                CreHeader::V12(h) => Some(h.$field),
                CreHeader::V90(h) => Some(h.$field),
                CreHeader::V22(_) => None,
            }
        }
        pub fn $write(cre: &mut Cre, value: u8) {
            match &mut cre.header {
                CreHeader::V10(h) => h.$field = value,
                CreHeader::V12(h) => h.$field = value,
                CreHeader::V90(h) => h.$field = value,
                CreHeader::V22(_) => {}
            }
        }
    };
}

adnd_skill!(lockpicking, set_lockpicking, lockpicking);
adnd_skill!(find_traps, set_find_traps, find_disarm_traps);
adnd_skill!(set_traps, set_set_traps, set_traps);
adnd_skill!(pick_pockets, set_pick_pockets, pick_pockets);
adnd_skill!(detect_illusion, set_detect_illusion, detect_illusion);
adnd_skill!(lore, set_lore, lore);

// ─── GAM-side fields (party-wide) ────────────────────────────────────

/// Party gold, on the GAM header (u32). Same field name + type
/// across every engine.
pub fn party_gold(gam: &ImportedGam) -> u32 {
    gam.header.party_gold
}

pub fn set_party_gold(gam: &mut ImportedGam, value: u32) {
    gam.header.party_gold = value;
}

/// Party reputation. Stored in the engine-specific GAM data as
/// `reputation * 10`; the existing accessor on `GamEngineData` does
/// the divide. We mirror that contract: read returns the
/// player-facing value (0..=20-ish), write multiplies by 10 on the
/// way to disk.
pub fn party_reputation(gam: &ImportedGam) -> u32 {
    gam.engine_data.reputation()
}

pub fn set_party_reputation(gam: &mut ImportedGam, value: u32) {
    let raw = value.saturating_mul(10);
    match &mut gam.engine_data {
        GamEngineData::Bg(d) => set_bg_reputation(d, raw),
        GamEngineData::Bg2(d) => set_bg2_reputation(d, raw),
        GamEngineData::Ee(d) => set_ee_reputation(d, raw),
        GamEngineData::Iwd(d) => set_iwd_reputation(d, raw),
        GamEngineData::Iwd2(d) => set_iwd2_reputation(d, raw),
        GamEngineData::Pst(d) => set_pst_reputation(d, raw),
    }
}

// Each engine variant exposes `reputation` as a public field — the
// trivial assignments below stay one-liners so the dispatch above
// reads like a flat table.
fn set_bg_reputation(d: &mut BgGamData, v: u32) {
    d.reputation = v;
}
fn set_bg2_reputation(d: &mut Bg2GamData, v: u32) {
    d.reputation = v;
}
fn set_ee_reputation(d: &mut EeGamData, v: u32) {
    d.reputation = v;
}
fn set_iwd_reputation(d: &mut IwdGamData, v: u32) {
    d.reputation = v;
}
fn set_iwd2_reputation(d: &mut Iwd2GamData, v: u32) {
    d.reputation = v;
}
fn set_pst_reputation(d: &mut PstGamData, v: u32) {
    d.reputation = v;
}
