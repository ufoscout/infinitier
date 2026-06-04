//! Read-only extraction for the Miscellaneous tab.
//!
//! The "Other" box is all CRE-header fields (turn-undead level,
//! tracking skill/target, the five creature scripts, the death /
//! script-name variable, and the actor enumeration "Identifier"). The
//! three time boxes are derived from save-global / party-slot clocks:
//!
//! * **World Time** / **Game Time** — the GAM `game_time`, in
//!   game-seconds (1 hour = 300 s, 1 day = 7200 s).
//! * **Joined Party** — how much game time has elapsed since the
//!   member joined: `game_time` (game-seconds → ticks ×15) minus the
//!   party-slot `join_time` (already in ticks), formatted via
//!   108000 ticks/day · 4500 ticks/hour · 75 ticks/minute.

use infinitier_core::imported_resource::gam::{ImportedGam, ImportedGamNpc};
use infinitier_core::resource::cre::{Cre, CreHeader};
use infinitier_core::resource::gam::Dhm;

/// Everything the Miscellaneous tab paints.
#[derive(Default)]
pub struct MiscData {
    pub turn_undead: u8,
    pub tracking_skill: u8,
    pub tracking_target: String,
    /// Actor enumeration "Identifier": global enum in the low word,
    /// local-area enum in the high word.
    pub identifier: u32,
    /// Death variable / scripting name.
    pub script_name: String,
    pub override_script: String,
    pub class_script: String,
    pub race_script: String,
    pub general_script: String,
    pub default_script: String,
    pub world_time: Dhm,
    pub game_time: Dhm,
    pub joined_party: Dhm,
}

pub fn misc_data(cre: &Cre, gam: &ImportedGam, npc: &ImportedGamNpc) -> MiscData {
    let game_time = gam.header.game_time;
    // "Joined Party" is the game time elapsed since the member joined:
    // now minus their join timestamp. Computed in ticks so the join
    // time's sub-game-second precision isn't lost.
    let joined = game_time
        .to_ticks()
        .saturating_sub(npc.char_stats.join_time);
    let mut data = MiscData {
        world_time: game_time.dhm(),
        game_time: game_time.dhm(),
        joined_party: joined.dhm(),
        ..MiscData::default()
    };

    // The "Other" box is the classic V1.0 header layout (BG/BG2/EE).
    // Other engines keep the time boxes but leave these blank.
    if let CreHeader::V10(h) = &cre.header {
        data.turn_undead = h.turn_undead_level;
        data.tracking_skill = h.tracking_skill;
        data.tracking_target = h.tracking_target.clone();
        data.identifier = u32::from(h.global_actor_enumeration_value)
            | (u32::from(h.local_area_actor_enumeration_value) << 16);
        data.script_name = h.death_variable_set_sprite_is_deadvariable.clone();
        data.override_script = h.creature_script_override.clone();
        data.class_script = h.creature_script_class.clone();
        data.race_script = h.creature_script_race.clone();
        data.general_script = h.creature_script_general.clone();
        data.default_script = h.creature_script_default.clone();
    }
    data
}
