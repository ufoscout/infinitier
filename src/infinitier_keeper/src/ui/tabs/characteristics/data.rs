//! Read-only extraction + resolution for the Characteristics tab.
//!
//! The identity fields (gender / race / alignment / class / kit /
//! enemy-ally / state) are stored in the CRE header as raw IDS values
//! and resolved here against the game's `*.IDS` files (`GENDER.IDS`,
//! `RACE.IDS`, `ALIGNMEN.IDS`, `CLASS.IDS`, `KIT.IDS`, `EA.IDS`,
//! `STATE.IDS`). The symbol from the IDS file is title-cased for
//! display so `LAWFUL_GOOD` reads "Lawful Good" and the multi-class
//! `MAGE_THIEF` reads "Mage / Thief".
//!
//! Creature-flags-derived state (the "Miscellaneous" checkboxes, the
//! dual-class original class, and the fallen-paladin/ranger status) is
//! decoded by the resource layer — see [`Cre::creature_flags`],
//! [`Cre::dual_class_original_class`], and [`Cre::is_fallen`] — so this
//! module does no creature-flags bit math of its own.
//!
//! The "Kill Stats" block (strongest vanquished, per-chapter and
//! per-game kill counts/XP) is **not** in the CRE — it lives in the
//! GAM party-member struct. The byte offsets below were confirmed
//! empirically against the BG2EE TOB reference save (Xor: Firkraag /
//! 64000, chapter 413 / 3090880, game 1813 / 8750681).

use infinitier_core::game::GameData;
use infinitier_core::imported_resource::cre::ImportedCre;
use infinitier_core::resource::Game;
use infinitier_core::resource::cre::{Cre, CreHeader, CreHeaderV22, CreatureFlags};
use infinitier_core::resource::gam::NpcCharStats;
use infinitier_core::resource::two_da::TwoDA;

/// Resolved, display-ready characteristics for one creature.
#[derive(Debug, Default, Clone)]
pub struct CharData {
    pub gender: String,
    pub race: String,
    /// IWD2-only subrace (e.g. "Elf Drow"); empty on every other engine,
    /// where the view hides the row.
    pub sub_race: String,
    pub alignment: String,
    pub class: String,
    pub original_class: String,
    /// How the CRE "kit" dword is presented — a kit on most engines, or
    /// a Deity + mage-specialisation pair on PST:EE.
    pub specialization: Specialization,
    pub racial_enemy: TlkLabel,
    pub enemy_ally: String,
    pub state: String,
    pub movement: i64,
    pub fallen: bool,
    pub dual_class: bool,
    pub kill: KillStats,
    /// CRE creature-flags, decoded by the resource layer. The
    /// "Miscellaneous" checkboxes render from [`CreatureFlags::MISC`].
    pub flags: CreatureFlags,
}

/// How the CRE "kit" dword (offset `0x0244`) is presented in the
/// identity column. BG / BG2 / IWD(:EE) store a kit there; PST:EE
/// repurposes the same dword as a Deity (low word) + mage-specialisation
/// (high word) pair — see NearInfinity's `Game.PSTEE` branch — so the
/// identity column shows those two fields instead.
#[derive(Debug, Clone)]
pub enum Specialization {
    /// A kit — its display name resolved at render time (see [`TlkLabel`]).
    /// "Base Class" for the true-class sentinel, or empty when the value
    /// isn't a known kit.
    Kit(TlkLabel),
    /// PST:EE — a deity / mage-specialisation pair, shown in place of the
    /// kit. Either string is empty when its IDS value doesn't resolve.
    PstReligion { deity: String, mage_type: String },
}

impl Default for Specialization {
    fn default() -> Self {
        Specialization::Kit(TlkLabel::default())
    }
}

/// A display label whose text comes from a `dialog.tlk` strref resolved
/// **at render time**, so the (memoised) tlk lookup stays in the view layer
/// rather than re-parsing the tlk on every repaint here. `strref == 0`
/// means "no strref" — show [`fallback`](Self::fallback) instead; the
/// fallback is also used when the strref doesn't resolve.
#[derive(Debug, Default, Clone)]
pub struct TlkLabel {
    pub strref: u32,
    pub fallback: String,
}

/// GAM-sourced kill statistics for one party slot.
#[derive(Debug, Default, Clone)]
pub struct KillStats {
    /// TLK strref of the strongest creature this character has slain.
    pub strongest_name_strref: u32,
    pub strongest_xp: u32,
    pub chapter_kills: u32,
    pub chapter_kills_xp: u32,
    pub game_kills: u32,
    pub game_kills_xp: u32,
}

/// Raw IDS bytes lifted from a CRE header before name resolution.
struct RawChar {
    gender: u8,
    race: u8,
    /// IWD2 (V2.2) subrace byte; 0 ("PURERACE") on the AD&D engines, which
    /// have no subrace field.
    subrace: u8,
    alignment: u8,
    class: u8,
    ea: u8,
    kit: u32,
    state_flags: u32,
}

impl CharData {
    /// Extract + resolve everything the tab paints. The kill
    /// statistics come from the GAM party slot's typed
    /// [`NpcCharStats`] block (parsed by the gam importer).
    pub fn resolve(
        imported: &ImportedCre,
        char_stats: &NpcCharStats,
        game_data: &GameData,
    ) -> CharData {
        let cre: &Cre = imported.cre();
        let kill = KillStats {
            strongest_name_strref: char_stats.most_powerful_vanquished_name,
            strongest_xp: char_stats.most_powerful_vanquished_xp,
            chapter_kills: char_stats.kills_number_chapter,
            chapter_kills_xp: char_stats.kills_xp_chapter,
            game_kills: char_stats.kills_number_game,
            game_kills_xp: char_stats.kills_xp_game,
        };
        // Creature-flags-derived fields come straight from the CRE's
        // typed accessors — the bit math lives on the resource type, not
        // here — and work on every header version (V2.2 included).
        let flags = cre.creature_flags();
        let original_class = cre.dual_class_original_class();
        let dual_class = original_class.is_some();
        let fallen = cre.is_fallen();
        let original_class_label = original_class
            .map(|c| pretty(c.symbol(), " "))
            .unwrap_or_default();

        let Some(raw) = raw_char(cre) else {
            // Engine variant we don't decode identity for yet (e.g.
            // IWD2 V2.2). Still surface the kill stats and flags.
            return CharData {
                kill,
                flags,
                dual_class,
                fallen,
                original_class: original_class_label,
                ..Default::default()
            };
        };

        // PST:EE repurposes the "kit" dword as a Deity (low word) +
        // mage-specialisation (high word) pair — see NearInfinity's
        // `Game.PSTEE` branch in `CreResource`. There we show those two
        // fields instead of a (meaningless) kit.
        let specialization = if game_data.game() == Game::Pstee {
            Specialization::PstReligion {
                deity: resolve_deity(game_data, (raw.kit & 0xFFFF) as i32),
                mage_type: resolve_mage_type(game_data, (raw.kit >> 16) as i32),
            }
        } else {
            Specialization::Kit(resolve_kit(game_data, imported))
        };

        // IWD2 (V2.2) diverges from the AD&D engines in three identity fields:
        //  - class is rebuilt from the per-class level array (the CLASS.IDS
        //    byte is stale once multiclassed);
        //  - alignment names come from ALIGNS.2DA, not ALIGNMEN.IDS (which
        //    IWD2 doesn't ship), so the IDS lookup would otherwise show blank;
        //  - a subrace byte (combined with the base race) resolves a SUBRACE
        //    name like "Elf Drow". None of these exist on the other engines.
        let is_iwd2 = matches!(cre.header, CreHeader::V22(_));
        let class = match &cre.header {
            CreHeader::V22(h) => {
                let by_levels = iwd2_class_label(h);
                if by_levels.is_empty() {
                    ids_pretty(game_data, "class", raw.class as i32, " / ")
                } else {
                    by_levels
                }
            }
            _ => ids_pretty(game_data, "class", raw.class as i32, " / "),
        };
        let alignment = if is_iwd2 {
            iwd2_alignment(game_data, raw.alignment)
        } else {
            ids_pretty(game_data, "alignmen", raw.alignment as i32, " ")
        };
        let sub_race = if is_iwd2 {
            iwd2_sub_race(game_data, raw.race, raw.subrace)
        } else {
            String::new()
        };

        CharData {
            gender: ids_pretty(game_data, "gender", raw.gender as i32, " "),
            race: ids_pretty(game_data, "race", raw.race as i32, " "),
            sub_race,
            alignment,
            class,
            original_class: original_class_label,
            specialization,
            racial_enemy: resolve_racial_enemy(game_data, imported),
            enemy_ally: ids_pretty(game_data, "ea", raw.ea as i32, " "),
            state: resolve_state(game_data, raw.state_flags),
            // No overall-move-rate field in the CRE header; the engine
            // derives movement from the avatar animation. EEKeeper
            // shows 0 here ("Zero is normal speed").
            movement: 0,
            fallen,
            dual_class,
            kill,
            flags,
        }
    }
}

/// Lift the identity bytes out of whichever CRE header version this
/// is. V10/V12/V90 share the layout (only the gender/kit field names
/// the generator picked differ); V2.2 (IWD2) keeps its identity bytes
/// at the end of its larger header.
fn raw_char(cre: &Cre) -> Option<RawChar> {
    match &cre.header {
        CreHeader::V10(h) => Some(RawChar {
            gender: h.gender_gender_ids_dictates_the_casting,
            race: h.race_race_ids,
            subrace: 0,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids,
            ea: h.enemy_ally_ea_ids,
            kit: h.kit_information_none_0x00000000_kit_barbarian,
            state_flags: h.permanent_status_flags_state_ids,
        }),
        CreHeader::V12(h) => Some(RawChar {
            gender: h.gender_gender_ids,
            race: h.race_race_ids,
            subrace: 0,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids,
            ea: h.enemy_ally_ea_ids,
            kit: h.kit_information_none_0x00000000_abjurer_0x00400000,
            state_flags: h.permanent_status_flags_state_ids,
        }),
        CreHeader::V90(h) => Some(RawChar {
            gender: h.gender_gender_ids,
            race: h.race_race_ids,
            subrace: 0,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids,
            ea: h.enemy_ally_ea_ids,
            kit: h.kit_information_none_abjurer_0x00400000_conjurer,
            state_flags: h.permanent_status_flags_state_ids,
        }),
        // IWD2 (3E). The single `class` byte is stale once a character
        // multiclasses — the real class is reconstructed from the per-class
        // level fields in `resolve` (see [`iwd2_class_label`]). The kit
        // bitfield and racial-enemy/dual-class concepts don't apply here, so
        // `Cre::kit`/`racial_enemy`/`dual_class_original_class` already return
        // `None` and the corresponding columns render blank.
        CreHeader::V22(h) => Some(RawChar {
            gender: h.sex_gender_ids,
            race: h.race_race_ids,
            subrace: h.subrace_subrace_ids,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids_not_updated_when,
            ea: h.enemy_ally_ea_ids,
            kit: h.kit_bitfield,
            state_flags: h.permanent_status_flags_state_ids,
        }),
    }
}

/// Reconstruct an IWD2 character's class from its per-class level fields.
/// IWD2 is 3rd-edition: multiclassing stacks independent class levels, and
/// the header's single CLASS.IDS byte is *not* updated when you multiclass
/// (so it can't be trusted). The class shown is therefore every base class
/// with a non-zero level, e.g. a Fighter/Wizard reads "Fighter / Wizard"
/// (matching EEKeeper's IWD2 class display). Returns the empty string when
/// no class has levels, letting the caller fall back to the CLASS.IDS byte.
fn iwd2_class_label(h: &CreHeaderV22) -> String {
    class_from_levels(&[
        (h.barbarian_levels, "Barbarian"),
        (h.bard_levels, "Bard"),
        (h.cleric_levels, "Cleric"),
        (h.druid_levels, "Druid"),
        (h.fighter_levels, "Fighter"),
        (h.monk, "Monk"),
        (h.paladin_levels, "Paladin"),
        (h.ranger_levels, "Ranger"),
        (h.rogue_levels, "Rogue"),
        (h.sorcerer_levels, "Sorcerer"),
        (h.wizard_levels, "Wizard"),
    ])
}

/// Join the names of every class with a non-zero level using `" / "`.
fn class_from_levels(levels: &[(u8, &str)]) -> String {
    levels
        .iter()
        .filter(|(level, _)| *level > 0)
        .map(|(_, name)| *name)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// Resolve an IDS value to its symbol, then title-case it for
/// display. Returns the empty string when the IDS file or the value
/// isn't found.
fn ids_pretty(game_data: &GameData, ids_file: &str, value: i32, sep: &str) -> String {
    ids_symbol(game_data, ids_file, value)
        .map(|s| pretty(&s, sep))
        .unwrap_or_default()
}

/// Look up the raw IDS symbol for `value` in `<ids_file>.IDS`.
fn ids_symbol(game_data: &GameData, ids_file: &str, value: i32) -> Option<String> {
    let ids = game_data.import_ids_by_name(ids_file).ok()?;
    ids.entries
        .iter()
        .find(|e| e.value == value)
        .map(|e| e.name.clone())
}

/// IWD2 alignment name, from `ALIGNS.2DA` rather than `ALIGNMEN.IDS` (IWD2
/// doesn't ship the latter). The CRE stores the usual two-axis byte
/// (`0x11` = lawful-good, `0x21` = neutral-good, …); `ALIGNS.2DA` maps it in
/// its `VALUE` column, and the matching row's (symbolic) name — `NEUTRAL_GOOD`
/// — title-cases to "Neutral Good". Blank when the table or value is missing.
fn iwd2_alignment(game_data: &GameData, value: u8) -> String {
    let Ok(aligns) = game_data.import_2da_by_name("aligns") else {
        return String::new();
    };
    alignment_from_2da(&aligns, value)
}

/// The `ALIGNS.2DA` row name whose `VALUE` column equals the alignment byte,
/// title-cased ("NEUTRAL_GOOD" → "Neutral Good"). Blank when there's no
/// `VALUE` column or no matching row.
fn alignment_from_2da(aligns: &TwoDA, value: u8) -> String {
    let Some(value_col) = aligns
        .headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case("VALUE"))
    else {
        return String::new();
    };
    aligns
        .rows
        .iter()
        .find(|(_, cells)| {
            cells.get(value_col).and_then(|c| parse_2da_int(c)) == Some(i64::from(value))
        })
        .map(|(name, _)| pretty(name, " "))
        .unwrap_or_default()
}

/// IWD2 subrace name (e.g. "Elf Drow"), from `SUBRACE.IDS`.
///
/// `SUBRACE.IDS` keys on a 32-bit packed value `(race << 16) | subrace`, but
/// the CRE stores only the low subrace byte — so the byte alone is ambiguous
/// (`1` is Aasimar for a human, Drow for an elf, …). We re-pack it with the
/// creature's `RACE.IDS` value to disambiguate. The all-zero byte is
/// `PURERACE` (value `0`, no race packed in). Blank when nothing matches.
fn iwd2_sub_race(game_data: &GameData, race: u8, subrace: u8) -> String {
    ids_symbol(game_data, "subrace", subrace_packed(race, subrace))
        .map(|s| pretty(&s, " "))
        .unwrap_or_default()
}

/// Re-pack the CRE's low subrace byte with the base `RACE.IDS` value into the
/// 32-bit key `SUBRACE.IDS` uses: `(race << 16) | subrace`. A zero subrace
/// byte is the `PURERACE` sentinel (value `0`), with no race packed in.
fn subrace_packed(race: u8, subrace: u8) -> i32 {
    if subrace == 0 {
        0
    } else {
        (i32::from(race) << 16) | i32::from(subrace)
    }
}

/// Parse a 2DA cell as an integer, accepting hex (`0x…`) or decimal.
fn parse_2da_int(cell: &str) -> Option<i64> {
    let cell = cell.trim();
    match cell.strip_prefix("0x").or_else(|| cell.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => cell.parse().ok(),
    }
}

/// `KIT.IDS` stores the kit in a word-swapped form versus the CRE
/// header dword (GemRB `GetActorBG`). Swap, then resolve. The
/// "true class" sentinel (no kit chosen) reads "Base Class" in
/// EEKeeper; an all-zero field means no kit at all.
///
/// A value with no `KIT.IDS` entry is shown blank, matching EEKeeper
/// (its kit combo simply has no matching row). This is also what makes
/// PST:EE read sensibly: there the dword isn't a kit at all but a
/// Deity (low word) + mage-specialisation (high word) pair (see
/// NearInfinity's `Game.PSTEE` branch), so only the Nameless One —
/// whose all-zero deity + generalist-mage value happens to swap to the
/// `TRUECLASS` sentinel — reads "Base Class"; everyone else's
/// deity/mage bytes don't form a valid kit and stay blank.
fn resolve_kit(game_data: &GameData, imported: &ImportedCre) -> TlkLabel {
    let Some(kit) = imported.cre().kit() else {
        return TlkLabel::default();
    };
    let swapped = ((kit & 0xFFFF) << 16) | (kit >> 16);
    if swapped == 0 {
        return TlkLabel::default();
    }
    // True-class sentinel reads "Base Class" (matching EEKeeper).
    if ids_symbol(game_data, "kit", swapped as i32).as_deref() == Some("TRUECLASS") {
        return TlkLabel {
            strref: 0,
            fallback: "Base Class".to_string(),
        };
    }
    // EEKeeper shows the kit's proper display name from KITLIST.2DA, e.g.
    // "Undead Hunter". The 2DA row-matching lives on [`ImportedCre`]; the view
    // resolves the returned strref against dialog.tlk (memoised). The
    // prettified KIT.IDS symbol is the fallback ("UNDEADHUNTER" →
    // "Undeadhunter") when KITLIST has no matching row.
    let fallback = ids_symbol(game_data, "kit", swapped as i32)
        .map(|s| pretty(&s, " "))
        .unwrap_or_default();
    let strref = imported.kit_strref(game_data).unwrap_or(0);
    TlkLabel { strref, fallback }
}

/// PST:EE deity — the low word of the "kit" dword, resolved against
/// `DEITY.IDS` (or NearInfinity's `DIETY.IDS` spelling fallback). `0`
/// and any unmapped value render blank.
fn resolve_deity(game_data: &GameData, value: i32) -> String {
    if value == 0 {
        return String::new();
    }
    ids_symbol(game_data, "deity", value)
        .or_else(|| ids_symbol(game_data, "diety", value))
        .map(|s| pretty(&s, " "))
        .unwrap_or_default()
}

/// PST:EE mage specialisation — the high word of the "kit" dword,
/// resolved against `MAGESPEC.IDS`, falling back to NearInfinity's
/// built-in school map when that IDS is absent. `0` (non-mage) is blank.
fn resolve_mage_type(game_data: &GameData, value: i32) -> String {
    if value == 0 {
        return String::new();
    }
    if let Some(s) = ids_symbol(game_data, "magespec", value) {
        return pretty(&s, " ");
    }
    // NearInfinity `MAGE_TYPE_MAP` fallback (CreResource).
    match value {
        0x0040 => "Abjurer",
        0x0080 => "Conjurer",
        0x0100 => "Diviner",
        0x0200 => "Enchanter",
        0x0400 => "Illusionist",
        0x0800 => "Invoker",
        0x1000 => "Necromancer",
        0x2000 => "Transmuter",
        0x4000 => "Generalist",
        _ => "",
    }
    .to_string()
}

/// `NO_RACE` / 0 means "no racial enemy" — render blank like EEKeeper.
fn resolve_racial_enemy(game_data: &GameData, imported: &ImportedCre) -> TlkLabel {
    let Some(value) = imported.cre().racial_enemy() else {
        return TlkLabel::default();
    };
    if value == 0 {
        return TlkLabel::default();
    }
    // Fallback: prettified RACE.IDS symbol ("SKELETON" → "Skeleton").
    let fallback = match ids_symbol(game_data, "race", value as i32) {
        Some(s) if s == "NO_RACE" || s == "ANYTHING" => String::new(),
        Some(s) => pretty(&s, " "),
        None => String::new(),
    };
    // EEKeeper shows the favored-enemy name from HATERACE.2DA. The 2DA
    // row-matching lives on [`ImportedCre`]; the view resolves the returned
    // strref against dialog.tlk (memoised) — e.g. "skeletal undead".
    let strref = imported.racial_enemy_strref(game_data).unwrap_or(0);
    TlkLabel { strref, fallback }
}

/// The permanent-status field is a STATE.IDS bitfield; the common
/// case is 0 ("State Normal"). Resolve the value directly and fall
/// back to a hex dump for exotic bit combinations.
fn resolve_state(game_data: &GameData, flags: u32) -> String {
    match ids_symbol(game_data, "state", flags as i32) {
        Some(s) => pretty(&s, " "),
        None => format!("0x{flags:08X}"),
    }
}

/// Title-case an `UPPER_SNAKE` IDS symbol, joining the words with
/// `sep` (`" "` for most fields, `" / "` for multi-class names so
/// `MAGE_THIEF` becomes "Mage / Thief").
fn pretty(symbol: &str, sep: &str) -> String {
    symbol
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(sep)
}

#[cfg(test)]
mod tests {
    // KITLIST / HATERACE 2DA name resolution lives on `Cre` (see
    // `infinitier_cre_resource`); this module only orchestrates the lookup +
    // the IDS-symbol fallback, and the view does the strref → tlk step.
    use super::{TwoDA, class_from_levels, pretty, raw_char};
    use infinitier_core::fs::{DataSource, Importer};
    use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
    use infinitier_core::resource::Game;
    use infinitier_core::resource::cre::CreHeader;
    use infinitier_core::resource::gam::GamImporter;

    #[test]
    fn pretty_single_word() {
        assert_eq!(pretty("PALADIN", " "), "Paladin");
        assert_eq!(pretty("INQUISITOR", " "), "Inquisitor");
    }

    #[test]
    fn pretty_alignment_uses_space() {
        assert_eq!(pretty("LAWFUL_GOOD", " "), "Lawful Good");
        assert_eq!(pretty("NEUTRAL_GOOD", " "), "Neutral Good");
    }

    #[test]
    fn pretty_multiclass_uses_slash() {
        assert_eq!(pretty("MAGE_THIEF", " / "), "Mage / Thief");
        assert_eq!(pretty("CLERIC_MAGE", " / "), "Cleric / Mage");
    }

    #[test]
    fn pretty_ea_titlecases() {
        assert_eq!(pretty("PC", " "), "Pc");
    }

    #[test]
    fn class_from_levels_single_class() {
        assert_eq!(
            class_from_levels(&[(5, "Fighter"), (0, "Wizard")]),
            "Fighter"
        );
    }

    #[test]
    fn class_from_levels_multiclass_joins_with_slash() {
        assert_eq!(
            class_from_levels(&[(0, "Barbarian"), (3, "Fighter"), (2, "Wizard")]),
            "Fighter / Wizard"
        );
    }

    #[test]
    fn class_from_levels_no_levels_is_empty() {
        assert_eq!(class_from_levels(&[(0, "Fighter"), (0, "Wizard")]), "");
    }

    #[test]
    fn subrace_packed_repacks_race_with_subrace_byte() {
        // PURERACE: a zero byte is value 0 regardless of race.
        assert_eq!(super::subrace_packed(1, 0), 0);
        // Human (RACE.IDS 1) + Aasimar (1) → 0x10001 (SUBRACE.IDS HUMAN_AASIMAR).
        assert_eq!(super::subrace_packed(1, 1), 0x0001_0001);
        // Human + Tiefling (2) → 0x10002 (HUMAN_TIEFLING).
        assert_eq!(super::subrace_packed(1, 2), 0x0001_0002);
        // Elf (RACE.IDS 2) + 1 → 0x20001 (ELF_DROW) — the byte alone (1) would
        // be ambiguous with the human's Aasimar without the race in the high
        // word.
        assert_eq!(super::subrace_packed(2, 1), 0x0002_0001);
    }

    /// IWD2 alignment names come from ALIGNS.2DA's `VALUE` column, not
    /// ALIGNMEN.IDS. The two-axis byte (`0x21` = neutral-good) maps to the
    /// row name, title-cased.
    #[test]
    fn alignment_from_2da_matches_value_column() {
        let aligns = TwoDA {
            headers: vec!["NAME_REF".into(), "VALUE".into(), "COLNAME".into()],
            default: String::new(),
            rows: [
                ("LAWFUL_GOOD", vec!["7186", "0x11", "L_G"]),
                ("NEUTRAL_GOOD", vec!["7183", "0x21", "N_G"]),
                ("CHAOTIC_GOOD", vec!["7189", "0x31", "C_G"]),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().map(str::to_string).collect()))
            .collect(),
        };
        assert_eq!(super::alignment_from_2da(&aligns, 0x11), "Lawful Good");
        assert_eq!(super::alignment_from_2da(&aligns, 0x21), "Neutral Good");
        assert_eq!(super::alignment_from_2da(&aligns, 0x31), "Chaotic Good");
        // No row carries this value.
        assert_eq!(super::alignment_from_2da(&aligns, 0x99), "");
    }

    /// IWD2 creatures are CRE V2.2; previously `raw_char` returned `None` for
    /// them, so the whole Characteristics tab rendered blank. It must now lift
    /// the identity bytes out of the larger header. Party slot 0 of the IWD2
    /// quick-save is a level-1 Fighter (ea 2, race 4, sex 1, alignment 18).
    #[test]
    fn raw_char_reads_v2_2_iwd2_header() {
        let path = infinitier_test_utils::get_assets_path()
            .join("SAV_GAM/iwd2/mpsave/000000002-Quick-Save/ICEWIND2.GAM");
        let gam = GamImporter {
            name: "iwd2",
            engine: Game::Iwd2.engine(),
        }
        .import(&DataSource::new(path.as_path()))
        .expect("import IWD2 GAM fixture");
        let imported =
            ImportedGam::load_with_tlk(gam, Game::Iwd2, None).expect("ImportedGam::load_with_tlk");
        let Some(NpcCre::Cre(cre)) = &imported.party_npcs[0].cre else {
            panic!("party slot 0 must carry an embedded CRE");
        };

        let raw = raw_char(cre.cre()).expect("V2.2 identity must be decoded");
        assert_eq!(raw.ea, 2);
        assert_eq!(raw.race, 4);
        assert_eq!(raw.gender, 1);
        assert_eq!(raw.alignment, 18);
        assert_eq!(raw.state_flags, 0x0002_0000);

        // The single class byte (5) is stale on multiclass; the displayed
        // class is reconstructed from the level fields — here a lone Fighter.
        let CreHeader::V22(h) = &cre.cre().header else {
            panic!("fixture must be a V2.2 header");
        };
        assert_eq!(super::iwd2_class_label(h), "Fighter");
    }
}
