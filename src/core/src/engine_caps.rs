//! Per-engine gameplay caps + 2DA-driven ability bonus tables.
//!
//! Two concerns live here:
//!
//! - **Cap ranges** ([`AbilityRange<T>`]): the `min..=max` an editable
//!   field may take. These are hardcoded because they're engine-binary
//!   constants, not 2DA-defined — ability scores (1..=25 / 1..=30),
//!   reputation (0..=20), morale (0..=20), the attacks-per-round byte
//!   (0..=10 documented). Storage caps (`u16::MAX` for HP, etc.) are
//!   the same shape.
//! - **Bonus lookup tables** ([`BonusTable`]): how a score maps to a
//!   to-hit / AC / HP modifier. These are loaded from the live game
//!   resources (`STRMOD.2DA`, `STRMODEX.2DA`, `DEXMOD.2DA`,
//!   `HPCONBON.2DA`) so a modded install gets the actual numbers it
//!   ships, not a copy of the vanilla distribution.
//!
//! The entry point is [`EngineCaps::new`] — it needs a [`GameData`]
//! so it can resolve and parse the 2DAs at construction time.

use std::io;

use infinitier_common::Engine;
use infinitier_two_da_resource::TwoDA;

use crate::game::GameData;
use crate::imported_resource::ImportedResource;
use crate::resource::ResourceType;

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

// ── Bonus lookup table (2DA-driven) ─────────────────────────────────

/// A sparse score → bonus lookup compiled from a single column of a
/// game-data 2DA file. The row label is parsed as the input score
/// (e.g. STR `1..=25` for `STRMOD.2DA`, percentile `1..=100` for
/// `STRMODEX.2DA`), and the column value is parsed as the bonus.
///
/// `lookup(score)` returns the value of the smallest row label that
/// is `>= score` — handling both dense tables (STRMOD has every row)
/// and sparse threshold tables (STRMODEX only has 1, 50, 75, 90,
/// 99, 100). Scores above the largest row clamp to that row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BonusTable {
    /// `(row_label, bonus)` sorted ascending by row label.
    entries: Vec<(u8, i8)>,
}

impl BonusTable {
    /// Parse `column` from `two_da` into a `BonusTable`. Rows whose
    /// label or value won't parse are skipped. Returns `None` if
    /// `column` doesn't appear in the 2DA's headers.
    pub fn from_two_da(two_da: &TwoDA, column: &str) -> Option<Self> {
        let col_idx = two_da
            .headers
            .iter()
            .position(|h| h.eq_ignore_ascii_case(column))?;
        let mut entries: Vec<(u8, i8)> = two_da
            .rows
            .iter()
            .filter_map(|(key, row)| {
                let k = key.trim().parse::<u8>().ok()?;
                let raw = row.get(col_idx)?;
                let v = raw.trim().parse::<i8>().ok()?;
                Some((k, v))
            })
            .collect();
        entries.sort_by_key(|(k, _)| *k);
        Some(Self { entries })
    }

    /// Try every `column` candidate in turn — useful when engines
    /// rename the column for the same datum (e.g. `AC_ADJ` vs `ACMOD`).
    pub fn from_two_da_any(two_da: &TwoDA, columns: &[&str]) -> Option<Self> {
        columns
            .iter()
            .find_map(|c| Self::from_two_da(two_da, c))
            .filter(|t| !t.is_empty())
    }

    /// Returns the bonus for `score`: the value of the smallest row
    /// label `>= score`. Scores above the maximum row use that
    /// maximum row's value; an empty table returns 0.
    pub fn lookup(&self, score: u8) -> i8 {
        for (key, val) in &self.entries {
            if score <= *key {
                return *val;
            }
        }
        self.entries.last().map(|(_, v)| *v).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Per-engine caps + bonuses ───────────────────────────────────────

/// Bundle of per-field caps + per-engine bonus tables. Built once at
/// keeper startup via [`EngineCaps::new`] and consulted thereafter
/// when the abilities tab needs to clamp an edit or display a live
/// bonus indicator.
#[derive(Debug, Clone)]
pub struct EngineCaps {
    /// The engine this `EngineCaps` was built for. Drives the d20
    /// vs AD&D dispatch in [`Self::ability_bonuses`].
    pub engine: Engine,

    // ── Ability scores ──────────────────────────────────────────
    /// STR / DEX / CON / INT / WIS / CHA. AD&D engines clamp at 25,
    /// IWD2 (d20) at 30.
    pub ability_score: AbilityRange<u8>,
    /// AD&D extraordinary-strength percentile (0..=100). Carried for
    /// every engine; the IWD2 CRE header doesn't store this byte so
    /// the caller never reads it on V2.2.
    pub strength_percentile: AbilityRange<u8>,

    // ── Combat & status ─────────────────────────────────────────
    pub current_hit_points: AbilityRange<u16>,
    pub max_hit_points: AbilityRange<u16>,
    pub armor_class: AbilityRange<i16>,
    pub thac0: AbilityRange<i8>,
    pub attacks_byte: AbilityRange<u8>,
    pub reputation: AbilityRange<u32>,
    pub party_gold: AbilityRange<u32>,
    pub fatigue: AbilityRange<u8>,
    pub intoxication: AbilityRange<u8>,
    pub luck: AbilityRange<u8>,

    // ── Experience & levels ─────────────────────────────────────
    pub experience: AbilityRange<u32>,
    pub xp_for_kill: AbilityRange<u32>,
    pub class_level: AbilityRange<u8>,

    // ── Morale ──────────────────────────────────────────────────
    pub morale: AbilityRange<u8>,
    pub morale_break: AbilityRange<u8>,
    pub morale_recovery: AbilityRange<u16>,

    // ── Thief skills ────────────────────────────────────────────
    pub thief_skill: AbilityRange<u8>,
    pub lore: AbilityRange<u8>,

    // ── AD&D bonus tables (loaded from 2DAs) ────────────────────
    /// `STRMOD.2DA` — to-hit bonus by strength score (rows 1..=25).
    /// Empty on IWD2 (d20).
    pub strmod_to_hit: BonusTable,
    /// `STRMODEX.2DA` — to-hit bonus by 18/XX percentile. Sparse —
    /// rows at the threshold percentiles (1, 50, 75, 90, 99, 100).
    /// Empty on IWD2 (d20).
    pub strmodex_to_hit: BonusTable,
    /// `DEXMOD.2DA` (`AC_ADJ` column) — AC bonus by dexterity.
    /// Negative = better AC in AD&D. Empty on IWD2.
    pub dexmod_ac: BonusTable,
    /// `HPCONBON.2DA` `OTHER` column — per-Hit-Die HP bonus by
    /// constitution for non-warriors (max +2 in AD&D). Empty on IWD2.
    pub hpconbon_hp: BonusTable,
    /// `HPCONBON.2DA` `WARRIOR` column — per-Hit-Die HP bonus by
    /// constitution for warriors (Fighter / Paladin / Ranger /
    /// Barbarian), which scales past +2 (CON 18 → +4, 22 → +6, …).
    /// Empty on IWD2. Falls back to the non-warrior column on
    /// pre-EE single-column `HPCONBON.2DA` files.
    pub hpconbon_hp_warrior: BonusTable,
    /// `LOREBON.2DA` (`VALUE` column) — signed Lore bonus by INT or
    /// WIS score (e.g. WIS 6 → −20, INT 23 → +30). Applied once for
    /// INT and once for WIS. Empty on IWD2 (Lore is a d20 skill there).
    pub lorebon: BonusTable,
}

/// Cap ranges that share their value across every IE engine —
/// extracted as a const so `EngineCaps::new` only has to spell out
/// per-engine variants for the rows that actually differ.
const SHARED_RANGES: EngineCapsRanges = EngineCapsRanges {
    ability_score: AbilityRange { min: 1, max: 25 },
    strength_percentile: AbilityRange { min: 0, max: 100 },
    current_hit_points: AbilityRange {
        min: u16::MIN,
        max: u16::MAX,
    },
    max_hit_points: AbilityRange {
        min: u16::MIN,
        max: u16::MAX,
    },
    armor_class: AbilityRange {
        min: i16::MIN,
        max: i16::MAX,
    },
    thac0: AbilityRange {
        min: i8::MIN,
        max: i8::MAX,
    },
    attacks_byte: AbilityRange { min: 0, max: 10 },
    reputation: AbilityRange { min: 0, max: 20 },
    party_gold: AbilityRange {
        min: u32::MIN,
        max: u32::MAX,
    },
    fatigue: AbilityRange {
        min: u8::MIN,
        max: u8::MAX,
    },
    intoxication: AbilityRange {
        min: u8::MIN,
        max: u8::MAX,
    },
    luck: AbilityRange {
        min: u8::MIN,
        max: u8::MAX,
    },
    experience: AbilityRange {
        min: u32::MIN,
        max: u32::MAX,
    },
    xp_for_kill: AbilityRange {
        min: u32::MIN,
        max: u32::MAX,
    },
    class_level: AbilityRange {
        min: u8::MIN,
        max: u8::MAX,
    },
    morale: AbilityRange { min: 0, max: 20 },
    morale_break: AbilityRange { min: 0, max: 20 },
    morale_recovery: AbilityRange {
        min: u16::MIN,
        max: u16::MAX,
    },
    thief_skill: AbilityRange {
        min: u8::MIN,
        max: u8::MAX,
    },
    lore: AbilityRange {
        min: u8::MIN,
        max: u8::MAX,
    },
};

/// Stand-alone copy of the cap-range fields of [`EngineCaps`]. Used
/// only by the `SHARED_RANGES` const + the cap-only constructor so
/// `EngineCaps::new` doesn't have to repeat the field list.
#[derive(Debug, Clone, Copy)]
struct EngineCapsRanges {
    ability_score: AbilityRange<u8>,
    strength_percentile: AbilityRange<u8>,
    current_hit_points: AbilityRange<u16>,
    max_hit_points: AbilityRange<u16>,
    armor_class: AbilityRange<i16>,
    thac0: AbilityRange<i8>,
    attacks_byte: AbilityRange<u8>,
    reputation: AbilityRange<u32>,
    party_gold: AbilityRange<u32>,
    fatigue: AbilityRange<u8>,
    intoxication: AbilityRange<u8>,
    luck: AbilityRange<u8>,
    experience: AbilityRange<u32>,
    xp_for_kill: AbilityRange<u32>,
    class_level: AbilityRange<u8>,
    morale: AbilityRange<u8>,
    morale_break: AbilityRange<u8>,
    morale_recovery: AbilityRange<u16>,
    thief_skill: AbilityRange<u8>,
    lore: AbilityRange<u8>,
}

impl EngineCaps {
    /// Build the caps + bonus tables for `game_data`'s engine.
    ///
    /// AD&D engines load the four standard 2DAs
    /// (`STRMOD` / `STRMODEX` / `DEXMOD` / `HPCONBON`); a missing or
    /// malformed table is fatal for those engines. IWD2 skips the
    /// loads entirely — its d20 modifier is a pure `(score - 10) / 2`
    /// formula that doesn't go through a table.
    pub fn new(game_data: &GameData) -> io::Result<Self> {
        let engine = game_data.game().engine();
        let ranges = match engine {
            Engine::Iwd2 => EngineCapsRanges {
                ability_score: AbilityRange { min: 1, max: 30 },
                ..SHARED_RANGES
            },
            _ => SHARED_RANGES,
        };
        let (strmod_to_hit, strmodex_to_hit, dexmod_ac, hpconbon_hp, hpconbon_hp_warrior, lorebon) =
            if matches!(engine, Engine::Iwd2) {
                (
                    BonusTable::default(),
                    BonusTable::default(),
                    BonusTable::default(),
                    BonusTable::default(),
                    BonusTable::default(),
                    BonusTable::default(),
                )
            } else {
                (
                    load_bonus_table(game_data, "strmod", &["STR_BONUS_TO_HIT", "TO_HIT"])?,
                    load_bonus_table(game_data, "strmodex", &["STR_BONUS_TO_HIT", "TO_HIT"])?,
                    // BG:EE / BG2:EE / IWDEE / PSTEE store the AC adjustment
                    // under the bare `"AC"` column; older releases (BG, BG2,
                    // IWD, Tutu, PST) use the `*_ADJ` suffix variant `"AC_ADJ"`;
                    // a handful of mods rename it to `"ACMOD"`.
                    load_bonus_table(game_data, "dexmod", &["AC_ADJ", "ACMOD", "AC"])?,
                    // BG:EE / BG2:EE / IWDEE / PSTEE split HPCONBON into
                    // `WARRIOR` / `OTHER` columns. `OTHER` is the
                    // non-warrior (max +2) bonus. Original (pre-EE)
                    // releases use a single `HP_BONUS` column; a few mods
                    // rename it `HPCONBON`.
                    load_bonus_table(game_data, "hpconbon", &["OTHER", "HP_BONUS", "HPCONBON"])?,
                    // Warrior column; on a single-column pre-EE file this
                    // falls back to the same bonus as non-warriors.
                    load_bonus_table(game_data, "hpconbon", &["WARRIOR", "HP_BONUS", "HPCONBON"])?,
                    load_bonus_table(game_data, "lorebon", &["VALUE", "LORE_BONUS"])?,
                )
            };
        Ok(Self {
            engine,
            ability_score: ranges.ability_score,
            strength_percentile: ranges.strength_percentile,
            current_hit_points: ranges.current_hit_points,
            max_hit_points: ranges.max_hit_points,
            armor_class: ranges.armor_class,
            thac0: ranges.thac0,
            attacks_byte: ranges.attacks_byte,
            reputation: ranges.reputation,
            party_gold: ranges.party_gold,
            fatigue: ranges.fatigue,
            intoxication: ranges.intoxication,
            luck: ranges.luck,
            experience: ranges.experience,
            xp_for_kill: ranges.xp_for_kill,
            class_level: ranges.class_level,
            morale: ranges.morale,
            morale_break: ranges.morale_break,
            morale_recovery: ranges.morale_recovery,
            thief_skill: ranges.thief_skill,
            lore: ranges.lore,
            strmod_to_hit,
            strmodex_to_hit,
            dexmod_ac,
            hpconbon_hp,
            hpconbon_hp_warrior,
            lorebon,
        })
    }

    /// AD&D 2e to-hit bonus from STR + the 18/XX percentile when
    /// applicable. Reads from `STRMOD.2DA` (and `STRMODEX.2DA` when
    /// STR == 18 and the percentile byte is non-zero).
    pub fn strength_to_hit_bonus(&self, strength: u8, percentile: u8) -> i8 {
        if strength == 18 && percentile > 0 {
            self.strmodex_to_hit.lookup(percentile)
        } else {
            self.strmod_to_hit.lookup(strength)
        }
    }

    /// AD&D 2e AC bonus from DEX (`DEXMOD.2DA` `AC_ADJ` column).
    /// Negative = better AC.
    pub fn dexterity_ac_bonus(&self, dexterity: u8) -> i8 {
        self.dexmod_ac.lookup(dexterity)
    }

    /// AD&D 2e per-Hit-Die HP bonus from CON (`HPCONBON.2DA`).
    /// `warrior` selects the larger `WARRIOR` column for Fighter /
    /// Paladin / Ranger / Barbarian (and combos that include one).
    pub fn constitution_hp_bonus(&self, constitution: u8) -> i8 {
        self.hpconbon_hp.lookup(constitution)
    }

    /// Warrior variant of [`Self::constitution_hp_bonus`].
    pub fn constitution_hp_bonus_warrior(&self, constitution: u8) -> i8 {
        self.hpconbon_hp_warrior.lookup(constitution)
    }

    /// Per-Hit-Die (per-level) CON contribution to HP, engine-aware so
    /// the value shown next to Constitution always matches the effective
    /// Max HP. AD&D reads the `HPCONBON.2DA` `WARRIOR` / `OTHER` column
    /// (`is_warrior`); IWD2 (d20) uses the ability modifier and ignores
    /// `is_warrior` (3e has no warrior CON-HP distinction).
    pub fn constitution_hp_per_level(&self, constitution: u8, is_warrior: bool) -> i32 {
        match self.engine {
            Engine::Iwd2 => i32::from(d20_modifier(constitution)),
            _ => i32::from(if is_warrior {
                self.constitution_hp_bonus_warrior(constitution)
            } else {
                self.constitution_hp_bonus(constitution)
            }),
        }
    }

    /// Total CON contribution to a creature's maximum HP, matching the
    /// engine's runtime adjustment: the per-level bonus times the number
    /// of HP-rolling levels, since the stored `maximum_hit_points`
    /// excludes the CON bonus. Mirrors GemRB `Actor::GetHpAdjustment`
    /// (AD&D: capped at the class HP-roll level; IWD2: plain
    /// `level × con-modifier`).
    pub fn max_hp_constitution_bonus(
        &self,
        constitution: u8,
        is_warrior: bool,
        levels_with_hp_roll: u32,
    ) -> i32 {
        self.constitution_hp_per_level(constitution, is_warrior) * levels_with_hp_roll as i32
    }

    /// AD&D Lore bonus from INT and WIS (`LOREBON.2DA`, applied once
    /// each, summed and signed). The engine adds this to the stored
    /// base Lore byte to get the displayed value. Mirrors GemRB
    /// `Modified[IE_LORE] += GetLoreBonus(INT) + GetLoreBonus(WIS)`.
    pub fn lore_bonus(&self, intelligence: u8, wisdom: u8) -> i32 {
        i32::from(self.lorebon.lookup(intelligence)) + i32::from(self.lorebon.lookup(wisdom))
    }

    /// Combined bonuses for one set of ability scores under the
    /// engine's rules. The strength percentile only matters when
    /// STR is exactly 18 and the percentile byte is non-zero.
    pub fn ability_bonuses(
        &self,
        strength: u8,
        strength_percentile: u8,
        dexterity: u8,
        constitution: u8,
    ) -> AbilityBonuses {
        match self.engine {
            Engine::Iwd2 => AbilityBonuses {
                thac0_from_strength: d20_modifier(strength),
                ac_from_dexterity: d20_modifier(dexterity),
                hp_per_level_from_constitution: d20_modifier(constitution),
            },
            Engine::Bg | Engine::Bg2 | Engine::Ee | Engine::Iwd | Engine::Pst => AbilityBonuses {
                thac0_from_strength: self.strength_to_hit_bonus(strength, strength_percentile),
                ac_from_dexterity: self.dexterity_ac_bonus(dexterity),
                hp_per_level_from_constitution: self.constitution_hp_bonus(constitution),
            },
        }
    }
}

/// Bundle of bonuses derived from a creature's ability scores. Sign
/// conventions match the engine the score belongs to:
///
/// - `thac0_from_strength` — positive = better to-hit (THAC0 goes
///   down). For IWD2 it's the d20 attack-roll modifier.
/// - `ac_from_dexterity` — for AD&D 2e, *negative* = better AC
///   (DEX 18 → -4). For IWD2 d20, *positive* = better AC.
/// - `hp_per_level_from_constitution` — positive = more HP per HD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AbilityBonuses {
    pub thac0_from_strength: i8,
    pub ac_from_dexterity: i8,
    pub hp_per_level_from_constitution: i8,
}

/// IWD2 (d20) ability modifier: `floor((score - 10) / 2)`. Pure
/// formula, no 2DA involved. Negative for scores below 10; positive
/// for 12+.
pub fn d20_modifier(score: u8) -> i8 {
    ((score as i16) - 10).div_euclid(2) as i8
}

/// Resolve `<name>.2DA` from `game_data` (override → BIFs), import
/// it, and project the first matching column into a [`BonusTable`].
fn load_bonus_table(
    game_data: &GameData,
    name: &str,
    column_candidates: &[&str],
) -> io::Result<BonusTable> {
    let resource = game_data
        .get_by_name_and_type(name, ResourceType::TwoDA)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}.2DA missing from game data", name.to_ascii_uppercase()),
            )
        })?;
    let imported = resource.import(game_data)?;
    let ImportedResource::TwoDA(two_da) = imported else {
        return Err(io::Error::other(format!(
            "{}.2DA did not import as a 2DA resource",
            name.to_ascii_uppercase()
        )));
    };
    BonusTable::from_two_da_any(&two_da, column_candidates).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}.2DA has none of the expected columns {:?}; available columns: {:?}",
                name.to_ascii_uppercase(),
                column_candidates,
                two_da.headers,
            ),
        )
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn make_two_da(headers: &[&str], rows: &[(&str, &[&str])]) -> TwoDA {
        TwoDA {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            default: "0".to_string(),
            rows: rows
                .iter()
                .map(|(k, vs)| (k.to_string(), vs.iter().map(|s| s.to_string()).collect()))
                .collect(),
        }
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
    fn d20_modifier_round_floors_negatives() {
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

    /// Build an `EngineCaps` carrying only the CON/Lore tables the
    /// Lore + Max-HP tests exercise; everything else gets the shared
    /// ranges / empty tables.
    fn caps_for_lore_hp(
        hpconbon_hp: BonusTable,
        hpconbon_hp_warrior: BonusTable,
        lorebon: BonusTable,
    ) -> EngineCaps {
        let r = SHARED_RANGES;
        EngineCaps {
            engine: Engine::Ee,
            ability_score: r.ability_score,
            strength_percentile: r.strength_percentile,
            current_hit_points: r.current_hit_points,
            max_hit_points: r.max_hit_points,
            armor_class: r.armor_class,
            thac0: r.thac0,
            attacks_byte: r.attacks_byte,
            reputation: r.reputation,
            party_gold: r.party_gold,
            fatigue: r.fatigue,
            intoxication: r.intoxication,
            luck: r.luck,
            experience: r.experience,
            xp_for_kill: r.xp_for_kill,
            class_level: r.class_level,
            morale: r.morale,
            morale_break: r.morale_break,
            morale_recovery: r.morale_recovery,
            thief_skill: r.thief_skill,
            lore: r.lore,
            strmod_to_hit: BonusTable::default(),
            strmodex_to_hit: BonusTable::default(),
            dexmod_ac: BonusTable::default(),
            hpconbon_hp,
            hpconbon_hp_warrior,
            lorebon,
        }
    }

    /// Tables shaped like the BG2:EE `HPCONBON.2DA` / `LOREBON.2DA`
    /// rows the reference party actually hits.
    fn ee_caps() -> EngineCaps {
        let hpconbon = make_two_da(
            &["OTHER", "WARRIOR"],
            &[
                ("9", &["0", "0"]),
                ("16", &["2", "2"]),
                ("17", &["2", "3"]),
                ("18", &["2", "4"]),
                ("22", &["2", "6"]),
            ],
        );
        let lorebon = make_two_da(
            &["VALUE"],
            &[
                ("6", &["-20"]),
                ("8", &["-10"]),
                ("9", &["-10"]),
                ("11", &["0"]),
                ("12", &["0"]),
                ("16", &["5"]),
                ("17", &["7"]),
                ("19", &["12"]),
                ("22", &["25"]),
                ("23", &["30"]),
            ],
        );
        caps_for_lore_hp(
            BonusTable::from_two_da(&hpconbon, "OTHER").unwrap(),
            BonusTable::from_two_da(&hpconbon, "WARRIOR").unwrap(),
            BonusTable::from_two_da(&lorebon, "VALUE").unwrap(),
        )
    }

    #[test]
    fn constitution_hp_bonus_picks_warrior_column() {
        let caps = ee_caps();
        // Non-warrior caps at +2; warriors scale past it.
        assert_eq!(caps.constitution_hp_bonus(22), 2);
        assert_eq!(caps.constitution_hp_bonus_warrior(22), 6);
        assert_eq!(caps.constitution_hp_bonus_warrior(18), 4);
        assert_eq!(caps.constitution_hp_bonus_warrior(17), 3);
        assert_eq!(caps.constitution_hp_bonus_warrior(9), 0);
    }

    #[test]
    fn max_hp_con_bonus_matches_reference_party() {
        let caps = ee_caps();
        // Xor: warrior, CON 22, 9 rolling levels → +6 × 9 = 54.
        assert_eq!(caps.max_hp_constitution_bonus(22, true, 9), 54);
        // Minsc: warrior, CON 18 → +4 × 9 = 36.
        assert_eq!(caps.max_hp_constitution_bonus(18, true, 9), 36);
        // Keldorn: warrior, CON 17 → +3 × 9 = 27.
        assert_eq!(caps.max_hp_constitution_bonus(17, true, 9), 27);
        // Nalia / Imoen: non-warrior, CON 16, 10 rolling levels → +2 × 10 = 20.
        assert_eq!(caps.max_hp_constitution_bonus(16, false, 10), 20);
        // Aerie: CON 9 → no bonus.
        assert_eq!(caps.max_hp_constitution_bonus(9, false, 9), 0);
    }

    #[test]
    fn iwd2_constitution_hp_uses_d20_modifier_not_tables() {
        // IWD2 ignores the (AD&D) HPCONBON tables and uses the d20
        // ability modifier on every level — no warrior distinction, no
        // HP-roll cap. This keeps the "+N HP/lvl" shown next to CON
        // consistent with the effective Max HP.
        let mut caps = ee_caps();
        caps.engine = Engine::Iwd2;
        assert_eq!(caps.constitution_hp_per_level(18, false), 4);
        assert_eq!(caps.constitution_hp_per_level(18, true), 4); // is_warrior ignored
        assert_eq!(caps.constitution_hp_per_level(9, false), -1);
        // CON 18 across 10 levels → +40 (level × modifier, uncapped).
        assert_eq!(caps.max_hp_constitution_bonus(18, false, 10), 40);
    }

    #[test]
    fn lore_bonus_is_signed_sum_of_int_and_wis() {
        let caps = ee_caps();
        // Xor: INT 22 (+25) + WIS 23 (+30) = +55 → 32 base → 87.
        assert_eq!(caps.lore_bonus(22, 23), 55);
        // Minsc: INT 8 (−10) + WIS 6 (−20) = −30 → 36 base → 6.
        assert_eq!(caps.lore_bonus(8, 6), -30);
        // Keldorn: INT 12 (0) + WIS 16 (+5) = +5.
        assert_eq!(caps.lore_bonus(12, 16), 5);
        // Nalia: INT 19 (+12) + WIS 9 (−10) = +2.
        assert_eq!(caps.lore_bonus(19, 9), 2);
    }

    #[test]
    fn bonus_table_dense_lookup_is_exact() {
        // STRMOD-shape table: dense rows 1..=4 carrying the bonus
        // directly. lookup returns the row's value at every step.
        let two_da = make_two_da(
            &["STR_BONUS_TO_HIT"],
            &[
                ("1", &["-5"]),
                ("2", &["-3"]),
                ("3", &["-3"]),
                ("4", &["-2"]),
            ],
        );
        let t = BonusTable::from_two_da(&two_da, "STR_BONUS_TO_HIT").unwrap();
        assert_eq!(t.lookup(1), -5);
        assert_eq!(t.lookup(2), -3);
        assert_eq!(t.lookup(3), -3);
        assert_eq!(t.lookup(4), -2);
        // Beyond the max row → max row's value.
        assert_eq!(t.lookup(50), -2);
    }

    #[test]
    fn bonus_table_sparse_lookup_picks_smallest_upper_bound() {
        // STRMODEX-shape table: sparse threshold rows. A query
        // between two rows resolves to the smaller of the two row
        // labels that is `>=` the query.
        let two_da = make_two_da(
            &["STR_BONUS_TO_HIT"],
            &[
                ("1", &["1"]),
                ("50", &["1"]),
                ("75", &["2"]),
                ("90", &["2"]),
                ("99", &["2"]),
                ("100", &["3"]),
            ],
        );
        let t = BonusTable::from_two_da(&two_da, "STR_BONUS_TO_HIT").unwrap();
        // Exact-row matches.
        assert_eq!(t.lookup(1), 1);
        assert_eq!(t.lookup(50), 1);
        assert_eq!(t.lookup(100), 3);
        // Between thresholds.
        assert_eq!(t.lookup(2), 1); // 2 → row 50
        assert_eq!(t.lookup(51), 2); // 51 → row 75
        assert_eq!(t.lookup(76), 2); // 76 → row 90
        assert_eq!(t.lookup(99), 2);
        // Above the max row clamps to it.
        assert_eq!(t.lookup(200), 3);
    }

    #[test]
    fn bonus_table_missing_column_returns_none() {
        let two_da = make_two_da(&["SOMETHING_ELSE"], &[("1", &["0"])]);
        assert!(BonusTable::from_two_da(&two_da, "STR_BONUS_TO_HIT").is_none());
    }

    #[test]
    fn bonus_table_skips_unparseable_rows() {
        // Garbage row labels or values are silently dropped — the
        // parseable rows still drive the lookup.
        let two_da = make_two_da(
            &["BONUS"],
            &[("****", &["7"]), ("ohno", &["3"]), ("5", &["2"])],
        );
        let t = BonusTable::from_two_da(&two_da, "BONUS").unwrap();
        assert_eq!(t.lookup(5), 2);
        // Only the row "5" survived; queries above use it.
        assert_eq!(t.lookup(100), 2);
    }

    #[test]
    fn bonus_table_from_two_da_any_picks_first_match() {
        let two_da = make_two_da(&["AC_ADJ"], &[("18", &["-4"])]);
        let t = BonusTable::from_two_da_any(&two_da, &["ACMOD", "AC_ADJ"]).unwrap();
        assert_eq!(t.lookup(18), -4);
    }

    #[test]
    fn bonus_table_default_lookup_yields_zero() {
        let t = BonusTable::default();
        assert_eq!(t.lookup(18), 0);
        assert!(t.is_empty());
    }

    #[test]
    fn empty_two_da_row_set_yields_empty_table() {
        // Avoid the "unused variant" warning from HashMap import
        // chain by hand-constructing an empty rows map.
        let two_da = TwoDA {
            headers: vec!["BONUS".to_string()],
            default: "0".to_string(),
            rows: HashMap::new(),
        };
        let t = BonusTable::from_two_da(&two_da, "BONUS").unwrap();
        assert!(t.is_empty());
    }

    // ── Building EngineCaps from real extracted 2DAs ─────────────────
    //
    // `assets/engine_caps/<key>/` holds STRMOD/STRMODEX/DEXMOD/HPCONBON/
    // LOREBON extracted from each install. `EngineCaps::new` must resolve
    // and parse the AD&D bonus tables (LOREBON included) into non-empty
    // tables — IWD2 (d20) skips them. Classic `bg` ships its 2DAs
    // XOR-encrypted on disk; the 2DA importer decrypts them transparently,
    // so every fixture builds.

    /// Build a [`GameData`] from the extracted fixtures in
    /// `assets/engine_caps/<game_key>/`, tagged with `game`.
    fn fixture_game_data(game_key: &str, game: infinitier_common::Game) -> GameData {
        use crate::game::{DataOrigin, GameResource};
        use infinitier_datasource::DataSource;
        use infinitier_test_utils::get_assets_path;

        let dir = get_assets_path().join("engine_caps").join(game_key);
        let mut resources = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}")) {
            let path = entry.unwrap().path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            let Some(rtype) = ResourceType::from_extension(&ext.to_ascii_lowercase()) else {
                continue;
            };
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap()
                .to_ascii_lowercase();
            resources.push(GameResource {
                game_type: game,
                name,
                r#type: rtype,
                file_size: path.metadata().ok().map(|m| m.len()),
                datasource: Some(DataSource::new(path.as_path())),
                data_origin: DataOrigin::Missing,
            });
        }
        GameData::new(resources, game, infinitier_fs::CaseInsensitiveFS::empty())
    }

    #[test]
    fn engine_caps_builds_from_bg() {
        // Classic BG's 2DAs are XOR-encrypted on disk; the 2DA importer
        // now decrypts them, so the tables parse and EngineCaps builds.
        let result = EngineCaps::new(&fixture_game_data("bg", infinitier_common::Game::Bg));
        assert!(
            result.is_ok(),
            "EngineCaps should build from bg fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_bg_ee() {
        let result = EngineCaps::new(&fixture_game_data("bg_ee", infinitier_common::Game::Bgee));
        assert!(
            result.is_ok(),
            "EngineCaps should build from bg_ee fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_bg2() {
        let result = EngineCaps::new(&fixture_game_data("bg2", infinitier_common::Game::Bg2));
        assert!(
            result.is_ok(),
            "EngineCaps should build from bg2 fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_bg2_ee() {
        let result = EngineCaps::new(&fixture_game_data("bg2_ee", infinitier_common::Game::Bg2ee));
        assert!(
            result.is_ok(),
            "EngineCaps should build from bg2_ee fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_iwd() {
        let result = EngineCaps::new(&fixture_game_data("iwd", infinitier_common::Game::Iwd));
        assert!(
            result.is_ok(),
            "EngineCaps should build from iwd fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_iwd_ee() {
        let result = EngineCaps::new(&fixture_game_data("iwd_ee", infinitier_common::Game::Iwdee));
        assert!(
            result.is_ok(),
            "EngineCaps should build from iwd_ee fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_iwd2() {
        // IWD2 is d20: EngineCaps::new skips the 2DA loads entirely, so it
        // builds even though IWD2 ships no DEXMOD.2DA.
        let result = EngineCaps::new(&fixture_game_data("iwd2", infinitier_common::Game::Iwd2));
        assert!(
            result.is_ok(),
            "EngineCaps should build from iwd2 fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_pst() {
        let result = EngineCaps::new(&fixture_game_data("pst", infinitier_common::Game::Pst));
        assert!(
            result.is_ok(),
            "EngineCaps should build from pst fixtures: {:?}",
            result.err()
        );
    }

    #[test]
    fn engine_caps_builds_from_pst_ee() {
        let result = EngineCaps::new(&fixture_game_data("pst_ee", infinitier_common::Game::Pstee));
        assert!(
            result.is_ok(),
            "EngineCaps should build from pst_ee fixtures: {:?}",
            result.err()
        );
    }
}
