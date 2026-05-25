//! Per-version CRE header structs, mirroring the IESDP `cre_v*.htm`
//! field tables.
//!
//! Each `CreHeaderV*` struct mirrors the documented fixed-width
//! header of the corresponding CRE version. Padding gaps between
//! documented fields are surfaced as `_padding_NN: Vec<u8>` so the
//! whole header round-trips byte-for-byte regardless of how field
//! names get refined over time.

#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]
#![allow(non_snake_case)]
#![allow(clippy::all)]
#![allow(unused)]

use encoding_rs::WINDOWS_1252;

/// Decode a fixed-width resref-shaped byte slice via WINDOWS-1252,
/// stripping trailing NULs. Shared by every generated parser.
fn read_resref(bytes: &[u8]) -> String {
    let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    let (decoded, _, _) = WINDOWS_1252.decode(&bytes[..end]);
    decoded.into_owned()
}

/// Encode a string via WINDOWS-1252 into a fixed-width zero-padded
/// slot. Shared by every generated serializer.
fn write_resref(out: &mut [u8], s: &str) {
    let (encoded, _, _) = WINDOWS_1252.encode(s);
    let n = encoded.len().min(out.len());
    out[..n].copy_from_slice(&encoded[..n]);
}

// ============================================================
//  V1_0 — 126 fields, header = 724 B
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreHeaderV10 {
    /// 0x0000 (4 B): Signature ('CRE ')
    pub signature: Vec<u8>,
    /// 0x0004 (4 B): Version ('V1.0')
    pub version: Vec<u8>,
    /// 0x0008 (4 B): Long name
    pub long_name: u32,
    /// 0x000C (4 B): Short name (tooltip)
    pub short_name_tooltip: u32,
    /// 0x0010 (4 B): Creature flags bit 0 Show longname in tooltip bit 1: No corpse bit 2: Keep corpse bit 3...
    pub creature_flags: u32,
    /// 0x0014 (4 B): XP (gained for killing this creature)
    pub xp_gained_for_killing_this_creature: u32,
    /// 0x0018 (4 B): Creature Power Level (for summoning spells) / XP of the creature (for party members)
    pub creature_power_level_for_summoning_spells: u32,
    /// 0x001C (4 B): Gold carried
    pub gold_carried: u32,
    /// 0x0020 (4 B): Permanent status flags ( STATE.IDS ) bit 0 -> SLEEPING bit 1 -> BERSERK bit 2 -> PANIC ...
    pub permanent_status_flags_state_ids: u32,
    /// 0x0024 (2 B): Current Hit Points
    pub current_hit_points: u16,
    /// 0x0026 (2 B): Maximum Hit Points
    pub maximum_hit_points: u16,
    /// 0x0028 (4 B): Animation ID BGEE: Animation slots have been (for the most parts) externalised. Symboli...
    pub animation_id: u32,
    /// 0x002C (1 B): Metal Colour Index
    pub metal_colour_index: u8,
    /// 0x002D (1 B): Minor Colour Index
    pub minor_colour_index: u8,
    /// 0x002E (1 B): Major Colour Index
    pub major_colour_index: u8,
    /// 0x002F (1 B): Skin Colour Index
    pub skin_colour_index: u8,
    /// 0x0030 (1 B): Leather Colour Index
    pub leather_colour_index: u8,
    /// 0x0031 (1 B): Armor Colour Index
    pub armor_colour_index: u8,
    /// 0x0032 (1 B): Hair Colour Index
    pub hair_colour_index: u8,
    /// 0x0033 (1 B): Eff structure version 0 -> Version 1 EFF 1 -> Version 2 EFF
    pub eff_structure_version_0_version_1: u8,
    /// 0x0034 (8 B): Small Portrait (BMP)
    pub small_portrait_bmp: String,
    /// 0x003C (8 B): Large Portrait (PSTEE: BAM, Other games: BMP)
    pub large_portrait_pstee_bam_other_games: String,
    /// 0x0044 (1 B): Reputation (minimum value: 0)
    pub reputation: i8,
    /// 0x0045 (1 B): Hide In Shadows (base)
    pub hide_in_shadows_base: u8,
    /// 0x0046 (2 B): Armor Class (Natural)
    pub armor_class_natural: i16,
    /// 0x0048 (2 B): Armor Class (Effective)
    pub armor_class_effective: i16,
    /// 0x004A (2 B): Armor Class (Crushing Attacks Modifier)
    pub armor_class_crushing_attacks_modifier: i16,
    /// 0x004C (2 B): Armor Class (Missile Attacks Modifier)
    pub armor_class_missile_attacks_modifier: i16,
    /// 0x004E (2 B): Armor Class (Piercing Attacks Modifier)
    pub armor_class_piercing_attacks_modifier: i16,
    /// 0x0050 (2 B): Armor Class (Slashing Attacks Modifier)
    pub armor_class_slashing_attacks_modifier: i16,
    /// 0x0052 (1 B): THAC0 (1-25)
    pub thac0: u8,
    /// 0x0053 (1 B): Number of attacks (0-10)
    pub number_of_attacks: u8,
    /// 0x0054 (1 B): Save versus death (0-20)
    pub save_versus_death: u8,
    /// 0x0055 (1 B): Save versus wands (0-20)
    pub save_versus_wands: u8,
    /// 0x0056 (1 B): Save versus polymorph (0-20)
    pub save_versus_polymorph: u8,
    /// 0x0057 (1 B): Save versus breath attacks (0-20)
    pub save_versus_breath_attacks: u8,
    /// 0x0058 (1 B): Save versus spells (0-20)
    pub save_versus_spells: u8,
    /// 0x0059 (1 B): Resist fire (0-100)
    pub resist_fire: u8,
    /// 0x005A (1 B): Resist cold (0-100)
    pub resist_cold: u8,
    /// 0x005B (1 B): Resist electricity (0-100)
    pub resist_electricity: u8,
    /// 0x005C (1 B): Resist acid (0-100)
    pub resist_acid: u8,
    /// 0x005D (1 B): Resist magic (0-100)
    pub resist_magic: u8,
    /// 0x005E (1 B): Resist magic fire (0-100)
    pub resist_magic_fire: u8,
    /// 0x005F (1 B): Resist magic cold (0-100)
    pub resist_magic_cold: u8,
    /// 0x0060 (1 B): Resist slashing (0-100)
    pub resist_slashing: u8,
    /// 0x0061 (1 B): Resist crushing (0-100)
    pub resist_crushing: u8,
    /// 0x0062 (1 B): Resist piercing (0-100)
    pub resist_piercing: u8,
    /// 0x0063 (1 B): Resist missile (0-100)
    pub resist_missile: u8,
    /// 0x0064 (1 B): Detect illusion (minimum value : 0)
    pub detect_illusion: u8,
    /// 0x0065 (1 B): Set traps
    pub set_traps: u8,
    /// 0x0066 (1 B): Lore (0-100)*
    pub lore: u8,
    /// 0x0067 (1 B): Lockpicking (minimum value: 0)
    pub lockpicking: u8,
    /// 0x0068 (1 B): Move Silently (minimum value: 0)
    pub move_silently: u8,
    /// 0x0069 (1 B): Find/disarm traps (minimum value: 0)
    pub find_disarm_traps: u8,
    /// 0x006A (1 B): Pick Pockets (minimum value: 0)
    pub pick_pockets: u8,
    /// 0x006B (1 B): Fatigue (0-100)
    pub fatigue: u8,
    /// 0x006C (1 B): Intoxication (0-100)
    pub intoxication: u8,
    /// 0x006D (1 B): Luck
    pub luck: u8,
    /// 0x006E (1 B): BG1: Large swords proficiency Other games: Unused proficiency Note: Proficiencies are p...
    pub bg1_large_swords_proficiency_other_games: u8,
    /// 0x006F (1 B): BG1: Small swords proficiency BG2: Unused proficiency Note: Proficiencies are packed in...
    pub bg1_small_swords_proficiency: u8,
    /// 0x0070 (1 B): BG1: Bows proficiency BG2: Unused proficiency Note: Proficiencies are packed into 3-bit...
    pub bg1_bows_proficiency: u8,
    /// 0x0071 (1 B): BG1: Spears proficiency BG2: Unused proficiency Note: Proficiencies are packed into 3-b...
    pub bg1_spears_proficiency: u8,
    /// 0x0072 (1 B): BG1: Blunt proficiency BG2: Unused proficiency Note: Proficiencies are packed into 3-bi...
    pub bg1_blunt_proficiency: u8,
    /// 0x0073 (1 B): BG1: Spiked proficiency BG2: Unused proficiency Note: Proficiencies are packed into 3-b...
    pub bg1_spiked_proficiency: u8,
    /// 0x0074 (1 B): BG1: Axe proficiency BG2: Unused proficiency Note: Proficiencies are packed into 3-bit ...
    pub bg1_axe_proficiency: u8,
    /// 0x0075 (1 B): BG1: Missile proficiency BG2: Unused proficiency Note: Proficiencies are packed into 3-...
    pub bg1_missile_proficiency: u8,
    /// 0x0076 (1 B): Unused proficiency (Proficiencies are packed into 3-bit chunks for primary and secondar...
    pub unused_proficiency_proficiencies_are_packed_into: u8,
    /// 0x0077 (1 B): Unused proficiency (Proficiencies are packed into 3-bit chunks for primary and secondar...
    pub unused_proficiency_proficiencies_are_packed_into_2: u8,
    /// 0x0078 (1 B): Unused proficiency (Proficiencies are packed into 3-bit chunks for primary and secondar...
    pub unused_proficiency_proficiencies_are_packed_into_3: u8,
    /// 0x0079 (1 B): Unused proficiency (Proficiencies are packed into 3-bit chunks for primary and secondar...
    pub unused_proficiency_proficiencies_are_packed_into_4: u8,
    /// 0x007A (1 B): Unused proficiency (Proficiencies are packed into 3-bit chunks for primary and secondar...
    pub unused_proficiency_proficiencies_are_packed_into_5: u8,
    /// 0x007B (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1: u8,
    /// 0x007C (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1_2: u8,
    /// 0x007D (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1_3: u8,
    /// 0x007E (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1_4: u8,
    /// 0x007F (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1_5: u8,
    /// 0x0080 (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1_6: u8,
    /// 0x0081 (1 B): BG1, BG2: Unused proficiency (Proficiencies are packed into 3-bit chunks for primary an...
    pub bg1_7: u8,
    /// 0x0082 (1 B): Turn undead level
    pub turn_undead_level: u8,
    /// 0x0083 (1 B): Tracking skill (0-100)
    pub tracking_skill: u8,
    /// 0x0084 (32 B): Tracking target
    pub tracking_target: Vec<u8>,
    /// 0x00A4 (400 B): Strrefs pertaining to the character. Most are connected with the sound-set (see SOUNDOF...
    pub strrefs_pertaining_to_the_character_most: Vec<u8>,
    /// 0x0234 (1 B): Level first class Highest attained level in class (0-100). For dual/multi class charact...
    pub level_first_class_highest_attained_level: u8,
    /// 0x0235 (1 B): Level second class Highest attained level in class (0-100)
    pub level_second_class_highest_attained_level: u8,
    /// 0x0236 (1 B): Level third class Highest attained level in class (0-100)
    pub level_third_class_highest_attained_level: u8,
    /// 0x0237 (1 B): Sex ( GENDER.IDS ) - checkable via the SEX stat. EE only: determines casting sound pref...
    pub sex_gender_ids_checkable_via_the: u8,
    /// 0x0238 (1 B): Strength (1-25)
    pub strength: u8,
    /// 0x0239 (1 B): Strength % Bonus (0-100)
    pub strength_bonus: u8,
    /// 0x023A (1 B): Intelligence (1-25)
    pub intelligence: u8,
    /// 0x023B (1 B): Wisdom (1-25)
    pub wisdom: u8,
    /// 0x023C (1 B): Dexterity (1-25)
    pub dexterity: u8,
    /// 0x023D (1 B): Constitution (1-25)
    pub constitution: u8,
    /// 0x023E (1 B): Charisma (1-25)
    pub charisma: u8,
    /// 0x023F (1 B): Morale Default value is 10 (capped 0 &mdash; 20 ) It is unclear what increases/decrease...
    pub morale_default_value_is_10_capped: u8,
    /// 0x0240 (1 B): Morale break See here for further details.
    pub morale_break_see_here_for_further: u8,
    /// 0x0241 (1 B): Racial enemy ( RACE.IDS )
    pub racial_enemy_race_ids: u8,
    /// 0x0242 (2 B): Morale Recovery Time See here for further details.
    pub morale_recovery_time_see_here_for: u16,
    /// 0x0244 (4 B): Kit information : NONE 0x00000000 KIT_BARBARIAN 0x00004000 KIT_TRUECLASS 0x40000000 KIT...
    pub kit_information_none_0x00000000_kit_barbarian: u32,
    /// 0x0248 (8 B): Creature script - Override
    pub creature_script_override: String,
    /// 0x0250 (8 B): Creature script - Class
    pub creature_script_class: String,
    /// 0x0258 (8 B): Creature script - Race
    pub creature_script_race: String,
    /// 0x0260 (8 B): Creature script - General
    pub creature_script_general: String,
    /// 0x0268 (8 B): Creature script - Default
    pub creature_script_default: String,
    /// 0x0270 (1 B): Enemy-Ally ( EA.IDS )
    pub enemy_ally_ea_ids: u8,
    /// 0x0271 (1 B): General ( GENERAL.IDS )
    pub general_general_ids: u8,
    /// 0x0272 (1 B): Race ( RACE.IDS )
    pub race_race_ids: u8,
    /// 0x0273 (1 B): Class ( CLASS.IDS )
    pub class_class_ids: u8,
    /// 0x0274 (1 B): Specific ( SPECIFIC.IDS )
    pub specific_specific_ids: u8,
    /// 0x0275 (1 B): Gender ( GENDER.IDS ). Dictates the casting voice, and checked for the summoning cap.
    pub gender_gender_ids_dictates_the_casting: u8,
    /// 0x0276 (5 B): OBJECT.IDS references
    pub object_ids_references: Vec<u8>,
    /// 0x027B (1 B): Alignment ( ALIGNMEN.IDS )
    pub alignment_alignmen_ids: u8,
    /// 0x027C (2 B): Global actor enumeration value
    pub global_actor_enumeration_value: u16,
    /// 0x027E (2 B): Local (area) actor enumeration value
    pub local_area_actor_enumeration_value: u16,
    /// 0x0280 (32 B): Death Variable (set SPRITE_IS_DEADvariable on death)
    pub death_variable_set_sprite_is_deadvariable: Vec<u8>,
    /// 0x02A0 (4 B): Known spells offset
    pub known_spells_offset: u32,
    /// 0x02A4 (4 B): Known spells count
    pub known_spells_count: u32,
    /// 0x02A8 (4 B): Spell memorization info offset
    pub spell_memorization_info_offset: u32,
    /// 0x02AC (4 B): Spell memorization info entries count
    pub spell_memorization_info_entries_count: u32,
    /// 0x02B0 (4 B): Memorized spells offset
    pub memorized_spells_offset: u32,
    /// 0x02B4 (4 B): Memorized spells count
    pub memorized_spells_count: u32,
    /// 0x02B8 (4 B): Offset to Item slots
    pub offset_to_item_slots: u32,
    /// 0x02BC (4 B): Offset to Items
    pub offset_to_items: u32,
    /// 0x02C0 (4 B): Count of Items
    pub count_of_items: u32,
    /// 0x02C4 (4 B): Offset to effects
    pub offset_to_effects: u32,
    /// 0x02C8 (4 B): Count of effects .
    pub count_of_effects: u32,
    /// 0x02CC (8 B): Dialog file
    pub dialog_file: String,
}

pub(crate) fn parse_header_v1_0(header: &[u8]) -> std::io::Result<CreHeaderV10> {
    debug_assert_eq!(header.len(), 724);
    let read_u8 = |o: usize| header[o];
    let read_i8 = |o: usize| header[o] as i8;
    let read_u16 = |o: usize| u16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_i16 = |o: usize| i16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_u32 = |o: usize| u32::from_le_bytes(header[o..o+4].try_into().unwrap());
    let read_i32 = |o: usize| i32::from_le_bytes(header[o..o+4].try_into().unwrap());
    Ok(CreHeaderV10 {
        signature: header[0x0000..0x0004].to_vec(),
        version: header[0x0004..0x0008].to_vec(),
        long_name: read_u32(0x0008),
        short_name_tooltip: read_u32(0x000C),
        creature_flags: read_u32(0x0010),
        xp_gained_for_killing_this_creature: read_u32(0x0014),
        creature_power_level_for_summoning_spells: read_u32(0x0018),
        gold_carried: read_u32(0x001C),
        permanent_status_flags_state_ids: read_u32(0x0020),
        current_hit_points: read_u16(0x0024),
        maximum_hit_points: read_u16(0x0026),
        animation_id: read_u32(0x0028),
        metal_colour_index: read_u8(0x002C),
        minor_colour_index: read_u8(0x002D),
        major_colour_index: read_u8(0x002E),
        skin_colour_index: read_u8(0x002F),
        leather_colour_index: read_u8(0x0030),
        armor_colour_index: read_u8(0x0031),
        hair_colour_index: read_u8(0x0032),
        eff_structure_version_0_version_1: read_u8(0x0033),
        small_portrait_bmp: read_resref(&header[0x0034..0x003C]),
        large_portrait_pstee_bam_other_games: read_resref(&header[0x003C..0x0044]),
        reputation: read_i8(0x0044),
        hide_in_shadows_base: read_u8(0x0045),
        armor_class_natural: read_i16(0x0046),
        armor_class_effective: read_i16(0x0048),
        armor_class_crushing_attacks_modifier: read_i16(0x004A),
        armor_class_missile_attacks_modifier: read_i16(0x004C),
        armor_class_piercing_attacks_modifier: read_i16(0x004E),
        armor_class_slashing_attacks_modifier: read_i16(0x0050),
        thac0: read_u8(0x0052),
        number_of_attacks: read_u8(0x0053),
        save_versus_death: read_u8(0x0054),
        save_versus_wands: read_u8(0x0055),
        save_versus_polymorph: read_u8(0x0056),
        save_versus_breath_attacks: read_u8(0x0057),
        save_versus_spells: read_u8(0x0058),
        resist_fire: read_u8(0x0059),
        resist_cold: read_u8(0x005A),
        resist_electricity: read_u8(0x005B),
        resist_acid: read_u8(0x005C),
        resist_magic: read_u8(0x005D),
        resist_magic_fire: read_u8(0x005E),
        resist_magic_cold: read_u8(0x005F),
        resist_slashing: read_u8(0x0060),
        resist_crushing: read_u8(0x0061),
        resist_piercing: read_u8(0x0062),
        resist_missile: read_u8(0x0063),
        detect_illusion: read_u8(0x0064),
        set_traps: read_u8(0x0065),
        lore: read_u8(0x0066),
        lockpicking: read_u8(0x0067),
        move_silently: read_u8(0x0068),
        find_disarm_traps: read_u8(0x0069),
        pick_pockets: read_u8(0x006A),
        fatigue: read_u8(0x006B),
        intoxication: read_u8(0x006C),
        luck: read_u8(0x006D),
        bg1_large_swords_proficiency_other_games: read_u8(0x006E),
        bg1_small_swords_proficiency: read_u8(0x006F),
        bg1_bows_proficiency: read_u8(0x0070),
        bg1_spears_proficiency: read_u8(0x0071),
        bg1_blunt_proficiency: read_u8(0x0072),
        bg1_spiked_proficiency: read_u8(0x0073),
        bg1_axe_proficiency: read_u8(0x0074),
        bg1_missile_proficiency: read_u8(0x0075),
        unused_proficiency_proficiencies_are_packed_into: read_u8(0x0076),
        unused_proficiency_proficiencies_are_packed_into_2: read_u8(0x0077),
        unused_proficiency_proficiencies_are_packed_into_3: read_u8(0x0078),
        unused_proficiency_proficiencies_are_packed_into_4: read_u8(0x0079),
        unused_proficiency_proficiencies_are_packed_into_5: read_u8(0x007A),
        bg1: read_u8(0x007B),
        bg1_2: read_u8(0x007C),
        bg1_3: read_u8(0x007D),
        bg1_4: read_u8(0x007E),
        bg1_5: read_u8(0x007F),
        bg1_6: read_u8(0x0080),
        bg1_7: read_u8(0x0081),
        turn_undead_level: read_u8(0x0082),
        tracking_skill: read_u8(0x0083),
        tracking_target: header[0x0084..0x00A4].to_vec(),
        strrefs_pertaining_to_the_character_most: header[0x00A4..0x0234].to_vec(),
        level_first_class_highest_attained_level: read_u8(0x0234),
        level_second_class_highest_attained_level: read_u8(0x0235),
        level_third_class_highest_attained_level: read_u8(0x0236),
        sex_gender_ids_checkable_via_the: read_u8(0x0237),
        strength: read_u8(0x0238),
        strength_bonus: read_u8(0x0239),
        intelligence: read_u8(0x023A),
        wisdom: read_u8(0x023B),
        dexterity: read_u8(0x023C),
        constitution: read_u8(0x023D),
        charisma: read_u8(0x023E),
        morale_default_value_is_10_capped: read_u8(0x023F),
        morale_break_see_here_for_further: read_u8(0x0240),
        racial_enemy_race_ids: read_u8(0x0241),
        morale_recovery_time_see_here_for: read_u16(0x0242),
        kit_information_none_0x00000000_kit_barbarian: read_u32(0x0244),
        creature_script_override: read_resref(&header[0x0248..0x0250]),
        creature_script_class: read_resref(&header[0x0250..0x0258]),
        creature_script_race: read_resref(&header[0x0258..0x0260]),
        creature_script_general: read_resref(&header[0x0260..0x0268]),
        creature_script_default: read_resref(&header[0x0268..0x0270]),
        enemy_ally_ea_ids: read_u8(0x0270),
        general_general_ids: read_u8(0x0271),
        race_race_ids: read_u8(0x0272),
        class_class_ids: read_u8(0x0273),
        specific_specific_ids: read_u8(0x0274),
        gender_gender_ids_dictates_the_casting: read_u8(0x0275),
        object_ids_references: header[0x0276..0x027B].to_vec(),
        alignment_alignmen_ids: read_u8(0x027B),
        global_actor_enumeration_value: read_u16(0x027C),
        local_area_actor_enumeration_value: read_u16(0x027E),
        death_variable_set_sprite_is_deadvariable: header[0x0280..0x02A0].to_vec(),
        known_spells_offset: read_u32(0x02A0),
        known_spells_count: read_u32(0x02A4),
        spell_memorization_info_offset: read_u32(0x02A8),
        spell_memorization_info_entries_count: read_u32(0x02AC),
        memorized_spells_offset: read_u32(0x02B0),
        memorized_spells_count: read_u32(0x02B4),
        offset_to_item_slots: read_u32(0x02B8),
        offset_to_items: read_u32(0x02BC),
        count_of_items: read_u32(0x02C0),
        offset_to_effects: read_u32(0x02C4),
        count_of_effects: read_u32(0x02C8),
        dialog_file: read_resref(&header[0x02CC..0x02D4]),
    })
}

pub(crate) fn serialize_header_v1_0(h: &CreHeaderV10) -> Vec<u8> {
    let mut buf = vec![0u8; 724];
    { let src = &h.signature; let n = src.len().min(4); buf[0x0000..0x0000+n].copy_from_slice(&src[..n]); }
    { let src = &h.version; let n = src.len().min(4); buf[0x0004..0x0004+n].copy_from_slice(&src[..n]); }
    buf[0x0008..0x000C].copy_from_slice(&h.long_name.to_le_bytes());
    buf[0x000C..0x0010].copy_from_slice(&h.short_name_tooltip.to_le_bytes());
    buf[0x0010..0x0014].copy_from_slice(&h.creature_flags.to_le_bytes());
    buf[0x0014..0x0018].copy_from_slice(&h.xp_gained_for_killing_this_creature.to_le_bytes());
    buf[0x0018..0x001C].copy_from_slice(&h.creature_power_level_for_summoning_spells.to_le_bytes());
    buf[0x001C..0x0020].copy_from_slice(&h.gold_carried.to_le_bytes());
    buf[0x0020..0x0024].copy_from_slice(&h.permanent_status_flags_state_ids.to_le_bytes());
    buf[0x0024..0x0026].copy_from_slice(&h.current_hit_points.to_le_bytes());
    buf[0x0026..0x0028].copy_from_slice(&h.maximum_hit_points.to_le_bytes());
    buf[0x0028..0x002C].copy_from_slice(&h.animation_id.to_le_bytes());
    buf[0x002C] = h.metal_colour_index;
    buf[0x002D] = h.minor_colour_index;
    buf[0x002E] = h.major_colour_index;
    buf[0x002F] = h.skin_colour_index;
    buf[0x0030] = h.leather_colour_index;
    buf[0x0031] = h.armor_colour_index;
    buf[0x0032] = h.hair_colour_index;
    buf[0x0033] = h.eff_structure_version_0_version_1;
    write_resref(&mut buf[0x0034..0x003C], &h.small_portrait_bmp);
    write_resref(&mut buf[0x003C..0x0044], &h.large_portrait_pstee_bam_other_games);
    buf[0x0044] = h.reputation as u8;
    buf[0x0045] = h.hide_in_shadows_base;
    buf[0x0046..0x0048].copy_from_slice(&h.armor_class_natural.to_le_bytes());
    buf[0x0048..0x004A].copy_from_slice(&h.armor_class_effective.to_le_bytes());
    buf[0x004A..0x004C].copy_from_slice(&h.armor_class_crushing_attacks_modifier.to_le_bytes());
    buf[0x004C..0x004E].copy_from_slice(&h.armor_class_missile_attacks_modifier.to_le_bytes());
    buf[0x004E..0x0050].copy_from_slice(&h.armor_class_piercing_attacks_modifier.to_le_bytes());
    buf[0x0050..0x0052].copy_from_slice(&h.armor_class_slashing_attacks_modifier.to_le_bytes());
    buf[0x0052] = h.thac0;
    buf[0x0053] = h.number_of_attacks;
    buf[0x0054] = h.save_versus_death;
    buf[0x0055] = h.save_versus_wands;
    buf[0x0056] = h.save_versus_polymorph;
    buf[0x0057] = h.save_versus_breath_attacks;
    buf[0x0058] = h.save_versus_spells;
    buf[0x0059] = h.resist_fire;
    buf[0x005A] = h.resist_cold;
    buf[0x005B] = h.resist_electricity;
    buf[0x005C] = h.resist_acid;
    buf[0x005D] = h.resist_magic;
    buf[0x005E] = h.resist_magic_fire;
    buf[0x005F] = h.resist_magic_cold;
    buf[0x0060] = h.resist_slashing;
    buf[0x0061] = h.resist_crushing;
    buf[0x0062] = h.resist_piercing;
    buf[0x0063] = h.resist_missile;
    buf[0x0064] = h.detect_illusion;
    buf[0x0065] = h.set_traps;
    buf[0x0066] = h.lore;
    buf[0x0067] = h.lockpicking;
    buf[0x0068] = h.move_silently;
    buf[0x0069] = h.find_disarm_traps;
    buf[0x006A] = h.pick_pockets;
    buf[0x006B] = h.fatigue;
    buf[0x006C] = h.intoxication;
    buf[0x006D] = h.luck;
    buf[0x006E] = h.bg1_large_swords_proficiency_other_games;
    buf[0x006F] = h.bg1_small_swords_proficiency;
    buf[0x0070] = h.bg1_bows_proficiency;
    buf[0x0071] = h.bg1_spears_proficiency;
    buf[0x0072] = h.bg1_blunt_proficiency;
    buf[0x0073] = h.bg1_spiked_proficiency;
    buf[0x0074] = h.bg1_axe_proficiency;
    buf[0x0075] = h.bg1_missile_proficiency;
    buf[0x0076] = h.unused_proficiency_proficiencies_are_packed_into;
    buf[0x0077] = h.unused_proficiency_proficiencies_are_packed_into_2;
    buf[0x0078] = h.unused_proficiency_proficiencies_are_packed_into_3;
    buf[0x0079] = h.unused_proficiency_proficiencies_are_packed_into_4;
    buf[0x007A] = h.unused_proficiency_proficiencies_are_packed_into_5;
    buf[0x007B] = h.bg1;
    buf[0x007C] = h.bg1_2;
    buf[0x007D] = h.bg1_3;
    buf[0x007E] = h.bg1_4;
    buf[0x007F] = h.bg1_5;
    buf[0x0080] = h.bg1_6;
    buf[0x0081] = h.bg1_7;
    buf[0x0082] = h.turn_undead_level;
    buf[0x0083] = h.tracking_skill;
    { let src = &h.tracking_target; let n = src.len().min(32); buf[0x0084..0x0084+n].copy_from_slice(&src[..n]); }
    { let src = &h.strrefs_pertaining_to_the_character_most; let n = src.len().min(400); buf[0x00A4..0x00A4+n].copy_from_slice(&src[..n]); }
    buf[0x0234] = h.level_first_class_highest_attained_level;
    buf[0x0235] = h.level_second_class_highest_attained_level;
    buf[0x0236] = h.level_third_class_highest_attained_level;
    buf[0x0237] = h.sex_gender_ids_checkable_via_the;
    buf[0x0238] = h.strength;
    buf[0x0239] = h.strength_bonus;
    buf[0x023A] = h.intelligence;
    buf[0x023B] = h.wisdom;
    buf[0x023C] = h.dexterity;
    buf[0x023D] = h.constitution;
    buf[0x023E] = h.charisma;
    buf[0x023F] = h.morale_default_value_is_10_capped;
    buf[0x0240] = h.morale_break_see_here_for_further;
    buf[0x0241] = h.racial_enemy_race_ids;
    buf[0x0242..0x0244].copy_from_slice(&h.morale_recovery_time_see_here_for.to_le_bytes());
    buf[0x0244..0x0248].copy_from_slice(&h.kit_information_none_0x00000000_kit_barbarian.to_le_bytes());
    write_resref(&mut buf[0x0248..0x0250], &h.creature_script_override);
    write_resref(&mut buf[0x0250..0x0258], &h.creature_script_class);
    write_resref(&mut buf[0x0258..0x0260], &h.creature_script_race);
    write_resref(&mut buf[0x0260..0x0268], &h.creature_script_general);
    write_resref(&mut buf[0x0268..0x0270], &h.creature_script_default);
    buf[0x0270] = h.enemy_ally_ea_ids;
    buf[0x0271] = h.general_general_ids;
    buf[0x0272] = h.race_race_ids;
    buf[0x0273] = h.class_class_ids;
    buf[0x0274] = h.specific_specific_ids;
    buf[0x0275] = h.gender_gender_ids_dictates_the_casting;
    { let src = &h.object_ids_references; let n = src.len().min(5); buf[0x0276..0x0276+n].copy_from_slice(&src[..n]); }
    buf[0x027B] = h.alignment_alignmen_ids;
    buf[0x027C..0x027E].copy_from_slice(&h.global_actor_enumeration_value.to_le_bytes());
    buf[0x027E..0x0280].copy_from_slice(&h.local_area_actor_enumeration_value.to_le_bytes());
    { let src = &h.death_variable_set_sprite_is_deadvariable; let n = src.len().min(32); buf[0x0280..0x0280+n].copy_from_slice(&src[..n]); }
    buf[0x02A0..0x02A4].copy_from_slice(&h.known_spells_offset.to_le_bytes());
    buf[0x02A4..0x02A8].copy_from_slice(&h.known_spells_count.to_le_bytes());
    buf[0x02A8..0x02AC].copy_from_slice(&h.spell_memorization_info_offset.to_le_bytes());
    buf[0x02AC..0x02B0].copy_from_slice(&h.spell_memorization_info_entries_count.to_le_bytes());
    buf[0x02B0..0x02B4].copy_from_slice(&h.memorized_spells_offset.to_le_bytes());
    buf[0x02B4..0x02B8].copy_from_slice(&h.memorized_spells_count.to_le_bytes());
    buf[0x02B8..0x02BC].copy_from_slice(&h.offset_to_item_slots.to_le_bytes());
    buf[0x02BC..0x02C0].copy_from_slice(&h.offset_to_items.to_le_bytes());
    buf[0x02C0..0x02C4].copy_from_slice(&h.count_of_items.to_le_bytes());
    buf[0x02C4..0x02C8].copy_from_slice(&h.offset_to_effects.to_le_bytes());
    buf[0x02C8..0x02CC].copy_from_slice(&h.count_of_effects.to_le_bytes());
    write_resref(&mut buf[0x02CC..0x02D4], &h.dialog_file);
    buf
}

// ============================================================
//  V1_2 — 169 fields, header = 888 B
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreHeaderV12 {
    /// 0x0000 (4 B): Signature ('CRE ')
    pub signature: Vec<u8>,
    /// 0x0004 (4 B): Version ('V1.2')
    pub version: Vec<u8>,
    /// 0x0008 (4 B): Long name
    pub long_name: u32,
    /// 0x000C (4 B): Short name (tooltip)
    pub short_name_tooltip: u32,
    /// 0x0010 (4 B): Creature flags bit 0 Show longname in tooltip bit 1 No corpse bit 2 Keep corpse bit 3 O...
    pub creature_flags: u32,
    /// 0x0014 (4 B): XP (gained for killing this creature)
    pub xp_gained_for_killing_this_creature: u32,
    /// 0x0018 (4 B): Creature Power Level (for summoning spells) / XP of the creature (for party members)
    pub creature_power_level_for_summoning_spells: u32,
    /// 0x001C (4 B): Gold carried
    pub gold_carried: u32,
    /// 0x0020 (4 B): Permanent status flags (STATE.IDS)
    pub permanent_status_flags_state_ids: u32,
    /// 0x0024 (2 B): Current Hit Points
    pub current_hit_points: u16,
    /// 0x0026 (2 B): Maximum Hit Points
    pub maximum_hit_points: u16,
    /// 0x0028 (4 B): Animation ID (ANIMATE.IDS) There is some structure to the ordering of these entries. BA...
    pub animation_id_animate_ids: u32,
    /// 0x002C (1 B): Metal Colour Index (BG1 animations)
    pub metal_colour_index_bg1_animations: u8,
    /// 0x002D (1 B): Minor Colour Index (BG1 animations)
    pub minor_colour_index_bg1_animations: u8,
    /// 0x002E (1 B): Major Colour Index (BG1 animations)
    pub major_colour_index_bg1_animations: u8,
    /// 0x002F (1 B): Skin Colour Index (BG1 animations)
    pub skin_colour_index_bg1_animations: u8,
    /// 0x0030 (1 B): Leather Colour Index (BG1 animations)
    pub leather_colour_index_bg1_animations: u8,
    /// 0x0031 (1 B): Armor Colour Index (BG1 animations)
    pub armor_colour_index_bg1_animations: u8,
    /// 0x0032 (1 B): Hair Colour Index (BG1 animations)
    pub hair_colour_index_bg1_animations: u8,
    /// 0x0033 (1 B): EFF structure version 0: Version 1 EFF 1: Version 2 EFF
    pub eff_structure_version_0_version_1: u8,
    /// 0x0034 (8 B): Small Portrait (BMP)
    pub small_portrait_bmp: String,
    /// 0x003C (8 B): Large Portrait (BMP)
    pub large_portrait_bmp: String,
    /// 0x0044 (2 B): Reputation (minimum value: 0)
    pub reputation: i16,
    /// 0x0046 (2 B): Armor Class (Natural)
    pub armor_class_natural: i16,
    /// 0x0048 (2 B): Armor Class (Effective)
    pub armor_class_effective: i16,
    /// 0x004A (2 B): Armor Class (Crushing Attacks Modifier)
    pub armor_class_crushing_attacks_modifier: i16,
    /// 0x004C (2 B): Armor Class (Missile Attacks Modifier)
    pub armor_class_missile_attacks_modifier: i16,
    /// 0x004E (2 B): Armor Class (Piercing Attacks Modifier)
    pub armor_class_piercing_attacks_modifier: i16,
    /// 0x0050 (2 B): Armor Class (Slashing Attacks Modifier)
    pub armor_class_slashing_attacks_modifier: i16,
    /// 0x0052 (1 B): THAC0 (1-25)
    pub thac0: u8,
    /// 0x0053 (1 B): Number of attacks (0-10)
    pub number_of_attacks: u8,
    /// 0x0054 (1 B): Save versus death (0-20)
    pub save_versus_death: u8,
    /// 0x0055 (1 B): Save versus wands (0-20)
    pub save_versus_wands: u8,
    /// 0x0056 (1 B): Save versus polymorph (0-20)
    pub save_versus_polymorph: u8,
    /// 0x0057 (1 B): Save versus breath attacks (0-20)
    pub save_versus_breath_attacks: u8,
    /// 0x0058 (1 B): Save versus spells (0-20)
    pub save_versus_spells: u8,
    /// 0x0059 (1 B): Resist fire (0-100)
    pub resist_fire: u8,
    /// 0x005A (1 B): Resist cold (0-100)
    pub resist_cold: u8,
    /// 0x005B (1 B): Resist electricity (0-100)
    pub resist_electricity: u8,
    /// 0x005C (1 B): Resist acid (0-100)
    pub resist_acid: u8,
    /// 0x005D (1 B): Resist magic (0-100)
    pub resist_magic: u8,
    /// 0x005E (1 B): Resist magic fire (0-100)
    pub resist_magic_fire: u8,
    /// 0x005F (1 B): Resist magic cold (0-100)
    pub resist_magic_cold: u8,
    /// 0x0060 (1 B): Resist slashing (0-100)
    pub resist_slashing: u8,
    /// 0x0061 (1 B): Resist crushing (0-100)
    pub resist_crushing: u8,
    /// 0x0062 (1 B): Resist piercing (0-100)
    pub resist_piercing: u8,
    /// 0x0063 (1 B): Resist missile (0-100)
    pub resist_missile: u8,
    /// 0x0064 (1 B): Detect illusion (minimum value : 0)
    pub detect_illusion: u8,
    /// 0x0065 (1 B): Set traps
    pub set_traps: u8,
    /// 0x0066 (1 B): Lore (0-100)*
    pub lore: u8,
    /// 0x0067 (1 B): Lockpicking (minimum value: 0)
    pub lockpicking: u8,
    /// 0x0068 (1 B): Stealth (minimum value: 0)
    pub stealth: u8,
    /// 0x0069 (1 B): Find/disarm traps (minimum value: 0)
    pub find_disarm_traps: u8,
    /// 0x006A (1 B): Pick pockets (minimum value: 0)
    pub pick_pockets: u8,
    /// 0x006B (1 B): Fatigue (0-100)
    pub fatigue: u8,
    /// 0x006C (1 B): Intoxication (0-100)
    pub intoxication: u8,
    /// 0x006D (1 B): Luck
    pub luck: u8,
    /// 0x006E (1 B): Fist proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and s...
    pub fist_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x006F (1 B): Edged proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and ...
    pub edged_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0070 (1 B): Hammer proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and...
    pub hammer_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0071 (1 B): Axe proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and se...
    pub axe_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0072 (1 B): Club proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and s...
    pub club_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0073 (1 B): Bow proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and se...
    pub bow_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0074 (1 B): Unused Proficiency Slots
    pub unused_proficiency_slots: u8,
    /// 0x0075 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot: u8,
    /// 0x0076 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_2: Vec<u8>,
    /// 0x0077 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_3: Vec<u8>,
    /// 0x0078 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_4: Vec<u8>,
    /// 0x0079 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_5: Vec<u8>,
    /// 0x007A (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_6: Vec<u8>,
    /// 0x007B (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_7: Vec<u8>,
    /// 0x007C (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_8: Vec<u8>,
    /// 0x007D (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_9: Vec<u8>,
    /// 0x007E (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_10: Vec<u8>,
    /// 0x007F (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_11: Vec<u8>,
    /// 0x0080 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_12: Vec<u8>,
    /// 0x0081 (1 B): Unused Proficiency Slot
    pub unused_proficiency_slot_13: Vec<u8>,
    /// 0x0082 (1 B): Turn undead level
    pub turn_undead_level: Vec<u8>,
    /// 0x0083 (1 B): Tracking skill (0-100)
    pub tracking_skill: u8,
    /// 0x0084 (32 B): Tracking target
    pub tracking_target: Vec<u8>,
    /// 0x00A4 (400 B): Strrefs pertaining to the character. Most are connected with the sound-set (see SOUNDOF...
    pub strrefs_pertaining_to_the_character_most: Vec<u8>,
    /// 0x0234 (1 B): Highest attained level in class (0-100). For dual/multi class characters, the levels fo...
    pub highest_attained_level_in_class: u8,
    /// 0x0235 (1 B): Highest attained level in class (0-100)
    pub highest_attained_level_in_class_2: u8,
    /// 0x0236 (1 B): Highest attained level in class (0-100)
    pub highest_attained_level_in_class_3: u8,
    /// 0x0237 (1 B): Sex (from gender.ids) - not changed by effects
    pub sex_from_gender_ids_not_changed: u8,
    /// 0x0238 (1 B): Strength (1-25)
    pub strength: u8,
    /// 0x0239 (1 B): Strength % Bonus (0-100)
    pub strength_bonus: u8,
    /// 0x023A (1 B): Intelligence (1-25)
    pub intelligence: u8,
    /// 0x023B (1 B): Wisdom (1-25)
    pub wisdom: u8,
    /// 0x023C (1 B): Dexterity (1-25)
    pub dexterity: u8,
    /// 0x023D (1 B): Constitution (1-25)
    pub constitution: u8,
    /// 0x023E (1 B): Charisma (1-25)
    pub charisma: u8,
    /// 0x023F (1 B): Morale
    pub morale: u8,
    /// 0x0240 (1 B): Morale break
    pub morale_break: u8,
    /// 0x0241 (1 B): Racial enemy (RACE.IDS)
    pub racial_enemy_race_ids: u8,
    /// 0x0242 (2 B): Morale Recovery Time
    pub morale_recovery_time: u16,
    /// 0x0244 (4 B): Kit information NONE 0x00000000 ABJURER 0x00400000 CONJURER 0x00800000 DIVINER 0x010000...
    pub kit_information_none_0x00000000_abjurer_0x00400000: u32,
    /// 0x0248 (8 B): Creature script - Override
    pub creature_script_override: String,
    /// 0x0250 (8 B): Creature script - Class
    pub creature_script_class: String,
    /// 0x0258 (8 B): Creature script - Race
    pub creature_script_race: String,
    /// 0x0260 (8 B): Creature script - General
    pub creature_script_general: String,
    /// 0x0268 (8 B): Creature script - Default
    pub creature_script_default: String,
    /// 0x0270 (36 B): Unknown
    pub unknown: Vec<u8>,
    /// 0x0294 (4 B): Offset to overlay section
    pub offset_to_overlay_section: u32,
    /// 0x0298 (4 B): Size of overlay section
    pub size_of_overlay_section: u32,
    /// 0x029C (4 B): XP (Secondary class)
    pub xp_secondary_class: u32,
    /// 0x02A0 (4 B): XP (Tertiary class)
    pub xp_tertiary_class: u32,
    /// 0x02A4 (2 B): Internal 0
    pub internal_0: u16,
    /// 0x02A6 (2 B): Internal 1
    pub internal_1: u16,
    /// 0x02A8 (2 B): Internal 2
    pub internal_2: u16,
    /// 0x02AA (2 B): Internal 3
    pub internal_3: u16,
    /// 0x02AC (2 B): Internal 4
    pub internal_4: u16,
    /// 0x02AE (2 B): Internal 5
    pub internal_5: u16,
    /// 0x02B0 (2 B): Internal 6
    pub internal_6: u16,
    /// 0x02B2 (2 B): Internal 7
    pub internal_7: u16,
    /// 0x02B4 (2 B): Internal 8
    pub internal_8: u16,
    /// 0x02B6 (2 B): Internal 9
    pub internal_9: u16,
    /// 0x02B8 (1 B): GOOD variable increment value
    pub good_variable_increment_value: u8,
    /// 0x02B9 (1 B): LAW variable increment value
    pub law_variable_increment_value: u8,
    /// 0x02BA (1 B): LADY variable increment value
    pub lady_variable_increment_value: u8,
    /// 0x02BB (1 B): MURDER variable increment value
    pub murder_variable_increment_value: u8,
    /// 0x02BC (32 B): Monstrous Compendium Entry
    pub monstrous_compendium_entry: Vec<u8>,
    /// 0x02DC (1 B): Dialog Activation Range
    pub dialog_activation_range: u8,
    /// 0x02DD (1 B): Selection circle size
    pub selection_circle_size: u8,
    /// 0x02DE (1 B): Unknown
    pub unknown_2: u8,
    /// 0x02DF (1 B): Number of Colours
    pub number_of_colours: u8,
    /// 0x02E0 (4 B): Attribute flags bit 0: Unused bit 1: Transparent bit 2: Unused bit 3: Unused bit 4: Inc...
    pub attribute_flags: u32,
    /// 0x02E4 (2 B): Colour 1 (Clownclr.ids)
    pub colour_1_clownclr_ids: u16,
    /// 0x02E6 (2 B): Colour 2 (Clownclr.ids)
    pub colour_2_clownclr_ids: u16,
    /// 0x02E8 (2 B): Colour 3 (Clownclr.ids)
    pub colour_3_clownclr_ids: u16,
    /// 0x02EA (2 B): Colour 4 (Clownclr.ids)
    pub colour_4_clownclr_ids: u16,
    /// 0x02EC (2 B): Colour 5 (Clownclr.ids)
    pub colour_5_clownclr_ids: u16,
    /// 0x02EE (2 B): Colour 6 (Clownclr.ids)
    pub colour_6_clownclr_ids: u16,
    /// 0x02F0 (2 B): Colour 7 (Clownclr.ids)
    pub colour_7_clownclr_ids: u16,
    /// 0x02F2 (3 B): Related to colours
    pub related_to_colours: Vec<u8>,
    /// 0x02F5 (1 B): Colour Placement 1 These fields are bitfields: bit 0: Plasma — shift palette color entr...
    pub colour_placement_1_these_fields_are: u8,
    /// 0x02F6 (1 B): Colour Placement 2
    pub colour_placement_2: u8,
    /// 0x02F7 (1 B): Colour Placement 3
    pub colour_placement_3: u8,
    /// 0x02F8 (1 B): Colour Placement 4
    pub colour_placement_4: u8,
    /// 0x02F9 (1 B): Colour Placement 5
    pub colour_placement_5: u8,
    /// 0x02FA (1 B): Colour Placement 6
    pub colour_placement_6: u8,
    /// 0x02FB (1 B): Colour Placement 7
    pub colour_placement_7: u8,
    /// 0x02FC (21 B): Unknown
    pub unknown_3: Vec<u8>,
    /// 0x0311 (1 B): Species (RACE.IDS)
    pub species_race_ids: u8,
    /// 0x0312 (1 B): Team (TEAM.IDS)
    pub team_team_ids: u8,
    /// 0x0313 (1 B): Faction (FACTION.IDS)
    pub faction_faction_ids: u8,
    /// 0x0314 (1 B): Enemy-Ally (EA.IDS)
    pub enemy_ally_ea_ids: u8,
    /// 0x0315 (1 B): General (GENERAL.IDS)
    pub general_general_ids: u8,
    /// 0x0316 (1 B): Race (RACE.IDS)
    pub race_race_ids: u8,
    /// 0x0317 (1 B): Class (CLASS.IDS)
    pub class_class_ids: u8,
    /// 0x0318 (1 B): Specific (SPECIFIC.IDS)
    pub specific_specific_ids: u8,
    /// 0x0319 (1 B): Gender (GENDER.IDS)
    pub gender_gender_ids: u8,
    /// 0x031A (5 B): OBJECT.IDS references
    pub object_ids_references: Vec<u8>,
    /// 0x031F (1 B): Alignment (ALIGNMEN.IDS)
    pub alignment_alignmen_ids: u8,
    /// 0x0320 (2 B): Global actor enumeration value
    pub global_actor_enumeration_value: u16,
    /// 0x0322 (2 B): Local (area) actor enumeration value
    pub local_area_actor_enumeration_value: u16,
    /// 0x0324 (32 B): Death Variable (set SPRITE_IS_DEADvariable on death)
    pub death_variable_set_sprite_is_deadvariable: Vec<u8>,
    /// 0x0344 (4 B): Known spells offset
    pub known_spells_offset: u32,
    /// 0x0348 (4 B): Known spells count
    pub known_spells_count: u32,
    /// 0x034C (4 B): Spell memorization info offset
    pub spell_memorization_info_offset: u32,
    /// 0x0350 (4 B): Spell memorization info entries count
    pub spell_memorization_info_entries_count: u32,
    /// 0x0354 (4 B): Memorized spells offset
    pub memorized_spells_offset: u32,
    /// 0x0358 (4 B): Memorized spells count
    pub memorized_spells_count: u32,
    /// 0x035C (4 B): Item slots offset
    pub item_slots_offset: u32,
    /// 0x0360 (4 B): Items offset
    pub items_offset: u32,
    /// 0x0364 (4 B): Items count
    pub items_count: u32,
    /// 0x0368 (4 B): Offset to effects
    pub offset_to_effects: u32,
    /// 0x036C (4 B): Count to effects
    pub count_to_effects: u32,
    /// 0x0370 (8 B): Dialog file
    pub dialog_file: String,
}

pub(crate) fn parse_header_v1_2(header: &[u8]) -> std::io::Result<CreHeaderV12> {
    debug_assert_eq!(header.len(), 888);
    let read_u8 = |o: usize| header[o];
    let read_i8 = |o: usize| header[o] as i8;
    let read_u16 = |o: usize| u16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_i16 = |o: usize| i16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_u32 = |o: usize| u32::from_le_bytes(header[o..o+4].try_into().unwrap());
    let read_i32 = |o: usize| i32::from_le_bytes(header[o..o+4].try_into().unwrap());
    Ok(CreHeaderV12 {
        signature: header[0x0000..0x0004].to_vec(),
        version: header[0x0004..0x0008].to_vec(),
        long_name: read_u32(0x0008),
        short_name_tooltip: read_u32(0x000C),
        creature_flags: read_u32(0x0010),
        xp_gained_for_killing_this_creature: read_u32(0x0014),
        creature_power_level_for_summoning_spells: read_u32(0x0018),
        gold_carried: read_u32(0x001C),
        permanent_status_flags_state_ids: read_u32(0x0020),
        current_hit_points: read_u16(0x0024),
        maximum_hit_points: read_u16(0x0026),
        animation_id_animate_ids: read_u32(0x0028),
        metal_colour_index_bg1_animations: read_u8(0x002C),
        minor_colour_index_bg1_animations: read_u8(0x002D),
        major_colour_index_bg1_animations: read_u8(0x002E),
        skin_colour_index_bg1_animations: read_u8(0x002F),
        leather_colour_index_bg1_animations: read_u8(0x0030),
        armor_colour_index_bg1_animations: read_u8(0x0031),
        hair_colour_index_bg1_animations: read_u8(0x0032),
        eff_structure_version_0_version_1: read_u8(0x0033),
        small_portrait_bmp: read_resref(&header[0x0034..0x003C]),
        large_portrait_bmp: read_resref(&header[0x003C..0x0044]),
        reputation: read_i16(0x0044),
        armor_class_natural: read_i16(0x0046),
        armor_class_effective: read_i16(0x0048),
        armor_class_crushing_attacks_modifier: read_i16(0x004A),
        armor_class_missile_attacks_modifier: read_i16(0x004C),
        armor_class_piercing_attacks_modifier: read_i16(0x004E),
        armor_class_slashing_attacks_modifier: read_i16(0x0050),
        thac0: read_u8(0x0052),
        number_of_attacks: read_u8(0x0053),
        save_versus_death: read_u8(0x0054),
        save_versus_wands: read_u8(0x0055),
        save_versus_polymorph: read_u8(0x0056),
        save_versus_breath_attacks: read_u8(0x0057),
        save_versus_spells: read_u8(0x0058),
        resist_fire: read_u8(0x0059),
        resist_cold: read_u8(0x005A),
        resist_electricity: read_u8(0x005B),
        resist_acid: read_u8(0x005C),
        resist_magic: read_u8(0x005D),
        resist_magic_fire: read_u8(0x005E),
        resist_magic_cold: read_u8(0x005F),
        resist_slashing: read_u8(0x0060),
        resist_crushing: read_u8(0x0061),
        resist_piercing: read_u8(0x0062),
        resist_missile: read_u8(0x0063),
        detect_illusion: read_u8(0x0064),
        set_traps: read_u8(0x0065),
        lore: read_u8(0x0066),
        lockpicking: read_u8(0x0067),
        stealth: read_u8(0x0068),
        find_disarm_traps: read_u8(0x0069),
        pick_pockets: read_u8(0x006A),
        fatigue: read_u8(0x006B),
        intoxication: read_u8(0x006C),
        luck: read_u8(0x006D),
        fist_proficiency_proficiencies_maybe_be_packed: read_u8(0x006E),
        edged_proficiency_proficiencies_maybe_be_packed: read_u8(0x006F),
        hammer_proficiency_proficiencies_maybe_be_packed: read_u8(0x0070),
        axe_proficiency_proficiencies_maybe_be_packed: read_u8(0x0071),
        club_proficiency_proficiencies_maybe_be_packed: read_u8(0x0072),
        bow_proficiency_proficiencies_maybe_be_packed: read_u8(0x0073),
        unused_proficiency_slots: read_u8(0x0074),
        unused_proficiency_slot: read_u8(0x0075),
        unused_proficiency_slot_2: header[0x0076..0x0077].to_vec(),
        unused_proficiency_slot_3: header[0x0077..0x0078].to_vec(),
        unused_proficiency_slot_4: header[0x0078..0x0079].to_vec(),
        unused_proficiency_slot_5: header[0x0079..0x007A].to_vec(),
        unused_proficiency_slot_6: header[0x007A..0x007B].to_vec(),
        unused_proficiency_slot_7: header[0x007B..0x007C].to_vec(),
        unused_proficiency_slot_8: header[0x007C..0x007D].to_vec(),
        unused_proficiency_slot_9: header[0x007D..0x007E].to_vec(),
        unused_proficiency_slot_10: header[0x007E..0x007F].to_vec(),
        unused_proficiency_slot_11: header[0x007F..0x0080].to_vec(),
        unused_proficiency_slot_12: header[0x0080..0x0081].to_vec(),
        unused_proficiency_slot_13: header[0x0081..0x0082].to_vec(),
        turn_undead_level: header[0x0082..0x0083].to_vec(),
        tracking_skill: read_u8(0x0083),
        tracking_target: header[0x0084..0x00A4].to_vec(),
        strrefs_pertaining_to_the_character_most: header[0x00A4..0x0234].to_vec(),
        highest_attained_level_in_class: read_u8(0x0234),
        highest_attained_level_in_class_2: read_u8(0x0235),
        highest_attained_level_in_class_3: read_u8(0x0236),
        sex_from_gender_ids_not_changed: read_u8(0x0237),
        strength: read_u8(0x0238),
        strength_bonus: read_u8(0x0239),
        intelligence: read_u8(0x023A),
        wisdom: read_u8(0x023B),
        dexterity: read_u8(0x023C),
        constitution: read_u8(0x023D),
        charisma: read_u8(0x023E),
        morale: read_u8(0x023F),
        morale_break: read_u8(0x0240),
        racial_enemy_race_ids: read_u8(0x0241),
        morale_recovery_time: read_u16(0x0242),
        kit_information_none_0x00000000_abjurer_0x00400000: read_u32(0x0244),
        creature_script_override: read_resref(&header[0x0248..0x0250]),
        creature_script_class: read_resref(&header[0x0250..0x0258]),
        creature_script_race: read_resref(&header[0x0258..0x0260]),
        creature_script_general: read_resref(&header[0x0260..0x0268]),
        creature_script_default: read_resref(&header[0x0268..0x0270]),
        unknown: header[0x0270..0x0294].to_vec(),
        offset_to_overlay_section: read_u32(0x0294),
        size_of_overlay_section: read_u32(0x0298),
        xp_secondary_class: read_u32(0x029C),
        xp_tertiary_class: read_u32(0x02A0),
        internal_0: read_u16(0x02A4),
        internal_1: read_u16(0x02A6),
        internal_2: read_u16(0x02A8),
        internal_3: read_u16(0x02AA),
        internal_4: read_u16(0x02AC),
        internal_5: read_u16(0x02AE),
        internal_6: read_u16(0x02B0),
        internal_7: read_u16(0x02B2),
        internal_8: read_u16(0x02B4),
        internal_9: read_u16(0x02B6),
        good_variable_increment_value: read_u8(0x02B8),
        law_variable_increment_value: read_u8(0x02B9),
        lady_variable_increment_value: read_u8(0x02BA),
        murder_variable_increment_value: read_u8(0x02BB),
        monstrous_compendium_entry: header[0x02BC..0x02DC].to_vec(),
        dialog_activation_range: read_u8(0x02DC),
        selection_circle_size: read_u8(0x02DD),
        unknown_2: read_u8(0x02DE),
        number_of_colours: read_u8(0x02DF),
        attribute_flags: read_u32(0x02E0),
        colour_1_clownclr_ids: read_u16(0x02E4),
        colour_2_clownclr_ids: read_u16(0x02E6),
        colour_3_clownclr_ids: read_u16(0x02E8),
        colour_4_clownclr_ids: read_u16(0x02EA),
        colour_5_clownclr_ids: read_u16(0x02EC),
        colour_6_clownclr_ids: read_u16(0x02EE),
        colour_7_clownclr_ids: read_u16(0x02F0),
        related_to_colours: header[0x02F2..0x02F5].to_vec(),
        colour_placement_1_these_fields_are: read_u8(0x02F5),
        colour_placement_2: read_u8(0x02F6),
        colour_placement_3: read_u8(0x02F7),
        colour_placement_4: read_u8(0x02F8),
        colour_placement_5: read_u8(0x02F9),
        colour_placement_6: read_u8(0x02FA),
        colour_placement_7: read_u8(0x02FB),
        unknown_3: header[0x02FC..0x0311].to_vec(),
        species_race_ids: read_u8(0x0311),
        team_team_ids: read_u8(0x0312),
        faction_faction_ids: read_u8(0x0313),
        enemy_ally_ea_ids: read_u8(0x0314),
        general_general_ids: read_u8(0x0315),
        race_race_ids: read_u8(0x0316),
        class_class_ids: read_u8(0x0317),
        specific_specific_ids: read_u8(0x0318),
        gender_gender_ids: read_u8(0x0319),
        object_ids_references: header[0x031A..0x031F].to_vec(),
        alignment_alignmen_ids: read_u8(0x031F),
        global_actor_enumeration_value: read_u16(0x0320),
        local_area_actor_enumeration_value: read_u16(0x0322),
        death_variable_set_sprite_is_deadvariable: header[0x0324..0x0344].to_vec(),
        known_spells_offset: read_u32(0x0344),
        known_spells_count: read_u32(0x0348),
        spell_memorization_info_offset: read_u32(0x034C),
        spell_memorization_info_entries_count: read_u32(0x0350),
        memorized_spells_offset: read_u32(0x0354),
        memorized_spells_count: read_u32(0x0358),
        item_slots_offset: read_u32(0x035C),
        items_offset: read_u32(0x0360),
        items_count: read_u32(0x0364),
        offset_to_effects: read_u32(0x0368),
        count_to_effects: read_u32(0x036C),
        dialog_file: read_resref(&header[0x0370..0x0378]),
    })
}

pub(crate) fn serialize_header_v1_2(h: &CreHeaderV12) -> Vec<u8> {
    let mut buf = vec![0u8; 888];
    { let src = &h.signature; let n = src.len().min(4); buf[0x0000..0x0000+n].copy_from_slice(&src[..n]); }
    { let src = &h.version; let n = src.len().min(4); buf[0x0004..0x0004+n].copy_from_slice(&src[..n]); }
    buf[0x0008..0x000C].copy_from_slice(&h.long_name.to_le_bytes());
    buf[0x000C..0x0010].copy_from_slice(&h.short_name_tooltip.to_le_bytes());
    buf[0x0010..0x0014].copy_from_slice(&h.creature_flags.to_le_bytes());
    buf[0x0014..0x0018].copy_from_slice(&h.xp_gained_for_killing_this_creature.to_le_bytes());
    buf[0x0018..0x001C].copy_from_slice(&h.creature_power_level_for_summoning_spells.to_le_bytes());
    buf[0x001C..0x0020].copy_from_slice(&h.gold_carried.to_le_bytes());
    buf[0x0020..0x0024].copy_from_slice(&h.permanent_status_flags_state_ids.to_le_bytes());
    buf[0x0024..0x0026].copy_from_slice(&h.current_hit_points.to_le_bytes());
    buf[0x0026..0x0028].copy_from_slice(&h.maximum_hit_points.to_le_bytes());
    buf[0x0028..0x002C].copy_from_slice(&h.animation_id_animate_ids.to_le_bytes());
    buf[0x002C] = h.metal_colour_index_bg1_animations;
    buf[0x002D] = h.minor_colour_index_bg1_animations;
    buf[0x002E] = h.major_colour_index_bg1_animations;
    buf[0x002F] = h.skin_colour_index_bg1_animations;
    buf[0x0030] = h.leather_colour_index_bg1_animations;
    buf[0x0031] = h.armor_colour_index_bg1_animations;
    buf[0x0032] = h.hair_colour_index_bg1_animations;
    buf[0x0033] = h.eff_structure_version_0_version_1;
    write_resref(&mut buf[0x0034..0x003C], &h.small_portrait_bmp);
    write_resref(&mut buf[0x003C..0x0044], &h.large_portrait_bmp);
    buf[0x0044..0x0046].copy_from_slice(&h.reputation.to_le_bytes());
    buf[0x0046..0x0048].copy_from_slice(&h.armor_class_natural.to_le_bytes());
    buf[0x0048..0x004A].copy_from_slice(&h.armor_class_effective.to_le_bytes());
    buf[0x004A..0x004C].copy_from_slice(&h.armor_class_crushing_attacks_modifier.to_le_bytes());
    buf[0x004C..0x004E].copy_from_slice(&h.armor_class_missile_attacks_modifier.to_le_bytes());
    buf[0x004E..0x0050].copy_from_slice(&h.armor_class_piercing_attacks_modifier.to_le_bytes());
    buf[0x0050..0x0052].copy_from_slice(&h.armor_class_slashing_attacks_modifier.to_le_bytes());
    buf[0x0052] = h.thac0;
    buf[0x0053] = h.number_of_attacks;
    buf[0x0054] = h.save_versus_death;
    buf[0x0055] = h.save_versus_wands;
    buf[0x0056] = h.save_versus_polymorph;
    buf[0x0057] = h.save_versus_breath_attacks;
    buf[0x0058] = h.save_versus_spells;
    buf[0x0059] = h.resist_fire;
    buf[0x005A] = h.resist_cold;
    buf[0x005B] = h.resist_electricity;
    buf[0x005C] = h.resist_acid;
    buf[0x005D] = h.resist_magic;
    buf[0x005E] = h.resist_magic_fire;
    buf[0x005F] = h.resist_magic_cold;
    buf[0x0060] = h.resist_slashing;
    buf[0x0061] = h.resist_crushing;
    buf[0x0062] = h.resist_piercing;
    buf[0x0063] = h.resist_missile;
    buf[0x0064] = h.detect_illusion;
    buf[0x0065] = h.set_traps;
    buf[0x0066] = h.lore;
    buf[0x0067] = h.lockpicking;
    buf[0x0068] = h.stealth;
    buf[0x0069] = h.find_disarm_traps;
    buf[0x006A] = h.pick_pockets;
    buf[0x006B] = h.fatigue;
    buf[0x006C] = h.intoxication;
    buf[0x006D] = h.luck;
    buf[0x006E] = h.fist_proficiency_proficiencies_maybe_be_packed;
    buf[0x006F] = h.edged_proficiency_proficiencies_maybe_be_packed;
    buf[0x0070] = h.hammer_proficiency_proficiencies_maybe_be_packed;
    buf[0x0071] = h.axe_proficiency_proficiencies_maybe_be_packed;
    buf[0x0072] = h.club_proficiency_proficiencies_maybe_be_packed;
    buf[0x0073] = h.bow_proficiency_proficiencies_maybe_be_packed;
    buf[0x0074] = h.unused_proficiency_slots;
    buf[0x0075] = h.unused_proficiency_slot;
    { let src = &h.unused_proficiency_slot_2; let n = src.len().min(1); buf[0x0076..0x0076+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_3; let n = src.len().min(1); buf[0x0077..0x0077+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_4; let n = src.len().min(1); buf[0x0078..0x0078+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_5; let n = src.len().min(1); buf[0x0079..0x0079+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_6; let n = src.len().min(1); buf[0x007A..0x007A+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_7; let n = src.len().min(1); buf[0x007B..0x007B+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_8; let n = src.len().min(1); buf[0x007C..0x007C+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_9; let n = src.len().min(1); buf[0x007D..0x007D+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_10; let n = src.len().min(1); buf[0x007E..0x007E+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_11; let n = src.len().min(1); buf[0x007F..0x007F+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_12; let n = src.len().min(1); buf[0x0080..0x0080+n].copy_from_slice(&src[..n]); }
    { let src = &h.unused_proficiency_slot_13; let n = src.len().min(1); buf[0x0081..0x0081+n].copy_from_slice(&src[..n]); }
    { let src = &h.turn_undead_level; let n = src.len().min(1); buf[0x0082..0x0082+n].copy_from_slice(&src[..n]); }
    buf[0x0083] = h.tracking_skill;
    { let src = &h.tracking_target; let n = src.len().min(32); buf[0x0084..0x0084+n].copy_from_slice(&src[..n]); }
    { let src = &h.strrefs_pertaining_to_the_character_most; let n = src.len().min(400); buf[0x00A4..0x00A4+n].copy_from_slice(&src[..n]); }
    buf[0x0234] = h.highest_attained_level_in_class;
    buf[0x0235] = h.highest_attained_level_in_class_2;
    buf[0x0236] = h.highest_attained_level_in_class_3;
    buf[0x0237] = h.sex_from_gender_ids_not_changed;
    buf[0x0238] = h.strength;
    buf[0x0239] = h.strength_bonus;
    buf[0x023A] = h.intelligence;
    buf[0x023B] = h.wisdom;
    buf[0x023C] = h.dexterity;
    buf[0x023D] = h.constitution;
    buf[0x023E] = h.charisma;
    buf[0x023F] = h.morale;
    buf[0x0240] = h.morale_break;
    buf[0x0241] = h.racial_enemy_race_ids;
    buf[0x0242..0x0244].copy_from_slice(&h.morale_recovery_time.to_le_bytes());
    buf[0x0244..0x0248].copy_from_slice(&h.kit_information_none_0x00000000_abjurer_0x00400000.to_le_bytes());
    write_resref(&mut buf[0x0248..0x0250], &h.creature_script_override);
    write_resref(&mut buf[0x0250..0x0258], &h.creature_script_class);
    write_resref(&mut buf[0x0258..0x0260], &h.creature_script_race);
    write_resref(&mut buf[0x0260..0x0268], &h.creature_script_general);
    write_resref(&mut buf[0x0268..0x0270], &h.creature_script_default);
    { let src = &h.unknown; let n = src.len().min(36); buf[0x0270..0x0270+n].copy_from_slice(&src[..n]); }
    buf[0x0294..0x0298].copy_from_slice(&h.offset_to_overlay_section.to_le_bytes());
    buf[0x0298..0x029C].copy_from_slice(&h.size_of_overlay_section.to_le_bytes());
    buf[0x029C..0x02A0].copy_from_slice(&h.xp_secondary_class.to_le_bytes());
    buf[0x02A0..0x02A4].copy_from_slice(&h.xp_tertiary_class.to_le_bytes());
    buf[0x02A4..0x02A6].copy_from_slice(&h.internal_0.to_le_bytes());
    buf[0x02A6..0x02A8].copy_from_slice(&h.internal_1.to_le_bytes());
    buf[0x02A8..0x02AA].copy_from_slice(&h.internal_2.to_le_bytes());
    buf[0x02AA..0x02AC].copy_from_slice(&h.internal_3.to_le_bytes());
    buf[0x02AC..0x02AE].copy_from_slice(&h.internal_4.to_le_bytes());
    buf[0x02AE..0x02B0].copy_from_slice(&h.internal_5.to_le_bytes());
    buf[0x02B0..0x02B2].copy_from_slice(&h.internal_6.to_le_bytes());
    buf[0x02B2..0x02B4].copy_from_slice(&h.internal_7.to_le_bytes());
    buf[0x02B4..0x02B6].copy_from_slice(&h.internal_8.to_le_bytes());
    buf[0x02B6..0x02B8].copy_from_slice(&h.internal_9.to_le_bytes());
    buf[0x02B8] = h.good_variable_increment_value;
    buf[0x02B9] = h.law_variable_increment_value;
    buf[0x02BA] = h.lady_variable_increment_value;
    buf[0x02BB] = h.murder_variable_increment_value;
    { let src = &h.monstrous_compendium_entry; let n = src.len().min(32); buf[0x02BC..0x02BC+n].copy_from_slice(&src[..n]); }
    buf[0x02DC] = h.dialog_activation_range;
    buf[0x02DD] = h.selection_circle_size;
    buf[0x02DE] = h.unknown_2;
    buf[0x02DF] = h.number_of_colours;
    buf[0x02E0..0x02E4].copy_from_slice(&h.attribute_flags.to_le_bytes());
    buf[0x02E4..0x02E6].copy_from_slice(&h.colour_1_clownclr_ids.to_le_bytes());
    buf[0x02E6..0x02E8].copy_from_slice(&h.colour_2_clownclr_ids.to_le_bytes());
    buf[0x02E8..0x02EA].copy_from_slice(&h.colour_3_clownclr_ids.to_le_bytes());
    buf[0x02EA..0x02EC].copy_from_slice(&h.colour_4_clownclr_ids.to_le_bytes());
    buf[0x02EC..0x02EE].copy_from_slice(&h.colour_5_clownclr_ids.to_le_bytes());
    buf[0x02EE..0x02F0].copy_from_slice(&h.colour_6_clownclr_ids.to_le_bytes());
    buf[0x02F0..0x02F2].copy_from_slice(&h.colour_7_clownclr_ids.to_le_bytes());
    { let src = &h.related_to_colours; let n = src.len().min(3); buf[0x02F2..0x02F2+n].copy_from_slice(&src[..n]); }
    buf[0x02F5] = h.colour_placement_1_these_fields_are;
    buf[0x02F6] = h.colour_placement_2;
    buf[0x02F7] = h.colour_placement_3;
    buf[0x02F8] = h.colour_placement_4;
    buf[0x02F9] = h.colour_placement_5;
    buf[0x02FA] = h.colour_placement_6;
    buf[0x02FB] = h.colour_placement_7;
    { let src = &h.unknown_3; let n = src.len().min(21); buf[0x02FC..0x02FC+n].copy_from_slice(&src[..n]); }
    buf[0x0311] = h.species_race_ids;
    buf[0x0312] = h.team_team_ids;
    buf[0x0313] = h.faction_faction_ids;
    buf[0x0314] = h.enemy_ally_ea_ids;
    buf[0x0315] = h.general_general_ids;
    buf[0x0316] = h.race_race_ids;
    buf[0x0317] = h.class_class_ids;
    buf[0x0318] = h.specific_specific_ids;
    buf[0x0319] = h.gender_gender_ids;
    { let src = &h.object_ids_references; let n = src.len().min(5); buf[0x031A..0x031A+n].copy_from_slice(&src[..n]); }
    buf[0x031F] = h.alignment_alignmen_ids;
    buf[0x0320..0x0322].copy_from_slice(&h.global_actor_enumeration_value.to_le_bytes());
    buf[0x0322..0x0324].copy_from_slice(&h.local_area_actor_enumeration_value.to_le_bytes());
    { let src = &h.death_variable_set_sprite_is_deadvariable; let n = src.len().min(32); buf[0x0324..0x0324+n].copy_from_slice(&src[..n]); }
    buf[0x0344..0x0348].copy_from_slice(&h.known_spells_offset.to_le_bytes());
    buf[0x0348..0x034C].copy_from_slice(&h.known_spells_count.to_le_bytes());
    buf[0x034C..0x0350].copy_from_slice(&h.spell_memorization_info_offset.to_le_bytes());
    buf[0x0350..0x0354].copy_from_slice(&h.spell_memorization_info_entries_count.to_le_bytes());
    buf[0x0354..0x0358].copy_from_slice(&h.memorized_spells_offset.to_le_bytes());
    buf[0x0358..0x035C].copy_from_slice(&h.memorized_spells_count.to_le_bytes());
    buf[0x035C..0x0360].copy_from_slice(&h.item_slots_offset.to_le_bytes());
    buf[0x0360..0x0364].copy_from_slice(&h.items_offset.to_le_bytes());
    buf[0x0364..0x0368].copy_from_slice(&h.items_count.to_le_bytes());
    buf[0x0368..0x036C].copy_from_slice(&h.offset_to_effects.to_le_bytes());
    buf[0x036C..0x0370].copy_from_slice(&h.count_to_effects.to_le_bytes());
    write_resref(&mut buf[0x0370..0x0378], &h.dialog_file);
    buf
}

// ============================================================
//  V9_0 — 138 fields, header = 828 B
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreHeaderV90 {
    /// 0x0000 (4 B): Signature ('CRE ')
    pub signature: Vec<u8>,
    /// 0x0004 (4 B): Version ('V9.0')
    pub version: Vec<u8>,
    /// 0x0008 (4 B): Long name
    pub long_name: u32,
    /// 0x000C (4 B): Short name (tooltip)
    pub short_name_tooltip: u32,
    /// 0x0010 (4 B): Creature flags bit 0 Show longname in tooltip bit 1 No corpse bit 2 Keep corpse bit 3 O...
    pub creature_flags: u32,
    /// 0x0014 (4 B): XP (gained for killing this creature)
    pub xp_gained_for_killing_this_creature: u32,
    /// 0x0018 (4 B): Creature Power Level (for summoning spells) / XP of the creature (for party members)
    pub creature_power_level_for_summoning_spells: u32,
    /// 0x001C (4 B): Gold carried
    pub gold_carried: u32,
    /// 0x0020 (4 B): Permanent status flags (STATE.IDS)
    pub permanent_status_flags_state_ids: u32,
    /// 0x0024 (2 B): Current Hit Points
    pub current_hit_points: u16,
    /// 0x0026 (2 B): Maximum Hit Points
    pub maximum_hit_points: u16,
    /// 0x0028 (4 B): Animation ID (ANIMATE.IDS) There is some structure to the ordering of these entries, ho...
    pub animation_id_animate_ids: u32,
    /// 0x002C (1 B): Metal Colour Index
    pub metal_colour_index: u8,
    /// 0x002D (1 B): Minor Colour Index
    pub minor_colour_index: u8,
    /// 0x002E (1 B): Major Colour Index
    pub major_colour_index: u8,
    /// 0x002F (1 B): Skin Colour Index
    pub skin_colour_index: u8,
    /// 0x0030 (1 B): Leather Colour Index
    pub leather_colour_index: u8,
    /// 0x0031 (1 B): Armor Colour Index
    pub armor_colour_index: u8,
    /// 0x0032 (1 B): Hair Colour Index
    pub hair_colour_index: u8,
    /// 0x0033 (1 B): EFF structure version 0: Version 1 EFF 1: Version 2 EFF
    pub eff_structure_version_0_version_1: u8,
    /// 0x0034 (8 B): Small Portrait
    pub small_portrait: String,
    /// 0x003C (8 B): Large Portrait
    pub large_portrait: String,
    /// 0x0044 (1 B): Reputation (minimum value: 0)
    pub reputation: i8,
    /// 0x0045 (1 B): Hide In Shadows (base)
    pub hide_in_shadows_base: u8,
    /// 0x0046 (2 B): Armor Class (Natural)
    pub armor_class_natural: i16,
    /// 0x0048 (2 B): Armor Class (Effective)
    pub armor_class_effective: i16,
    /// 0x004A (2 B): Armor Class (Crushing Attacks Modifier)
    pub armor_class_crushing_attacks_modifier: i16,
    /// 0x004C (2 B): Armor Class (Missile Attacks Modifier)
    pub armor_class_missile_attacks_modifier: i16,
    /// 0x004E (2 B): Armor Class (Piercing Attacks Modifier)
    pub armor_class_piercing_attacks_modifier: i16,
    /// 0x0050 (2 B): Armor Class (Slashing Attacks Modifier)
    pub armor_class_slashing_attacks_modifier: i16,
    /// 0x0052 (1 B): THAC0 (1-25)
    pub thac0: u8,
    /// 0x0053 (1 B): Number of attacks (0-10)
    pub number_of_attacks: u8,
    /// 0x0054 (1 B): Save versus death (0-20)
    pub save_versus_death: u8,
    /// 0x0055 (1 B): Save versus wands (0-20)
    pub save_versus_wands: u8,
    /// 0x0056 (1 B): Save versus polymorph (0-20)
    pub save_versus_polymorph: u8,
    /// 0x0057 (1 B): Save versus breath attacks (0-20)
    pub save_versus_breath_attacks: u8,
    /// 0x0058 (1 B): Save versus spells (0-20)
    pub save_versus_spells: u8,
    /// 0x0059 (1 B): Resist fire (0-100)
    pub resist_fire: u8,
    /// 0x005A (1 B): Resist cold (0-100)
    pub resist_cold: u8,
    /// 0x005B (1 B): Resist electricity (0-100)
    pub resist_electricity: u8,
    /// 0x005C (1 B): Resist acid (0-100)
    pub resist_acid: u8,
    /// 0x005D (1 B): Resist magic (0-100)
    pub resist_magic: u8,
    /// 0x005E (1 B): Resist magic fire (0-100)
    pub resist_magic_fire: u8,
    /// 0x005F (1 B): Resist magic cold (0-100)
    pub resist_magic_cold: u8,
    /// 0x0060 (1 B): Resist slashing (0-100)
    pub resist_slashing: u8,
    /// 0x0061 (1 B): Resist crushing (0-100)
    pub resist_crushing: u8,
    /// 0x0062 (1 B): Resist piercing (0-100)
    pub resist_piercing: u8,
    /// 0x0063 (1 B): Resist missile (0-100)
    pub resist_missile: u8,
    /// 0x0064 (1 B): Detect illusion (minimum value : 0)
    pub detect_illusion: u8,
    /// 0x0065 (1 B): Set traps
    pub set_traps: u8,
    /// 0x0066 (1 B): Lore (0-100)*
    pub lore: u8,
    /// 0x0067 (1 B): Lockpicking (minimum value: 0)
    pub lockpicking: u8,
    /// 0x0068 (1 B): Stealth (minimum value: 0)
    pub stealth: u8,
    /// 0x0069 (1 B): Find/disarm traps (minimum value: 0)
    pub find_disarm_traps: u8,
    /// 0x006A (1 B): Pick pockets (minimum value: 0)
    pub pick_pockets: u8,
    /// 0x006B (1 B): Fatigue (0-100)
    pub fatigue: u8,
    /// 0x006C (1 B): Intoxication (0-100)
    pub intoxication: u8,
    /// 0x006D (1 B): Luck
    pub luck: u8,
    /// 0x006E (1 B): Large swords proficiency (Proficiencies maybe be packed into 3-bit chunks for the prima...
    pub large_swords_proficiency_proficiencies_maybe_be: u8,
    /// 0x006F (1 B): Small swords proficiency (Proficiencies maybe be packed into 3-bit chunks for the prima...
    pub small_swords_proficiency_proficiencies_maybe_be: u8,
    /// 0x0070 (1 B): Bows proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and s...
    pub bows_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0071 (1 B): Spears proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and...
    pub spears_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0072 (1 B): Axe proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and se...
    pub axe_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0073 (1 B): Missile proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub missile_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0074 (1 B): Great Swords proficiency (Proficiencies maybe be packed into 3-bit chunks for the prima...
    pub great_swords_proficiency_proficiencies_maybe_be: u8,
    /// 0x0075 (1 B): Daggers proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub daggers_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0076 (1 B): Halberd proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub halberd_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0077 (1 B): Mace proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and s...
    pub mace_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0078 (1 B): Flail proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and ...
    pub flail_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x0079 (1 B): Hammers proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub hammers_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x007A (1 B): Clubs proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary and ...
    pub clubs_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x007B (1 B): Quarterstaves proficiency (Proficiencies maybe be packed into 3-bit chunks for the prim...
    pub quarterstaves_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x007C (1 B): Crossbow proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary a...
    pub crossbow_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x007D (1 B): Unknown proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub unknown_proficiency_proficiencies_maybe_be_packed: u8,
    /// 0x007E (1 B): Unknown proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub unknown_proficiency_proficiencies_maybe_be_packed_2: u8,
    /// 0x007F (1 B): Unknown proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub unknown_proficiency_proficiencies_maybe_be_packed_3: u8,
    /// 0x0080 (1 B): Unknown proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub unknown_proficiency_proficiencies_maybe_be_packed_4: u8,
    /// 0x0081 (1 B): Unknown proficiency (Proficiencies maybe be packed into 3-bit chunks for the primary an...
    pub unknown_proficiency_proficiencies_maybe_be_packed_5: u8,
    /// 0x0082 (1 B): Turn undead level
    pub turn_undead_level: u8,
    /// 0x0083 (1 B): Tracking skill (0-100)
    pub tracking_skill: u8,
    /// 0x0084 (32 B): Tracking target
    pub tracking_target: Vec<u8>,
    /// 0x00A4 (400 B): Strrefs pertaining to the character. Most are connected with the sound-set (see SOUNDOF...
    pub strrefs_pertaining_to_the_character_most: Vec<u8>,
    /// 0x0234 (1 B): Highest attained level in class (0-100). For dual/multi class characters, the levels fo...
    pub highest_attained_level_in_class: u8,
    /// 0x0235 (1 B): Highest attained level in class (0-100)
    pub highest_attained_level_in_class_2: u8,
    /// 0x0236 (1 B): Highest attained level in class (0-100)
    pub highest_attained_level_in_class_3: u8,
    /// 0x0237 (1 B): Sex (from gender.ids) - checkable via the sex stat
    pub sex_from_gender_ids_checkable_via: u8,
    /// 0x0238 (1 B): Strength (1-25)
    pub strength: u8,
    /// 0x0239 (1 B): Strength % Bonus (0-100)
    pub strength_bonus: u8,
    /// 0x023A (1 B): Intelligence (1-25)
    pub intelligence: u8,
    /// 0x023B (1 B): Wisdom (1-25)
    pub wisdom: u8,
    /// 0x023C (1 B): Dexterity (1-25)
    pub dexterity: u8,
    /// 0x023D (1 B): Constitution (1-25)
    pub constitution: u8,
    /// 0x023E (1 B): Charisma (1-25)
    pub charisma: u8,
    /// 0x023F (1 B): Morale
    pub morale: u8,
    /// 0x0240 (1 B): Morale break
    pub morale_break: u8,
    /// 0x0241 (1 B): Racial enemy (RACE.IDS)
    pub racial_enemy_race_ids: u8,
    /// 0x0242 (2 B): Morale Recovery Time
    pub morale_recovery_time: u16,
    /// 0x0244 (4 B): Kit information NONE ABJURER 0x00400000 CONJURER 0x00800000 DIVINER 0x01000000 ENCHANTE...
    pub kit_information_none_abjurer_0x00400000_conjurer: u32,
    /// 0x0248 (8 B): Creature script - Override
    pub creature_script_override: String,
    /// 0x0250 (8 B): Creature script - Class
    pub creature_script_class: String,
    /// 0x0258 (8 B): Creature script - Race
    pub creature_script_race: String,
    /// 0x0260 (8 B): Creature script - General
    pub creature_script_general: String,
    /// 0x0268 (8 B): Creature script - Default
    pub creature_script_default: String,
    /// 0x0270 (1 B): Visible (0 = No, 1 = Yes)
    pub visible_0_no_1_yes: u8,
    /// 0x0271 (1 B): Set _DEAD variable on death (0 = No, 1 = Yes)
    pub set_dead_variable_on_death_0: u8,
    /// 0x0272 (1 B): Set KILL_<scriptname>_CNT on death (0 = No, 1 = Yes)
    pub set_kill_scriptname_cnt_on_death: u8,
    /// 0x0273 (1 B): Unknown
    pub unknown: u8,
    /// 0x0274 (10 B): Internal variables
    pub internal_variables: Vec<u8>,
    /// 0x027E (32 B): Secondary death variable (set to 1 on death)
    pub secondary_death_variable_set_to_1: Vec<u8>,
    /// 0x029E (32 B): Tertiary death variable (incremented by 1 on death)
    pub tertiary_death_variable_incremented_by_1: Vec<u8>,
    /// 0x02BE (2 B): Determines whether the engine automatically saves the creature's location when it is ad...
    pub determines_whether_the_engine_automatically_saves: u16,
    /// 0x02C0 (2 B): Saved X coordinate
    pub saved_x_coordinate: u16,
    /// 0x02C2 (2 B): Saved Y coordinate
    pub saved_y_coordinate: u16,
    /// 0x02C4 (2 B): Saved orientation
    pub saved_orientation: u16,
    /// 0x02C6 (18 B): Unknown
    pub unknown_2: Vec<u8>,
    /// 0x02D8 (1 B): Enemy-Ally (EA.IDS)
    pub enemy_ally_ea_ids: u8,
    /// 0x02D9 (1 B): General (GENERAL.IDS)
    pub general_general_ids: u8,
    /// 0x02DA (1 B): Race (RACE.IDS)
    pub race_race_ids: u8,
    /// 0x02DB (1 B): Class (CLASS.IDS)
    pub class_class_ids: u8,
    /// 0x02DC (1 B): Specific (SPECIFIC.IDS)
    pub specific_specific_ids: u8,
    /// 0x02DD (1 B): Gender (GENDER.IDS)
    pub gender_gender_ids: u8,
    /// 0x02DE (5 B): OBJECT.IDS references
    pub object_ids_references: Vec<u8>,
    /// 0x02E3 (1 B): Alignment (ALIGNMEN.IDS)
    pub alignment_alignmen_ids: u8,
    /// 0x02E4 (2 B): Global actor enumeration value
    pub global_actor_enumeration_value: u16,
    /// 0x02E6 (2 B): Local (area) actor enumeration value
    pub local_area_actor_enumeration_value: u16,
    /// 0x02E8 (32 B): Death Variable (set SPRITE_IS_DEADvariable on death)
    pub death_variable_set_sprite_is_deadvariable: Vec<u8>,
    /// 0x0308 (4 B): Known spells offset
    pub known_spells_offset: u32,
    /// 0x030C (4 B): Known spells count
    pub known_spells_count: u32,
    /// 0x0310 (4 B): Spell memorization info offset
    pub spell_memorization_info_offset: u32,
    /// 0x0314 (4 B): Spell memorization info entries count
    pub spell_memorization_info_entries_count: u32,
    /// 0x0318 (4 B): Memorized spells offset
    pub memorized_spells_offset: u32,
    /// 0x031C (4 B): Memorized spells count
    pub memorized_spells_count: u32,
    /// 0x0320 (4 B): Offset to Item slots
    pub offset_to_item_slots: u32,
    /// 0x0324 (4 B): Offset to Items
    pub offset_to_items: u32,
    /// 0x0328 (4 B): Count of Items
    pub count_of_items: u32,
    /// 0x032C (4 B): Offset to effects
    pub offset_to_effects: u32,
    /// 0x0330 (4 B): Count of effects
    pub count_of_effects: u32,
    /// 0x0334 (8 B): Dialog file
    pub dialog_file: String,
}

pub(crate) fn parse_header_v9_0(header: &[u8]) -> std::io::Result<CreHeaderV90> {
    debug_assert_eq!(header.len(), 828);
    let read_u8 = |o: usize| header[o];
    let read_i8 = |o: usize| header[o] as i8;
    let read_u16 = |o: usize| u16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_i16 = |o: usize| i16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_u32 = |o: usize| u32::from_le_bytes(header[o..o+4].try_into().unwrap());
    let read_i32 = |o: usize| i32::from_le_bytes(header[o..o+4].try_into().unwrap());
    Ok(CreHeaderV90 {
        signature: header[0x0000..0x0004].to_vec(),
        version: header[0x0004..0x0008].to_vec(),
        long_name: read_u32(0x0008),
        short_name_tooltip: read_u32(0x000C),
        creature_flags: read_u32(0x0010),
        xp_gained_for_killing_this_creature: read_u32(0x0014),
        creature_power_level_for_summoning_spells: read_u32(0x0018),
        gold_carried: read_u32(0x001C),
        permanent_status_flags_state_ids: read_u32(0x0020),
        current_hit_points: read_u16(0x0024),
        maximum_hit_points: read_u16(0x0026),
        animation_id_animate_ids: read_u32(0x0028),
        metal_colour_index: read_u8(0x002C),
        minor_colour_index: read_u8(0x002D),
        major_colour_index: read_u8(0x002E),
        skin_colour_index: read_u8(0x002F),
        leather_colour_index: read_u8(0x0030),
        armor_colour_index: read_u8(0x0031),
        hair_colour_index: read_u8(0x0032),
        eff_structure_version_0_version_1: read_u8(0x0033),
        small_portrait: read_resref(&header[0x0034..0x003C]),
        large_portrait: read_resref(&header[0x003C..0x0044]),
        reputation: read_i8(0x0044),
        hide_in_shadows_base: read_u8(0x0045),
        armor_class_natural: read_i16(0x0046),
        armor_class_effective: read_i16(0x0048),
        armor_class_crushing_attacks_modifier: read_i16(0x004A),
        armor_class_missile_attacks_modifier: read_i16(0x004C),
        armor_class_piercing_attacks_modifier: read_i16(0x004E),
        armor_class_slashing_attacks_modifier: read_i16(0x0050),
        thac0: read_u8(0x0052),
        number_of_attacks: read_u8(0x0053),
        save_versus_death: read_u8(0x0054),
        save_versus_wands: read_u8(0x0055),
        save_versus_polymorph: read_u8(0x0056),
        save_versus_breath_attacks: read_u8(0x0057),
        save_versus_spells: read_u8(0x0058),
        resist_fire: read_u8(0x0059),
        resist_cold: read_u8(0x005A),
        resist_electricity: read_u8(0x005B),
        resist_acid: read_u8(0x005C),
        resist_magic: read_u8(0x005D),
        resist_magic_fire: read_u8(0x005E),
        resist_magic_cold: read_u8(0x005F),
        resist_slashing: read_u8(0x0060),
        resist_crushing: read_u8(0x0061),
        resist_piercing: read_u8(0x0062),
        resist_missile: read_u8(0x0063),
        detect_illusion: read_u8(0x0064),
        set_traps: read_u8(0x0065),
        lore: read_u8(0x0066),
        lockpicking: read_u8(0x0067),
        stealth: read_u8(0x0068),
        find_disarm_traps: read_u8(0x0069),
        pick_pockets: read_u8(0x006A),
        fatigue: read_u8(0x006B),
        intoxication: read_u8(0x006C),
        luck: read_u8(0x006D),
        large_swords_proficiency_proficiencies_maybe_be: read_u8(0x006E),
        small_swords_proficiency_proficiencies_maybe_be: read_u8(0x006F),
        bows_proficiency_proficiencies_maybe_be_packed: read_u8(0x0070),
        spears_proficiency_proficiencies_maybe_be_packed: read_u8(0x0071),
        axe_proficiency_proficiencies_maybe_be_packed: read_u8(0x0072),
        missile_proficiency_proficiencies_maybe_be_packed: read_u8(0x0073),
        great_swords_proficiency_proficiencies_maybe_be: read_u8(0x0074),
        daggers_proficiency_proficiencies_maybe_be_packed: read_u8(0x0075),
        halberd_proficiency_proficiencies_maybe_be_packed: read_u8(0x0076),
        mace_proficiency_proficiencies_maybe_be_packed: read_u8(0x0077),
        flail_proficiency_proficiencies_maybe_be_packed: read_u8(0x0078),
        hammers_proficiency_proficiencies_maybe_be_packed: read_u8(0x0079),
        clubs_proficiency_proficiencies_maybe_be_packed: read_u8(0x007A),
        quarterstaves_proficiency_proficiencies_maybe_be_packed: read_u8(0x007B),
        crossbow_proficiency_proficiencies_maybe_be_packed: read_u8(0x007C),
        unknown_proficiency_proficiencies_maybe_be_packed: read_u8(0x007D),
        unknown_proficiency_proficiencies_maybe_be_packed_2: read_u8(0x007E),
        unknown_proficiency_proficiencies_maybe_be_packed_3: read_u8(0x007F),
        unknown_proficiency_proficiencies_maybe_be_packed_4: read_u8(0x0080),
        unknown_proficiency_proficiencies_maybe_be_packed_5: read_u8(0x0081),
        turn_undead_level: read_u8(0x0082),
        tracking_skill: read_u8(0x0083),
        tracking_target: header[0x0084..0x00A4].to_vec(),
        strrefs_pertaining_to_the_character_most: header[0x00A4..0x0234].to_vec(),
        highest_attained_level_in_class: read_u8(0x0234),
        highest_attained_level_in_class_2: read_u8(0x0235),
        highest_attained_level_in_class_3: read_u8(0x0236),
        sex_from_gender_ids_checkable_via: read_u8(0x0237),
        strength: read_u8(0x0238),
        strength_bonus: read_u8(0x0239),
        intelligence: read_u8(0x023A),
        wisdom: read_u8(0x023B),
        dexterity: read_u8(0x023C),
        constitution: read_u8(0x023D),
        charisma: read_u8(0x023E),
        morale: read_u8(0x023F),
        morale_break: read_u8(0x0240),
        racial_enemy_race_ids: read_u8(0x0241),
        morale_recovery_time: read_u16(0x0242),
        kit_information_none_abjurer_0x00400000_conjurer: read_u32(0x0244),
        creature_script_override: read_resref(&header[0x0248..0x0250]),
        creature_script_class: read_resref(&header[0x0250..0x0258]),
        creature_script_race: read_resref(&header[0x0258..0x0260]),
        creature_script_general: read_resref(&header[0x0260..0x0268]),
        creature_script_default: read_resref(&header[0x0268..0x0270]),
        visible_0_no_1_yes: read_u8(0x0270),
        set_dead_variable_on_death_0: read_u8(0x0271),
        set_kill_scriptname_cnt_on_death: read_u8(0x0272),
        unknown: read_u8(0x0273),
        internal_variables: header[0x0274..0x027E].to_vec(),
        secondary_death_variable_set_to_1: header[0x027E..0x029E].to_vec(),
        tertiary_death_variable_incremented_by_1: header[0x029E..0x02BE].to_vec(),
        determines_whether_the_engine_automatically_saves: read_u16(0x02BE),
        saved_x_coordinate: read_u16(0x02C0),
        saved_y_coordinate: read_u16(0x02C2),
        saved_orientation: read_u16(0x02C4),
        unknown_2: header[0x02C6..0x02D8].to_vec(),
        enemy_ally_ea_ids: read_u8(0x02D8),
        general_general_ids: read_u8(0x02D9),
        race_race_ids: read_u8(0x02DA),
        class_class_ids: read_u8(0x02DB),
        specific_specific_ids: read_u8(0x02DC),
        gender_gender_ids: read_u8(0x02DD),
        object_ids_references: header[0x02DE..0x02E3].to_vec(),
        alignment_alignmen_ids: read_u8(0x02E3),
        global_actor_enumeration_value: read_u16(0x02E4),
        local_area_actor_enumeration_value: read_u16(0x02E6),
        death_variable_set_sprite_is_deadvariable: header[0x02E8..0x0308].to_vec(),
        known_spells_offset: read_u32(0x0308),
        known_spells_count: read_u32(0x030C),
        spell_memorization_info_offset: read_u32(0x0310),
        spell_memorization_info_entries_count: read_u32(0x0314),
        memorized_spells_offset: read_u32(0x0318),
        memorized_spells_count: read_u32(0x031C),
        offset_to_item_slots: read_u32(0x0320),
        offset_to_items: read_u32(0x0324),
        count_of_items: read_u32(0x0328),
        offset_to_effects: read_u32(0x032C),
        count_of_effects: read_u32(0x0330),
        dialog_file: read_resref(&header[0x0334..0x033C]),
    })
}

pub(crate) fn serialize_header_v9_0(h: &CreHeaderV90) -> Vec<u8> {
    let mut buf = vec![0u8; 828];
    { let src = &h.signature; let n = src.len().min(4); buf[0x0000..0x0000+n].copy_from_slice(&src[..n]); }
    { let src = &h.version; let n = src.len().min(4); buf[0x0004..0x0004+n].copy_from_slice(&src[..n]); }
    buf[0x0008..0x000C].copy_from_slice(&h.long_name.to_le_bytes());
    buf[0x000C..0x0010].copy_from_slice(&h.short_name_tooltip.to_le_bytes());
    buf[0x0010..0x0014].copy_from_slice(&h.creature_flags.to_le_bytes());
    buf[0x0014..0x0018].copy_from_slice(&h.xp_gained_for_killing_this_creature.to_le_bytes());
    buf[0x0018..0x001C].copy_from_slice(&h.creature_power_level_for_summoning_spells.to_le_bytes());
    buf[0x001C..0x0020].copy_from_slice(&h.gold_carried.to_le_bytes());
    buf[0x0020..0x0024].copy_from_slice(&h.permanent_status_flags_state_ids.to_le_bytes());
    buf[0x0024..0x0026].copy_from_slice(&h.current_hit_points.to_le_bytes());
    buf[0x0026..0x0028].copy_from_slice(&h.maximum_hit_points.to_le_bytes());
    buf[0x0028..0x002C].copy_from_slice(&h.animation_id_animate_ids.to_le_bytes());
    buf[0x002C] = h.metal_colour_index;
    buf[0x002D] = h.minor_colour_index;
    buf[0x002E] = h.major_colour_index;
    buf[0x002F] = h.skin_colour_index;
    buf[0x0030] = h.leather_colour_index;
    buf[0x0031] = h.armor_colour_index;
    buf[0x0032] = h.hair_colour_index;
    buf[0x0033] = h.eff_structure_version_0_version_1;
    write_resref(&mut buf[0x0034..0x003C], &h.small_portrait);
    write_resref(&mut buf[0x003C..0x0044], &h.large_portrait);
    buf[0x0044] = h.reputation as u8;
    buf[0x0045] = h.hide_in_shadows_base;
    buf[0x0046..0x0048].copy_from_slice(&h.armor_class_natural.to_le_bytes());
    buf[0x0048..0x004A].copy_from_slice(&h.armor_class_effective.to_le_bytes());
    buf[0x004A..0x004C].copy_from_slice(&h.armor_class_crushing_attacks_modifier.to_le_bytes());
    buf[0x004C..0x004E].copy_from_slice(&h.armor_class_missile_attacks_modifier.to_le_bytes());
    buf[0x004E..0x0050].copy_from_slice(&h.armor_class_piercing_attacks_modifier.to_le_bytes());
    buf[0x0050..0x0052].copy_from_slice(&h.armor_class_slashing_attacks_modifier.to_le_bytes());
    buf[0x0052] = h.thac0;
    buf[0x0053] = h.number_of_attacks;
    buf[0x0054] = h.save_versus_death;
    buf[0x0055] = h.save_versus_wands;
    buf[0x0056] = h.save_versus_polymorph;
    buf[0x0057] = h.save_versus_breath_attacks;
    buf[0x0058] = h.save_versus_spells;
    buf[0x0059] = h.resist_fire;
    buf[0x005A] = h.resist_cold;
    buf[0x005B] = h.resist_electricity;
    buf[0x005C] = h.resist_acid;
    buf[0x005D] = h.resist_magic;
    buf[0x005E] = h.resist_magic_fire;
    buf[0x005F] = h.resist_magic_cold;
    buf[0x0060] = h.resist_slashing;
    buf[0x0061] = h.resist_crushing;
    buf[0x0062] = h.resist_piercing;
    buf[0x0063] = h.resist_missile;
    buf[0x0064] = h.detect_illusion;
    buf[0x0065] = h.set_traps;
    buf[0x0066] = h.lore;
    buf[0x0067] = h.lockpicking;
    buf[0x0068] = h.stealth;
    buf[0x0069] = h.find_disarm_traps;
    buf[0x006A] = h.pick_pockets;
    buf[0x006B] = h.fatigue;
    buf[0x006C] = h.intoxication;
    buf[0x006D] = h.luck;
    buf[0x006E] = h.large_swords_proficiency_proficiencies_maybe_be;
    buf[0x006F] = h.small_swords_proficiency_proficiencies_maybe_be;
    buf[0x0070] = h.bows_proficiency_proficiencies_maybe_be_packed;
    buf[0x0071] = h.spears_proficiency_proficiencies_maybe_be_packed;
    buf[0x0072] = h.axe_proficiency_proficiencies_maybe_be_packed;
    buf[0x0073] = h.missile_proficiency_proficiencies_maybe_be_packed;
    buf[0x0074] = h.great_swords_proficiency_proficiencies_maybe_be;
    buf[0x0075] = h.daggers_proficiency_proficiencies_maybe_be_packed;
    buf[0x0076] = h.halberd_proficiency_proficiencies_maybe_be_packed;
    buf[0x0077] = h.mace_proficiency_proficiencies_maybe_be_packed;
    buf[0x0078] = h.flail_proficiency_proficiencies_maybe_be_packed;
    buf[0x0079] = h.hammers_proficiency_proficiencies_maybe_be_packed;
    buf[0x007A] = h.clubs_proficiency_proficiencies_maybe_be_packed;
    buf[0x007B] = h.quarterstaves_proficiency_proficiencies_maybe_be_packed;
    buf[0x007C] = h.crossbow_proficiency_proficiencies_maybe_be_packed;
    buf[0x007D] = h.unknown_proficiency_proficiencies_maybe_be_packed;
    buf[0x007E] = h.unknown_proficiency_proficiencies_maybe_be_packed_2;
    buf[0x007F] = h.unknown_proficiency_proficiencies_maybe_be_packed_3;
    buf[0x0080] = h.unknown_proficiency_proficiencies_maybe_be_packed_4;
    buf[0x0081] = h.unknown_proficiency_proficiencies_maybe_be_packed_5;
    buf[0x0082] = h.turn_undead_level;
    buf[0x0083] = h.tracking_skill;
    { let src = &h.tracking_target; let n = src.len().min(32); buf[0x0084..0x0084+n].copy_from_slice(&src[..n]); }
    { let src = &h.strrefs_pertaining_to_the_character_most; let n = src.len().min(400); buf[0x00A4..0x00A4+n].copy_from_slice(&src[..n]); }
    buf[0x0234] = h.highest_attained_level_in_class;
    buf[0x0235] = h.highest_attained_level_in_class_2;
    buf[0x0236] = h.highest_attained_level_in_class_3;
    buf[0x0237] = h.sex_from_gender_ids_checkable_via;
    buf[0x0238] = h.strength;
    buf[0x0239] = h.strength_bonus;
    buf[0x023A] = h.intelligence;
    buf[0x023B] = h.wisdom;
    buf[0x023C] = h.dexterity;
    buf[0x023D] = h.constitution;
    buf[0x023E] = h.charisma;
    buf[0x023F] = h.morale;
    buf[0x0240] = h.morale_break;
    buf[0x0241] = h.racial_enemy_race_ids;
    buf[0x0242..0x0244].copy_from_slice(&h.morale_recovery_time.to_le_bytes());
    buf[0x0244..0x0248].copy_from_slice(&h.kit_information_none_abjurer_0x00400000_conjurer.to_le_bytes());
    write_resref(&mut buf[0x0248..0x0250], &h.creature_script_override);
    write_resref(&mut buf[0x0250..0x0258], &h.creature_script_class);
    write_resref(&mut buf[0x0258..0x0260], &h.creature_script_race);
    write_resref(&mut buf[0x0260..0x0268], &h.creature_script_general);
    write_resref(&mut buf[0x0268..0x0270], &h.creature_script_default);
    buf[0x0270] = h.visible_0_no_1_yes;
    buf[0x0271] = h.set_dead_variable_on_death_0;
    buf[0x0272] = h.set_kill_scriptname_cnt_on_death;
    buf[0x0273] = h.unknown;
    { let src = &h.internal_variables; let n = src.len().min(10); buf[0x0274..0x0274+n].copy_from_slice(&src[..n]); }
    { let src = &h.secondary_death_variable_set_to_1; let n = src.len().min(32); buf[0x027E..0x027E+n].copy_from_slice(&src[..n]); }
    { let src = &h.tertiary_death_variable_incremented_by_1; let n = src.len().min(32); buf[0x029E..0x029E+n].copy_from_slice(&src[..n]); }
    buf[0x02BE..0x02C0].copy_from_slice(&h.determines_whether_the_engine_automatically_saves.to_le_bytes());
    buf[0x02C0..0x02C2].copy_from_slice(&h.saved_x_coordinate.to_le_bytes());
    buf[0x02C2..0x02C4].copy_from_slice(&h.saved_y_coordinate.to_le_bytes());
    buf[0x02C4..0x02C6].copy_from_slice(&h.saved_orientation.to_le_bytes());
    { let src = &h.unknown_2; let n = src.len().min(18); buf[0x02C6..0x02C6+n].copy_from_slice(&src[..n]); }
    buf[0x02D8] = h.enemy_ally_ea_ids;
    buf[0x02D9] = h.general_general_ids;
    buf[0x02DA] = h.race_race_ids;
    buf[0x02DB] = h.class_class_ids;
    buf[0x02DC] = h.specific_specific_ids;
    buf[0x02DD] = h.gender_gender_ids;
    { let src = &h.object_ids_references; let n = src.len().min(5); buf[0x02DE..0x02DE+n].copy_from_slice(&src[..n]); }
    buf[0x02E3] = h.alignment_alignmen_ids;
    buf[0x02E4..0x02E6].copy_from_slice(&h.global_actor_enumeration_value.to_le_bytes());
    buf[0x02E6..0x02E8].copy_from_slice(&h.local_area_actor_enumeration_value.to_le_bytes());
    { let src = &h.death_variable_set_sprite_is_deadvariable; let n = src.len().min(32); buf[0x02E8..0x02E8+n].copy_from_slice(&src[..n]); }
    buf[0x0308..0x030C].copy_from_slice(&h.known_spells_offset.to_le_bytes());
    buf[0x030C..0x0310].copy_from_slice(&h.known_spells_count.to_le_bytes());
    buf[0x0310..0x0314].copy_from_slice(&h.spell_memorization_info_offset.to_le_bytes());
    buf[0x0314..0x0318].copy_from_slice(&h.spell_memorization_info_entries_count.to_le_bytes());
    buf[0x0318..0x031C].copy_from_slice(&h.memorized_spells_offset.to_le_bytes());
    buf[0x031C..0x0320].copy_from_slice(&h.memorized_spells_count.to_le_bytes());
    buf[0x0320..0x0324].copy_from_slice(&h.offset_to_item_slots.to_le_bytes());
    buf[0x0324..0x0328].copy_from_slice(&h.offset_to_items.to_le_bytes());
    buf[0x0328..0x032C].copy_from_slice(&h.count_of_items.to_le_bytes());
    buf[0x032C..0x0330].copy_from_slice(&h.offset_to_effects.to_le_bytes());
    buf[0x0330..0x0334].copy_from_slice(&h.count_of_effects.to_le_bytes());
    write_resref(&mut buf[0x0334..0x033C], &h.dialog_file);
    buf
}

// ============================================================
//  V2_2 — 329 fields, header = 1582 B
// ============================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreHeaderV22 {
    /// 0x0000 (4 B): Signature ('CRE ')
    pub signature: Vec<u8>,
    /// 0x0004 (4 B): Version ('V2.2')
    pub version: Vec<u8>,
    /// 0x0008 (4 B): Long name
    pub long_name: u32,
    /// 0x000C (4 B): Short name (tooltip)
    pub short_name_tooltip: u32,
    /// 0x0010 (4 B): Creature flags bit 0 Damage don't stop casting bit 1 No corpse bit 2 Keep corpse bit 3 ...
    pub creature_flags: u32,
    /// 0x0014 (4 B): XP (gained for killing this creature)
    pub xp_gained_for_killing_this_creature: u32,
    /// 0x0018 (4 B): Creature Power Level (for summoning spells) / XP of the creature (for party members)
    pub creature_power_level_for_summoning_spells: u32,
    /// 0x001C (4 B): Gold carried
    pub gold_carried: u32,
    /// 0x0020 (4 B): Permanent status flags (STATE.IDS)
    pub permanent_status_flags_state_ids: u32,
    /// 0x0024 (2 B): Current Hit Points
    pub current_hit_points: u16,
    /// 0x0026 (2 B): Maximum Hit Points
    pub maximum_hit_points: u16,
    /// 0x0028 (4 B): Animation ID (ANIMATE.IDS) 0x002c
    pub animation_id_animate_ids_0x002c: u32,
    /// 0x002C (1 B): gap of 1 bytes between documented fields
    pub _padding_01: Vec<u8>,
    /// 0x002D (1 B): Minor Colour Index (BG1 animations)
    pub minor_colour_index_bg1_animations: u8,
    /// 0x002E (1 B): Major Colour Index (BG1 animations)
    pub major_colour_index_bg1_animations: u8,
    /// 0x002F (1 B): Skin Colour Index (BG1 animations)
    pub skin_colour_index_bg1_animations: u8,
    /// 0x0030 (1 B): Leather Colour Index (BG1 animations)
    pub leather_colour_index_bg1_animations: u8,
    /// 0x0031 (1 B): Armor Colour Index (BG1 animations)
    pub armor_colour_index_bg1_animations: u8,
    /// 0x0032 (1 B): Hair Colour Index (BG1 animations)
    pub hair_colour_index_bg1_animations: u8,
    /// 0x0033 (1 B): EFF structure version 0: Version 1 EFF 1: Version 2 EFF
    pub eff_structure_version_0_version_1: u8,
    /// 0x0034 (8 B): Small Portrait (BMP)
    pub small_portrait_bmp: String,
    /// 0x003C (8 B): Large Portrait (BMP)
    pub large_portrait_bmp: String,
    /// 0x0044 (1 B): Reputation (minimum value: 0)
    pub reputation: i8,
    /// 0x0045 (1 B): Hide In Shadows (base)
    pub hide_in_shadows_base: u8,
    /// 0x0046 (2 B): Armor Class
    pub armor_class: i16,
    /// 0x0048 (2 B): Armor Class (Crushing Attacks Modifier)
    pub armor_class_crushing_attacks_modifier: i16,
    /// 0x004A (2 B): Armor Class (Missile Attacks Modifier)
    pub armor_class_missile_attacks_modifier: i16,
    /// 0x004C (2 B): Armor Class (Piercing Attacks Modifier)
    pub armor_class_piercing_attacks_modifier: i16,
    /// 0x004E (2 B): Armor Class (Slashing Attacks Modifier)
    pub armor_class_slashing_attacks_modifier: i16,
    /// 0x0050 (1 B): Base Attack Bonus (BAB) for non party characters
    pub base_attack_bonus_bab_for_non: u8,
    /// 0x0051 (1 B): Number of attacks (0-10)
    pub number_of_attacks: u8,
    /// 0x0052 (1 B): Save versus Fortitude (0-20)
    pub save_versus_fortitude: u8,
    /// 0x0053 (1 B): Save versus Reflex (0-20)
    pub save_versus_reflex: u8,
    /// 0x0054 (1 B): Save versus Will (0-20)
    pub save_versus_will: u8,
    /// 0x0055 (1 B): Resist fire (0-100)
    pub resist_fire: u8,
    /// 0x0056 (1 B): Resist cold (0-100)
    pub resist_cold: u8,
    /// 0x0057 (1 B): Resist electricity (0-100)
    pub resist_electricity: u8,
    /// 0x0058 (1 B): Resist acid (0-100)
    pub resist_acid: u8,
    /// 0x0059 (1 B): Resist magic (0-100)
    pub resist_magic: u8,
    /// 0x005A (1 B): Resist magic fire (0-100)
    pub resist_magic_fire: u8,
    /// 0x005B (1 B): Resist magic cold (0-100)
    pub resist_magic_cold: u8,
    /// 0x005C (1 B): Resist slashing (0-100)
    pub resist_slashing: u8,
    /// 0x005D (1 B): Resist crushing (0-100)
    pub resist_crushing: u8,
    /// 0x005E (1 B): Resist piercing (0-100)
    pub resist_piercing: u8,
    /// 0x005F (1 B): Resist missile (0-100)
    pub resist_missile: u8,
    /// 0x0060 (1 B): Resist magic damage (0-100)
    pub resist_magic_damage: u8,
    /// 0x0061 (4 B): Unknown. Further resistances?
    pub unknown_further_resistances: Vec<u8>,
    /// 0x0065 (1 B): Fatigue
    pub fatigue: u8,
    /// 0x0066 (1 B): Intoxication
    pub intoxication: u8,
    /// 0x0067 (1 B): Luck
    pub luck: u8,
    /// 0x0068 (1 B): Turn undead level
    pub turn_undead_level: u8,
    /// 0x0069 (33 B): Unknown
    pub unknown: Vec<u8>,
    /// 0x008A (1 B): Total Levels
    pub total_levels: u8,
    /// 0x008B (1 B): Barbarian Levels
    pub barbarian_levels: u8,
    /// 0x008C (1 B): Bard Levels
    pub bard_levels: u8,
    /// 0x008D (1 B): Cleric Levels
    pub cleric_levels: u8,
    /// 0x008E (1 B): Druid Levels
    pub druid_levels: u8,
    /// 0x008F (1 B): Fighter Levels
    pub fighter_levels: u8,
    /// 0x0090 (1 B): Monk
    pub monk: u8,
    /// 0x0091 (1 B): Paladin Levels
    pub paladin_levels: u8,
    /// 0x0092 (1 B): Ranger Levels
    pub ranger_levels: u8,
    /// 0x0093 (1 B): Rogue Levels
    pub rogue_levels: u8,
    /// 0x0094 (1 B): Sorcerer Levels
    pub sorcerer_levels: u8,
    /// 0x0095 (1 B): Wizard Levels
    pub wizard_levels: u8,
    /// 0x0096 (22 B): Unknown
    pub unknown_2: Vec<u8>,
    /// 0x00AC (256 B): Strref's - most are connected with the sound-set
    pub strref_s_most_are_connected_with: Vec<u8>,
    /// 0x01AC (8 B): Team Script
    pub team_script: String,
    /// 0x01B4 (8 B): Special Script 1
    pub special_script_1: String,
    /// 0x01BC (1 B): Creature enchantment level
    pub creature_enchantment_level: u8,
    /// 0x01BD (3 B): Unknown
    pub unknown_3: Vec<u8>,
    /// 0x01C0 (4 B): Feats 1
    pub feats_1: u32,
    /// 0x01C4 (4 B): Feats 2
    pub feats_2: u32,
    /// 0x01C8 (4 B): Feats 3
    pub feats_3: u32,
    /// 0x01CC (12 B): Unknown
    pub unknown_4: Vec<u8>,
    /// 0x01D8 (1 B): MW: Bow
    pub mw_bow: u8,
    /// 0x01D9 (1 B): SW: Crossbow
    pub sw_crossbow: Vec<u8>,
    /// 0x01DA (1 B): SW: Missile
    pub sw_missile: u8,
    /// 0x01DB (1 B): MW: Axe
    pub mw_axe: u8,
    /// 0x01DC (1 B): SW: Mace
    pub sw_mace: u8,
    /// 0x01DD (1 B): MW: Flail
    pub mw_flail: u8,
    /// 0x01DE (1 B): MW: Polearm
    pub mw_polearm: u8,
    /// 0x01DF (1 B): MW: Hammer
    pub mw_hammer: u8,
    /// 0x01E0 (1 B): SW: Quarterstaff
    pub sw_quarterstaff: u8,
    /// 0x01E1 (1 B): MW: Great Sword
    pub mw_great_sword: u8,
    /// 0x01E2 (1 B): MW: Large Sword
    pub mw_large_sword: u8,
    /// 0x01E3 (1 B): SW: Small Blade
    pub sw_small_blade: u8,
    /// 0x01E4 (1 B): Toughness
    pub toughness: u8,
    /// 0x01E5 (1 B): Armored Arcana
    pub armored_arcana: u8,
    /// 0x01E6 (1 B): Cleave
    pub cleave: u8,
    /// 0x01E7 (1 B): Armor Proficiency
    pub armor_proficiency: u8,
    /// 0x01E8 (1 B): SF: Enchantment
    pub sf_enchantment: u8,
    /// 0x01E9 (1 B): SF: Evocation
    pub sf_evocation: u8,
    /// 0x01EA (1 B): SF: Necromancy
    pub sf_necromancy: u8,
    /// 0x01EB (1 B): SF: Transmutation
    pub sf_transmutation: u8,
    /// 0x01EC (1 B): Spell Penetration
    pub spell_penetration: u8,
    /// 0x01ED (1 B): Extra Rage
    pub extra_rage: u8,
    /// 0x01EE (1 B): Extra Wild Shape
    pub extra_wild_shape: u8,
    /// 0x01EF (1 B): Extra Smiting
    pub extra_smiting: u8,
    /// 0x01F0 (1 B): Extra Turning
    pub extra_turning: u8,
    /// 0x01F1 (1 B): EW: Bastard Sword
    pub ew_bastard_sword: u8,
    /// 0x01F2 (38 B): Unknown
    pub unknown_5: Vec<u8>,
    /// 0x0218 (1 B): Alchemy
    pub alchemy: u8,
    /// 0x0219 (1 B): Animal Empathy
    pub animal_empathy: u8,
    /// 0x021A (1 B): Bluff
    pub bluff: u8,
    /// 0x021B (1 B): Concentration
    pub concentration: u8,
    /// 0x021C (1 B): Diplomacy
    pub diplomacy: u8,
    /// 0x021D (1 B): Disable Device
    pub disable_device: u8,
    /// 0x021E (1 B): Hide
    pub hide: u8,
    /// 0x021F (1 B): Intimidate
    pub intimidate: u8,
    /// 0x0220 (1 B): Knowledge (Arcana)
    pub knowledge_arcana: u8,
    /// 0x0221 (1 B): Move Silently
    pub move_silently: u8,
    /// 0x0222 (1 B): Open Lock
    pub open_lock: u8,
    /// 0x0223 (1 B): Pick Pocket
    pub pick_pocket: u8,
    /// 0x0224 (1 B): Search
    pub search: u8,
    /// 0x0225 (1 B): Spellcraft
    pub spellcraft: u8,
    /// 0x0226 (1 B): Use Magic Device
    pub use_magic_device: u8,
    /// 0x0227 (1 B): Wilderness Law
    pub wilderness_law: u8,
    /// 0x0228 (50 B): Unknown
    pub unknown_6: Vec<u8>,
    /// 0x025A (1 B): XP category (values from moncrate.2da)
    pub xp_category_values_from_moncrate_2da: u8,
    /// 0x025B (1 B): Favoured Enemy 1
    pub favoured_enemy_1: u8,
    /// 0x025C (1 B): Favoured Enemy 2
    pub favoured_enemy_2: u8,
    /// 0x025D (1 B): Favoured Enemy 3
    pub favoured_enemy_3: u8,
    /// 0x025E (1 B): Favoured Enemy 4
    pub favoured_enemy_4: u8,
    /// 0x025F (1 B): Favoured Enemy 5
    pub favoured_enemy_5: u8,
    /// 0x0260 (1 B): Favoured Enemy 6
    pub favoured_enemy_6: u8,
    /// 0x0261 (1 B): Favoured Enemy 7
    pub favoured_enemy_7: u8,
    /// 0x0262 (1 B): Favoured Enemy 8
    pub favoured_enemy_8: u8,
    /// 0x0263 (1 B): Subrace (subrace.ids)
    pub subrace_subrace_ids: u8,
    /// 0x0264 (2 B): Unknown
    pub unknown_7: u16,
    /// 0x0266 (1 B): Strength (1-25)
    pub strength: u8,
    /// 0x0267 (1 B): Intelligence (1-25)
    pub intelligence: u8,
    /// 0x0268 (1 B): Wisdom (1-25)
    pub wisdom: u8,
    /// 0x0269 (1 B): Dexterity (1-25)
    pub dexterity: u8,
    /// 0x026A (1 B): Constitution (1-25)
    pub constitution: u8,
    /// 0x026B (1 B): Charisma (1-25)
    pub charisma: u8,
    /// 0x026C (4 B): Unknown
    pub unknown_8: u32,
    /// 0x0270 (4 B): Kit (bitfield)
    pub kit_bitfield: u32,
    /// 0x0274 (8 B): Creature script - Override
    pub creature_script_override: String,
    /// 0x027C (8 B): Creature script - Special Script 3
    pub creature_script_special_script_3: String,
    /// 0x0284 (8 B): Creature script - Special Script 2
    pub creature_script_special_script_2: String,
    /// 0x028C (8 B): gap of 8 bytes between documented fields
    pub _padding_02: Vec<u8>,
    /// 0x0294 (8 B): Creature script - Movement Script
    pub creature_script_movement_script: String,
    /// 0x029C (1 B): Visible (0 = No, 1 = Yes)
    pub visible_0_no_1_yes: u8,
    /// 0x029D (1 B): Set <scriptname>_DEAD variable on death (0 = no, 1 = yes) Also increment KILL_<scriptna...
    pub set_scriptname_dead_variable_on_death: u8,
    /// 0x029E (1 B): Set KILL_<racename>_CNT on death (0 = no, 1 = yes)
    pub set_kill_racename_cnt_on_death: u8,
    /// 0x029F (1 B): Unknown
    pub unknown_9: u8,
    /// 0x02A0 (10 B): 'Internals' - as used by SetInternal
    pub internals_as_used_by_setinternal: Vec<u8>,
    /// 0x02AA (32 B): Secondary death variable (set to 1 on death)
    pub secondary_death_variable_set_to_1: Vec<u8>,
    /// 0x02CA (32 B): Tertiary death variable (incremented by 1 on death) Note: Two death variables can be st...
    pub tertiary_death_variable_incremented_by_1: Vec<u8>,
    /// 0x02EA (2 B): Unknown
    pub unknown_10: u16,
    /// 0x02EC (2 B): Saved Location X coordinate
    pub saved_location_x_coordinate: u16,
    /// 0x02EE (1 B): gap of 1 bytes between documented fields
    pub _padding_03: Vec<u8>,
    /// 0x02EF (2 B): Saved Location Y coordinate
    pub saved_location_y_coordinate: u16,
    /// 0x02F1 (1 B): gap of 1 bytes between documented fields
    pub _padding_04: Vec<u8>,
    /// 0x02F2 (15 B): Unknown
    pub unknown_11: Vec<u8>,
    /// 0x0301 (1 B): Minimum transparency (fade in/fade out)
    pub minimum_transparency_fade_in_fade_out: u8,
    /// 0x0302 (1 B): Fade speed (fade in/fade out)
    pub fade_speed_fade_in_fade_out: u8,
    /// 0x0303 (1 B): Specflag values bit 0: Automatic concentration success, no morale failure bit 1: Immune...
    pub specflag_values: u8,
    /// 0x0304 (1 B): Visible
    pub visible: u8,
    /// 0x0305 (1 B): Unknown
    pub unknown_12: u8,
    /// 0x0306 (1 B): Unknown
    pub unknown_13: u8,
    /// 0x0307 (1 B): Remaining skill points (after level up)
    pub remaining_skill_points_after_level_up: u8,
    /// 0x0308 (124 B): Unknown
    pub unknown_14: Vec<u8>,
    /// 0x0384 (1 B): Enemy-Ally (EA.IDS)
    pub enemy_ally_ea_ids: u8,
    /// 0x0385 (1 B): General (GENERAL.IDS)
    pub general_general_ids: u8,
    /// 0x0386 (1 B): Race (RACE.IDS)
    pub race_race_ids: u8,
    /// 0x0387 (1 B): Class (CLASS.IDS) — not updated when you multiclass
    pub class_class_ids_not_updated_when: u8,
    /// 0x0388 (1 B): Specific (SPECIFIC.IDS)
    pub specific_specific_ids: u8,
    /// 0x0389 (1 B): Sex (GENDER.IDS)
    pub sex_gender_ids: u8,
    /// 0x038A (5 B): OBJECT.IDS references
    pub object_ids_references: Vec<u8>,
    /// 0x038F (1 B): Alignment (ALIGNMEN.IDS)
    pub alignment_alignmen_ids: u8,
    /// 0x0390 (2 B): Global actor enumeration value
    pub global_actor_enumeration_value: u16,
    /// 0x0392 (2 B): Local (area) actor enumeration value
    pub local_area_actor_enumeration_value: u16,
    /// 0x0394 (32 B): Death Variable
    pub death_variable: Vec<u8>,
    /// 0x03B4 (2 B): AVClass value (duplicate of class, used for object matching)
    pub avclass_value_duplicate_of_class_used: u16,
    /// 0x03B6 (2 B): ClassMsk bitfield value (duplicate of class, used for object matching)
    pub classmsk_bitfield_value_duplicate_of_class: u16,
    /// 0x03B8 (2 B): Unknown
    pub unknown_15: u16,
    /// 0x03BA (4 B): Bard Spell Offset (Level 1)
    pub bard_spell_offset_level_1: u32,
    /// 0x03BE (4 B): Bard Spell Offset (Level 2)
    pub bard_spell_offset_level_2: u32,
    /// 0x03C2 (4 B): Bard Spell Offset (Level 3)
    pub bard_spell_offset_level_3: u32,
    /// 0x03C6 (4 B): Bard Spell Offset (Level 4)
    pub bard_spell_offset_level_4: u32,
    /// 0x03CA (4 B): Bard Spell Offset (Level 5)
    pub bard_spell_offset_level_5: u32,
    /// 0x03CE (4 B): Bard Spell Offset (Level 6)
    pub bard_spell_offset_level_6: u32,
    /// 0x03D2 (4 B): Bard Spell Offset (Level 7)
    pub bard_spell_offset_level_7: u32,
    /// 0x03D6 (4 B): Bard Spell Offset (Level 8)
    pub bard_spell_offset_level_8: u32,
    /// 0x03DA (4 B): Bard Spell Offset (Level 9)
    pub bard_spell_offset_level_9: u32,
    /// 0x03DE (4 B): Cleric Spell Offset (Level 1)
    pub cleric_spell_offset_level_1: u32,
    /// 0x03E2 (4 B): Cleric Spell Offset (Level 2)
    pub cleric_spell_offset_level_2: u32,
    /// 0x03E6 (4 B): Cleric Spell Offset (Level 3)
    pub cleric_spell_offset_level_3: u32,
    /// 0x03EA (4 B): Cleric Spell Offset (Level 4)
    pub cleric_spell_offset_level_4: u32,
    /// 0x03EE (4 B): Cleric Spell Offset (Level 5)
    pub cleric_spell_offset_level_5: u32,
    /// 0x03F2 (4 B): Cleric Spell Offset (Level 6)
    pub cleric_spell_offset_level_6: u32,
    /// 0x03F6 (4 B): Cleric Spell Offset (Level 7)
    pub cleric_spell_offset_level_7: u32,
    /// 0x03FA (4 B): Cleric Spell Offset (Level 8)
    pub cleric_spell_offset_level_8: u32,
    /// 0x03FE (4 B): Cleric Spell Offset (Level 9)
    pub cleric_spell_offset_level_9: u32,
    /// 0x0402 (4 B): Druid Spell Offset (Level 1)
    pub druid_spell_offset_level_1: u32,
    /// 0x0406 (4 B): Druid Spell Offset (Level 2)
    pub druid_spell_offset_level_2: u32,
    /// 0x040A (4 B): Druid Spell Offset (Level 3)
    pub druid_spell_offset_level_3: u32,
    /// 0x040E (4 B): Druid Spell Offset (Level 4)
    pub druid_spell_offset_level_4: u32,
    /// 0x0412 (4 B): Druid Spell Offset (Level 5)
    pub druid_spell_offset_level_5: u32,
    /// 0x0416 (4 B): Druid Spell Offset (Level 6)
    pub druid_spell_offset_level_6: u32,
    /// 0x041A (4 B): Druid Spell Offset (Level 7)
    pub druid_spell_offset_level_7: u32,
    /// 0x041E (4 B): Druid Spell Offset (Level 8)
    pub druid_spell_offset_level_8: u32,
    /// 0x0422 (4 B): Druid Spell Offset (Level 9)
    pub druid_spell_offset_level_9: u32,
    /// 0x0426 (4 B): Paladin Spell Offset (Level 1)
    pub paladin_spell_offset_level_1: u32,
    /// 0x042A (4 B): Paladin Spell Offset (Level 2)
    pub paladin_spell_offset_level_2: u32,
    /// 0x042E (4 B): Paladin Spell Offset (Level 3)
    pub paladin_spell_offset_level_3: u32,
    /// 0x0432 (4 B): Paladin Spell Offset (Level 4)
    pub paladin_spell_offset_level_4: u32,
    /// 0x0436 (4 B): Paladin Spell Offset (Level 5)
    pub paladin_spell_offset_level_5: u32,
    /// 0x043A (4 B): Paladin Spell Offset (Level 6)
    pub paladin_spell_offset_level_6: u32,
    /// 0x043E (4 B): Paladin Spell Offset (Level 7)
    pub paladin_spell_offset_level_7: u32,
    /// 0x0442 (4 B): Paladin Spell Offset (Level 8)
    pub paladin_spell_offset_level_8: u32,
    /// 0x0446 (4 B): Paladin Spell Offset (Level 9)
    pub paladin_spell_offset_level_9: u32,
    /// 0x044A (4 B): Ranger Spell Offset (Level 1)
    pub ranger_spell_offset_level_1: u32,
    /// 0x044E (4 B): Ranger Spell Offset (Level 2)
    pub ranger_spell_offset_level_2: u32,
    /// 0x0452 (4 B): Ranger Spell Offset (Level 3)
    pub ranger_spell_offset_level_3: u32,
    /// 0x0456 (4 B): Ranger Spell Offset (Level 4)
    pub ranger_spell_offset_level_4: u32,
    /// 0x045A (4 B): Ranger Spell Offset (Level 5)
    pub ranger_spell_offset_level_5: u32,
    /// 0x045E (4 B): Ranger Spell Offset (Level 6)
    pub ranger_spell_offset_level_6: u32,
    /// 0x0462 (4 B): Ranger Spell Offset (Level 7)
    pub ranger_spell_offset_level_7: u32,
    /// 0x0466 (4 B): Ranger Spell Offset (Level 8)
    pub ranger_spell_offset_level_8: u32,
    /// 0x046A (4 B): Ranger Spell Offset (Level 9)
    pub ranger_spell_offset_level_9: u32,
    /// 0x046E (4 B): Sorcerer Spell Offset (Level 1)
    pub sorcerer_spell_offset_level_1: u32,
    /// 0x0472 (4 B): Sorcerer Spell Offset (Level 2)
    pub sorcerer_spell_offset_level_2: u32,
    /// 0x0476 (4 B): Sorcerer Spell Offset (Level 3)
    pub sorcerer_spell_offset_level_3: u32,
    /// 0x047A (4 B): Sorcerer Spell Offset (Level 4)
    pub sorcerer_spell_offset_level_4: u32,
    /// 0x047E (4 B): Sorcerer Spell Offset (Level 5)
    pub sorcerer_spell_offset_level_5: u32,
    /// 0x0482 (4 B): Sorcerer Spell Offset (Level 6)
    pub sorcerer_spell_offset_level_6: u32,
    /// 0x0486 (4 B): Sorcerer Spell Offset (Level 7)
    pub sorcerer_spell_offset_level_7: u32,
    /// 0x048A (4 B): Sorcerer Spell Offset (Level 8)
    pub sorcerer_spell_offset_level_8: u32,
    /// 0x048E (4 B): Sorcerer Spell Offset (Level 9)
    pub sorcerer_spell_offset_level_9: u32,
    /// 0x0492 (4 B): Wizard Spell Offset (Level 1)
    pub wizard_spell_offset_level_1: u32,
    /// 0x0496 (4 B): Wizard Spell Offset (Level 2)
    pub wizard_spell_offset_level_2: u32,
    /// 0x049A (4 B): Wizard Spell Offset (Level 3)
    pub wizard_spell_offset_level_3: u32,
    /// 0x049E (4 B): Wizard Spell Offset (Level 4)
    pub wizard_spell_offset_level_4: u32,
    /// 0x04A2 (4 B): Wizard Spell Offset (Level 5)
    pub wizard_spell_offset_level_5: u32,
    /// 0x04A6 (4 B): Wizard Spell Offset (Level 6)
    pub wizard_spell_offset_level_6: u32,
    /// 0x04AA (4 B): Wizard Spell Offset (Level 7)
    pub wizard_spell_offset_level_7: u32,
    /// 0x04AE (4 B): Wizard Spell Offset (Level 8)
    pub wizard_spell_offset_level_8: u32,
    /// 0x04B2 (4 B): Wizard Spell Offset (Level 9)
    pub wizard_spell_offset_level_9: u32,
    /// 0x04B6 (4 B): Bard Spell Count (Level 1)
    pub bard_spell_count_level_1: u32,
    /// 0x04BA (4 B): Bard Spell Count (Level 2)
    pub bard_spell_count_level_2: u32,
    /// 0x04BE (4 B): Bard Spell Count (Level 3)
    pub bard_spell_count_level_3: u32,
    /// 0x04C2 (4 B): Bard Spell Count (Level 4)
    pub bard_spell_count_level_4: u32,
    /// 0x04C6 (4 B): Bard Spell Count (Level 5)
    pub bard_spell_count_level_5: u32,
    /// 0x04CA (4 B): Bard Spell Count (Level 6)
    pub bard_spell_count_level_6: u32,
    /// 0x04CE (4 B): Bard Spell Count (Level 7)
    pub bard_spell_count_level_7: u32,
    /// 0x04D2 (4 B): Bard Spell Count (Level 8)
    pub bard_spell_count_level_8: u32,
    /// 0x04D6 (4 B): Bard Spell Count (Level 9)
    pub bard_spell_count_level_9: u32,
    /// 0x04DA (4 B): Cleric Spell Count (Level 1)
    pub cleric_spell_count_level_1: u32,
    /// 0x04DE (4 B): Cleric Spell Count (Level 2)
    pub cleric_spell_count_level_2: u32,
    /// 0x04E2 (4 B): Cleric Spell Count (Level 3)
    pub cleric_spell_count_level_3: u32,
    /// 0x04E6 (4 B): Cleric Spell Count (Level 4)
    pub cleric_spell_count_level_4: u32,
    /// 0x04EA (4 B): Cleric Spell Count (Level 5)
    pub cleric_spell_count_level_5: u32,
    /// 0x04EE (4 B): Cleric Spell Count (Level 6)
    pub cleric_spell_count_level_6: u32,
    /// 0x04F2 (4 B): Cleric Spell Count (Level 7)
    pub cleric_spell_count_level_7: u32,
    /// 0x04F6 (4 B): Cleric Spell Count (Level 8)
    pub cleric_spell_count_level_8: u32,
    /// 0x04FA (4 B): Cleric Spell Count (Level 9)
    pub cleric_spell_count_level_9: u32,
    /// 0x04FE (4 B): Druid Spell Count (Level 1)
    pub druid_spell_count_level_1: u32,
    /// 0x0502 (4 B): Druid Spell Count (Level 2)
    pub druid_spell_count_level_2: u32,
    /// 0x0506 (4 B): Druid Spell Count (Level 3)
    pub druid_spell_count_level_3: u32,
    /// 0x050A (4 B): Druid Spell Count (Level 4)
    pub druid_spell_count_level_4: u32,
    /// 0x050E (4 B): Druid Spell Count (Level 5)
    pub druid_spell_count_level_5: u32,
    /// 0x0512 (4 B): Druid Spell Count (Level 6)
    pub druid_spell_count_level_6: u32,
    /// 0x0516 (4 B): Druid Spell Count (Level 7)
    pub druid_spell_count_level_7: u32,
    /// 0x051A (4 B): Druid Spell Count (Level 8)
    pub druid_spell_count_level_8: u32,
    /// 0x051E (4 B): Druid Spell Count (Level 9)
    pub druid_spell_count_level_9: u32,
    /// 0x0522 (4 B): Paladin Spell Count (Level 1)
    pub paladin_spell_count_level_1: u32,
    /// 0x0526 (4 B): Paladin Spell Count (Level 2)
    pub paladin_spell_count_level_2: u32,
    /// 0x052A (4 B): Paladin Spell Count (Level 3)
    pub paladin_spell_count_level_3: u32,
    /// 0x052E (4 B): Paladin Spell Count (Level 4)
    pub paladin_spell_count_level_4: u32,
    /// 0x0532 (4 B): Paladin Spell Count (Level 5)
    pub paladin_spell_count_level_5: u32,
    /// 0x0536 (4 B): Paladin Spell Count (Level 6)
    pub paladin_spell_count_level_6: u32,
    /// 0x053A (4 B): Paladin Spell Count (Level 7)
    pub paladin_spell_count_level_7: u32,
    /// 0x053E (4 B): Paladin Spell Count (Level 8)
    pub paladin_spell_count_level_8: u32,
    /// 0x0542 (4 B): Paladin Spell Count (Level 9)
    pub paladin_spell_count_level_9: u32,
    /// 0x0546 (4 B): Ranger Spell Count (Level 1)
    pub ranger_spell_count_level_1: u32,
    /// 0x054A (4 B): Ranger Spell Count (Level 2)
    pub ranger_spell_count_level_2: u32,
    /// 0x054E (4 B): Ranger Spell Count (Level 3)
    pub ranger_spell_count_level_3: u32,
    /// 0x0552 (4 B): Ranger Spell Count (Level 4)
    pub ranger_spell_count_level_4: u32,
    /// 0x0556 (4 B): Ranger Spell Count (Level 5)
    pub ranger_spell_count_level_5: u32,
    /// 0x055A (4 B): Ranger Spell Count (Level 6)
    pub ranger_spell_count_level_6: u32,
    /// 0x055E (4 B): Ranger Spell Count (Level 7)
    pub ranger_spell_count_level_7: u32,
    /// 0x0562 (4 B): Ranger Spell Count (Level 8)
    pub ranger_spell_count_level_8: u32,
    /// 0x0566 (4 B): Ranger Spell Count (Level 9)
    pub ranger_spell_count_level_9: u32,
    /// 0x056A (4 B): Sorcerer Spell Count (Level 1)
    pub sorcerer_spell_count_level_1: u32,
    /// 0x056E (4 B): Sorcerer Spell Count (Level 2)
    pub sorcerer_spell_count_level_2: u32,
    /// 0x0572 (4 B): Sorcerer Spell Count (Level 3)
    pub sorcerer_spell_count_level_3: u32,
    /// 0x0576 (4 B): Sorcerer Spell Count (Level 4)
    pub sorcerer_spell_count_level_4: u32,
    /// 0x057A (4 B): Sorcerer Spell Count (Level 5)
    pub sorcerer_spell_count_level_5: u32,
    /// 0x057E (4 B): Sorcerer Spell Count (Level 6)
    pub sorcerer_spell_count_level_6: u32,
    /// 0x0582 (4 B): Sorcerer Spell Count (Level 7)
    pub sorcerer_spell_count_level_7: u32,
    /// 0x0586 (4 B): Sorcerer Spell Count (Level 8)
    pub sorcerer_spell_count_level_8: u32,
    /// 0x058A (4 B): Sorcerer Spell Count (Level 9)
    pub sorcerer_spell_count_level_9: u32,
    /// 0x058E (4 B): Wizard Spell Count (Level 1)
    pub wizard_spell_count_level_1: u32,
    /// 0x0592 (4 B): Wizard Spell Count (Level 2)
    pub wizard_spell_count_level_2: u32,
    /// 0x0596 (4 B): Wizard Spell Count (Level 3)
    pub wizard_spell_count_level_3: u32,
    /// 0x059A (4 B): Wizard Spell Count (Level 4)
    pub wizard_spell_count_level_4: u32,
    /// 0x059E (4 B): Wizard Spell Count (Level 5)
    pub wizard_spell_count_level_5: u32,
    /// 0x05A2 (4 B): Wizard Spell Count (Level 6)
    pub wizard_spell_count_level_6: u32,
    /// 0x05A6 (4 B): Wizard Spell Count (Level 7)
    pub wizard_spell_count_level_7: u32,
    /// 0x05AA (4 B): Wizard Spell Count (Level 8)
    pub wizard_spell_count_level_8: u32,
    /// 0x05AE (4 B): Wizard Spell Count (Level 9)
    pub wizard_spell_count_level_9: u32,
    /// 0x05B2 (4 B): Domain1 Spell Offset
    pub domain1_spell_offset: u32,
    /// 0x05B6 (4 B): Domain2 Spell Offset
    pub domain2_spell_offset: u32,
    /// 0x05BA (4 B): Domain3 Spell Offset
    pub domain3_spell_offset: u32,
    /// 0x05BE (4 B): Domain4 Spell Offset
    pub domain4_spell_offset: u32,
    /// 0x05C2 (4 B): Domain5 Spell Offset
    pub domain5_spell_offset: u32,
    /// 0x05C6 (4 B): Domain6 Spell Offset
    pub domain6_spell_offset: u32,
    /// 0x05CA (4 B): Domain7 Spell Offset
    pub domain7_spell_offset: u32,
    /// 0x05CE (4 B): Domain8 Spell Offset
    pub domain8_spell_offset: u32,
    /// 0x05D2 (4 B): Domain9 Spell Offset
    pub domain9_spell_offset: u32,
    /// 0x05D6 (4 B): Domain1 Spell Count
    pub domain1_spell_count: u32,
    /// 0x05DA (4 B): Domain2 Spell Count
    pub domain2_spell_count: u32,
    /// 0x05DE (4 B): Domain3 Spell Count
    pub domain3_spell_count: u32,
    /// 0x05E2 (4 B): Domain4 Spell Count
    pub domain4_spell_count: u32,
    /// 0x05E6 (4 B): Domain5 Spell Count
    pub domain5_spell_count: u32,
    /// 0x05EA (4 B): Domain6 Spell Count
    pub domain6_spell_count: u32,
    /// 0x05EE (4 B): Domain7 Spell Count
    pub domain7_spell_count: u32,
    /// 0x05F2 (4 B): Domain8 Spell Count
    pub domain8_spell_count: u32,
    /// 0x05F6 (4 B): Domain9 Spell Count
    pub domain9_spell_count: u32,
    /// 0x05FA (4 B): Abilities Offset
    pub abilities_offset: u32,
    /// 0x05FE (4 B): Abilities Count
    pub abilities_count: u32,
    /// 0x0602 (4 B): Song Offset
    pub song_offset: u32,
    /// 0x0606 (4 B): Song Count
    pub song_count: u32,
    /// 0x060A (4 B): Shapes Offset
    pub shapes_offset: u32,
    /// 0x060E (4 B): Shapes Count
    pub shapes_count: u32,
    /// 0x0612 (4 B): Item slots Offset
    pub item_slots_offset: u32,
    /// 0x0616 (4 B): Item Offset
    pub item_offset: u32,
    /// 0x061A (4 B): Item Count
    pub item_count: u32,
    /// 0x061E (4 B): Effects Offset
    pub effects_offset: u32,
    /// 0x0622 (4 B): Effects Count
    pub effects_count: u32,
    /// 0x0626 (8 B): Dialog
    pub dialog: String,
}

pub(crate) fn parse_header_v2_2(header: &[u8]) -> std::io::Result<CreHeaderV22> {
    debug_assert_eq!(header.len(), 1582);
    let read_u8 = |o: usize| header[o];
    let read_i8 = |o: usize| header[o] as i8;
    let read_u16 = |o: usize| u16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_i16 = |o: usize| i16::from_le_bytes(header[o..o+2].try_into().unwrap());
    let read_u32 = |o: usize| u32::from_le_bytes(header[o..o+4].try_into().unwrap());
    let read_i32 = |o: usize| i32::from_le_bytes(header[o..o+4].try_into().unwrap());
    Ok(CreHeaderV22 {
        signature: header[0x0000..0x0004].to_vec(),
        version: header[0x0004..0x0008].to_vec(),
        long_name: read_u32(0x0008),
        short_name_tooltip: read_u32(0x000C),
        creature_flags: read_u32(0x0010),
        xp_gained_for_killing_this_creature: read_u32(0x0014),
        creature_power_level_for_summoning_spells: read_u32(0x0018),
        gold_carried: read_u32(0x001C),
        permanent_status_flags_state_ids: read_u32(0x0020),
        current_hit_points: read_u16(0x0024),
        maximum_hit_points: read_u16(0x0026),
        animation_id_animate_ids_0x002c: read_u32(0x0028),
        _padding_01: header[0x002C..0x002D].to_vec(),
        minor_colour_index_bg1_animations: read_u8(0x002D),
        major_colour_index_bg1_animations: read_u8(0x002E),
        skin_colour_index_bg1_animations: read_u8(0x002F),
        leather_colour_index_bg1_animations: read_u8(0x0030),
        armor_colour_index_bg1_animations: read_u8(0x0031),
        hair_colour_index_bg1_animations: read_u8(0x0032),
        eff_structure_version_0_version_1: read_u8(0x0033),
        small_portrait_bmp: read_resref(&header[0x0034..0x003C]),
        large_portrait_bmp: read_resref(&header[0x003C..0x0044]),
        reputation: read_i8(0x0044),
        hide_in_shadows_base: read_u8(0x0045),
        armor_class: read_i16(0x0046),
        armor_class_crushing_attacks_modifier: read_i16(0x0048),
        armor_class_missile_attacks_modifier: read_i16(0x004A),
        armor_class_piercing_attacks_modifier: read_i16(0x004C),
        armor_class_slashing_attacks_modifier: read_i16(0x004E),
        base_attack_bonus_bab_for_non: read_u8(0x0050),
        number_of_attacks: read_u8(0x0051),
        save_versus_fortitude: read_u8(0x0052),
        save_versus_reflex: read_u8(0x0053),
        save_versus_will: read_u8(0x0054),
        resist_fire: read_u8(0x0055),
        resist_cold: read_u8(0x0056),
        resist_electricity: read_u8(0x0057),
        resist_acid: read_u8(0x0058),
        resist_magic: read_u8(0x0059),
        resist_magic_fire: read_u8(0x005A),
        resist_magic_cold: read_u8(0x005B),
        resist_slashing: read_u8(0x005C),
        resist_crushing: read_u8(0x005D),
        resist_piercing: read_u8(0x005E),
        resist_missile: read_u8(0x005F),
        resist_magic_damage: read_u8(0x0060),
        unknown_further_resistances: header[0x0061..0x0065].to_vec(),
        fatigue: read_u8(0x0065),
        intoxication: read_u8(0x0066),
        luck: read_u8(0x0067),
        turn_undead_level: read_u8(0x0068),
        unknown: header[0x0069..0x008A].to_vec(),
        total_levels: read_u8(0x008A),
        barbarian_levels: read_u8(0x008B),
        bard_levels: read_u8(0x008C),
        cleric_levels: read_u8(0x008D),
        druid_levels: read_u8(0x008E),
        fighter_levels: read_u8(0x008F),
        monk: read_u8(0x0090),
        paladin_levels: read_u8(0x0091),
        ranger_levels: read_u8(0x0092),
        rogue_levels: read_u8(0x0093),
        sorcerer_levels: read_u8(0x0094),
        wizard_levels: read_u8(0x0095),
        unknown_2: header[0x0096..0x00AC].to_vec(),
        strref_s_most_are_connected_with: header[0x00AC..0x01AC].to_vec(),
        team_script: read_resref(&header[0x01AC..0x01B4]),
        special_script_1: read_resref(&header[0x01B4..0x01BC]),
        creature_enchantment_level: read_u8(0x01BC),
        unknown_3: header[0x01BD..0x01C0].to_vec(),
        feats_1: read_u32(0x01C0),
        feats_2: read_u32(0x01C4),
        feats_3: read_u32(0x01C8),
        unknown_4: header[0x01CC..0x01D8].to_vec(),
        mw_bow: read_u8(0x01D8),
        sw_crossbow: header[0x01D9..0x01DA].to_vec(),
        sw_missile: read_u8(0x01DA),
        mw_axe: read_u8(0x01DB),
        sw_mace: read_u8(0x01DC),
        mw_flail: read_u8(0x01DD),
        mw_polearm: read_u8(0x01DE),
        mw_hammer: read_u8(0x01DF),
        sw_quarterstaff: read_u8(0x01E0),
        mw_great_sword: read_u8(0x01E1),
        mw_large_sword: read_u8(0x01E2),
        sw_small_blade: read_u8(0x01E3),
        toughness: read_u8(0x01E4),
        armored_arcana: read_u8(0x01E5),
        cleave: read_u8(0x01E6),
        armor_proficiency: read_u8(0x01E7),
        sf_enchantment: read_u8(0x01E8),
        sf_evocation: read_u8(0x01E9),
        sf_necromancy: read_u8(0x01EA),
        sf_transmutation: read_u8(0x01EB),
        spell_penetration: read_u8(0x01EC),
        extra_rage: read_u8(0x01ED),
        extra_wild_shape: read_u8(0x01EE),
        extra_smiting: read_u8(0x01EF),
        extra_turning: read_u8(0x01F0),
        ew_bastard_sword: read_u8(0x01F1),
        unknown_5: header[0x01F2..0x0218].to_vec(),
        alchemy: read_u8(0x0218),
        animal_empathy: read_u8(0x0219),
        bluff: read_u8(0x021A),
        concentration: read_u8(0x021B),
        diplomacy: read_u8(0x021C),
        disable_device: read_u8(0x021D),
        hide: read_u8(0x021E),
        intimidate: read_u8(0x021F),
        knowledge_arcana: read_u8(0x0220),
        move_silently: read_u8(0x0221),
        open_lock: read_u8(0x0222),
        pick_pocket: read_u8(0x0223),
        search: read_u8(0x0224),
        spellcraft: read_u8(0x0225),
        use_magic_device: read_u8(0x0226),
        wilderness_law: read_u8(0x0227),
        unknown_6: header[0x0228..0x025A].to_vec(),
        xp_category_values_from_moncrate_2da: read_u8(0x025A),
        favoured_enemy_1: read_u8(0x025B),
        favoured_enemy_2: read_u8(0x025C),
        favoured_enemy_3: read_u8(0x025D),
        favoured_enemy_4: read_u8(0x025E),
        favoured_enemy_5: read_u8(0x025F),
        favoured_enemy_6: read_u8(0x0260),
        favoured_enemy_7: read_u8(0x0261),
        favoured_enemy_8: read_u8(0x0262),
        subrace_subrace_ids: read_u8(0x0263),
        unknown_7: read_u16(0x0264),
        strength: read_u8(0x0266),
        intelligence: read_u8(0x0267),
        wisdom: read_u8(0x0268),
        dexterity: read_u8(0x0269),
        constitution: read_u8(0x026A),
        charisma: read_u8(0x026B),
        unknown_8: read_u32(0x026C),
        kit_bitfield: read_u32(0x0270),
        creature_script_override: read_resref(&header[0x0274..0x027C]),
        creature_script_special_script_3: read_resref(&header[0x027C..0x0284]),
        creature_script_special_script_2: read_resref(&header[0x0284..0x028C]),
        _padding_02: header[0x028C..0x0294].to_vec(),
        creature_script_movement_script: read_resref(&header[0x0294..0x029C]),
        visible_0_no_1_yes: read_u8(0x029C),
        set_scriptname_dead_variable_on_death: read_u8(0x029D),
        set_kill_racename_cnt_on_death: read_u8(0x029E),
        unknown_9: read_u8(0x029F),
        internals_as_used_by_setinternal: header[0x02A0..0x02AA].to_vec(),
        secondary_death_variable_set_to_1: header[0x02AA..0x02CA].to_vec(),
        tertiary_death_variable_incremented_by_1: header[0x02CA..0x02EA].to_vec(),
        unknown_10: read_u16(0x02EA),
        saved_location_x_coordinate: read_u16(0x02EC),
        _padding_03: header[0x02EE..0x02EF].to_vec(),
        saved_location_y_coordinate: read_u16(0x02EF),
        _padding_04: header[0x02F1..0x02F2].to_vec(),
        unknown_11: header[0x02F2..0x0301].to_vec(),
        minimum_transparency_fade_in_fade_out: read_u8(0x0301),
        fade_speed_fade_in_fade_out: read_u8(0x0302),
        specflag_values: read_u8(0x0303),
        visible: read_u8(0x0304),
        unknown_12: read_u8(0x0305),
        unknown_13: read_u8(0x0306),
        remaining_skill_points_after_level_up: read_u8(0x0307),
        unknown_14: header[0x0308..0x0384].to_vec(),
        enemy_ally_ea_ids: read_u8(0x0384),
        general_general_ids: read_u8(0x0385),
        race_race_ids: read_u8(0x0386),
        class_class_ids_not_updated_when: read_u8(0x0387),
        specific_specific_ids: read_u8(0x0388),
        sex_gender_ids: read_u8(0x0389),
        object_ids_references: header[0x038A..0x038F].to_vec(),
        alignment_alignmen_ids: read_u8(0x038F),
        global_actor_enumeration_value: read_u16(0x0390),
        local_area_actor_enumeration_value: read_u16(0x0392),
        death_variable: header[0x0394..0x03B4].to_vec(),
        avclass_value_duplicate_of_class_used: read_u16(0x03B4),
        classmsk_bitfield_value_duplicate_of_class: read_u16(0x03B6),
        unknown_15: read_u16(0x03B8),
        bard_spell_offset_level_1: read_u32(0x03BA),
        bard_spell_offset_level_2: read_u32(0x03BE),
        bard_spell_offset_level_3: read_u32(0x03C2),
        bard_spell_offset_level_4: read_u32(0x03C6),
        bard_spell_offset_level_5: read_u32(0x03CA),
        bard_spell_offset_level_6: read_u32(0x03CE),
        bard_spell_offset_level_7: read_u32(0x03D2),
        bard_spell_offset_level_8: read_u32(0x03D6),
        bard_spell_offset_level_9: read_u32(0x03DA),
        cleric_spell_offset_level_1: read_u32(0x03DE),
        cleric_spell_offset_level_2: read_u32(0x03E2),
        cleric_spell_offset_level_3: read_u32(0x03E6),
        cleric_spell_offset_level_4: read_u32(0x03EA),
        cleric_spell_offset_level_5: read_u32(0x03EE),
        cleric_spell_offset_level_6: read_u32(0x03F2),
        cleric_spell_offset_level_7: read_u32(0x03F6),
        cleric_spell_offset_level_8: read_u32(0x03FA),
        cleric_spell_offset_level_9: read_u32(0x03FE),
        druid_spell_offset_level_1: read_u32(0x0402),
        druid_spell_offset_level_2: read_u32(0x0406),
        druid_spell_offset_level_3: read_u32(0x040A),
        druid_spell_offset_level_4: read_u32(0x040E),
        druid_spell_offset_level_5: read_u32(0x0412),
        druid_spell_offset_level_6: read_u32(0x0416),
        druid_spell_offset_level_7: read_u32(0x041A),
        druid_spell_offset_level_8: read_u32(0x041E),
        druid_spell_offset_level_9: read_u32(0x0422),
        paladin_spell_offset_level_1: read_u32(0x0426),
        paladin_spell_offset_level_2: read_u32(0x042A),
        paladin_spell_offset_level_3: read_u32(0x042E),
        paladin_spell_offset_level_4: read_u32(0x0432),
        paladin_spell_offset_level_5: read_u32(0x0436),
        paladin_spell_offset_level_6: read_u32(0x043A),
        paladin_spell_offset_level_7: read_u32(0x043E),
        paladin_spell_offset_level_8: read_u32(0x0442),
        paladin_spell_offset_level_9: read_u32(0x0446),
        ranger_spell_offset_level_1: read_u32(0x044A),
        ranger_spell_offset_level_2: read_u32(0x044E),
        ranger_spell_offset_level_3: read_u32(0x0452),
        ranger_spell_offset_level_4: read_u32(0x0456),
        ranger_spell_offset_level_5: read_u32(0x045A),
        ranger_spell_offset_level_6: read_u32(0x045E),
        ranger_spell_offset_level_7: read_u32(0x0462),
        ranger_spell_offset_level_8: read_u32(0x0466),
        ranger_spell_offset_level_9: read_u32(0x046A),
        sorcerer_spell_offset_level_1: read_u32(0x046E),
        sorcerer_spell_offset_level_2: read_u32(0x0472),
        sorcerer_spell_offset_level_3: read_u32(0x0476),
        sorcerer_spell_offset_level_4: read_u32(0x047A),
        sorcerer_spell_offset_level_5: read_u32(0x047E),
        sorcerer_spell_offset_level_6: read_u32(0x0482),
        sorcerer_spell_offset_level_7: read_u32(0x0486),
        sorcerer_spell_offset_level_8: read_u32(0x048A),
        sorcerer_spell_offset_level_9: read_u32(0x048E),
        wizard_spell_offset_level_1: read_u32(0x0492),
        wizard_spell_offset_level_2: read_u32(0x0496),
        wizard_spell_offset_level_3: read_u32(0x049A),
        wizard_spell_offset_level_4: read_u32(0x049E),
        wizard_spell_offset_level_5: read_u32(0x04A2),
        wizard_spell_offset_level_6: read_u32(0x04A6),
        wizard_spell_offset_level_7: read_u32(0x04AA),
        wizard_spell_offset_level_8: read_u32(0x04AE),
        wizard_spell_offset_level_9: read_u32(0x04B2),
        bard_spell_count_level_1: read_u32(0x04B6),
        bard_spell_count_level_2: read_u32(0x04BA),
        bard_spell_count_level_3: read_u32(0x04BE),
        bard_spell_count_level_4: read_u32(0x04C2),
        bard_spell_count_level_5: read_u32(0x04C6),
        bard_spell_count_level_6: read_u32(0x04CA),
        bard_spell_count_level_7: read_u32(0x04CE),
        bard_spell_count_level_8: read_u32(0x04D2),
        bard_spell_count_level_9: read_u32(0x04D6),
        cleric_spell_count_level_1: read_u32(0x04DA),
        cleric_spell_count_level_2: read_u32(0x04DE),
        cleric_spell_count_level_3: read_u32(0x04E2),
        cleric_spell_count_level_4: read_u32(0x04E6),
        cleric_spell_count_level_5: read_u32(0x04EA),
        cleric_spell_count_level_6: read_u32(0x04EE),
        cleric_spell_count_level_7: read_u32(0x04F2),
        cleric_spell_count_level_8: read_u32(0x04F6),
        cleric_spell_count_level_9: read_u32(0x04FA),
        druid_spell_count_level_1: read_u32(0x04FE),
        druid_spell_count_level_2: read_u32(0x0502),
        druid_spell_count_level_3: read_u32(0x0506),
        druid_spell_count_level_4: read_u32(0x050A),
        druid_spell_count_level_5: read_u32(0x050E),
        druid_spell_count_level_6: read_u32(0x0512),
        druid_spell_count_level_7: read_u32(0x0516),
        druid_spell_count_level_8: read_u32(0x051A),
        druid_spell_count_level_9: read_u32(0x051E),
        paladin_spell_count_level_1: read_u32(0x0522),
        paladin_spell_count_level_2: read_u32(0x0526),
        paladin_spell_count_level_3: read_u32(0x052A),
        paladin_spell_count_level_4: read_u32(0x052E),
        paladin_spell_count_level_5: read_u32(0x0532),
        paladin_spell_count_level_6: read_u32(0x0536),
        paladin_spell_count_level_7: read_u32(0x053A),
        paladin_spell_count_level_8: read_u32(0x053E),
        paladin_spell_count_level_9: read_u32(0x0542),
        ranger_spell_count_level_1: read_u32(0x0546),
        ranger_spell_count_level_2: read_u32(0x054A),
        ranger_spell_count_level_3: read_u32(0x054E),
        ranger_spell_count_level_4: read_u32(0x0552),
        ranger_spell_count_level_5: read_u32(0x0556),
        ranger_spell_count_level_6: read_u32(0x055A),
        ranger_spell_count_level_7: read_u32(0x055E),
        ranger_spell_count_level_8: read_u32(0x0562),
        ranger_spell_count_level_9: read_u32(0x0566),
        sorcerer_spell_count_level_1: read_u32(0x056A),
        sorcerer_spell_count_level_2: read_u32(0x056E),
        sorcerer_spell_count_level_3: read_u32(0x0572),
        sorcerer_spell_count_level_4: read_u32(0x0576),
        sorcerer_spell_count_level_5: read_u32(0x057A),
        sorcerer_spell_count_level_6: read_u32(0x057E),
        sorcerer_spell_count_level_7: read_u32(0x0582),
        sorcerer_spell_count_level_8: read_u32(0x0586),
        sorcerer_spell_count_level_9: read_u32(0x058A),
        wizard_spell_count_level_1: read_u32(0x058E),
        wizard_spell_count_level_2: read_u32(0x0592),
        wizard_spell_count_level_3: read_u32(0x0596),
        wizard_spell_count_level_4: read_u32(0x059A),
        wizard_spell_count_level_5: read_u32(0x059E),
        wizard_spell_count_level_6: read_u32(0x05A2),
        wizard_spell_count_level_7: read_u32(0x05A6),
        wizard_spell_count_level_8: read_u32(0x05AA),
        wizard_spell_count_level_9: read_u32(0x05AE),
        domain1_spell_offset: read_u32(0x05B2),
        domain2_spell_offset: read_u32(0x05B6),
        domain3_spell_offset: read_u32(0x05BA),
        domain4_spell_offset: read_u32(0x05BE),
        domain5_spell_offset: read_u32(0x05C2),
        domain6_spell_offset: read_u32(0x05C6),
        domain7_spell_offset: read_u32(0x05CA),
        domain8_spell_offset: read_u32(0x05CE),
        domain9_spell_offset: read_u32(0x05D2),
        domain1_spell_count: read_u32(0x05D6),
        domain2_spell_count: read_u32(0x05DA),
        domain3_spell_count: read_u32(0x05DE),
        domain4_spell_count: read_u32(0x05E2),
        domain5_spell_count: read_u32(0x05E6),
        domain6_spell_count: read_u32(0x05EA),
        domain7_spell_count: read_u32(0x05EE),
        domain8_spell_count: read_u32(0x05F2),
        domain9_spell_count: read_u32(0x05F6),
        abilities_offset: read_u32(0x05FA),
        abilities_count: read_u32(0x05FE),
        song_offset: read_u32(0x0602),
        song_count: read_u32(0x0606),
        shapes_offset: read_u32(0x060A),
        shapes_count: read_u32(0x060E),
        item_slots_offset: read_u32(0x0612),
        item_offset: read_u32(0x0616),
        item_count: read_u32(0x061A),
        effects_offset: read_u32(0x061E),
        effects_count: read_u32(0x0622),
        dialog: read_resref(&header[0x0626..0x062E]),
    })
}

pub(crate) fn serialize_header_v2_2(h: &CreHeaderV22) -> Vec<u8> {
    let mut buf = vec![0u8; 1582];
    { let src = &h.signature; let n = src.len().min(4); buf[0x0000..0x0000+n].copy_from_slice(&src[..n]); }
    { let src = &h.version; let n = src.len().min(4); buf[0x0004..0x0004+n].copy_from_slice(&src[..n]); }
    buf[0x0008..0x000C].copy_from_slice(&h.long_name.to_le_bytes());
    buf[0x000C..0x0010].copy_from_slice(&h.short_name_tooltip.to_le_bytes());
    buf[0x0010..0x0014].copy_from_slice(&h.creature_flags.to_le_bytes());
    buf[0x0014..0x0018].copy_from_slice(&h.xp_gained_for_killing_this_creature.to_le_bytes());
    buf[0x0018..0x001C].copy_from_slice(&h.creature_power_level_for_summoning_spells.to_le_bytes());
    buf[0x001C..0x0020].copy_from_slice(&h.gold_carried.to_le_bytes());
    buf[0x0020..0x0024].copy_from_slice(&h.permanent_status_flags_state_ids.to_le_bytes());
    buf[0x0024..0x0026].copy_from_slice(&h.current_hit_points.to_le_bytes());
    buf[0x0026..0x0028].copy_from_slice(&h.maximum_hit_points.to_le_bytes());
    buf[0x0028..0x002C].copy_from_slice(&h.animation_id_animate_ids_0x002c.to_le_bytes());
    { let src = &h._padding_01; let n = src.len().min(1); buf[0x002C..0x002C+n].copy_from_slice(&src[..n]); }
    buf[0x002D] = h.minor_colour_index_bg1_animations;
    buf[0x002E] = h.major_colour_index_bg1_animations;
    buf[0x002F] = h.skin_colour_index_bg1_animations;
    buf[0x0030] = h.leather_colour_index_bg1_animations;
    buf[0x0031] = h.armor_colour_index_bg1_animations;
    buf[0x0032] = h.hair_colour_index_bg1_animations;
    buf[0x0033] = h.eff_structure_version_0_version_1;
    write_resref(&mut buf[0x0034..0x003C], &h.small_portrait_bmp);
    write_resref(&mut buf[0x003C..0x0044], &h.large_portrait_bmp);
    buf[0x0044] = h.reputation as u8;
    buf[0x0045] = h.hide_in_shadows_base;
    buf[0x0046..0x0048].copy_from_slice(&h.armor_class.to_le_bytes());
    buf[0x0048..0x004A].copy_from_slice(&h.armor_class_crushing_attacks_modifier.to_le_bytes());
    buf[0x004A..0x004C].copy_from_slice(&h.armor_class_missile_attacks_modifier.to_le_bytes());
    buf[0x004C..0x004E].copy_from_slice(&h.armor_class_piercing_attacks_modifier.to_le_bytes());
    buf[0x004E..0x0050].copy_from_slice(&h.armor_class_slashing_attacks_modifier.to_le_bytes());
    buf[0x0050] = h.base_attack_bonus_bab_for_non;
    buf[0x0051] = h.number_of_attacks;
    buf[0x0052] = h.save_versus_fortitude;
    buf[0x0053] = h.save_versus_reflex;
    buf[0x0054] = h.save_versus_will;
    buf[0x0055] = h.resist_fire;
    buf[0x0056] = h.resist_cold;
    buf[0x0057] = h.resist_electricity;
    buf[0x0058] = h.resist_acid;
    buf[0x0059] = h.resist_magic;
    buf[0x005A] = h.resist_magic_fire;
    buf[0x005B] = h.resist_magic_cold;
    buf[0x005C] = h.resist_slashing;
    buf[0x005D] = h.resist_crushing;
    buf[0x005E] = h.resist_piercing;
    buf[0x005F] = h.resist_missile;
    buf[0x0060] = h.resist_magic_damage;
    { let src = &h.unknown_further_resistances; let n = src.len().min(4); buf[0x0061..0x0061+n].copy_from_slice(&src[..n]); }
    buf[0x0065] = h.fatigue;
    buf[0x0066] = h.intoxication;
    buf[0x0067] = h.luck;
    buf[0x0068] = h.turn_undead_level;
    { let src = &h.unknown; let n = src.len().min(33); buf[0x0069..0x0069+n].copy_from_slice(&src[..n]); }
    buf[0x008A] = h.total_levels;
    buf[0x008B] = h.barbarian_levels;
    buf[0x008C] = h.bard_levels;
    buf[0x008D] = h.cleric_levels;
    buf[0x008E] = h.druid_levels;
    buf[0x008F] = h.fighter_levels;
    buf[0x0090] = h.monk;
    buf[0x0091] = h.paladin_levels;
    buf[0x0092] = h.ranger_levels;
    buf[0x0093] = h.rogue_levels;
    buf[0x0094] = h.sorcerer_levels;
    buf[0x0095] = h.wizard_levels;
    { let src = &h.unknown_2; let n = src.len().min(22); buf[0x0096..0x0096+n].copy_from_slice(&src[..n]); }
    { let src = &h.strref_s_most_are_connected_with; let n = src.len().min(256); buf[0x00AC..0x00AC+n].copy_from_slice(&src[..n]); }
    write_resref(&mut buf[0x01AC..0x01B4], &h.team_script);
    write_resref(&mut buf[0x01B4..0x01BC], &h.special_script_1);
    buf[0x01BC] = h.creature_enchantment_level;
    { let src = &h.unknown_3; let n = src.len().min(3); buf[0x01BD..0x01BD+n].copy_from_slice(&src[..n]); }
    buf[0x01C0..0x01C4].copy_from_slice(&h.feats_1.to_le_bytes());
    buf[0x01C4..0x01C8].copy_from_slice(&h.feats_2.to_le_bytes());
    buf[0x01C8..0x01CC].copy_from_slice(&h.feats_3.to_le_bytes());
    { let src = &h.unknown_4; let n = src.len().min(12); buf[0x01CC..0x01CC+n].copy_from_slice(&src[..n]); }
    buf[0x01D8] = h.mw_bow;
    { let src = &h.sw_crossbow; let n = src.len().min(1); buf[0x01D9..0x01D9+n].copy_from_slice(&src[..n]); }
    buf[0x01DA] = h.sw_missile;
    buf[0x01DB] = h.mw_axe;
    buf[0x01DC] = h.sw_mace;
    buf[0x01DD] = h.mw_flail;
    buf[0x01DE] = h.mw_polearm;
    buf[0x01DF] = h.mw_hammer;
    buf[0x01E0] = h.sw_quarterstaff;
    buf[0x01E1] = h.mw_great_sword;
    buf[0x01E2] = h.mw_large_sword;
    buf[0x01E3] = h.sw_small_blade;
    buf[0x01E4] = h.toughness;
    buf[0x01E5] = h.armored_arcana;
    buf[0x01E6] = h.cleave;
    buf[0x01E7] = h.armor_proficiency;
    buf[0x01E8] = h.sf_enchantment;
    buf[0x01E9] = h.sf_evocation;
    buf[0x01EA] = h.sf_necromancy;
    buf[0x01EB] = h.sf_transmutation;
    buf[0x01EC] = h.spell_penetration;
    buf[0x01ED] = h.extra_rage;
    buf[0x01EE] = h.extra_wild_shape;
    buf[0x01EF] = h.extra_smiting;
    buf[0x01F0] = h.extra_turning;
    buf[0x01F1] = h.ew_bastard_sword;
    { let src = &h.unknown_5; let n = src.len().min(38); buf[0x01F2..0x01F2+n].copy_from_slice(&src[..n]); }
    buf[0x0218] = h.alchemy;
    buf[0x0219] = h.animal_empathy;
    buf[0x021A] = h.bluff;
    buf[0x021B] = h.concentration;
    buf[0x021C] = h.diplomacy;
    buf[0x021D] = h.disable_device;
    buf[0x021E] = h.hide;
    buf[0x021F] = h.intimidate;
    buf[0x0220] = h.knowledge_arcana;
    buf[0x0221] = h.move_silently;
    buf[0x0222] = h.open_lock;
    buf[0x0223] = h.pick_pocket;
    buf[0x0224] = h.search;
    buf[0x0225] = h.spellcraft;
    buf[0x0226] = h.use_magic_device;
    buf[0x0227] = h.wilderness_law;
    { let src = &h.unknown_6; let n = src.len().min(50); buf[0x0228..0x0228+n].copy_from_slice(&src[..n]); }
    buf[0x025A] = h.xp_category_values_from_moncrate_2da;
    buf[0x025B] = h.favoured_enemy_1;
    buf[0x025C] = h.favoured_enemy_2;
    buf[0x025D] = h.favoured_enemy_3;
    buf[0x025E] = h.favoured_enemy_4;
    buf[0x025F] = h.favoured_enemy_5;
    buf[0x0260] = h.favoured_enemy_6;
    buf[0x0261] = h.favoured_enemy_7;
    buf[0x0262] = h.favoured_enemy_8;
    buf[0x0263] = h.subrace_subrace_ids;
    buf[0x0264..0x0266].copy_from_slice(&h.unknown_7.to_le_bytes());
    buf[0x0266] = h.strength;
    buf[0x0267] = h.intelligence;
    buf[0x0268] = h.wisdom;
    buf[0x0269] = h.dexterity;
    buf[0x026A] = h.constitution;
    buf[0x026B] = h.charisma;
    buf[0x026C..0x0270].copy_from_slice(&h.unknown_8.to_le_bytes());
    buf[0x0270..0x0274].copy_from_slice(&h.kit_bitfield.to_le_bytes());
    write_resref(&mut buf[0x0274..0x027C], &h.creature_script_override);
    write_resref(&mut buf[0x027C..0x0284], &h.creature_script_special_script_3);
    write_resref(&mut buf[0x0284..0x028C], &h.creature_script_special_script_2);
    { let src = &h._padding_02; let n = src.len().min(8); buf[0x028C..0x028C+n].copy_from_slice(&src[..n]); }
    write_resref(&mut buf[0x0294..0x029C], &h.creature_script_movement_script);
    buf[0x029C] = h.visible_0_no_1_yes;
    buf[0x029D] = h.set_scriptname_dead_variable_on_death;
    buf[0x029E] = h.set_kill_racename_cnt_on_death;
    buf[0x029F] = h.unknown_9;
    { let src = &h.internals_as_used_by_setinternal; let n = src.len().min(10); buf[0x02A0..0x02A0+n].copy_from_slice(&src[..n]); }
    { let src = &h.secondary_death_variable_set_to_1; let n = src.len().min(32); buf[0x02AA..0x02AA+n].copy_from_slice(&src[..n]); }
    { let src = &h.tertiary_death_variable_incremented_by_1; let n = src.len().min(32); buf[0x02CA..0x02CA+n].copy_from_slice(&src[..n]); }
    buf[0x02EA..0x02EC].copy_from_slice(&h.unknown_10.to_le_bytes());
    buf[0x02EC..0x02EE].copy_from_slice(&h.saved_location_x_coordinate.to_le_bytes());
    { let src = &h._padding_03; let n = src.len().min(1); buf[0x02EE..0x02EE+n].copy_from_slice(&src[..n]); }
    buf[0x02EF..0x02F1].copy_from_slice(&h.saved_location_y_coordinate.to_le_bytes());
    { let src = &h._padding_04; let n = src.len().min(1); buf[0x02F1..0x02F1+n].copy_from_slice(&src[..n]); }
    { let src = &h.unknown_11; let n = src.len().min(15); buf[0x02F2..0x02F2+n].copy_from_slice(&src[..n]); }
    buf[0x0301] = h.minimum_transparency_fade_in_fade_out;
    buf[0x0302] = h.fade_speed_fade_in_fade_out;
    buf[0x0303] = h.specflag_values;
    buf[0x0304] = h.visible;
    buf[0x0305] = h.unknown_12;
    buf[0x0306] = h.unknown_13;
    buf[0x0307] = h.remaining_skill_points_after_level_up;
    { let src = &h.unknown_14; let n = src.len().min(124); buf[0x0308..0x0308+n].copy_from_slice(&src[..n]); }
    buf[0x0384] = h.enemy_ally_ea_ids;
    buf[0x0385] = h.general_general_ids;
    buf[0x0386] = h.race_race_ids;
    buf[0x0387] = h.class_class_ids_not_updated_when;
    buf[0x0388] = h.specific_specific_ids;
    buf[0x0389] = h.sex_gender_ids;
    { let src = &h.object_ids_references; let n = src.len().min(5); buf[0x038A..0x038A+n].copy_from_slice(&src[..n]); }
    buf[0x038F] = h.alignment_alignmen_ids;
    buf[0x0390..0x0392].copy_from_slice(&h.global_actor_enumeration_value.to_le_bytes());
    buf[0x0392..0x0394].copy_from_slice(&h.local_area_actor_enumeration_value.to_le_bytes());
    { let src = &h.death_variable; let n = src.len().min(32); buf[0x0394..0x0394+n].copy_from_slice(&src[..n]); }
    buf[0x03B4..0x03B6].copy_from_slice(&h.avclass_value_duplicate_of_class_used.to_le_bytes());
    buf[0x03B6..0x03B8].copy_from_slice(&h.classmsk_bitfield_value_duplicate_of_class.to_le_bytes());
    buf[0x03B8..0x03BA].copy_from_slice(&h.unknown_15.to_le_bytes());
    buf[0x03BA..0x03BE].copy_from_slice(&h.bard_spell_offset_level_1.to_le_bytes());
    buf[0x03BE..0x03C2].copy_from_slice(&h.bard_spell_offset_level_2.to_le_bytes());
    buf[0x03C2..0x03C6].copy_from_slice(&h.bard_spell_offset_level_3.to_le_bytes());
    buf[0x03C6..0x03CA].copy_from_slice(&h.bard_spell_offset_level_4.to_le_bytes());
    buf[0x03CA..0x03CE].copy_from_slice(&h.bard_spell_offset_level_5.to_le_bytes());
    buf[0x03CE..0x03D2].copy_from_slice(&h.bard_spell_offset_level_6.to_le_bytes());
    buf[0x03D2..0x03D6].copy_from_slice(&h.bard_spell_offset_level_7.to_le_bytes());
    buf[0x03D6..0x03DA].copy_from_slice(&h.bard_spell_offset_level_8.to_le_bytes());
    buf[0x03DA..0x03DE].copy_from_slice(&h.bard_spell_offset_level_9.to_le_bytes());
    buf[0x03DE..0x03E2].copy_from_slice(&h.cleric_spell_offset_level_1.to_le_bytes());
    buf[0x03E2..0x03E6].copy_from_slice(&h.cleric_spell_offset_level_2.to_le_bytes());
    buf[0x03E6..0x03EA].copy_from_slice(&h.cleric_spell_offset_level_3.to_le_bytes());
    buf[0x03EA..0x03EE].copy_from_slice(&h.cleric_spell_offset_level_4.to_le_bytes());
    buf[0x03EE..0x03F2].copy_from_slice(&h.cleric_spell_offset_level_5.to_le_bytes());
    buf[0x03F2..0x03F6].copy_from_slice(&h.cleric_spell_offset_level_6.to_le_bytes());
    buf[0x03F6..0x03FA].copy_from_slice(&h.cleric_spell_offset_level_7.to_le_bytes());
    buf[0x03FA..0x03FE].copy_from_slice(&h.cleric_spell_offset_level_8.to_le_bytes());
    buf[0x03FE..0x0402].copy_from_slice(&h.cleric_spell_offset_level_9.to_le_bytes());
    buf[0x0402..0x0406].copy_from_slice(&h.druid_spell_offset_level_1.to_le_bytes());
    buf[0x0406..0x040A].copy_from_slice(&h.druid_spell_offset_level_2.to_le_bytes());
    buf[0x040A..0x040E].copy_from_slice(&h.druid_spell_offset_level_3.to_le_bytes());
    buf[0x040E..0x0412].copy_from_slice(&h.druid_spell_offset_level_4.to_le_bytes());
    buf[0x0412..0x0416].copy_from_slice(&h.druid_spell_offset_level_5.to_le_bytes());
    buf[0x0416..0x041A].copy_from_slice(&h.druid_spell_offset_level_6.to_le_bytes());
    buf[0x041A..0x041E].copy_from_slice(&h.druid_spell_offset_level_7.to_le_bytes());
    buf[0x041E..0x0422].copy_from_slice(&h.druid_spell_offset_level_8.to_le_bytes());
    buf[0x0422..0x0426].copy_from_slice(&h.druid_spell_offset_level_9.to_le_bytes());
    buf[0x0426..0x042A].copy_from_slice(&h.paladin_spell_offset_level_1.to_le_bytes());
    buf[0x042A..0x042E].copy_from_slice(&h.paladin_spell_offset_level_2.to_le_bytes());
    buf[0x042E..0x0432].copy_from_slice(&h.paladin_spell_offset_level_3.to_le_bytes());
    buf[0x0432..0x0436].copy_from_slice(&h.paladin_spell_offset_level_4.to_le_bytes());
    buf[0x0436..0x043A].copy_from_slice(&h.paladin_spell_offset_level_5.to_le_bytes());
    buf[0x043A..0x043E].copy_from_slice(&h.paladin_spell_offset_level_6.to_le_bytes());
    buf[0x043E..0x0442].copy_from_slice(&h.paladin_spell_offset_level_7.to_le_bytes());
    buf[0x0442..0x0446].copy_from_slice(&h.paladin_spell_offset_level_8.to_le_bytes());
    buf[0x0446..0x044A].copy_from_slice(&h.paladin_spell_offset_level_9.to_le_bytes());
    buf[0x044A..0x044E].copy_from_slice(&h.ranger_spell_offset_level_1.to_le_bytes());
    buf[0x044E..0x0452].copy_from_slice(&h.ranger_spell_offset_level_2.to_le_bytes());
    buf[0x0452..0x0456].copy_from_slice(&h.ranger_spell_offset_level_3.to_le_bytes());
    buf[0x0456..0x045A].copy_from_slice(&h.ranger_spell_offset_level_4.to_le_bytes());
    buf[0x045A..0x045E].copy_from_slice(&h.ranger_spell_offset_level_5.to_le_bytes());
    buf[0x045E..0x0462].copy_from_slice(&h.ranger_spell_offset_level_6.to_le_bytes());
    buf[0x0462..0x0466].copy_from_slice(&h.ranger_spell_offset_level_7.to_le_bytes());
    buf[0x0466..0x046A].copy_from_slice(&h.ranger_spell_offset_level_8.to_le_bytes());
    buf[0x046A..0x046E].copy_from_slice(&h.ranger_spell_offset_level_9.to_le_bytes());
    buf[0x046E..0x0472].copy_from_slice(&h.sorcerer_spell_offset_level_1.to_le_bytes());
    buf[0x0472..0x0476].copy_from_slice(&h.sorcerer_spell_offset_level_2.to_le_bytes());
    buf[0x0476..0x047A].copy_from_slice(&h.sorcerer_spell_offset_level_3.to_le_bytes());
    buf[0x047A..0x047E].copy_from_slice(&h.sorcerer_spell_offset_level_4.to_le_bytes());
    buf[0x047E..0x0482].copy_from_slice(&h.sorcerer_spell_offset_level_5.to_le_bytes());
    buf[0x0482..0x0486].copy_from_slice(&h.sorcerer_spell_offset_level_6.to_le_bytes());
    buf[0x0486..0x048A].copy_from_slice(&h.sorcerer_spell_offset_level_7.to_le_bytes());
    buf[0x048A..0x048E].copy_from_slice(&h.sorcerer_spell_offset_level_8.to_le_bytes());
    buf[0x048E..0x0492].copy_from_slice(&h.sorcerer_spell_offset_level_9.to_le_bytes());
    buf[0x0492..0x0496].copy_from_slice(&h.wizard_spell_offset_level_1.to_le_bytes());
    buf[0x0496..0x049A].copy_from_slice(&h.wizard_spell_offset_level_2.to_le_bytes());
    buf[0x049A..0x049E].copy_from_slice(&h.wizard_spell_offset_level_3.to_le_bytes());
    buf[0x049E..0x04A2].copy_from_slice(&h.wizard_spell_offset_level_4.to_le_bytes());
    buf[0x04A2..0x04A6].copy_from_slice(&h.wizard_spell_offset_level_5.to_le_bytes());
    buf[0x04A6..0x04AA].copy_from_slice(&h.wizard_spell_offset_level_6.to_le_bytes());
    buf[0x04AA..0x04AE].copy_from_slice(&h.wizard_spell_offset_level_7.to_le_bytes());
    buf[0x04AE..0x04B2].copy_from_slice(&h.wizard_spell_offset_level_8.to_le_bytes());
    buf[0x04B2..0x04B6].copy_from_slice(&h.wizard_spell_offset_level_9.to_le_bytes());
    buf[0x04B6..0x04BA].copy_from_slice(&h.bard_spell_count_level_1.to_le_bytes());
    buf[0x04BA..0x04BE].copy_from_slice(&h.bard_spell_count_level_2.to_le_bytes());
    buf[0x04BE..0x04C2].copy_from_slice(&h.bard_spell_count_level_3.to_le_bytes());
    buf[0x04C2..0x04C6].copy_from_slice(&h.bard_spell_count_level_4.to_le_bytes());
    buf[0x04C6..0x04CA].copy_from_slice(&h.bard_spell_count_level_5.to_le_bytes());
    buf[0x04CA..0x04CE].copy_from_slice(&h.bard_spell_count_level_6.to_le_bytes());
    buf[0x04CE..0x04D2].copy_from_slice(&h.bard_spell_count_level_7.to_le_bytes());
    buf[0x04D2..0x04D6].copy_from_slice(&h.bard_spell_count_level_8.to_le_bytes());
    buf[0x04D6..0x04DA].copy_from_slice(&h.bard_spell_count_level_9.to_le_bytes());
    buf[0x04DA..0x04DE].copy_from_slice(&h.cleric_spell_count_level_1.to_le_bytes());
    buf[0x04DE..0x04E2].copy_from_slice(&h.cleric_spell_count_level_2.to_le_bytes());
    buf[0x04E2..0x04E6].copy_from_slice(&h.cleric_spell_count_level_3.to_le_bytes());
    buf[0x04E6..0x04EA].copy_from_slice(&h.cleric_spell_count_level_4.to_le_bytes());
    buf[0x04EA..0x04EE].copy_from_slice(&h.cleric_spell_count_level_5.to_le_bytes());
    buf[0x04EE..0x04F2].copy_from_slice(&h.cleric_spell_count_level_6.to_le_bytes());
    buf[0x04F2..0x04F6].copy_from_slice(&h.cleric_spell_count_level_7.to_le_bytes());
    buf[0x04F6..0x04FA].copy_from_slice(&h.cleric_spell_count_level_8.to_le_bytes());
    buf[0x04FA..0x04FE].copy_from_slice(&h.cleric_spell_count_level_9.to_le_bytes());
    buf[0x04FE..0x0502].copy_from_slice(&h.druid_spell_count_level_1.to_le_bytes());
    buf[0x0502..0x0506].copy_from_slice(&h.druid_spell_count_level_2.to_le_bytes());
    buf[0x0506..0x050A].copy_from_slice(&h.druid_spell_count_level_3.to_le_bytes());
    buf[0x050A..0x050E].copy_from_slice(&h.druid_spell_count_level_4.to_le_bytes());
    buf[0x050E..0x0512].copy_from_slice(&h.druid_spell_count_level_5.to_le_bytes());
    buf[0x0512..0x0516].copy_from_slice(&h.druid_spell_count_level_6.to_le_bytes());
    buf[0x0516..0x051A].copy_from_slice(&h.druid_spell_count_level_7.to_le_bytes());
    buf[0x051A..0x051E].copy_from_slice(&h.druid_spell_count_level_8.to_le_bytes());
    buf[0x051E..0x0522].copy_from_slice(&h.druid_spell_count_level_9.to_le_bytes());
    buf[0x0522..0x0526].copy_from_slice(&h.paladin_spell_count_level_1.to_le_bytes());
    buf[0x0526..0x052A].copy_from_slice(&h.paladin_spell_count_level_2.to_le_bytes());
    buf[0x052A..0x052E].copy_from_slice(&h.paladin_spell_count_level_3.to_le_bytes());
    buf[0x052E..0x0532].copy_from_slice(&h.paladin_spell_count_level_4.to_le_bytes());
    buf[0x0532..0x0536].copy_from_slice(&h.paladin_spell_count_level_5.to_le_bytes());
    buf[0x0536..0x053A].copy_from_slice(&h.paladin_spell_count_level_6.to_le_bytes());
    buf[0x053A..0x053E].copy_from_slice(&h.paladin_spell_count_level_7.to_le_bytes());
    buf[0x053E..0x0542].copy_from_slice(&h.paladin_spell_count_level_8.to_le_bytes());
    buf[0x0542..0x0546].copy_from_slice(&h.paladin_spell_count_level_9.to_le_bytes());
    buf[0x0546..0x054A].copy_from_slice(&h.ranger_spell_count_level_1.to_le_bytes());
    buf[0x054A..0x054E].copy_from_slice(&h.ranger_spell_count_level_2.to_le_bytes());
    buf[0x054E..0x0552].copy_from_slice(&h.ranger_spell_count_level_3.to_le_bytes());
    buf[0x0552..0x0556].copy_from_slice(&h.ranger_spell_count_level_4.to_le_bytes());
    buf[0x0556..0x055A].copy_from_slice(&h.ranger_spell_count_level_5.to_le_bytes());
    buf[0x055A..0x055E].copy_from_slice(&h.ranger_spell_count_level_6.to_le_bytes());
    buf[0x055E..0x0562].copy_from_slice(&h.ranger_spell_count_level_7.to_le_bytes());
    buf[0x0562..0x0566].copy_from_slice(&h.ranger_spell_count_level_8.to_le_bytes());
    buf[0x0566..0x056A].copy_from_slice(&h.ranger_spell_count_level_9.to_le_bytes());
    buf[0x056A..0x056E].copy_from_slice(&h.sorcerer_spell_count_level_1.to_le_bytes());
    buf[0x056E..0x0572].copy_from_slice(&h.sorcerer_spell_count_level_2.to_le_bytes());
    buf[0x0572..0x0576].copy_from_slice(&h.sorcerer_spell_count_level_3.to_le_bytes());
    buf[0x0576..0x057A].copy_from_slice(&h.sorcerer_spell_count_level_4.to_le_bytes());
    buf[0x057A..0x057E].copy_from_slice(&h.sorcerer_spell_count_level_5.to_le_bytes());
    buf[0x057E..0x0582].copy_from_slice(&h.sorcerer_spell_count_level_6.to_le_bytes());
    buf[0x0582..0x0586].copy_from_slice(&h.sorcerer_spell_count_level_7.to_le_bytes());
    buf[0x0586..0x058A].copy_from_slice(&h.sorcerer_spell_count_level_8.to_le_bytes());
    buf[0x058A..0x058E].copy_from_slice(&h.sorcerer_spell_count_level_9.to_le_bytes());
    buf[0x058E..0x0592].copy_from_slice(&h.wizard_spell_count_level_1.to_le_bytes());
    buf[0x0592..0x0596].copy_from_slice(&h.wizard_spell_count_level_2.to_le_bytes());
    buf[0x0596..0x059A].copy_from_slice(&h.wizard_spell_count_level_3.to_le_bytes());
    buf[0x059A..0x059E].copy_from_slice(&h.wizard_spell_count_level_4.to_le_bytes());
    buf[0x059E..0x05A2].copy_from_slice(&h.wizard_spell_count_level_5.to_le_bytes());
    buf[0x05A2..0x05A6].copy_from_slice(&h.wizard_spell_count_level_6.to_le_bytes());
    buf[0x05A6..0x05AA].copy_from_slice(&h.wizard_spell_count_level_7.to_le_bytes());
    buf[0x05AA..0x05AE].copy_from_slice(&h.wizard_spell_count_level_8.to_le_bytes());
    buf[0x05AE..0x05B2].copy_from_slice(&h.wizard_spell_count_level_9.to_le_bytes());
    buf[0x05B2..0x05B6].copy_from_slice(&h.domain1_spell_offset.to_le_bytes());
    buf[0x05B6..0x05BA].copy_from_slice(&h.domain2_spell_offset.to_le_bytes());
    buf[0x05BA..0x05BE].copy_from_slice(&h.domain3_spell_offset.to_le_bytes());
    buf[0x05BE..0x05C2].copy_from_slice(&h.domain4_spell_offset.to_le_bytes());
    buf[0x05C2..0x05C6].copy_from_slice(&h.domain5_spell_offset.to_le_bytes());
    buf[0x05C6..0x05CA].copy_from_slice(&h.domain6_spell_offset.to_le_bytes());
    buf[0x05CA..0x05CE].copy_from_slice(&h.domain7_spell_offset.to_le_bytes());
    buf[0x05CE..0x05D2].copy_from_slice(&h.domain8_spell_offset.to_le_bytes());
    buf[0x05D2..0x05D6].copy_from_slice(&h.domain9_spell_offset.to_le_bytes());
    buf[0x05D6..0x05DA].copy_from_slice(&h.domain1_spell_count.to_le_bytes());
    buf[0x05DA..0x05DE].copy_from_slice(&h.domain2_spell_count.to_le_bytes());
    buf[0x05DE..0x05E2].copy_from_slice(&h.domain3_spell_count.to_le_bytes());
    buf[0x05E2..0x05E6].copy_from_slice(&h.domain4_spell_count.to_le_bytes());
    buf[0x05E6..0x05EA].copy_from_slice(&h.domain5_spell_count.to_le_bytes());
    buf[0x05EA..0x05EE].copy_from_slice(&h.domain6_spell_count.to_le_bytes());
    buf[0x05EE..0x05F2].copy_from_slice(&h.domain7_spell_count.to_le_bytes());
    buf[0x05F2..0x05F6].copy_from_slice(&h.domain8_spell_count.to_le_bytes());
    buf[0x05F6..0x05FA].copy_from_slice(&h.domain9_spell_count.to_le_bytes());
    buf[0x05FA..0x05FE].copy_from_slice(&h.abilities_offset.to_le_bytes());
    buf[0x05FE..0x0602].copy_from_slice(&h.abilities_count.to_le_bytes());
    buf[0x0602..0x0606].copy_from_slice(&h.song_offset.to_le_bytes());
    buf[0x0606..0x060A].copy_from_slice(&h.song_count.to_le_bytes());
    buf[0x060A..0x060E].copy_from_slice(&h.shapes_offset.to_le_bytes());
    buf[0x060E..0x0612].copy_from_slice(&h.shapes_count.to_le_bytes());
    buf[0x0612..0x0616].copy_from_slice(&h.item_slots_offset.to_le_bytes());
    buf[0x0616..0x061A].copy_from_slice(&h.item_offset.to_le_bytes());
    buf[0x061A..0x061E].copy_from_slice(&h.item_count.to_le_bytes());
    buf[0x061E..0x0622].copy_from_slice(&h.effects_offset.to_le_bytes());
    buf[0x0622..0x0626].copy_from_slice(&h.effects_count.to_le_bytes());
    write_resref(&mut buf[0x0626..0x062E], &h.dialog);
    buf
}

