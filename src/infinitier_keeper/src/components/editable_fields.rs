//! Editor scaffolding for every editable row on the *Abilities*
//! tab — combat / status / experience / morale / thief-skills, plus
//! the 6 ability scores and the AD&D strength-percentile field.
//!
//! Each row owns an [`InputState`] entity and a subscription that
//! commits on Blur / Enter:
//!
//! 1. Read the InputState's text.
//! 2. Parse to the field's storage type (`u8`, `u16`, `i16`, `u32`).
//! 3. Clamp through the matching range in
//!    [`crate::state::KeeperState::engine_caps`].
//! 4. Write to the current party member's CRE (or the GAM, for
//!    reputation / gold).
//! 5. Force a UI re-render so the next paint refreshes the input
//!    text to the clamped value.
//!
//! Per-version dispatch (V10 / V12 / V90 / V22) lives in
//! [`crate::cre_fields`]; this module decides *which* field, in
//! *which* units, with *which* range.

use std::collections::HashMap;

use gpui::{App, AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::select::{SelectEvent, SelectItem, SelectState};
use infinitier_core::engine_caps::{AbilityRange, EngineCaps};
use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
use infinitier_core::resource::cre::Cre;

use crate::app::KeeperApp;
use crate::components::cre_fields;
use crate::state::KeeperState;

/// Logical grouping for the UI — every variant of [`EditableField`]
/// maps to exactly one of these. The abilities tab renders a card
/// per [`Section`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    AbilityScores,
    CombatStatus,
    ExperienceLevels,
    Morale,
    ThiefSkills,
}

/// Every editable row the keeper knows about. Declaration order
/// matches the screenshot layout — `Section::fields` walks them in
/// this order so the cards' row sequence is stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditableField {
    // ── Ability scores ──
    Strength,
    StrengthPct,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
    // ── Combat & status ──
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
    // ── Experience & levels ──
    Experience,
    XpForKill,
    Level1,
    Level2,
    Level3,
    // ── Morale ──
    Morale,
    MoraleBreak,
    MoraleRecovery,
    // ── Thief skills ──
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
    /// Compile-time list of every variant. Order matches the cards on
    /// the abilities tab.
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

    /// `true` when the value lives on the GAM (party-wide) rather
    /// than on the per-character CRE. Used by the commit handler to
    /// decide which mutable borrow it needs.
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
            Self::MoveSilently => cre_fields::move_silently_label(cre),
            Self::Lockpicking => "Open Locks",
            Self::FindTraps => "Find Traps",
            Self::SetTraps => "Set Traps",
            Self::PickPockets => "Pick Pockets",
            Self::DetectIllusion => "Detect Illusions",
            Self::Lore => "Lore",
        }
    }

    /// `true` when this field is meaningful for the given CRE
    /// version. The UI hides rows that return `false` rather than
    /// showing a blank input.
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
            // Every other field is universal across CRE versions.
            _ => true,
        }
    }

    /// Read the field as the text the InputState should display.
    /// Empty string when the field isn't visible for this CRE.
    pub fn read_text(self, cre: &Cre, gam: &ImportedGam) -> String {
        match self {
            Self::Strength => cre.strength().to_string(),
            Self::StrengthPct => cre.strength_bonus().map(|v| v.to_string()).unwrap_or_default(),
            Self::Dexterity => cre.dexterity().to_string(),
            Self::Constitution => cre.constitution().to_string(),
            Self::Intelligence => cre.intelligence().to_string(),
            Self::Wisdom => cre.wisdom().to_string(),
            Self::Charisma => cre.charisma().to_string(),
            Self::CurrentHp => cre_fields::current_hit_points(cre).to_string(),
            Self::MaxHp => cre_fields::max_hit_points(cre).to_string(),
            Self::AcNatural => cre_fields::ac_natural(cre).to_string(),
            Self::AcEffective => cre_fields::ac_effective(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::Thac0 => cre_fields::thac0_or_bab(cre).to_string(),
            Self::Attacks => cre_fields::attacks_byte(cre).to_string(),
            Self::Reputation => cre_fields::party_reputation(gam).to_string(),
            Self::PartyGold => cre_fields::party_gold(gam).to_string(),
            Self::Fatigue => cre_fields::fatigue(cre).to_string(),
            Self::Intoxication => cre_fields::intoxication(cre).to_string(),
            Self::Luck => cre_fields::luck(cre).to_string(),
            Self::Experience => cre_fields::experience(cre).to_string(),
            Self::XpForKill => cre_fields::xp_for_kill(cre).to_string(),
            Self::Level1 => cre_fields::level_first_class(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::Level2 => cre_fields::level_second_class(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::Level3 => cre_fields::level_third_class(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::Morale => cre_fields::morale(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::MoraleBreak => cre_fields::morale_break(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::MoraleRecovery => cre_fields::morale_recovery(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::HideInShadows => cre_fields::hide_in_shadows(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::MoveSilently => cre_fields::move_silently(cre).to_string(),
            Self::Lockpicking => cre_fields::lockpicking(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::FindTraps => cre_fields::find_traps(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::SetTraps => cre_fields::set_traps(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::PickPockets => cre_fields::pick_pockets(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::DetectIllusion => cre_fields::detect_illusion(cre).map(|v| v.to_string()).unwrap_or_default(),
            Self::Lore => cre_fields::lore(cre).map(|v| v.to_string()).unwrap_or_default(),
        }
    }

    /// Parse + clamp + write for the CRE-side fields. No-op (and
    /// debug-asserts) for the GAM-side ones — those go through
    /// [`Self::write_clamped_gam`] instead.
    fn write_clamped_cre(self, cre: &mut Cre, raw: &str, caps: &EngineCaps) {
        match self {
            // ── u8 fields ──
            Self::Strength => {
                write_u8(raw, caps.ability_score, cre.strength(), |v| {
                    cre.set_strength(v)
                })
            }
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
            Self::Wisdom => {
                write_u8(raw, caps.ability_score, cre.wisdom(), |v| cre.set_wisdom(v))
            }
            Self::Charisma => write_u8(raw, caps.ability_score, cre.charisma(), |v| {
                cre.set_charisma(v)
            }),
            Self::Thac0 => write_i8(raw, caps.thac0, cre_fields::thac0_or_bab(cre), |v| {
                cre_fields::set_thac0_or_bab(cre, v)
            }),
            // Attacks is edited through a dropdown (see
            // `commit_attacks_selection`), never via this text path.
            Self::Attacks => debug_assert!(false, "Attacks uses a dropdown, not a text input"),
            Self::Fatigue => write_u8(raw, caps.fatigue, cre_fields::fatigue(cre), |v| {
                cre_fields::set_fatigue(cre, v)
            }),
            Self::Intoxication => write_u8(
                raw,
                caps.intoxication,
                cre_fields::intoxication(cre),
                |v| cre_fields::set_intoxication(cre, v),
            ),
            Self::Luck => write_u8(raw, caps.luck, cre_fields::luck(cre), |v| {
                cre_fields::set_luck(cre, v)
            }),
            Self::Level1 => {
                let current = cre_fields::level_first_class(cre).unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| {
                    cre_fields::set_level_first_class(cre, v)
                })
            }
            Self::Level2 => {
                let current = cre_fields::level_second_class(cre).unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| {
                    cre_fields::set_level_second_class(cre, v)
                })
            }
            Self::Level3 => {
                let current = cre_fields::level_third_class(cre).unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| {
                    cre_fields::set_level_third_class(cre, v)
                })
            }
            Self::Morale => {
                let current = cre_fields::morale(cre).unwrap_or(0);
                write_u8(raw, caps.morale, current, |v| cre_fields::set_morale(cre, v))
            }
            Self::MoraleBreak => {
                let current = cre_fields::morale_break(cre).unwrap_or(0);
                write_u8(raw, caps.morale_break, current, |v| {
                    cre_fields::set_morale_break(cre, v)
                })
            }
            Self::HideInShadows => {
                let current = cre_fields::hide_in_shadows(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre_fields::set_hide_in_shadows(cre, v)
                })
            }
            Self::MoveSilently => {
                write_u8(raw, caps.thief_skill, cre_fields::move_silently(cre), |v| {
                    cre_fields::set_move_silently(cre, v)
                })
            }
            Self::Lockpicking => {
                let current = cre_fields::lockpicking(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre_fields::set_lockpicking(cre, v)
                })
            }
            Self::FindTraps => {
                let current = cre_fields::find_traps(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre_fields::set_find_traps(cre, v)
                })
            }
            Self::SetTraps => {
                let current = cre_fields::set_traps(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre_fields::set_set_traps(cre, v)
                })
            }
            Self::PickPockets => {
                let current = cre_fields::pick_pockets(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre_fields::set_pick_pockets(cre, v)
                })
            }
            Self::DetectIllusion => {
                let current = cre_fields::detect_illusion(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| {
                    cre_fields::set_detect_illusion(cre, v)
                })
            }
            Self::Lore => {
                let current = cre_fields::lore(cre).unwrap_or(0);
                write_u8(raw, caps.lore, current, |v| cre_fields::set_lore(cre, v))
            }
            // ── u16 fields ──
            Self::CurrentHp => write_u16(
                raw,
                caps.current_hit_points,
                cre_fields::current_hit_points(cre),
                |v| cre_fields::set_current_hit_points(cre, v),
            ),
            Self::MaxHp => write_u16(
                raw,
                caps.max_hit_points,
                cre_fields::max_hit_points(cre),
                |v| cre_fields::set_max_hit_points(cre, v),
            ),
            Self::MoraleRecovery => {
                let current = cre_fields::morale_recovery(cre).unwrap_or(0);
                write_u16(raw, caps.morale_recovery, current, |v| {
                    cre_fields::set_morale_recovery(cre, v)
                })
            }
            // ── i16 fields ──
            Self::AcNatural => write_i16(
                raw,
                caps.armor_class,
                cre_fields::ac_natural(cre),
                |v| cre_fields::set_ac_natural(cre, v),
            ),
            Self::AcEffective => {
                let current = cre_fields::ac_effective(cre).unwrap_or(0);
                write_i16(raw, caps.armor_class, current, |v| {
                    cre_fields::set_ac_effective(cre, v)
                })
            }
            // ── u32 (CRE-side) ──
            Self::Experience => {
                write_u32(raw, caps.experience, cre_fields::experience(cre), |v| {
                    cre_fields::set_experience(cre, v)
                })
            }
            Self::XpForKill => write_u32(
                raw,
                caps.xp_for_kill,
                cre_fields::xp_for_kill(cre),
                |v| cre_fields::set_xp_for_kill(cre, v),
            ),
            // GAM-side fields are routed through `write_clamped_gam`.
            Self::Reputation | Self::PartyGold => {
                debug_assert!(false, "{self:?} is a GAM-side field");
            }
        }
    }

    /// Parse + clamp + write for the GAM-side fields (reputation,
    /// party gold). No-op for any CRE-side field — those go through
    /// [`Self::write_clamped_cre`].
    fn write_clamped_gam(self, gam: &mut ImportedGam, raw: &str, caps: &EngineCaps) {
        match self {
            Self::Reputation => write_u32(
                raw,
                caps.reputation,
                cre_fields::party_reputation(gam),
                |v| cre_fields::set_party_reputation(gam, v),
            ),
            Self::PartyGold => {
                write_u32(raw, caps.party_gold, cre_fields::party_gold(gam), |v| {
                    cre_fields::set_party_gold(gam, v)
                })
            }
            _ => debug_assert!(false, "{self:?} is a CRE-side field"),
        }
    }
}

// ── Type-specific parse/clamp/write helpers ──────────────────────────

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
    byte: u8,
    label: &'static str,
}

impl AttacksOption {
    /// Every documented attacks-per-round value, in the order shown
    /// in the dropdown — integers first, then halves. Mirrors
    /// [`infinitier_core::resource::cre::NumberOfAttacks`].
    pub const ALL: &'static [AttacksOption] = &[
        AttacksOption { byte: 0, label: "0" },
        AttacksOption { byte: 6, label: "0.5" },
        AttacksOption { byte: 1, label: "1" },
        AttacksOption { byte: 7, label: "1.5" },
        AttacksOption { byte: 2, label: "2" },
        AttacksOption { byte: 8, label: "2.5" },
        AttacksOption { byte: 3, label: "3" },
        AttacksOption { byte: 9, label: "3.5" },
        AttacksOption { byte: 4, label: "4" },
        AttacksOption { byte: 10, label: "4.5" },
        AttacksOption { byte: 5, label: "5" },
    ];
}

impl SelectItem for AttacksOption {
    type Value = u8;
    fn title(&self) -> SharedString {
        SharedString::from(self.label)
    }
    fn value(&self) -> &u8 {
        &self.byte
    }
}

// ── InputState scaffold ──────────────────────────────────────────────

/// Owns one [`InputState`] entity per text-edit [`EditableField`] plus
/// a [`SelectState`] for the Attacks dropdown, and the commit
/// subscriptions wiring them to the CRE / GAM. Held by
/// [`KeeperApp`]; built lazily on the first render so we don't need
/// `&mut Window` in `KeeperApp::new`.
pub struct KeeperEditors {
    inputs: HashMap<EditableField, Entity<InputState>>,
    /// Dropdown state for `EditableField::Attacks`. Stored
    /// separately because the value is not a free text edit — the
    /// underlying byte is one of 11 enum variants and the player
    /// sees the attacks-per-round string ("0.5", "1.5", …), not the
    /// raw byte.
    pub attacks: Entity<SelectState<Vec<AttacksOption>>>,
    _subs: Vec<Subscription>,
}

impl KeeperEditors {
    pub fn new(window: &mut Window, cx: &mut Context<KeeperApp>) -> Self {
        let mut inputs = HashMap::with_capacity(EditableField::ALL.len() - 1);
        let mut subs = Vec::with_capacity(EditableField::ALL.len());
        for &field in EditableField::ALL {
            // The Attacks row uses a dropdown (see `attacks` below)
            // — no free-text Input for it.
            if field == EditableField::Attacks {
                continue;
            }
            let state: Entity<InputState> = cx.new(|cx| InputState::new(window, cx));
            subs.push(cx.subscribe(&state, move |this, entity, event, cx| {
                commit_on_blur_or_enter(this, field, &entity, event, cx);
            }));
            inputs.insert(field, state);
        }

        let attacks: Entity<SelectState<Vec<AttacksOption>>> =
            cx.new(|cx| SelectState::new(AttacksOption::ALL.to_vec(), None, window, cx));
        subs.push(cx.subscribe(&attacks, commit_attacks_selection));

        Self {
            inputs,
            attacks,
            _subs: subs,
        }
    }

    pub fn input(&self, field: EditableField) -> &Entity<InputState> {
        self.inputs
            .get(&field)
            .expect("every text-edit EditableField has an InputState")
    }

    /// Push the current value of every field into its InputState
    /// (and the Attacks byte into the Attacks dropdown). Called by
    /// [`crate::app::KeeperApp::render`] whenever the selected party
    /// slot changes.
    pub fn rebind_to(&self, cre: &Cre, gam: &ImportedGam, window: &mut Window, cx: &mut App) {
        for &field in EditableField::ALL {
            if field == EditableField::Attacks {
                continue;
            }
            let text = field.read_text(cre, gam);
            self.input(field).update(cx, |state, cx| {
                state.set_value(text, window, cx);
            });
        }
        let attacks_byte = cre_fields::attacks_byte(cre);
        self.attacks.update(cx, |state, cx| {
            state.set_selected_value(&attacks_byte, window, cx);
        });
    }
}

fn commit_attacks_selection(
    this: &mut KeeperApp,
    _entity: Entity<SelectState<Vec<AttacksOption>>>,
    event: &SelectEvent<Vec<AttacksOption>>,
    cx: &mut Context<KeeperApp>,
) {
    let SelectEvent::Confirm(Some(byte)) = event else {
        return;
    };
    let Some(idx) = this.selected_party else {
        return;
    };
    let Some(npc) = this.state.imported_gam.party_npcs.get_mut(idx) else {
        return;
    };
    let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut() else {
        return;
    };
    cre_fields::set_attacks_byte(boxed, *byte);
    this.editors_bound_to = None;
    cx.notify();
}

fn commit_on_blur_or_enter(
    this: &mut KeeperApp,
    field: EditableField,
    entity: &Entity<InputState>,
    event: &InputEvent,
    cx: &mut Context<KeeperApp>,
) {
    // Re-render on every InputState event (including each keystroke)
    // so the derived bonus row next to ability-score editors updates
    // live, not just on commit.
    cx.notify();
    if !matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
        return;
    }
    let raw = entity.read(cx).value().to_string();
    // Split-borrow `state.imported_gam` (mut) and
    // `state.engine_caps` (immut) — disjoint fields of the same
    // `KeeperState`, so destructuring lets us hold both at once.
    let KeeperState {
        imported_gam,
        engine_caps,
        ..
    } = &mut this.state;

    if field.is_gam_field() {
        field.write_clamped_gam(imported_gam, &raw, engine_caps);
    } else if let Some(idx) = this.selected_party
        && let Some(npc) = imported_gam.party_npcs.get_mut(idx)
        && let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut()
    {
        field.write_clamped_cre(boxed, &raw, engine_caps);
    }

    // Force a UI re-render at the next paint. `set_value` needs
    // `&mut Window`, which the subscription callback doesn't get;
    // clearing `editors_bound_to` makes the next render's re-bind
    // path push the clamped value back into the InputState.
    this.editors_bound_to = None;
    cx.notify();
}
