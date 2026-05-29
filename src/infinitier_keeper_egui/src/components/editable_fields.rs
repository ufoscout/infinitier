//! Per-field editor scaffolding for the egui keeper.
//!
//! Mirrors the GPUI keeper's `editable_fields` (same enum, same
//! per-version dispatch, same clamping helpers) but with the
//! immediate-mode plumbing collapsed to plain `HashMap<…, String>`
//! buffers — egui has no InputState entities, no subscriptions, no
//! lazy init. Each frame:
//!
//! 1. [`KeeperEditors::prepare`] re-pulls the buffer values from the
//!    current CRE / GAM if the selected party slot has changed
//!    since the last frame (or if a commit just snapped a value to
//!    its clamped form).
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
use infinitier_core::engine_caps::{self, AbilityRange, EngineCaps};
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

    /// Parse + clamp + write to a CRE. No-op on GAM-side fields.
    fn write_clamped_cre(self, cre: &mut Cre, raw: &str, caps: &EngineCaps) {
        match self {
            Self::Strength => write_u8(raw, caps.ability_score, cre.strength(), |v| cre.set_strength(v)),
            Self::StrengthPct => write_u8(
                raw,
                caps.strength_percentile,
                cre.strength_bonus().unwrap_or(0),
                |v| cre.set_strength_bonus(v),
            ),
            Self::Dexterity => write_u8(raw, caps.ability_score, cre.dexterity(), |v| cre.set_dexterity(v)),
            Self::Constitution => write_u8(raw, caps.ability_score, cre.constitution(), |v| cre.set_constitution(v)),
            Self::Intelligence => write_u8(raw, caps.ability_score, cre.intelligence(), |v| cre.set_intelligence(v)),
            Self::Wisdom => write_u8(raw, caps.ability_score, cre.wisdom(), |v| cre.set_wisdom(v)),
            Self::Charisma => write_u8(raw, caps.ability_score, cre.charisma(), |v| cre.set_charisma(v)),
            Self::Thac0 => write_i8(raw, caps.thac0, cre_fields::thac0_or_bab(cre), |v| cre_fields::set_thac0_or_bab(cre, v)),
            // Attacks is edited through a dropdown — never via this path.
            Self::Attacks => debug_assert!(false, "Attacks uses a dropdown, not a text input"),
            Self::Fatigue => write_u8(raw, caps.fatigue, cre_fields::fatigue(cre), |v| cre_fields::set_fatigue(cre, v)),
            Self::Intoxication => write_u8(raw, caps.intoxication, cre_fields::intoxication(cre), |v| cre_fields::set_intoxication(cre, v)),
            Self::Luck => write_u8(raw, caps.luck, cre_fields::luck(cre), |v| cre_fields::set_luck(cre, v)),
            Self::Level1 => {
                let current = cre_fields::level_first_class(cre).unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| cre_fields::set_level_first_class(cre, v))
            }
            Self::Level2 => {
                let current = cre_fields::level_second_class(cre).unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| cre_fields::set_level_second_class(cre, v))
            }
            Self::Level3 => {
                let current = cre_fields::level_third_class(cre).unwrap_or(0);
                write_u8(raw, caps.class_level, current, |v| cre_fields::set_level_third_class(cre, v))
            }
            Self::Morale => {
                let current = cre_fields::morale(cre).unwrap_or(0);
                write_u8(raw, caps.morale, current, |v| cre_fields::set_morale(cre, v))
            }
            Self::MoraleBreak => {
                let current = cre_fields::morale_break(cre).unwrap_or(0);
                write_u8(raw, caps.morale_break, current, |v| cre_fields::set_morale_break(cre, v))
            }
            Self::HideInShadows => {
                let current = cre_fields::hide_in_shadows(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre_fields::set_hide_in_shadows(cre, v))
            }
            Self::MoveSilently => write_u8(raw, caps.thief_skill, cre_fields::move_silently(cre), |v| cre_fields::set_move_silently(cre, v)),
            Self::Lockpicking => {
                let current = cre_fields::lockpicking(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre_fields::set_lockpicking(cre, v))
            }
            Self::FindTraps => {
                let current = cre_fields::find_traps(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre_fields::set_find_traps(cre, v))
            }
            Self::SetTraps => {
                let current = cre_fields::set_traps(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre_fields::set_set_traps(cre, v))
            }
            Self::PickPockets => {
                let current = cre_fields::pick_pockets(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre_fields::set_pick_pockets(cre, v))
            }
            Self::DetectIllusion => {
                let current = cre_fields::detect_illusion(cre).unwrap_or(0);
                write_u8(raw, caps.thief_skill, current, |v| cre_fields::set_detect_illusion(cre, v))
            }
            Self::Lore => {
                let current = cre_fields::lore(cre).unwrap_or(0);
                write_u8(raw, caps.lore, current, |v| cre_fields::set_lore(cre, v))
            }
            Self::CurrentHp => write_u16(raw, caps.current_hit_points, cre_fields::current_hit_points(cre), |v| cre_fields::set_current_hit_points(cre, v)),
            Self::MaxHp => write_u16(raw, caps.max_hit_points, cre_fields::max_hit_points(cre), |v| cre_fields::set_max_hit_points(cre, v)),
            Self::MoraleRecovery => {
                let current = cre_fields::morale_recovery(cre).unwrap_or(0);
                write_u16(raw, caps.morale_recovery, current, |v| cre_fields::set_morale_recovery(cre, v))
            }
            Self::AcNatural => write_i16(raw, caps.armor_class, cre_fields::ac_natural(cre), |v| cre_fields::set_ac_natural(cre, v)),
            Self::AcEffective => {
                let current = cre_fields::ac_effective(cre).unwrap_or(0);
                write_i16(raw, caps.armor_class, current, |v| cre_fields::set_ac_effective(cre, v))
            }
            Self::Experience => write_u32(raw, caps.experience, cre_fields::experience(cre), |v| cre_fields::set_experience(cre, v)),
            Self::XpForKill => write_u32(raw, caps.xp_for_kill, cre_fields::xp_for_kill(cre), |v| cre_fields::set_xp_for_kill(cre, v)),
            // GAM-side fields are routed through `write_clamped_gam`.
            Self::Reputation | Self::PartyGold => {
                debug_assert!(false, "{self:?} is a GAM-side field");
            }
        }
    }

    fn write_clamped_gam(self, gam: &mut ImportedGam, raw: &str, caps: &EngineCaps) {
        match self {
            Self::Reputation => write_u32(raw, caps.reputation, cre_fields::party_reputation(gam), |v| cre_fields::set_party_reputation(gam, v)),
            Self::PartyGold => write_u32(raw, caps.party_gold, cre_fields::party_gold(gam), |v| cre_fields::set_party_gold(gam, v)),
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
    pub byte: u8,
    pub label: &'static str,
}

impl AttacksOption {
    /// Every documented attacks-per-round value, in dropdown order
    /// — integers interleaved with halves. Mirrors
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
    /// Last party slot we synced from. `None` means "force a rebind
    /// on next prepare" — used after commits to refresh the buffers
    /// with the clamped value.
    bound_to: Option<usize>,
}

impl KeeperEditors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Per-frame sync. Refresh all in-flight text buffers from the
    /// active CRE / GAM whenever the selected slot changes.
    pub fn prepare(&mut self, state: &AppState) {
        if self.bound_to == state.selected_party_index {
            return;
        }
        self.refresh_from_state(state);
        self.bound_to = state.selected_party_index;
    }

    fn refresh_from_state(&mut self, state: &AppState) {
        let Some(cre) = selected_cre(state) else {
            self.inputs.clear();
            self.attacks_idx = None;
            return;
        };
        for &field in EditableField::ALL {
            if field == EditableField::Attacks {
                continue;
            }
            self.inputs
                .insert(field, field.read_text(cre, &state.save));
        }
        self.attacks_idx = AttacksOption::index_for_byte(cre_fields::attacks_byte(cre));
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
        let response = ui.add(
            egui_components::Input::new(&mut buf)
                .width(width)
                .small(),
        );
        let committed = response.lost_focus();
        if committed {
            commit(field, &buf, state);
            if let Some(cre) = selected_cre(state) {
                buf = field.read_text(cre, &state.save);
            }
        }
        *self.inputs.entry(field).or_default() = buf;
        committed
    }
}

fn commit(field: EditableField, raw: &str, state: &mut AppState) {
    let caps = engine_caps::caps_for(state.game_data.game().engine());
    if field.is_gam_field() {
        field.write_clamped_gam(&mut state.save, raw, &caps);
        return;
    }
    let Some(idx) = state.selected_party_index else {
        return;
    };
    let Some(npc) = state.save.party_npcs.get_mut(idx) else {
        return;
    };
    let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut() else {
        return;
    };
    field.write_clamped_cre(boxed, raw, &caps);
}

/// Commit the Attacks dropdown's current selection back to the CRE.
/// Called by the abilities tab when the user picks a new option.
pub fn commit_attacks(idx: usize, state: &mut AppState) {
    let Some(option) = AttacksOption::ALL.get(idx) else {
        return;
    };
    let Some(slot) = state.selected_party_index else {
        return;
    };
    let Some(npc) = state.save.party_npcs.get_mut(slot) else {
        return;
    };
    let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut() else {
        return;
    };
    cre_fields::set_attacks_byte(boxed, option.byte);
}

fn selected_cre(state: &AppState) -> Option<&Cre> {
    let idx = state.selected_party_index?;
    let npc = state.save.party_npcs.get(idx)?;
    match npc.cre.as_ref()? {
        NpcCre::Cre(boxed) => Some(boxed.as_ref()),
        NpcCre::Ref(_) => None,
    }
}
