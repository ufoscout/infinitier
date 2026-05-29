//! Per-engine gameplay ranges for the editable fields the keeper
//! surfaces.
//!
//! Each [`AbilityRange<T>`] describes the inclusive `min..=max` an
//! editable field should accept. Two distinct conventions are mixed
//! together here:
//!
//! - **Hardcoded gameplay clamps** — where the IE engine itself
//!   actively clamps or refuses out-of-range values. These match
//!   what NearInfinity / GemRB enforce. Examples: ability scores
//!   (1..=25 / 1..=30), reputation (0..=20), the attacks-per-round
//!   byte (0..=10 documented), morale (0..=20).
//! - **Storage-type maxes** — for everything else. The CRE / GAM
//!   formats store these fields as concrete integers (`u8`, `u16`,
//!   `i16`, `u32`); the engine accepts any value the type can hold.
//!   Caps in that case are `T::MIN..=T::MAX`. Examples: HP (`u16`),
//!   AC (`i16`), gold (`u32`), experience (`u32`).
//!
//! See [`EngineCaps`] for the per-field list. The function entry
//! point is [`caps_for`].

use infinitier_common::Engine;

/// Inclusive `(min, max)` range over a numeric type. The type
/// parameter lets each [`EngineCaps`] field carry its on-disk width
/// (`u8` for the small stats, `u16` for HP / morale-recovery, `i16`
/// for AC, `u32` for gold / experience).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbilityRange<T: Copy + Ord> {
    pub min: T,
    pub max: T,
}

impl<T: Copy + Ord> AbilityRange<T> {
    /// `true` when `value` lies inside `min..=max`.
    pub fn contains(self, value: T) -> bool {
        value >= self.min && value <= self.max
    }

    /// Clamp `value` to `min..=max`. Returns the closest in-range
    /// value for any input.
    pub fn clamp(self, value: T) -> T {
        value.clamp(self.min, self.max)
    }
}

/// Bundle of per-field caps. One value per editable row currently
/// surfaced by the keeper's *Abilities* tab — ability scores plus
/// the combat / experience / morale / skill rows on the same screen.
///
/// Fields share a single value across every engine where the cap is
/// the same; only the entries that genuinely differ between engines
/// (ability score range today) get per-variant values in [`caps_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineCaps {
    // ── Ability scores ──────────────────────────────────────────
    /// STR / DEX / CON / INT / WIS / CHA. AD&D engines clamp at 25,
    /// IWD2 (d20) at 30.
    pub ability_score: AbilityRange<u8>,
    /// AD&D extraordinary-strength percentile (0..=100). Carried for
    /// every engine; the IWD2 CRE header doesn't store this byte so
    /// the caller never reads it on V2.2.
    pub strength_percentile: AbilityRange<u8>,

    // ── Combat & status ─────────────────────────────────────────
    /// Current hit points. Storage is `u16` in every CRE version;
    /// the engine displays values > 32767 as negative but the file
    /// happily accepts them.
    pub current_hit_points: AbilityRange<u16>,
    /// Maximum hit points (same storage as `current_hit_points`).
    pub max_hit_points: AbilityRange<u16>,
    /// Armor class — lower is better. Stored as `i16`; vanilla AD&D
    /// gameplay sits in roughly `-20..=20` but the engine doesn't
    /// clamp the field so we expose the full `i16` range.
    pub armor_class: AbilityRange<i16>,
    /// THAC0 (AD&D) or BAB (IWD2 d20). Stored as one byte; the
    /// engine reinterprets it as signed for AD&D (THAC0 can be
    /// negative for high-level characters), so we surface the
    /// full `i8` range here. Vanilla gameplay range is roughly
    /// `-5..=20` for THAC0 and `0..=30` for BAB, but the engine
    /// doesn't clamp.
    pub thac0: AbilityRange<i8>,
    /// Raw attacks-per-round byte. Variants 0..=10 are documented in
    /// the IESDP (mapped to 1, 2, 3, 4, 5, 0.5, 1.5, 2.5, 3.5, 4.5);
    /// the editor exposes the same range so unknown bytes don't leak
    /// in via the input.
    pub attacks_byte: AbilityRange<u8>,
    /// Party reputation. Stored as `u32` in the GAM header but
    /// hardcoded-clamped to `0..=20` by every IE engine.
    pub reputation: AbilityRange<u32>,
    /// Party gold (`u32` in the GAM header). The engine handles any
    /// `u32` value although the UI cap is "around 999 999" in most
    /// vanilla games.
    pub party_gold: AbilityRange<u32>,
    /// Fatigue (`u8` storage). Engine uses higher values to slow
    /// recovery — no hardcoded clamp.
    pub fatigue: AbilityRange<u8>,
    /// Intoxication (`u8`). Vanilla "fully drunk" sits around 200.
    pub intoxication: AbilityRange<u8>,
    /// Luck (`u8`). Engine reads values 128..=255 as negative for
    /// roll-modifier purposes; storage is unsigned.
    pub luck: AbilityRange<u8>,

    // ── Experience & levels ─────────────────────────────────────
    /// Current experience (`u32`). Class-level XP caps live in
    /// `XPCAP.2DA` / `XPLEVEL.2DA` — they're per-class game data,
    /// not engine code, so we expose the storage range here.
    pub experience: AbilityRange<u32>,
    /// XP awarded for killing this creature (`u32`).
    pub xp_for_kill: AbilityRange<u32>,
    /// Per-class level (`u8`). Vanilla games cap around 50; storage
    /// is the full byte.
    pub class_level: AbilityRange<u8>,

    // ── Morale ──────────────────────────────────────────────────
    /// Current morale. Default 10. Vanilla AD&D gameplay range is
    /// 0..=20 — engine actively clamps reads/writes there.
    pub morale: AbilityRange<u8>,
    /// Morale-break threshold (`u8`); also bounded by the engine at
    /// 0..=20.
    pub morale_break: AbilityRange<u8>,
    /// Morale recovery time in ticks (`u16`).
    pub morale_recovery: AbilityRange<u16>,

    // ── Thief skills (AD&D V1.0/V1.2/V9.0 — V2.2 uses d20 skills,
    //    not surfaced here yet) ──────────────────────────────────
    /// Per-skill cap shared by Hide in Shadows / Move Silently /
    /// Open Locks / Find Traps / Set Traps / Pick Pockets / Detect
    /// Illusions (`u8`). Vanilla play caps each skill around 100;
    /// the engine accepts higher.
    pub thief_skill: AbilityRange<u8>,
    /// Lore (`u8`). Bards in particular can push lore well past
    /// 100; the engine accepts the full byte.
    pub lore: AbilityRange<u8>,
}

/// Caps shared by every IE engine. The two `match` branches in
/// [`caps_for`] only need to override the values that actually
/// differ between AD&D and d20.
const SHARED_CAPS: EngineCaps = EngineCaps {
    // Filled by `caps_for` per-branch (AD&D vs d20). The placeholder
    // here is the AD&D value — `Iwd2` swaps it for 1..=30.
    ability_score: AbilityRange { min: 1, max: 25 },
    strength_percentile: AbilityRange { min: 0, max: 100 },
    current_hit_points: AbilityRange { min: u16::MIN, max: u16::MAX },
    max_hit_points: AbilityRange { min: u16::MIN, max: u16::MAX },
    armor_class: AbilityRange { min: i16::MIN, max: i16::MAX },
    thac0: AbilityRange { min: i8::MIN, max: i8::MAX },
    attacks_byte: AbilityRange { min: 0, max: 10 },
    reputation: AbilityRange { min: 0, max: 20 },
    party_gold: AbilityRange { min: u32::MIN, max: u32::MAX },
    fatigue: AbilityRange { min: u8::MIN, max: u8::MAX },
    intoxication: AbilityRange { min: u8::MIN, max: u8::MAX },
    luck: AbilityRange { min: u8::MIN, max: u8::MAX },
    experience: AbilityRange { min: u32::MIN, max: u32::MAX },
    xp_for_kill: AbilityRange { min: u32::MIN, max: u32::MAX },
    class_level: AbilityRange { min: u8::MIN, max: u8::MAX },
    morale: AbilityRange { min: 0, max: 20 },
    morale_break: AbilityRange { min: 0, max: 20 },
    morale_recovery: AbilityRange { min: u16::MIN, max: u16::MAX },
    thief_skill: AbilityRange { min: u8::MIN, max: u8::MAX },
    lore: AbilityRange { min: u8::MIN, max: u8::MAX },
};

/// Practical gameplay caps for `engine`'s editable fields. See the
/// module docs for what counts as "practical".
pub fn caps_for(engine: Engine) -> EngineCaps {
    match engine {
        Engine::Bg | Engine::Bg2 | Engine::Ee | Engine::Iwd | Engine::Pst => SHARED_CAPS,
        Engine::Iwd2 => EngineCaps {
            // d20 ability scores cap higher than AD&D 2e.
            ability_score: AbilityRange { min: 1, max: 30 },
            ..SHARED_CAPS
        },
    }
}

// ── Ability-score derived bonuses ────────────────────────────────────
//
// The AD&D 2e bonuses below are 1-for-1 with the vanilla game data
// (`STRMOD.2DA`, `STRMODEX.2DA`, `DEXMOD.2DA`, `HPCONBON.2DA`) — the
// IE engine reads those tables at startup and converts them into the
// same step functions. Modders who edit the 2DAs will get different
// numbers; here we surface the well-known vanilla distribution.
//
// IWD2 (d20) uses `(score - 10) / 2` for every modifier — no table
// lookup.

/// Bundle of bonuses derived from a creature's ability scores. Sign
/// conventions match the engine the score belongs to:
///
/// - `thac0_from_strength` — positive = better to-hit (THAC0 goes
///   down). For IWD2 it's the d20 attack-roll modifier.
/// - `ac_from_dexterity` — for AD&D 2e, *negative* = better AC
///   (DEX 18 → -4). For IWD2 d20, *positive* = better AC.
/// - `hp_per_level_from_constitution` — positive = more HP per HD.
///   AD&D uses the non-warrior `HPCONBON.2DA` row; warrior bonuses
///   from `HPWAR.2DA` are larger but require class info we don't
///   yet wire through the keeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AbilityBonuses {
    pub thac0_from_strength: i8,
    pub ac_from_dexterity: i8,
    pub hp_per_level_from_constitution: i8,
}

/// Combined bonuses for one set of ability scores under `engine`'s
/// rules. The strength percentile only matters when STR is exactly
/// 18 (and the byte is non-zero — see [`EngineCaps::strength_percentile`]
/// for the storage convention).
pub fn ability_bonuses(
    engine: Engine,
    strength: u8,
    strength_percentile: u8,
    dexterity: u8,
    constitution: u8,
) -> AbilityBonuses {
    match engine {
        Engine::Iwd2 => AbilityBonuses {
            thac0_from_strength: d20_modifier(strength),
            ac_from_dexterity: d20_modifier(dexterity),
            hp_per_level_from_constitution: d20_modifier(constitution),
        },
        Engine::Bg | Engine::Bg2 | Engine::Ee | Engine::Iwd | Engine::Pst => AbilityBonuses {
            thac0_from_strength: strength_to_hit_bonus_adnd(strength, strength_percentile),
            ac_from_dexterity: dexterity_ac_bonus_adnd(dexterity),
            hp_per_level_from_constitution: constitution_hp_bonus_adnd(constitution),
        },
    }
}

/// AD&D 2e to-hit bonus from STR (vanilla `STRMOD.2DA`), plus the
/// 18/XX percentile bonus (`STRMODEX.2DA`) when STR is 18 and the
/// percentile byte is non-zero. Higher = better.
pub fn strength_to_hit_bonus_adnd(strength: u8, percentile: u8) -> i8 {
    // STRMODEX overrides the STRMOD STR=18 row whenever a percentile
    // is set — a STR 18/00 fighter (byte 100) hits at +3, not +1.
    if strength == 18 && percentile > 0 {
        return match percentile {
            1..=50 => 1,
            51..=75 => 2,
            76..=90 => 2,
            91..=99 => 2,
            100 => 3,
            _ => 0, // 101..=255 isn't a real percentile
        };
    }
    match strength {
        0 => 0,
        1 => -5,
        2 | 3 => -3,
        4 | 5 => -2,
        6 | 7 => -1,
        8..=16 => 0,
        17 | 18 => 1,
        19 | 20 => 3,
        21 | 22 => 4,
        23 => 5,
        24 => 6,
        25.. => 7,
    }
}

/// AD&D 2e AC bonus from DEX (vanilla `DEXMOD.2DA`). Negative =
/// better AC.
pub fn dexterity_ac_bonus_adnd(dexterity: u8) -> i8 {
    match dexterity {
        0..=3 => 4,
        4 => 3,
        5 => 2,
        6 => 1,
        7..=14 => 0,
        15 => -1,
        16 => -2,
        17 => -3,
        18..=u8::MAX => -4,
    }
}

/// AD&D 2e per-level HP bonus from CON (vanilla `HPCONBON.2DA`, the
/// non-warrior row). Positive = more HP per HD. Warriors get larger
/// bonuses via `HPWAR.2DA` (CON 17→+3, 18→+4, 19+→+5) — not surfaced
/// here yet because the keeper doesn't track class.
pub fn constitution_hp_bonus_adnd(constitution: u8) -> i8 {
    match constitution {
        0..=3 => -2,
        4 | 5 => -1,
        6 => -1,
        7..=14 => 0,
        15 => 1,
        16..=u8::MAX => 2,
    }
}

/// IWD2 (d20) ability modifier: `floor((score - 10) / 2)`. Negative
/// for scores below 10; positive for 12+.
pub fn d20_modifier(score: u8) -> i8 {
    ((score as i16) - 10).div_euclid(2) as i8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ad_and_d_engines_share_one_to_twenty_five_for_abilities() {
        for engine in [
            Engine::Bg,
            Engine::Bg2,
            Engine::Ee,
            Engine::Iwd,
            Engine::Pst,
        ] {
            let caps = caps_for(engine);
            assert_eq!(
                caps.ability_score,
                AbilityRange { min: 1, max: 25 },
                "{engine:?} uses AD&D 2e ability cap",
            );
            assert_eq!(caps.strength_percentile, AbilityRange { min: 0, max: 100 });
        }
    }

    #[test]
    fn iwd2_uses_d20_thirty_cap() {
        let caps = caps_for(Engine::Iwd2);
        assert_eq!(caps.ability_score, AbilityRange { min: 1, max: 30 });
    }

    #[test]
    fn hardcoded_gameplay_clamps() {
        let caps = caps_for(Engine::Bg);
        assert_eq!(caps.reputation, AbilityRange { min: 0, max: 20 });
        assert_eq!(caps.morale, AbilityRange { min: 0, max: 20 });
        assert_eq!(caps.morale_break, AbilityRange { min: 0, max: 20 });
        assert_eq!(caps.attacks_byte, AbilityRange { min: 0, max: 10 });
    }

    #[test]
    fn storage_max_caps_match_their_type() {
        let caps = caps_for(Engine::Bg);
        assert_eq!(caps.current_hit_points.max, u16::MAX);
        assert_eq!(caps.max_hit_points.max, u16::MAX);
        assert_eq!(caps.armor_class.min, i16::MIN);
        assert_eq!(caps.armor_class.max, i16::MAX);
        assert_eq!(caps.party_gold.max, u32::MAX);
        assert_eq!(caps.experience.max, u32::MAX);
        assert_eq!(caps.xp_for_kill.max, u32::MAX);
        assert_eq!(caps.morale_recovery.max, u16::MAX);
        assert_eq!(caps.thief_skill.max, u8::MAX);
        assert_eq!(caps.lore.max, u8::MAX);
        assert_eq!(caps.class_level.max, u8::MAX);
        assert_eq!(caps.thac0.min, i8::MIN);
        assert_eq!(caps.thac0.max, i8::MAX);
        assert_eq!(caps.fatigue.max, u8::MAX);
        assert_eq!(caps.intoxication.max, u8::MAX);
        assert_eq!(caps.luck.max, u8::MAX);
    }

    #[test]
    fn range_contains_and_clamp_behave_for_u8() {
        let r = AbilityRange { min: 1u8, max: 25 };
        assert!(!r.contains(0));
        assert!(r.contains(1));
        assert!(r.contains(25));
        assert!(!r.contains(26));
        assert_eq!(r.clamp(0), 1);
        assert_eq!(r.clamp(18), 18);
        assert_eq!(r.clamp(255), 25);
    }

    #[test]
    fn range_contains_and_clamp_behave_for_i16() {
        let r = AbilityRange {
            min: -10i16,
            max: 10,
        };
        assert!(!r.contains(-11));
        assert!(r.contains(-10));
        assert!(r.contains(10));
        assert!(!r.contains(11));
        assert_eq!(r.clamp(-100), -10);
        assert_eq!(r.clamp(0), 0);
        assert_eq!(r.clamp(100), 10);
    }

    #[test]
    fn range_contains_and_clamp_behave_for_u32() {
        let r = AbilityRange { min: 0u32, max: 20 };
        assert_eq!(r.clamp(0), 0);
        assert_eq!(r.clamp(50), 20);
    }

    #[test]
    fn strength_to_hit_strmod_known_values() {
        // Spot-check the published STRMOD.2DA rows.
        assert_eq!(strength_to_hit_bonus_adnd(1, 0), -5);
        assert_eq!(strength_to_hit_bonus_adnd(3, 0), -3);
        assert_eq!(strength_to_hit_bonus_adnd(10, 0), 0);
        assert_eq!(strength_to_hit_bonus_adnd(17, 0), 1);
        // STR 18 with no percentile uses the STRMOD row → +1.
        assert_eq!(strength_to_hit_bonus_adnd(18, 0), 1);
        assert_eq!(strength_to_hit_bonus_adnd(19, 0), 3);
        assert_eq!(strength_to_hit_bonus_adnd(25, 0), 7);
    }

    #[test]
    fn strength_to_hit_strmodex_overrides_at_18() {
        // 18/00 (byte 100) is the strongest percentile.
        assert_eq!(strength_to_hit_bonus_adnd(18, 100), 3);
        // 18/50 falls in the 01-50 bucket.
        assert_eq!(strength_to_hit_bonus_adnd(18, 50), 1);
        // 18/51 jumps to the next bucket.
        assert_eq!(strength_to_hit_bonus_adnd(18, 51), 2);
        assert_eq!(strength_to_hit_bonus_adnd(18, 99), 2);
        // Percentile is only meaningful when STR == 18 — STR 17 with
        // a junk percentile must still use STRMOD.
        assert_eq!(strength_to_hit_bonus_adnd(17, 100), 1);
    }

    #[test]
    fn dexterity_ac_known_values() {
        // DEXMOD.2DA spot checks. Negative = better AC in AD&D 2e.
        assert_eq!(dexterity_ac_bonus_adnd(3), 4);
        assert_eq!(dexterity_ac_bonus_adnd(10), 0);
        assert_eq!(dexterity_ac_bonus_adnd(15), -1);
        assert_eq!(dexterity_ac_bonus_adnd(18), -4);
        // High DEX caps at -4 in vanilla AD&D 2e.
        assert_eq!(dexterity_ac_bonus_adnd(25), -4);
    }

    #[test]
    fn constitution_hp_known_values() {
        // HPCONBON.2DA (non-warrior).
        assert_eq!(constitution_hp_bonus_adnd(3), -2);
        assert_eq!(constitution_hp_bonus_adnd(10), 0);
        assert_eq!(constitution_hp_bonus_adnd(15), 1);
        assert_eq!(constitution_hp_bonus_adnd(18), 2);
        assert_eq!(constitution_hp_bonus_adnd(25), 2);
    }

    #[test]
    fn d20_modifier_round_floors_negatives() {
        // d20 mod = floor((score - 10) / 2).
        assert_eq!(d20_modifier(10), 0);
        assert_eq!(d20_modifier(11), 0);
        assert_eq!(d20_modifier(12), 1);
        assert_eq!(d20_modifier(18), 4);
        assert_eq!(d20_modifier(30), 10);
        // Negative scores round toward -∞, not toward zero.
        assert_eq!(d20_modifier(9), -1);
        assert_eq!(d20_modifier(8), -1);
        assert_eq!(d20_modifier(7), -2);
        assert_eq!(d20_modifier(1), -5);
    }

    #[test]
    fn ability_bonuses_dispatch_per_engine() {
        let adnd = ability_bonuses(Engine::Bg2, 18, 100, 18, 16);
        assert_eq!(adnd.thac0_from_strength, 3); // 18/00 STRMODEX row
        assert_eq!(adnd.ac_from_dexterity, -4); // DEX 18 — AD&D sign
        assert_eq!(adnd.hp_per_level_from_constitution, 2);

        let d20 = ability_bonuses(Engine::Iwd2, 18, 0, 18, 16);
        assert_eq!(d20.thac0_from_strength, 4);
        assert_eq!(d20.ac_from_dexterity, 4); // d20 sign — high = good
        assert_eq!(d20.hp_per_level_from_constitution, 3);
    }
}
