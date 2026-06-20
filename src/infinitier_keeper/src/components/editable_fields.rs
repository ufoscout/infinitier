//! Per-field editor scaffolding for the egui keeper.
//!
//! Mirrors the GPUI keeper's `editable_fields` (same enum, same
//! per-version dispatch, same clamping helpers) but with the
//! immediate-mode plumbing collapsed to plain `HashMap<…, String>`
//! buffers — egui has no InputState entities, no subscriptions, no
//! lazy init. Each frame:
//!
//! 1. [`KeeperEditors::prepare`] re-mirrors every buffer from the
//!    current CRE / GAM, so the displayed values always track the
//!    active save / party slot. Only the field with keyboard focus is
//!    skipped, so in-flight typing isn't clobbered.
//! 2. The abilities tab renders one [`KeeperEditors::show_input`]
//!    per visible row, which wraps `egui::TextEdit::singleline`
//!    around the buffer and commits on focus-loss / Enter.
//!
//! The pattern is "take the String out of the map, hand it to the
//! widget, put it back" — this satisfies the borrow checker (the
//! TextEdit holds a `&mut String` while the rest of `self` needs to
//! be mutable for `state.save` writes).

use std::collections::HashMap;

use eframe::egui;
use infinitier_core::engine_caps::{AbilityRange, EngineCaps};
use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
use infinitier_core::resource::cre::Cre;

use crate::components::cre_fields;
use crate::state::AppState;

/// Logical grouping for the abilities tab. Each card on screen
/// corresponds to one [`Section`]; the tab iterates
/// [`EditableField::ALL`] and filters by section + `is_visible`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    AbilityScores,
    CombatStatus,
    ExperienceLevels,
    Morale,
    ThiefSkills,
    Iwd2ClassLevels,
}

/// Every editable row the keeper knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditableField {
    Strength,
    StrengthPct,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
    CurrentHp,
    MaxHp,
    AcNatural,
    AcEffective,
    Thac0,
    Attacks,
    Reputation,
    PartyGold,
    Fatigue,
    Intoxication,
    Luck,
    Experience,
    XpForKill,
    Level1,
    Level2,
    Level3,
    Morale,
    MoraleBreak,
    MoraleRecovery,
    HideInShadows,
    MoveSilently,
    Lockpicking,
    FindTraps,
    SetTraps,
    PickPockets,
    DetectIllusion,
    Lore,
    // IWD2 (V2.2) per-class levels
    BarbarianLevel,
    BardLevel,
    ClericLevel,
    DruidLevel,
    FighterLevel,
    MonkLevel,
    PaladinLevel,
    RangerLevel,
    RogueLevel,
    SorcererLevel,
    WizardLevel,
}

impl EditableField {
    pub const ALL: &'static [EditableField] = &[
        EditableField::Strength,
        EditableField::StrengthPct,
        EditableField::Dexterity,
        EditableField::Constitution,
        EditableField::Intelligence,
        EditableField::Wisdom,
        EditableField::Charisma,
        EditableField::CurrentHp,
        EditableField::MaxHp,
        EditableField::AcNatural,
        EditableField::AcEffective,
        EditableField::Thac0,
        EditableField::Attacks,
        EditableField::Reputation,
        EditableField::PartyGold,
        EditableField::Fatigue,
        EditableField::Intoxication,
        EditableField::Luck,
        EditableField::Experience,
        EditableField::XpForKill,
        EditableField::Level1,
        EditableField::Level2,
        EditableField::Level3,
        EditableField::Morale,
        EditableField::MoraleBreak,
        EditableField::MoraleRecovery,
        EditableField::HideInShadows,
        EditableField::MoveSilently,
        EditableField::Lockpicking,
        EditableField::FindTraps,
        EditableField::SetTraps,
        EditableField::PickPockets,
        EditableField::DetectIllusion,
        EditableField::Lore,
        EditableField::BarbarianLevel,
        EditableField::BardLevel,
        EditableField::ClericLevel,
        EditableField::DruidLevel,
        EditableField::FighterLevel,
        EditableField::MonkLevel,
        EditableField::PaladinLevel,
        EditableField::RangerLevel,
        EditableField::RogueLevel,
        EditableField::SorcererLevel,
        EditableField::WizardLevel,
    ];

    pub fn section(self) -> Section {
        match self {
            Self::Strength
            | Self::StrengthPct
            | Self::Dexterity
            | Self::Constitution
            | Self::Intelligence
            | Self::Wisdom
            | Self::Charisma => Section::AbilityScores,
            Self::CurrentHp
            | Self::MaxHp
            | Self::AcNatural
            | Self::AcEffective
            | Self::Thac0
            | Self::Attacks
            | Self::Reputation
            | Self::PartyGold
            | Self::Fatigue
            | Self::Intoxication
            | Self::Luck => Section::CombatStatus,
            Self::Experience | Self::XpForKill | Self::Level1 | Self::Level2 | Self::Level3 => {
                Section::ExperienceLevels
            }
            Self::Morale | Self::MoraleBreak | Self::MoraleRecovery => Section::Morale,
            Self::HideInShadows
            | Self::MoveSilently
            | Self::Lockpicking
            | Self::FindTraps
            | Self::SetTraps
            | Self::PickPockets
            | Self::DetectIllusion
            | Self::Lore => Section::ThiefSkills,
            Self::BarbarianLevel
            | Self::BardLevel
            | Self::ClericLevel
            | Self::DruidLevel
            | Self::FighterLevel
            | Self::MonkLevel
            | Self::PaladinLevel
            | Self::RangerLevel
            | Self::RogueLevel
            | Self::SorcererLevel
            | Self::WizardLevel => Section::Iwd2ClassLevels,
        }
    }

    /// `true` when the value lives on the party-wide GAM rather
    /// than on the per-character CRE.
    fn is_gam_field(self) -> bool {
        matches!(self, Self::Reputation | Self::PartyGold)
    }

    pub fn label(self, cre: &Cre) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::StrengthPct => "Strength %",
            Self::Dexterity => "Dexterity",
            Self::Constitution => "Constitution",
            Self::Intelligence => "Intelligence",
            Self::Wisdom => "Wisdom",
            Self::Charisma => "Charisma",
            Self::CurrentHp => "Current HP",
            Self::MaxHp => "Max HP",
            Self::AcNatural => "AC (natural)",
            Self::AcEffective => "AC (effective)",
            Self::Thac0 => "THAC0",
            Self::Attacks => "Attacks",
            Self::Reputation => "Reputation (party)",
            Self::PartyGold => "Gold (party)",
            Self::Fatigue => "Fatigue",
            Self::Intoxication => "Intoxication",
            Self::Luck => "Luck",
            Self::Experience => "Experience",
            Self::XpForKill => "Exp for kill",
            Self::Level1 => "Level (1st class)",
            Self::Level2 => "Level (2nd class)",
            Self::Level3 => "Level (3rd class)",
            Self::Morale => "Morale",
            Self::MoraleBreak => "Morale break",
            Self::MoraleRecovery => "Morale recovery",
            Self::HideInShadows => "Hide in Shadows",
            Self::MoveSilently => cre.move_silently_label(),
            Self::Lockpicking => "Open Locks",
            Self::FindTraps => "Find Traps",
            Self::SetTraps => "Set Traps",
            Self::PickPockets => "Pick Pockets",
            Self::DetectIllusion => "Detect Illusions",
            Self::Lore => "Lore",
            Self::BarbarianLevel => "Barbarian",
            Self::BardLevel => "Bard",
            Self::ClericLevel => "Cleric",
            Self::DruidLevel => "Druid",
            Self::FighterLevel => "Fighter",
            Self::MonkLevel => "Monk",
            Self::PaladinLevel => "Paladin",
            Self::RangerLevel => "Ranger",
            Self::RogueLevel => "Rogue",
            Self::SorcererLevel => "Sorcerer",
            Self::WizardLevel => "Wizard",
        }
    }

    pub fn is_visible(self, cre: &Cre) -> bool {
        match self {
            Self::StrengthPct => cre.strength_bonus().is_some(),
            Self::AcEffective => cre.ac_effective().is_some(),
            Self::Level1 => cre.level_first_class().is_some(),
            Self::Level2 => cre.level_second_class().is_some(),
            Self::Level3 => cre.level_third_class().is_some(),
            Self::Morale => cre.morale().is_some(),
            Self::MoraleBreak => cre.morale_break().is_some(),
            Self::MoraleRecovery => cre.morale_recovery().is_some(),
            Self::HideInShadows => cre.hide_in_shadows().is_some(),
            Self::Lockpicking => cre.lockpicking().is_some(),
            Self::FindTraps => cre.find_traps().is_some(),
            Self::SetTraps => cre.set_traps().is_some(),
            Self::PickPockets => cre.pick_pockets().is_some(),
            Self::DetectIllusion => cre.detect_illusion().is_some(),
            Self::Lore => cre.lore().is_some(),
            Self::BarbarianLevel
            | Self::BardLevel
            | Self::ClericLevel
            | Self::DruidLevel
            | Self::FighterLevel
            | Self::MonkLevel
            | Self::PaladinLevel
            | Self::RangerLevel
            | Self::RogueLevel
            | Self::SorcererLevel
            | Self::WizardLevel => cre.barbarian_level().is_some(),
            _ => true,
        }
    }

    pub fn read_text(self, cre: &Cre, gam: &ImportedGam) -> String {
        match self {
            Self::Strength => cre.strength().to_string(),
            Self::StrengthPct => cre
                .strength_bonus()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::Dexterity => cre.dexterity().to_string(),
            Self::Constitution => cre.constitution().to_string(),
            Self::Intelligence => cre.intelligence().to_string(),
            Self::Wisdom => cre.wisdom().to_string(),
            Self::Charisma => cre.charisma().to_string(),
            Self::CurrentHp => cre.current_hit_points().to_string(),
            Self::MaxHp => cre.maximum_hit_points().to_string(),
            Self::AcNatural => cre.ac_natural().to_string(),
            Self::AcEffective => cre
                .ac_effective()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::Thac0 => cre.thac0_or_bab().to_string(),
            Self::Attacks => cre.attacks_byte().to_string(),
            Self::Reputation => cre_fields::party_reputation(gam).to_string(),
            Self::PartyGold => cre_fields::party_gold(gam).to_string(),
            Self::Fatigue => cre.fatigue().to_string(),
            Self::Intoxication => cre.intoxication().to_string(),
            Self::Luck => cre.luck().to_string(),
            Self::Experience => cre.experience().to_string(),
            Self::XpForKill => cre.xp_for_kill().to_string(),
            Self::Level1 => cre
                .level_first_class()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::Level2 => cre
                .level_second_class()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::Level3 => cre
                .level_third_class()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::Morale => cre.morale().map(|v| v.to_string()).unwrap_or_default(),
            Self::MoraleBreak => cre
                .morale_break()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::MoraleRecovery => cre
                .morale_recovery()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::HideInShadows => cre
                .hide_in_shadows()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::MoveSilently => cre.move_silently().to_string(),
            Self::Lockpicking => cre.lockpicking().map(|v| v.to_string()).unwrap_or_default(),
            Self::FindTraps => cre.find_traps().map(|v| v.to_string()).unwrap_or_default(),
            Self::SetTraps => cre.set_traps().map(|v| v.to_string()).unwrap_or_default(),
            Self::PickPockets => cre
                .pick_pockets()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::DetectIllusion => cre
                .detect_illusion()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::Lore => cre.lore().map(|v| v.to_string()).unwrap_or_default(),
            Self::BarbarianLevel => cre
                .barbarian_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::BardLevel => cre.bard_level().map(|v| v.to_string()).unwrap_or_default(),
            Self::ClericLevel => cre
                .cleric_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::DruidLevel => cre.druid_level().map(|v| v.to_string()).unwrap_or_default(),
            Self::FighterLevel => cre
                .fighter_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::MonkLevel => cre.monk_level().map(|v| v.to_string()).unwrap_or_default(),
            Self::PaladinLevel => cre
                .paladin_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::RangerLevel => cre
                .ranger_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::RogueLevel => cre.rogue_level().map(|v| v.to_string()).unwrap_or_default(),
            Self::SorcererLevel => cre
                .sorcerer_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
            Self::WizardLevel => cre
                .wizard_level()
                .map(|v| v.to_string())
                .unwrap_or_default(),
        }
    }

    /// Parse + clamp + write to a CRE. No-op on GAM-side fields.
    fn write_clamped_cre(self, cre: &mut Cre, raw: &str, caps: &EngineCaps) {
        match self {
            Self::Strength => write_u8(raw, caps.ability_score, cre.strength(), |v| {
                cre.set_strength(v)
            }),
            Self::StrengthPct => write_u8(
                raw,
                caps.strength_percentile,
                cre.strength_bonus().unwrap_or(0),
                |v| cre.set_strength_bonus(v),
            ),
            Self::Dexterity => write_u8(raw, caps.ability_score, cre.dexterity(), |v| {
                cre.set_dexterity(v)
            }),
            Self::Constitution => write_u8(raw, caps.ability_score, cre.constitution(), |v| {
                cre.set_constitution(v)
            }),
            Self::Intelligence => write_u8(raw, caps.ability_score, cre.intelligence(), |v| {
                cre.set_intelligence(v)
            }),
            Self::Wisdom => write_u8(raw, caps.ability_score, cre.wisdom(), |v| cre.set_wisdom(v)),
            Self::Charisma => write_u8(raw, caps.ability_score, cre.charisma(), |v| {
                cre.set_charisma(v)
            }),
            Self::Thac0 => write_i8(raw, caps.thac0, cre.thac0_or_bab(), |v| {
                cre.set_thac0_or_bab(v)
            }),
            // Attacks is edited through a dropdown — never via this path.
            Self::Attacks => debug_assert!(false, "Attacks uses a dropdown, not a text input"),
            Self::Fatigue => write_u8(raw, caps.fatigue, cre.fatigue(), |v| cre.set_fatigue(v)),
            Self::Intoxication => write_u8(raw, caps.intoxication, cre.intoxication(), |v| {
                cre.set_intoxication(v)
            }),
            Self::Luck => write_u8(raw, caps.luck, cre.luck(), |v| cre.set_luck(v)),
            Self::Level1 => {
                let current = cre.level_first_class().unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| {
                    cre.set_level_first_class(v)
                })
            }
            Self::Level2 => {
                let current = cre.level_second_class().unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| {
                    cre.set_level_second_class(v)
                })
            }
            Self::Level3 => {
                let current = cre.level_third_class().unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| {
                    cre.set_level_third_class(v)
                })
            }
            Self::Morale => {
                let current = cre.morale().unwrap_or(0);
                write_u8(raw, caps.morale, current, |v| cre.set_morale(v))
            }
            Self::MoraleBreak => {
                let current = cre.morale_break().unwrap_or(0);
                write_u8(raw, caps.morale_break, current, |v| cre.set_morale_break(v))
            }
            Self::HideInShadows => {
                let current = cre.hide_in_shadows().unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre.set_hide_in_shadows(v)
                })
            }
            Self::MoveSilently => write_u8(raw, caps.thief_skill, cre.move_silently(), |v| {
                cre.set_move_silently(v)
            }),
            Self::Lockpicking => {
                let current = cre.lockpicking().unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre.set_lockpicking(v))
            }
            Self::FindTraps => {
                let current = cre.find_traps().unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre.set_find_traps(v))
            }
            Self::SetTraps => {
                let current = cre.set_traps().unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre.set_set_traps(v))
            }
            Self::PickPockets => {
                let current = cre.pick_pockets().unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre.set_pick_pockets(v))
            }
            Self::DetectIllusion => {
                let current = cre.detect_illusion().unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre.set_detect_illusion(v)
                })
            }
            Self::Lore => {
                let current = cre.lore().unwrap_or(0);
                write_u8(raw, caps.lore, current, |v| cre.set_lore(v))
            }
            Self::CurrentHp => write_u16(
                raw,
                caps.current_hit_points,
                cre.current_hit_points(),
                |v| cre.set_current_hit_points(v),
            ),
            Self::MaxHp => write_u16(raw, caps.max_hit_points, cre.maximum_hit_points(), |v| {
                cre.set_maximum_hit_points(v)
            }),
            Self::MoraleRecovery => {
                let current = cre.morale_recovery().unwrap_or(0);
                write_u16(raw, caps.morale_recovery, current, |v| {
                    cre.set_morale_recovery(v)
                })
            }
            Self::AcNatural => write_i16(raw, caps.armor_class, cre.ac_natural(), |v| {
                cre.set_ac_natural(v)
            }),
            Self::AcEffective => {
                let current = cre.ac_effective().unwrap_or(0);
                write_i16(raw, caps.armor_class, current, |v| cre.set_ac_effective(v))
            }
            Self::Experience => write_u32(raw, caps.experience, cre.experience(), |v| {
                cre.set_experience(v)
            }),
            Self::XpForKill => write_u32(raw, caps.xp_for_kill, cre.xp_for_kill(), |v| {
                cre.set_xp_for_kill(v)
            }),
            Self::BarbarianLevel => {
                write_class_level(cre, raw, caps, Cre::barbarian_level, Cre::set_barbarian_level)
            }
            Self::BardLevel => {
                write_class_level(cre, raw, caps, Cre::bard_level, Cre::set_bard_level)
            }
            Self::ClericLevel => {
                write_class_level(cre, raw, caps, Cre::cleric_level, Cre::set_cleric_level)
            }
            Self::DruidLevel => {
                write_class_level(cre, raw, caps, Cre::druid_level, Cre::set_druid_level)
            }
            Self::FighterLevel => {
                write_class_level(cre, raw, caps, Cre::fighter_level, Cre::set_fighter_level)
            }
            Self::MonkLevel => {
                write_class_level(cre, raw, caps, Cre::monk_level, Cre::set_monk_level)
            }
            Self::PaladinLevel => {
                write_class_level(cre, raw, caps, Cre::paladin_level, Cre::set_paladin_level)
            }
            Self::RangerLevel => {
                write_class_level(cre, raw, caps, Cre::ranger_level, Cre::set_ranger_level)
            }
            Self::RogueLevel => {
                write_class_level(cre, raw, caps, Cre::rogue_level, Cre::set_rogue_level)
            }
            Self::SorcererLevel => {
                write_class_level(cre, raw, caps, Cre::sorcerer_level, Cre::set_sorcerer_level)
            }
            Self::WizardLevel => {
                write_class_level(cre, raw, caps, Cre::wizard_level, Cre::set_wizard_level)
            }
            // GAM-side fields are routed through `write_clamped_gam`.
            Self::Reputation | Self::PartyGold => {
                debug_assert!(false, "{self:?} is a GAM-side field");
            }
        }
    }

    fn write_clamped_gam(self, gam: &mut ImportedGam, raw: &str, caps: &EngineCaps) {
        match self {
            Self::Reputation => write_u32(
                raw,
                caps.reputation,
                cre_fields::party_reputation(gam),
                |v| cre_fields::set_party_reputation(gam, v),
            ),
            Self::PartyGold => write_u32(raw, caps.party_gold, cre_fields::party_gold(gam), |v| {
                cre_fields::set_party_gold(gam, v)
            }),
            _ => debug_assert!(false, "{self:?} is a CRE-side field"),
        }
    }
}

// ── Type-specific parse/clamp/write helpers ──────────────────────────

/// The IWD2 (CRE V2.2) engine stores the party-visible `total_levels` as
/// a single `u8` (sum of all eleven per-class levels), so the eleven
/// levels must sum to at most 255.
const IWD2_TOTAL_LEVELS_MAX: u16 = u8::MAX as u16;

/// Sum of every IWD2 (V2.2) per-class level, as `u16` to avoid the
/// saturating overflow `total_levels` hits past 255. Zero on non-V2.2
/// CREs (every per-class getter returns `None`).
fn iwd2_class_levels_total(cre: &Cre) -> u16 {
    [
        cre.barbarian_level(),
        cre.bard_level(),
        cre.cleric_level(),
        cre.druid_level(),
        cre.fighter_level(),
        cre.monk_level(),
        cre.paladin_level(),
        cre.ranger_level(),
        cre.rogue_level(),
        cre.sorcerer_level(),
        cre.wizard_level(),
    ]
    .into_iter()
    .flatten()
    .map(u16::from)
    .sum()
}

/// Parse + clamp + write one IWD2 per-class level. On top of the normal
/// per-field `class_level` clamp, the value is capped so the eleven
/// classes can't sum past 255 — the edited class takes at most
/// `255 - (sum of the other ten)`, clamping the just-edited value.
fn write_class_level(
    cre: &mut Cre,
    raw: &str,
    caps: &EngineCaps,
    get: fn(&Cre) -> Option<u8>,
    set: fn(&mut Cre, u8),
) {
    let current = get(cre).unwrap_or(0);
    let parsed: u8 = match raw.trim().parse::<u32>() {
        Ok(n) => n.min(u8::MAX as u32) as u8,
        Err(_) => current,
    };
    let clamped = caps.class_level.clamp(parsed);
    // Headroom left for this class once the other ten are accounted for.
    let others = iwd2_class_levels_total(cre).saturating_sub(u16::from(current));
    let max_for_field = IWD2_TOTAL_LEVELS_MAX.saturating_sub(others) as u8;
    set(cre, clamped.min(max_for_field));
}

fn write_u8<F: FnOnce(u8)>(raw: &str, range: AbilityRange<u8>, current: u8, write: F) {
    let parsed: u8 = match raw.trim().parse::<u32>() {
        Ok(n) => n.min(u8::MAX as u32) as u8,
        Err(_) => current,
    };
    write(range.clamp(parsed));
}

fn write_i8<F: FnOnce(i8)>(raw: &str, range: AbilityRange<i8>, current: i8, write: F) {
    let parsed: i8 = match raw.trim().parse::<i32>() {
        Ok(n) => n.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
        Err(_) => current,
    };
    write(range.clamp(parsed));
}

fn write_u16<F: FnOnce(u16)>(raw: &str, range: AbilityRange<u16>, current: u16, write: F) {
    let parsed: u16 = match raw.trim().parse::<u32>() {
        Ok(n) => n.min(u16::MAX as u32) as u16,
        Err(_) => current,
    };
    write(range.clamp(parsed));
}

fn write_i16<F: FnOnce(i16)>(raw: &str, range: AbilityRange<i16>, current: i16, write: F) {
    let parsed: i16 = match raw.trim().parse::<i32>() {
        Ok(n) => n.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        Err(_) => current,
    };
    write(range.clamp(parsed));
}

fn write_u32<F: FnOnce(u32)>(raw: &str, range: AbilityRange<u32>, current: u32, write: F) {
    let parsed: u32 = match raw.trim().parse::<u64>() {
        Ok(n) => n.min(u32::MAX as u64) as u32,
        Err(_) => current,
    };
    write(range.clamp(parsed));
}

// ── Attacks dropdown ─────────────────────────────────────────────────

/// One row of the Attacks dropdown. `byte` is the raw on-disk value
/// (`0..=10` for the documented variants); `label` is the
/// player-facing attacks-per-round string ("0.5", "1", "1.5"…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttacksOption {
    pub byte: u8,
    pub label: &'static str,
}

impl AttacksOption {
    /// Every documented attacks-per-round value, in dropdown order
    /// — integers interleaved with halves. Mirrors
    /// [`infinitier_core::resource::cre::NumberOfAttacks`].
    pub const ALL: &'static [AttacksOption] = &[
        AttacksOption {
            byte: 0,
            label: "0",
        },
        AttacksOption {
            byte: 6,
            label: "0.5",
        },
        AttacksOption {
            byte: 1,
            label: "1",
        },
        AttacksOption {
            byte: 7,
            label: "1.5",
        },
        AttacksOption {
            byte: 2,
            label: "2",
        },
        AttacksOption {
            byte: 8,
            label: "2.5",
        },
        AttacksOption {
            byte: 3,
            label: "3",
        },
        AttacksOption {
            byte: 9,
            label: "3.5",
        },
        AttacksOption {
            byte: 4,
            label: "4",
        },
        AttacksOption {
            byte: 10,
            label: "4.5",
        },
        AttacksOption {
            byte: 5,
            label: "5",
        },
    ];

    pub fn index_for_byte(byte: u8) -> Option<usize> {
        Self::ALL.iter().position(|o| o.byte == byte)
    }
}

// ── KeeperEditors — the immediate-mode editor scaffold ───────────────

/// In-flight text + Attacks-dropdown index for every editable field.
/// One instance lives on [`crate::app::KeeperApp`]; the abilities tab
/// reads / writes through it. Cheap to construct (just two
/// allocations) so there's no lazy-init dance like the GPUI keeper —
/// `Default::default()` is the right call from `KeeperApp::new`.
#[derive(Default)]
pub struct KeeperEditors {
    inputs: HashMap<EditableField, String>,
    /// Selected index into [`AttacksOption::ALL`]. `None` when the
    /// active CRE's attacks byte isn't a documented variant.
    pub attacks_idx: Option<usize>,
    /// The field currently being edited (has keyboard focus), if any.
    /// [`Self::prepare`] leaves this one buffer alone so in-flight text
    /// isn't clobbered while every other field re-mirrors the model.
    focused_field: Option<EditableField>,
}

impl KeeperEditors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Per-frame sync. Mirror every editable field's buffer from the
    /// active CRE / GAM so the view always reflects the current save —
    /// no cache invalidation to get wrong. The only field left untouched
    /// is the one being edited right now ([`Self::focused_field`]),
    /// whose in-flight text stands until it commits on focus loss.
    ///
    /// Because it reads the model directly every frame, any change to
    /// what's active (switching party slot, switching save tab, opening
    /// a different save) is reflected the moment the model changes —
    /// there's nothing to stale.
    pub fn prepare(&mut self, state: &AppState) {
        let Some(cre) = selected_cre(state) else {
            self.inputs.clear();
            self.attacks_idx = None;
            return;
        };
        for &field in EditableField::ALL {
            if field == EditableField::Attacks || self.focused_field == Some(field) {
                continue;
            }
            self.inputs
                .insert(field, field.read_text(cre, &state.active().save));
        }
        self.attacks_idx = AttacksOption::index_for_byte(cre.attacks_byte());
    }

    /// Direct text buffer access. Used by the abilities tab to read
    /// the in-flight value for live bonus computation without going
    /// through `show_input`.
    pub fn text(&self, field: EditableField) -> &str {
        self.inputs.get(&field).map(String::as_str).unwrap_or("")
    }

    /// Render a themed `Input` (bordered + rounded — the shadcn
    /// look) bound to `field`. On focus-loss (or Enter), parse +
    /// clamp + write back to the CRE / GAM, then refresh the buffer
    /// with the clamped string so the user sees the rounded value.
    ///
    /// Returns `true` when the value was committed this frame.
    pub fn show_input(
        &mut self,
        ui: &mut egui::Ui,
        field: EditableField,
        state: &mut AppState,
        width: f32,
    ) -> bool {
        let mut buf = std::mem::take(self.inputs.entry(field).or_default());
        let response = ui.add(egui_components::Input::new(&mut buf).width(width).small());
        // Track which field owns keyboard focus so `prepare` can leave
        // its in-flight text alone while re-mirroring every other field.
        if response.has_focus() {
            self.focused_field = Some(field);
        } else if self.focused_field == Some(field) {
            self.focused_field = None;
        }
        let committed = response.lost_focus();
        if committed {
            commit(field, &buf, state);
            if let Some(cre) = selected_cre(state) {
                buf = field.read_text(cre, &state.active().save);
            }
        }
        *self.inputs.entry(field).or_default() = buf;
        committed
    }
}

fn commit(field: EditableField, raw: &str, state: &mut AppState) {
    // Split-borrow: `state.engine_caps` (immutable) and the active
    // tab's `save` (mutable) are disjoint fields of `state`, so
    // Rust lets us hold both simultaneously by destructuring
    // explicit references. `tabs[*active_tab]` reaches into one
    // element of `tabs` — disjoint from `engine_caps`.
    let AppState {
        engine_caps,
        tabs,
        active_tab,
        ..
    } = state;
    let active = &mut tabs[*active_tab];
    if field.is_gam_field() {
        field.write_clamped_gam(&mut active.save, raw, engine_caps);
        return;
    }
    let Some(idx) = active.selected_party_index else {
        return;
    };
    let Some(npc) = active.save.party_npcs.get_mut(idx) else {
        return;
    };
    let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut() else {
        return;
    };
    field.write_clamped_cre(boxed, raw, engine_caps);
}

/// Commit the Attacks dropdown's current selection back to the CRE.
/// Called by the abilities tab when the user picks a new option.
pub fn commit_attacks(idx: usize, state: &mut AppState) {
    let Some(option) = AttacksOption::ALL.get(idx) else {
        return;
    };
    let active = state.active_mut();
    let Some(slot) = active.selected_party_index else {
        return;
    };
    let Some(npc) = active.save.party_npcs.get_mut(slot) else {
        return;
    };
    let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut() else {
        return;
    };
    boxed.set_attacks_byte(option.byte);
}

fn selected_cre(state: &AppState) -> Option<&Cre> {
    let active = state.active();
    let idx = active.selected_party_index?;
    let npc = active.save.party_npcs.get(idx)?;
    match npc.cre.as_ref()? {
        NpcCre::Cre(boxed) => Some(boxed.as_ref()),
        NpcCre::Ref(_) => None,
    }
}
