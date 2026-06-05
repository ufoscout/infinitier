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

use std::collections::HashMap;
use std::io;

use infinitier_common::{Engine, Game};
use infinitier_two_da_resource::TwoDA;

use crate::game::GameData;

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
    /// `HPCLASS.2DA`: class / kit / multiclass-component symbol →
    /// its per-level HP table name (`hpwar`, `hpwiz`, …), lowercased.
    /// Empty on IWD2 or a stripped install (callers then fall back).
    class_hp_tables: HashMap<String, String>,
    /// Per HP table (`hpwar`, …): its Hit-Die size and the highest
    /// level that still rolls a die. Derived from the `HP*.2DA` tables.
    hp_table_profiles: HashMap<String, HpTableProfile>,
    /// `CLASS.IDS`: class value → symbol (e.g. `3` → `PALADIN`,
    /// `21` → `MAGE_THIEF`). Loaded once so [`Self::class_hp_profile`]
    /// can resolve a raw CRE class byte without re-importing the IDS.
    class_symbols: HashMap<i32, String>,
    /// Torment (PST / PSTEE) credits the Constitution HP bonus on
    /// *every* level — there's no Hit-Die cap, unlike the other AD&D
    /// games. Captured from the [`Game`] at construction because PSTEE
    /// shares [`Engine::Ee`] with BG:EE, so the engine alone can't tell
    /// them apart.
    con_hp_uncapped: bool,
}

/// Hit-Die shape of a class's `HP*.2DA` table.
#[derive(Debug, Clone, Copy)]
struct HpTableProfile {
    /// Largest `SIDES` (HD size) in the table. `>= 10` (d10 / d12)
    /// marks the warrior group (Fighter / Paladin / Ranger /
    /// Barbarian), who draw the larger CON→HP bonus.
    sides: u8,
    /// Highest level whose `ROLLS` column is `>= 1` — the last level
    /// that earns a Hit Die (and the CON HP bonus).
    roll_cap: u8,
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
        // Class → HP-table mapping + each table's HD shape, used to
        // classify warriors and the CON HP-roll cap from stock data
        // (no hardcoded class lists). IWD2 (d20) doesn't use it.
        let (class_hp_tables, hp_table_profiles) = if matches!(engine, Engine::Iwd2) {
            (HashMap::new(), HashMap::new())
        } else {
            load_class_hp_data(game_data)
        };
        let class_symbols = load_class_symbols(game_data);
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
            class_hp_tables,
            hp_table_profiles,
            class_symbols,
            con_hp_uncapped: matches!(game_data.game(), Game::Pst | Game::Pstee),
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

    /// CON contribution to a **multi-class** creature's maximum HP, per
    /// the BG / Torment "Hit Dice" rules: the per-level CON bonus (the
    /// *warrior* column when any component is a warrior) is applied to
    /// the **sum** of the class levels — each capped at the governing
    /// Hit-Die `cap` — then divided by the number of classes and
    /// rounded down:
    ///
    /// `floor( con_per_level × Σ min(level_c, cap) / num_classes )`
    ///
    /// `class_levels` must hold exactly the real classes (length =
    /// number of classes); the divisor is `class_levels.len()`. Pass
    /// `cap = u8::MAX` to disable the Hit-Die cap (Torment).
    ///
    /// Examples: a Fighter 9 / Thief 11 with CON 16, cap 9 →
    /// `floor(2 × (9 + 9) / 2) = 18` (BG); the same uncapped (cap =
    /// `u8::MAX`) → `floor(2 × (9 + 11) / 2) = 20` (Torment).
    ///
    /// Only meaningful for true multi-class creatures — *dual*-class
    /// follows different rules and must not be routed here.
    pub fn max_hp_constitution_bonus_multiclass(
        &self,
        constitution: u8,
        is_warrior: bool,
        cap: u8,
        class_levels: &[u8],
    ) -> i32 {
        if class_levels.is_empty() {
            return 0;
        }
        let per_level = self.constitution_hp_per_level(constitution, is_warrior);
        let summed_levels: i32 = class_levels.iter().map(|&l| i32::from(cap.min(l))).sum();
        // Round toward negative infinity so a CON *penalty* rounds the
        // same way the engine floors the bonus.
        (per_level * summed_levels).div_euclid(class_levels.len() as i32)
    }

    /// Number of classes a creature's `CLASS.IDS` byte names (e.g.
    /// `FIGHTER` → 1, `FIGHTER_MAGE` → 2, `FIGHTER_MAGE_THIEF` → 3).
    /// This drives the multi-class HP split and — unlike counting
    /// non-zero level fields — is robust to Torment's habit of storing
    /// `1` (not `0`) in the unused class-level slots of single-class
    /// characters. `1` for IWD2 (handled as a single total level) and
    /// for any byte not found in `CLASS.IDS`.
    pub fn class_count(&self, class_id: u8) -> usize {
        if self.engine == Engine::Iwd2 {
            return 1;
        }
        self.class_symbols
            .get(&i32::from(class_id))
            .map(|symbol| symbol.split('_').filter(|c| !c.is_empty()).count().max(1))
            .unwrap_or(1)
    }

    /// Total CON→HP bonus for a creature's maximum HP, dispatching on
    /// the engine's rules and the creature's class shape.
    ///
    /// - **Single-class** (`class_count == 1`) or **dual-class**: the
    ///   per-level bonus over the (capped) primary level.
    /// - **Multi-class** (`class_count >= 2`, not dual): split across
    ///   classes via [`Self::max_hp_constitution_bonus_multiclass`],
    ///   summing the first `class_count` entries of `class_levels`.
    /// - **Torment** (PST / PSTEE): the Hit-Die cap is removed, so the
    ///   bonus applies on every level.
    #[allow(clippy::too_many_arguments)] // distinct HP inputs, not a bag of options
    pub fn max_hp_constitution_bonus_for(
        &self,
        constitution: u8,
        is_warrior: bool,
        hp_roll_cap: u8,
        primary_level: u8,
        class_levels: [u8; 3],
        class_count: usize,
        is_dual: bool,
    ) -> i32 {
        let cap = if self.con_hp_uncapped {
            u8::MAX
        } else {
            hp_roll_cap
        };
        if class_count >= 2 && !is_dual {
            let n = class_count.min(class_levels.len());
            self.max_hp_constitution_bonus_multiclass(
                constitution,
                is_warrior,
                cap,
                &class_levels[..n],
            )
        } else {
            self.max_hp_constitution_bonus(
                constitution,
                is_warrior,
                u32::from(cap.min(primary_level)),
            )
        }
    }

    /// Warrior flag + CON HP-roll cap for a creature's class, from its
    /// raw `CLASS.IDS` byte. Single entry point for HP math:
    ///
    /// - **IWD2** (d20): the CON modifier applies every level with no
    ///   Hit-Die cap and no warrior distinction → `(false, u8::MAX)`.
    /// - **Enhanced Editions**: data-driven from `HPCLASS.2DA` →
    ///   `HP*.2DA` (see [`Self::class_hp_profile_from_tables`]).
    /// - **Classic BG/BG2/IWD/PST**: those engines hardcoded the
    ///   class→HP-table map and don't ship `HPCLASS.2DA`, so fall back
    ///   to the name heuristic ([`hp_profile_heuristic`]).
    ///
    /// An unrecognised class byte yields the conservative
    /// `(false, 9)` (non-warrior, AD&D Hit-Die cap).
    pub fn class_hp_profile(&self, class_id: u8) -> (bool, u8) {
        if self.engine == Engine::Iwd2 {
            return (false, u8::MAX);
        }
        let Some(symbol) = self.class_symbols.get(&i32::from(class_id)) else {
            return (false, 9);
        };
        self.class_hp_profile_from_tables(symbol)
            .unwrap_or_else(|| hp_profile_heuristic(symbol))
    }

    /// Data-driven warrior + CON-HP-roll cap from stock 2DAs
    /// (`HPCLASS.2DA` → `HP*.2DA`): a class is a *warrior* (larger
    /// CON→HP bonus) when its Hit Die is d10+ (`SIDES >= 10`); a
    /// component's cap is the highest level that still rolls a die.
    ///
    /// The cap is the level past which CON stops adding HP. The engine
    /// credits it against a single Hit-Die progression, and the
    /// **warrior** group's (lower) cap governs the whole character when
    /// any component is a warrior: a Fighter/Thief caps CON HP at the
    /// Fighter's level 9, *not* the Thief's 10 (verified in IWD:EE — a
    /// Fighter 9 / Thief 11 with CON 16 gets +18, i.e. `2 × 9`). Pure
    /// non-warrior multis use the largest component cap.
    ///
    /// `None` when `HPCLASS.2DA` isn't available (classic engines) so
    /// the caller can fall back to the heuristic.
    fn class_hp_profile_from_tables(&self, class_symbol: &str) -> Option<(bool, u8)> {
        if self.class_hp_tables.is_empty() {
            return None;
        }
        let mut is_warrior = false;
        let mut max_cap = 0u8;
        let mut warrior_cap: Option<u8> = None;
        for component in class_symbol.split('_').filter(|s| !s.is_empty()) {
            let table = self.class_hp_tables.get(&component.to_ascii_lowercase())?;
            let profile = self.hp_table_profiles.get(table)?;
            let warrior = profile.sides >= 10;
            is_warrior |= warrior;
            max_cap = max_cap.max(profile.roll_cap);
            if warrior {
                warrior_cap =
                    Some(warrior_cap.map_or(profile.roll_cap, |c| c.min(profile.roll_cap)));
            }
        }
        let cap = warrior_cap.unwrap_or(max_cap);
        (cap > 0).then_some((is_warrior, cap))
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

/// Case-insensitive column index by header name.
fn column_index(two_da: &TwoDA, header: &str) -> Option<usize> {
    two_da
        .headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(header))
}

/// Build the class→HP-table map (`HPCLASS.2DA`) plus each referenced
/// table's HD shape (`HP*.2DA` `SIDES` / `ROLLS`). Returns empty maps
/// (callers fall back) when `HPCLASS.2DA` is missing or has no `TABLE`
/// column.
fn load_class_hp_data(
    game_data: &GameData,
) -> (HashMap<String, String>, HashMap<String, HpTableProfile>) {
    let mut class_tables = HashMap::new();
    let mut profiles = HashMap::new();

    let Ok(hpclass) = game_data.import_2da_by_name("hpclass") else {
        return (class_tables, profiles);
    };
    let Some(table_col) = column_index(&hpclass, "TABLE") else {
        return (class_tables, profiles);
    };
    for (class, row) in &hpclass.rows {
        if let Some(table) = row.get(table_col) {
            class_tables.insert(
                class.to_ascii_lowercase(),
                table.trim().to_ascii_lowercase(),
            );
        }
    }

    for table in class_tables.values() {
        if profiles.contains_key(table) {
            continue;
        }
        let Ok(two_da) = game_data.import_2da_by_name(table) else {
            continue;
        };
        let sides_col = column_index(&two_da, "SIDES");
        let rolls_col = column_index(&two_da, "ROLLS");
        let mut sides = 0u8;
        let mut roll_cap = 0u8;
        for (level, row) in &two_da.rows {
            let level: u32 = level.trim().parse().unwrap_or(0);
            let s = sides_col
                .and_then(|i| row.get(i))
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(0);
            let rolls = rolls_col
                .and_then(|i| row.get(i))
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(0);
            sides = sides.max(s.clamp(0, 255) as u8);
            if rolls >= 1 && level <= u8::MAX as u32 {
                roll_cap = roll_cap.max(level as u8);
            }
        }
        profiles.insert(table.clone(), HpTableProfile { sides, roll_cap });
    }

    (class_tables, profiles)
}

/// Load `CLASS.IDS` (value → symbol) once. Best-effort: an empty map
/// (missing IDS) just makes [`EngineCaps::class_hp_profile`] return
/// its conservative default for every class.
fn load_class_symbols(game_data: &GameData) -> HashMap<i32, String> {
    match game_data.import_ids_by_name("class") {
        Ok(ids) => ids.entries.iter().map(|e| (e.value, e.name.clone())).collect(),
        Err(_) => HashMap::new(),
    }
}

/// Heuristic warrior + HP-roll cap from a `CLASS.IDS` symbol, used for
/// the classic (pre-EE) BG/BG2/IWD/PST engines, which don't ship
/// `HPCLASS.2DA` (they hardcoded the class→HP-table map; the Enhanced
/// Editions externalised it). Warriors (Fighter / Paladin / Ranger /
/// Barbarian, including combos that contain one) use the larger
/// CON→HP column; rogues and wizards roll Hit Dice through level 10,
/// everyone else through 9 (AD&D 2e). For combos the highest
/// component cap wins.
fn hp_profile_heuristic(class_symbol: &str) -> (bool, u8) {
    let has = |needle: &str| class_symbol.contains(needle);
    let is_warrior = has("FIGHTER") || has("PALADIN") || has("RANGER") || has("BARBARIAN");
    // Warriors credit CON HP only through their Hit-Die cap (level 9),
    // even when multiclassed with a rogue/mage; the warrior cap governs.
    // Pure rogue/arcane classes roll Hit Dice through 10.
    let cap = if is_warrior {
        9
    } else if has("MAGE") || has("SORCERER") || has("THIEF") || has("BARD") {
        10
    } else {
        9
    };
    (is_warrior, cap)
}

fn load_bonus_table(
    game_data: &GameData,
    name: &str,
    column_candidates: &[&str],
) -> io::Result<BonusTable> {
    let two_da = game_data.import_2da_by_name(name)?;
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

    use crate::resource::ResourceType;

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
            class_hp_tables: HashMap::new(),
            hp_table_profiles: HashMap::new(),
            class_symbols: HashMap::new(),
            con_hp_uncapped: false,
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

    /// Populate an `ee_caps()` with the HPCLASS-derived tables + a few
    /// CLASS.IDS rows, mirroring real BG2:EE data.
    fn ee_caps_with_classes() -> EngineCaps {
        let mut caps = ee_caps();
        for (c, t) in [
            ("paladin", "hpwar"),
            ("fighter", "hpwar"),
            ("mage", "hpwiz"),
            ("thief", "hprog"),
            ("cleric", "hpprs"),
        ] {
            caps.class_hp_tables.insert(c.into(), t.into());
        }
        for (t, sides, cap) in [
            ("hpwar", 10, 9),
            ("hpwiz", 4, 10),
            ("hprog", 6, 10),
            ("hpprs", 8, 9),
        ] {
            caps.hp_table_profiles.insert(
                t.into(),
                HpTableProfile {
                    sides,
                    roll_cap: cap,
                },
            );
        }
        for (v, s) in [
            (3, "PALADIN"),
            (21, "MAGE_THIEF"),
            (16, "CLERIC_MAGE"),
            (8, "FIGHTER_MAGE"),
        ] {
            caps.class_symbols.insert(v, s.into());
        }
        caps
    }

    #[test]
    fn class_hp_profile_from_tables_data_driven() {
        let caps = ee_caps_with_classes();
        // Single warrior class (d10 HD) → warrior, cap 9.
        assert_eq!(
            caps.class_hp_profile_from_tables("PALADIN"),
            Some((true, 9))
        );
        // Mage/Thief: not warrior (d4/d6), cap = max(10, 10).
        assert_eq!(
            caps.class_hp_profile_from_tables("MAGE_THIEF"),
            Some((false, 10))
        );
        // Cleric/Mage: not warrior, cap = max(9, 10).
        assert_eq!(
            caps.class_hp_profile_from_tables("CLERIC_MAGE"),
            Some((false, 10))
        );
        // Fighter/Mage: warrior (fighter d10) → the warrior cap (9)
        // governs, not the mage's 10.
        assert_eq!(
            caps.class_hp_profile_from_tables("FIGHTER_MAGE"),
            Some((true, 9))
        );
        // Fighter/Thief: same — the warrior cap (9) governs, not the
        // thief's 10. (IWD:EE: Fighter 9 / Thief 11, CON 16 → +18.)
        assert_eq!(
            caps.class_hp_profile_from_tables("FIGHTER_THIEF"),
            Some((true, 9))
        );
        // Unknown component → None so the caller falls back.
        assert_eq!(caps.class_hp_profile_from_tables("GISH"), None);
        // No tables loaded (classic engines) → None.
        assert_eq!(ee_caps().class_hp_profile_from_tables("PALADIN"), None);
    }

    #[test]
    fn class_hp_profile_resolves_class_byte() {
        let caps = ee_caps_with_classes();
        // Resolves CLASS.IDS byte → symbol → data-driven profile.
        assert_eq!(caps.class_hp_profile(3), (true, 9)); // PALADIN
        assert_eq!(caps.class_hp_profile(21), (false, 10)); // MAGE_THIEF
        // Unrecognised class byte → conservative default.
        assert_eq!(caps.class_hp_profile(200), (false, 9));
    }

    #[test]
    fn class_hp_profile_falls_back_to_heuristic_without_hpclass() {
        // Classic engine: CLASS.IDS present, but no HPCLASS tables.
        let mut caps = ee_caps();
        caps.class_symbols.insert(3, "PALADIN".into());
        caps.class_symbols.insert(21, "MAGE_THIEF".into());
        caps.class_symbols.insert(9, "FIGHTER_THIEF".into());
        assert_eq!(caps.class_hp_profile(3), (true, 9)); // heuristic: warrior, 9
        assert_eq!(caps.class_hp_profile(21), (false, 10)); // heuristic: mage/thief, 10
        // Warrior+rogue multi: warrior cap (9) governs, not the thief's 10.
        assert_eq!(caps.class_hp_profile(9), (true, 9));
    }

    #[test]
    fn multiclass_con_bonus_splits_across_classes() {
        let caps = ee_caps();
        // Fighter 9 / Thief 11, CON 16 (+2), governing cap 9:
        // floor(2 × (min(9,9) + min(9,11)) / 2) = floor(36/2) = 18.
        assert_eq!(
            caps.max_hp_constitution_bonus_multiclass(16, true, 9, &[9, 11]),
            18
        );
        // Cleric 9 / Ranger 8, CON 16, cap 9:
        // floor(2 × (9 + 8) / 2) = floor(34/2) = 17.
        assert_eq!(
            caps.max_hp_constitution_bonus_multiclass(16, true, 9, &[9, 8]),
            17
        );
        // Wiki example — Fighter/Mage/Cleric 7/7/7, CON 18 (warrior +4):
        // floor(4 × (7+7+7) / 3) = floor(84/3) = 28.
        assert_eq!(
            caps.max_hp_constitution_bonus_multiclass(18, true, 7, &[7, 7, 7]),
            28
        );
        // Uncapped (Torment): a Fighter 9 / Mage 11 credits CON on every
        // level → floor(2 × (9 + 11) / 2) = 20, not 18.
        assert_eq!(
            caps.max_hp_constitution_bonus_multiclass(16, true, u8::MAX, &[9, 11]),
            20
        );
    }

    #[test]
    fn class_count_from_symbol() {
        let caps = ee_caps_with_classes();
        // ee_caps_with_classes maps 3 → PALADIN, 21 → MAGE_THIEF,
        // 16 → CLERIC_MAGE, 8 → FIGHTER_MAGE.
        assert_eq!(caps.class_count(3), 1); // single
        assert_eq!(caps.class_count(21), 2); // double
        assert_eq!(caps.class_count(8), 2);
        // Unknown byte / IWD2 fall back to a single class.
        assert_eq!(caps.class_count(200), 1);
    }

    #[test]
    fn con_hp_bonus_uncapped_for_torment_games() {
        let mut caps = ee_caps();

        // Torment (PST/PSTEE): CON credits on every level (no Hit-Die
        // cap), so a level-12 mage with CON 16 (+2) gets the full +24.
        // Note the unused class-level slots are `1`, not `0` — the
        // single-class path uses `primary_level`, so they're ignored.
        caps.con_hp_uncapped = true;
        assert_eq!(
            caps.max_hp_constitution_bonus_for(16, false, 10, 12, [12, 1, 1], 1, false),
            24
        );
        // A Fighter 9 / Mage 11 multi-class, uncapped → floor(2×(9+11)/2) = 20.
        assert_eq!(
            caps.max_hp_constitution_bonus_for(16, true, 9, 11, [9, 11, 1], 2, false),
            20
        );

        // Non-Torment caps the same mage at level 10 → +20.
        caps.con_hp_uncapped = false;
        assert_eq!(
            caps.max_hp_constitution_bonus_for(16, false, 10, 12, [12, 0, 0], 1, false),
            20
        );
    }

    #[test]
    fn class_hp_profile_iwd2_is_uncapped_non_warrior() {
        let mut caps = ee_caps_with_classes();
        caps.engine = Engine::Iwd2;
        // IWD2 ignores class tables: d20 modifier every level, no cap.
        assert_eq!(caps.class_hp_profile(3), (false, u8::MAX));
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
                imported: None,
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
