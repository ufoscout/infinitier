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
    /// `HPCONBON.2DA` — per-level HP bonus by constitution
    /// (non-warrior table; warriors get larger bonuses via
    /// `HPWAR.2DA` but we don't surface class-dependent caps yet).
    /// Empty on IWD2.
    pub hpconbon_hp: BonusTable,
}

/// Cap ranges that share their value across every IE engine —
/// extracted as a const so `EngineCaps::new` only has to spell out
/// per-engine variants for the rows that actually differ.
const SHARED_RANGES: EngineCapsRanges = EngineCapsRanges {
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
        let (strmod_to_hit, strmodex_to_hit, dexmod_ac, hpconbon_hp) =
            if matches!(engine, Engine::Iwd2) {
                (
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
                    // `WARRIOR` / `OTHER` columns. The previous hardcoded
                    // ad&d implementation was not class-aware and capped at
                    // the non-warrior +2 bonus — `OTHER` preserves that
                    // behaviour. Original (pre-EE) releases use a single
                    // `HP_BONUS` column; a few mods rename it `HPCONBON`.
                    load_bonus_table(game_data, "hpconbon", &["HP_BONUS", "HPCONBON", "OTHER"])?,
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

    /// AD&D 2e per-level HP bonus from CON (`HPCONBON.2DA`).
    pub fn constitution_hp_bonus(&self, constitution: u8) -> i8 {
        self.hpconbon_hp.lookup(constitution)
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
        let r = AbilityRange { min: -10i16, max: 10 };
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
}
