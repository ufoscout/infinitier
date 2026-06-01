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
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::resource::ResourceType;
use infinitier_core::resource::cre::{Cre, CreHeader};

/// Resolved, display-ready characteristics for one creature.
#[derive(Debug, Default, Clone)]
pub struct CharData {
    pub gender: String,
    pub race: String,
    pub alignment: String,
    pub class: String,
    pub original_class: String,
    pub kit: String,
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
    /// Extract + resolve everything the tab paints. `npc_raw` is the
    /// GAM party-slot byte struct ([`ImportedGamNpc::raw`]); the kill
    /// statistics are read from it directly.
    pub fn resolve(cre: &Cre, npc_raw: &[u8], game_data: &GameData) -> CharData {
        let kill = kill_stats(npc_raw);
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
            kit: resolve_kit(game_data, raw.kit),
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

/// Read the kill statistics out of the GAM party-member struct. The
/// offsets are into the raw NPC record (BG/BG2/EE GAM party layout);
/// reads past the end of `raw` yield 0 so a short/foreign record
/// degrades gracefully.
fn kill_stats(raw: &[u8]) -> KillStats {
    let rd = |o: usize| -> u32 {
        raw.get(o..o + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0)
    };
    KillStats {
        strongest_name_strref: rd(0xE4),
        strongest_xp: rd(0xE8),
        chapter_kills_xp: rd(0xF8),
        chapter_kills: rd(0xFC),
        game_kills_xp: rd(0x100),
        game_kills: rd(0x104),
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
    match game_data.import_by_name_and_type(ids_file, ResourceType::Ids) {
        Ok(Some(ImportedResource::Ids(ids))) => ids
            .entries
            .iter()
            .find(|e| e.value == value)
            .map(|e| e.name.clone()),
        _ => None,
    }
}

/// `KIT.IDS` stores the kit in a word-swapped form versus the CRE
/// header dword (GemRB `GetActorBG`). Swap, then resolve. The
/// "true class" sentinel (no kit chosen) reads "Base Class" in
/// EEKeeper; an all-zero field means no kit at all.
fn resolve_kit(game_data: &GameData, kit: u32) -> String {
    let swapped = ((kit & 0xFFFF) << 16) | (kit >> 16);
    if swapped == 0 {
        return String::new();
    }
    match ids_symbol(game_data, "kit", swapped as i32) {
        Some(s) if s == "TRUECLASS" => "Base Class".to_string(),
        Some(s) => pretty(&s, " "),
        None => format!("0x{swapped:04X}"),
    }
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
                Some(first) => first.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(sep)
}

/// The "Miscellaneous" checkboxes, in EEKeeper's column-major order
/// (left column top-to-bottom, then right column). Each entry is
/// `(label, creature-flags bit)`.
///
/// The four corpse/party/export bits are confirmed against the
/// reference saves and IESDP/GemRB (`ie_stats.h`). The remaining
/// Enhanced-Edition-specific bits are best-effort — their exact bit
/// positions are not yet verified, so they may render incorrectly
/// for saves that set them. (Deliberately none map to bit 0x400000,
/// which the reference saves set on every party member without any
/// EEKeeper checkbox lighting up.)
pub const MISC_FLAGS: &[(&str, u32)] = &[
    // left column
    ("Exportable", 0x0000_0800),
    ("Been In Party", 0x0000_8000),
    ("Uninterruptible", 0x0001_0000),
    ("Nightmare Mode", 0x0004_0000),
    ("Disabled (Enhanced Edition)", 0x0010_0000),
    // right column
    ("Identified", 0x0000_1000),
    ("No Corpse", 0x0000_0002),
    ("Permanent Corpse", 0x0000_0004),
    ("ToMP Disabled", 0x0008_0000),
    ("Enhanced", 0x0020_0000),
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
