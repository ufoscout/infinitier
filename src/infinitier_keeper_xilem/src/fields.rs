//! Read-only field enumeration for the Abilities tab.
//!
//! This is the read-only half of the egui keeper's `editable_fields`:
//! the same [`EditableField`] enum, [`Section`] grouping, per-version
//! `label` / `is_visible`, and a `read_text` that formats the current
//! value for display. The write/commit/clamp plumbing is intentionally
//! dropped — this port is read-only.

use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::cre::Cre;

use crate::cre_fields;

/// Logical grouping for the abilities tab — one card per [`Section`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    AbilityScores,
    CombatStatus,
    ExperienceLevels,
    Morale,
    ThiefSkills,
}

/// Every value row the abilities tab can display.
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
        }
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
            Self::MoveSilently => cre_fields::move_silently_label(cre),
            Self::Lockpicking => "Open Locks",
            Self::FindTraps => "Find Traps",
            Self::SetTraps => "Set Traps",
            Self::PickPockets => "Pick Pockets",
            Self::DetectIllusion => "Detect Illusions",
            Self::Lore => "Lore",
        }
    }

    pub fn is_visible(self, cre: &Cre) -> bool {
        match self {
            Self::StrengthPct => cre.strength_bonus().is_some(),
            Self::AcEffective => cre_fields::ac_effective(cre).is_some(),
            Self::Level1 => cre_fields::level_first_class(cre).is_some(),
            Self::Level2 => cre_fields::level_second_class(cre).is_some(),
            Self::Level3 => cre_fields::level_third_class(cre).is_some(),
            Self::Morale => cre_fields::morale(cre).is_some(),
            Self::MoraleBreak => cre_fields::morale_break(cre).is_some(),
            Self::MoraleRecovery => cre_fields::morale_recovery(cre).is_some(),
            Self::HideInShadows => cre_fields::hide_in_shadows(cre).is_some(),
            Self::Lockpicking => cre_fields::lockpicking(cre).is_some(),
            Self::FindTraps => cre_fields::find_traps(cre).is_some(),
            Self::SetTraps => cre_fields::set_traps(cre).is_some(),
            Self::PickPockets => cre_fields::pick_pockets(cre).is_some(),
            Self::DetectIllusion => cre_fields::detect_illusion(cre).is_some(),
            Self::Lore => cre_fields::lore(cre).is_some(),
            _ => true,
        }
    }

    /// Current value formatted for display.
    pub fn read_text(self, cre: &Cre, gam: &ImportedGam) -> String {
        match self {
            Self::Strength => cre.strength().to_string(),
            Self::StrengthPct => {
                cre.strength_bonus().map(|v| v.to_string()).unwrap_or_default()
            }
            Self::Dexterity => cre.dexterity().to_string(),
            Self::Constitution => cre.constitution().to_string(),
            Self::Intelligence => cre.intelligence().to_string(),
            Self::Wisdom => cre.wisdom().to_string(),
            Self::Charisma => cre.charisma().to_string(),
            Self::CurrentHp => cre_fields::current_hit_points(cre).to_string(),
            Self::MaxHp => cre_fields::max_hit_points(cre).to_string(),
            Self::AcNatural => cre_fields::ac_natural(cre).to_string(),
            Self::AcEffective => {
                cre_fields::ac_effective(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::Thac0 => cre_fields::thac0_or_bab(cre).to_string(),
            // Read-only: show the player-facing attacks-per-round label.
            Self::Attacks => AttacksOption::label_for_byte(cre_fields::attacks_byte(cre)),
            Self::Reputation => cre_fields::party_reputation(gam).to_string(),
            Self::PartyGold => cre_fields::party_gold(gam).to_string(),
            Self::Fatigue => cre_fields::fatigue(cre).to_string(),
            Self::Intoxication => cre_fields::intoxication(cre).to_string(),
            Self::Luck => cre_fields::luck(cre).to_string(),
            Self::Experience => cre_fields::experience(cre).to_string(),
            Self::XpForKill => cre_fields::xp_for_kill(cre).to_string(),
            Self::Level1 => {
                cre_fields::level_first_class(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::Level2 => {
                cre_fields::level_second_class(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::Level3 => {
                cre_fields::level_third_class(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::Morale => cre_fields::morale(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::MoraleBreak => {
                cre_fields::morale_break(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::MoraleRecovery => {
                cre_fields::morale_recovery(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::HideInShadows => {
                cre_fields::hide_in_shadows(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::MoveSilently => cre_fields::move_silently(cre).to_string(),
            Self::Lockpicking => {
                cre_fields::lockpicking(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::FindTraps => {
                cre_fields::find_traps(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::SetTraps => {
                cre_fields::set_traps(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::PickPockets => {
                cre_fields::pick_pockets(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::DetectIllusion => {
                cre_fields::detect_illusion(cre).map(|v| v.to_string()).unwrap_or_default()
            }
            Self::Lore => cre_fields::lore(cre).map(|v| v.to_string()).unwrap_or_default(),
        }
    }
}

/// Attacks-per-round byte → player-facing label, mirroring the egui
/// keeper's `AttacksOption` table.
pub struct AttacksOption;

impl AttacksOption {
    const TABLE: &'static [(u8, &'static str)] = &[
        (0, "0"),
        (6, "0.5"),
        (1, "1"),
        (7, "1.5"),
        (2, "2"),
        (8, "2.5"),
        (3, "3"),
        (9, "3.5"),
        (4, "4"),
        (10, "4.5"),
        (5, "5"),
    ];

    pub fn label_for_byte(byte: u8) -> String {
        Self::TABLE
            .iter()
            .find(|(b, _)| *b == byte)
            .map(|(_, l)| (*l).to_string())
            .unwrap_or_else(|| format!("? ({byte})"))
    }
}
