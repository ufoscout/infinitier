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
//! Dual-class state is derived from the `MC_WAS_*` bits packed into
//! the CRE creature-flags dword — exactly one of those bits set means
//! the creature dual-classed *out of* that class (GemRB
//! `Actor::IsDualClassed`), and the bit names the original class.
//!
//! The "Kill Stats" block (strongest vanquished, per-chapter and
//! per-game kill counts/XP) is **not** in the CRE — it lives in the
//! GAM party-member struct. The byte offsets below were confirmed
//! empirically against the BG2EE TOB reference save (Xor: Firkraag /
//! 64000, chapter 413 / 3090880, game 1813 / 8750681).

use infinitier_core::game::GameData;
use infinitier_core::resource::Game;
use infinitier_core::resource::cre::{Cre, CreHeader};
use infinitier_core::resource::gam::NpcCharStats;

/// Resolved, display-ready characteristics for one creature.
#[derive(Debug, Default, Clone)]
pub struct CharData {
    pub gender: String,
    pub race: String,
    pub alignment: String,
    pub class: String,
    pub original_class: String,
    /// How the CRE "kit" dword is presented — a kit on most engines, or
    /// a Deity + mage-specialisation pair on PST:EE.
    pub specialization: Specialization,
    pub racial_enemy: String,
    pub enemy_ally: String,
    pub state: String,
    pub movement: i64,
    pub fallen: bool,
    pub dual_class: bool,
    pub kill: KillStats,
    /// Raw CRE creature-flags dword — decoded into the "Miscellaneous"
    /// checkboxes via [`MISC_FLAGS`].
    pub flags: u32,
}

/// How the CRE "kit" dword (offset `0x0244`) is presented in the
/// identity column. BG / BG2 / IWD(:EE) store a kit there; PST:EE
/// repurposes the same dword as a Deity (low word) + mage-specialisation
/// (high word) pair — see NearInfinity's `Game.PSTEE` branch — so the
/// identity column shows those two fields instead.
#[derive(Debug, Clone)]
pub enum Specialization {
    /// A kit name — "Base Class" for the true-class sentinel, or empty
    /// when the value isn't a known `KIT.IDS` kit.
    Kit(String),
    /// PST:EE — a deity / mage-specialisation pair, shown in place of the
    /// kit. Either string is empty when its IDS value doesn't resolve.
    PstReligion { deity: String, mage_type: String },
}

impl Default for Specialization {
    fn default() -> Self {
        Specialization::Kit(String::new())
    }
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
    alignment: u8,
    class: u8,
    ea: u8,
    racial_enemy: u8,
    kit: u32,
    creature_flags: u32,
    state_flags: u32,
}

impl CharData {
    /// Extract + resolve everything the tab paints. The kill
    /// statistics come from the GAM party slot's typed
    /// [`NpcCharStats`] block (parsed by the gam importer).
    pub fn resolve(cre: &Cre, char_stats: &NpcCharStats, game_data: &GameData) -> CharData {
        let kill = KillStats {
            strongest_name_strref: char_stats.most_powerful_vanquished_name,
            strongest_xp: char_stats.most_powerful_vanquished_xp,
            chapter_kills: char_stats.kills_number_chapter,
            chapter_kills_xp: char_stats.kills_xp_chapter,
            game_kills: char_stats.kills_number_game,
            game_kills_xp: char_stats.kills_xp_game,
        };
        let Some(raw) = raw_char(cre) else {
            // Engine variant we don't decode identity for yet (e.g.
            // IWD2 V2.2). Still surface the kill stats.
            return CharData {
                kill,
                ..Default::default()
            };
        };

        let mc = raw.creature_flags;
        // MC_WAS_* bits (GemRB `ie_stats.h`): exactly one set ⇒ the
        // creature dual-classed and the bit names its first class.
        let was = [
            (0x0008u32, "FIGHTER"),
            (0x0010, "MAGE"),
            (0x0020, "CLERIC"),
            (0x0040, "THIEF"),
            (0x0080, "DRUID"),
            (0x0100, "RANGER"),
        ];
        let original: Vec<&str> = was
            .iter()
            .filter(|(b, _)| mc & b != 0)
            .map(|(_, n)| *n)
            .collect();
        let dual_class = original.len() == 1;

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
            Specialization::Kit(resolve_kit(game_data, raw.kit))
        };

        CharData {
            gender: ids_pretty(game_data, "gender", raw.gender as i32, " "),
            race: ids_pretty(game_data, "race", raw.race as i32, " "),
            alignment: ids_pretty(game_data, "alignmen", raw.alignment as i32, " "),
            class: ids_pretty(game_data, "class", raw.class as i32, " / "),
            original_class: if dual_class {
                pretty(original[0], " ")
            } else {
                String::new()
            },
            specialization,
            racial_enemy: resolve_racial_enemy(game_data, raw.racial_enemy),
            enemy_ally: ids_pretty(game_data, "ea", raw.ea as i32, " "),
            state: resolve_state(game_data, raw.state_flags),
            // No overall-move-rate field in the CRE header; the engine
            // derives movement from the avatar animation. EEKeeper
            // shows 0 here ("Zero is normal speed").
            movement: 0,
            fallen: mc & 0x0200 != 0 || mc & 0x0400 != 0, // fallen paladin / ranger
            dual_class,
            kill,
            flags: mc,
        }
    }
}

/// Lift the identity bytes out of whichever CRE header version this
/// is. V10/V12/V90 share the layout (only the gender/kit field names
/// the generator picked differ); V2.2 (IWD2) is not decoded here.
fn raw_char(cre: &Cre) -> Option<RawChar> {
    match &cre.header {
        CreHeader::V10(h) => Some(RawChar {
            gender: h.gender_gender_ids_dictates_the_casting,
            race: h.race_race_ids,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids,
            ea: h.enemy_ally_ea_ids,
            racial_enemy: h.racial_enemy_race_ids,
            kit: h.kit_information_none_0x00000000_kit_barbarian,
            creature_flags: h.creature_flags,
            state_flags: h.permanent_status_flags_state_ids,
        }),
        CreHeader::V12(h) => Some(RawChar {
            gender: h.gender_gender_ids,
            race: h.race_race_ids,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids,
            ea: h.enemy_ally_ea_ids,
            racial_enemy: h.racial_enemy_race_ids,
            kit: h.kit_information_none_0x00000000_abjurer_0x00400000,
            creature_flags: h.creature_flags,
            state_flags: h.permanent_status_flags_state_ids,
        }),
        CreHeader::V90(h) => Some(RawChar {
            gender: h.gender_gender_ids,
            race: h.race_race_ids,
            alignment: h.alignment_alignmen_ids,
            class: h.class_class_ids,
            ea: h.enemy_ally_ea_ids,
            racial_enemy: h.racial_enemy_race_ids,
            kit: h.kit_information_none_abjurer_0x00400000_conjurer,
            creature_flags: h.creature_flags,
            state_flags: h.permanent_status_flags_state_ids,
        }),
        CreHeader::V22(_) => None,
    }
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
fn resolve_kit(game_data: &GameData, kit: u32) -> String {
    let swapped = ((kit & 0xFFFF) << 16) | (kit >> 16);
    if swapped == 0 {
        return String::new();
    }
    match ids_symbol(game_data, "kit", swapped as i32) {
        Some(s) if s == "TRUECLASS" => "Base Class".to_string(),
        Some(s) => pretty(&s, " "),
        None => String::new(),
    }
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
fn resolve_racial_enemy(game_data: &GameData, value: u8) -> String {
    if value == 0 {
        return String::new();
    }
    match ids_symbol(game_data, "race", value as i32) {
        Some(s) if s == "NO_RACE" || s == "ANYTHING" => String::new(),
        Some(s) => pretty(&s, " "),
        None => String::new(),
    }
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

/// The eight "Miscellaneous" checkboxes, in EEKeeper's column-major
/// order (left column top-to-bottom, then right column). Each entry
/// is `(label, creature-flags bit)`.
///
/// Confirmed bits (IESDP / GemRB `ie_stats.h`, cross-checked against
/// the reference saves): Exportable, Been In Party, No Corpse,
/// Permanent Corpse, plus the two Enhanced-Edition "disabled" bits —
/// `MC_NO_NIGHTMARE_MODS` (0x400000) and `MC_NO_TOOLTIPS` (0x800000).
/// `Uninterruptible` is bit 0 (the EE "damage doesn't interrupt
/// casting" reinterpretation) and `Identified` is bit 12; both are
/// best-effort and may render incorrectly for creatures that set
/// them.
pub const MISC_FLAGS: &[(&str, u32)] = &[
    // left column
    ("Exportable", 0x0000_0800),
    ("Been In Party", 0x0000_8000),
    ("Uninterruptible", 0x0000_0001),
    ("Nightmare Mode Disabled (Enhanced Edition)", 0x0040_0000),
    // right column
    ("Identified", 0x0000_1000),
    ("No Corpse", 0x0000_0002),
    ("Permanent Corpse", 0x0000_0004),
    ("Tooltip Disabled (Enhanced Edition)", 0x0080_0000),
];

#[cfg(test)]
mod tests {
    use super::pretty;

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
}
