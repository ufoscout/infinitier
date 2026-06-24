//! Abilities tab — every visible row is editable.
//!
//! Mirrors the GPUI keeper's editable layout (3 columns of cards,
//! per-section field filter via [`EditableField::section`] +
//! [`EditableField::is_visible`], live "effective THAC0 / AC / Max HP"
//! summaries derived from the *in-flight* input text so the totals
//! update on every keystroke). The actual text-buffer plumbing
//! lives on [`KeeperEditors`]; this module only paints rows and
//! routes commits through the editor.
//!
//! egui's borrow rules want us to NOT hold a `&Cre` while we call
//! `editors.show_input(..., &mut state, ...)` — so each card pulls a
//! [`CreSnapshot`] of the immutable values it needs (labels,
//! visibility flags, current bytes for the live-bonus math), drops
//! the `&state.save` borrow, and then renders the rows.

use std::collections::HashMap;

use eframe::egui;
use egui_components::scroll_area::ScrollArea;
use egui_components::{Card, Label, LabelTone, Select, Size};
use infinitier_core::engine_caps::ThiefSkill;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::Engine;
use infinitier_core::resource::cre::{Cre, CreHeader, CreHeaderV22};

use crate::components::editable_fields::{
    AttacksOption, EditableField, KeeperEditors, Section, commit_attacks,
};
use crate::state::AppState;

const INPUT_WIDTH: f32 = 110.0;

pub struct AbilitiesTab;

impl AbilitiesTab {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState, editors: &mut KeeperEditors) {
        let Some(snapshot) = CreSnapshot::capture(state) else {
            ui.label("Empty party slot — no creature record to edit.");
            return;
        };
        let engine = state.game_data.game().engine();

        ScrollArea::vertical().show(ui, |ui| {
            ui.columns(3, |cols| {
                ability_scores_card(&mut cols[0], &snapshot, state, editors, engine);
                cols[0].add_space(8.0);
                combat_status_card(&mut cols[0], &snapshot, state, editors, engine);

                experience_levels_card(&mut cols[1], &snapshot, state, editors);
                cols[1].add_space(8.0);
                morale_card(&mut cols[1], &snapshot, state, editors);

                skills_card(&mut cols[2], &snapshot, state, editors);
            });
        });
    }
}

// ── Card builders ────────────────────────────────────────────────────

fn ability_scores_card(
    ui: &mut egui::Ui,
    snap: &CreSnapshot,
    state: &mut AppState,
    editors: &mut KeeperEditors,
    engine: Engine,
) {
    Card::new()
        .title("Ability scores")
        .divider()
        .show(ui, |ui| {
            // Live bonuses derived from in-flight input text — falls
            // back to the committed CRE values when a field is empty.
            let strength = read_u8(editors, EditableField::Strength, snap.strength);
            let strength_pct = read_u8(
                editors,
                EditableField::StrengthPct,
                snap.strength_pct.unwrap_or(0),
            );
            let dexterity = read_u8(editors, EditableField::Dexterity, snap.dexterity);
            let constitution = read_u8(editors, EditableField::Constitution, snap.constitution);
            let bonuses =
                state
                    .engine_caps
                    .ability_bonuses(strength, strength_pct, dexterity, constitution);
            // Per-level CON→HP bonus, engine-aware (AD&D warrior/normal
            // HPCONBON column, or the IWD2 d20 modifier) so the value
            // shown here always matches the effective Max HP below.
            let con_hp_per_level = state
                .engine_caps
                .constitution_hp_per_level(constitution, snap.is_warrior)
                as i8;

            editable_row(
                ui,
                EditableField::Strength,
                snap,
                state,
                editors,
                Some(format_bonus(
                    engine,
                    BonusKind::ToHit,
                    bonuses.thac0_from_strength,
                )),
            );
            if snap.is_visible(EditableField::StrengthPct) {
                editable_row(ui, EditableField::StrengthPct, snap, state, editors, None);
            }
            editable_row(
                ui,
                EditableField::Dexterity,
                snap,
                state,
                editors,
                Some(format_bonus(
                    engine,
                    BonusKind::Ac,
                    bonuses.ac_from_dexterity,
                )),
            );
            editable_row(
                ui,
                EditableField::Constitution,
                snap,
                state,
                editors,
                Some(format_bonus(
                    engine,
                    BonusKind::HpPerLevel,
                    con_hp_per_level,
                )),
            );
            editable_row(ui, EditableField::Intelligence, snap, state, editors, None);
            editable_row(ui, EditableField::Wisdom, snap, state, editors, None);
            editable_row(ui, EditableField::Charisma, snap, state, editors, None);

            // "Total" — derived, read-only.
            read_only_row(ui, "Total", &snap.ability_total.to_string());
        });
}

fn combat_status_card(
    ui: &mut egui::Ui,
    snap: &CreSnapshot,
    state: &mut AppState,
    editors: &mut KeeperEditors,
    engine: Engine,
) {
    Card::new()
        .title("Combat & status")
        .divider()
        .show(ui, |ui| {
            let strength = read_u8(editors, EditableField::Strength, snap.strength);
            let strength_pct = read_u8(
                editors,
                EditableField::StrengthPct,
                snap.strength_pct.unwrap_or(0),
            );
            let dexterity = read_u8(editors, EditableField::Dexterity, snap.dexterity);
            let constitution = read_u8(editors, EditableField::Constitution, snap.constitution);
            let bonuses =
                state
                    .engine_caps
                    .ability_bonuses(strength, strength_pct, dexterity, constitution);
            let thac0_base = read_i8(editors, EditableField::Thac0, snap.thac0_or_bab);
            let ac_base = read_i16(editors, EditableField::AcNatural, snap.ac_natural);
            let max_hp_base = read_u16(editors, EditableField::MaxHp, snap.max_hit_points);
            let effective_thac0: i32 = match engine {
                Engine::Iwd2 => i32::from(thac0_base) + i32::from(bonuses.thac0_from_strength),
                _ => i32::from(thac0_base) - i32::from(bonuses.thac0_from_strength),
            };
            let effective_ac: i32 = i32::from(ac_base) + i32::from(bonuses.ac_from_dexterity);
            // The stored `maximum_hit_points` excludes the CON bonus
            // (the engine re-applies it at runtime — unlike THAC0/AC,
            // current HP can exceed stored max). Effective = stored +
            // CON bonus, whose rules (multi-class split, Hit-Die cap or
            // not for Torment, …) all live in `EngineCaps`.
            let con_bonus = state.engine_caps.max_hp_constitution_bonus_for(
                constitution,
                snap.is_warrior,
                snap.hp_roll_cap,
                snap.primary_level,
                snap.class_levels,
                snap.class_count,
                snap.is_dual,
            );
            let effective_max_hp: i32 = i32::from(max_hp_base) + con_bonus;

            for &field in EditableField::ALL {
                if field.section() != Section::CombatStatus || !snap.is_visible(field) {
                    continue;
                }
                let bonus = match field {
                    EditableField::Thac0 => Some(format!("(effective: {effective_thac0})")),
                    EditableField::AcNatural => Some(format!("(effective: {effective_ac})")),
                    EditableField::MaxHp => Some(format!("(effective: {effective_max_hp})")),
                    _ => None,
                };
                editable_row(ui, field, snap, state, editors, bonus);
            }
        });
}

fn experience_levels_card(
    ui: &mut egui::Ui,
    snap: &CreSnapshot,
    state: &mut AppState,
    editors: &mut KeeperEditors,
) {
    Card::new()
        .title("Experience & levels")
        .divider()
        .show(ui, |ui| {
            for &field in EditableField::ALL {
                if field.section() != Section::ExperienceLevels || !snap.is_visible(field) {
                    continue;
                }
                editable_row(ui, field, snap, state, editors, None);
            }
            if let Some((total, breakdown)) = &snap.iwd2_levels {
                read_only_row(ui, "Total levels", &total.to_string());
                read_only_row(ui, "Per-class levels", breakdown);
            }
        });
}

fn morale_card(
    ui: &mut egui::Ui,
    snap: &CreSnapshot,
    state: &mut AppState,
    editors: &mut KeeperEditors,
) {
    Card::new().title("Morale").divider().show(ui, |ui| {
        let mut painted = false;
        for &field in EditableField::ALL {
            if field.section() != Section::Morale || !snap.is_visible(field) {
                continue;
            }
            editable_row(ui, field, snap, state, editors, None);
            painted = true;
        }
        if !painted {
            read_only_row(ui, "Morale system", "disabled (d20)");
        }
    });
}

fn skills_card(
    ui: &mut egui::Ui,
    snap: &CreSnapshot,
    state: &mut AppState,
    editors: &mut KeeperEditors,
) {
    // IWD2 (V2.2): the d20 skills replace the AD&D thief skills. They are
    // plain stored bytes (no racial / dexterity bonus overlay), so each
    // row is a bare editable input.
    if snap.is_visible(EditableField::Alchemy) {
        Card::new().title("Skills").divider().show(ui, |ui| {
            for &field in EditableField::ALL {
                if field.section() != Section::D20Skills || !snap.is_visible(field) {
                    continue;
                }
                editable_row(ui, field, snap, state, editors, None);
            }
        });
        return;
    }
    // Effective Lore = stored base + signed INT/WIS bonus (LOREBON.2DA).
    // The base byte is what the editor stores; the game displays the
    // bonused total. Read in-flight INT/WIS/Lore so the preview tracks
    // live edits, mirroring the effective THAC0 / AC / Max HP rows.
    let intelligence = read_u8(editors, EditableField::Intelligence, snap.intelligence);
    let wisdom = read_u8(editors, EditableField::Wisdom, snap.wisdom);
    let lore_base = read_u8(editors, EditableField::Lore, snap.lore);
    let effective_lore =
        (i32::from(lore_base) + state.engine_caps.lore_bonus(intelligence, wisdom)).max(0);
    // The thief-skill rows show the effective value: stored base byte
    // plus the racial adjustment (SKILLRAC.2DA, by race) plus the
    // dexterity adjustment (SKILLDEX.2DA, by DEX) — both applied by the
    // engine at runtime, neither baked into the stored byte. Read DEX
    // in-flight so the previews track edits.
    let dexterity = read_u8(editors, EditableField::Dexterity, snap.dexterity);

    Card::new().title("Thief Skills").divider().show(ui, |ui| {
        for &field in EditableField::ALL {
            if field.section() != Section::ThiefSkills || !snap.is_visible(field) {
                continue;
            }
            let bonus = if field == EditableField::Lore {
                Some(format!("(effective: {effective_lore})"))
            } else if let Some(skill) = thief_skill_of(field) {
                let base = read_u8(editors, field, snap.thief_skill_base(field));
                let effective = (i32::from(base)
                    + state
                        .engine_caps
                        .thief_skill_bonus(skill, dexterity, snap.race))
                .max(0);
                Some(format!("(effective: {effective})"))
            } else {
                None
            };
            editable_row(ui, field, snap, state, editors, bonus);
        }
    });
}

/// Map an editable thief-skill field to its [`ThiefSkill`] for the
/// SKILLDEX dexterity lookup. `None` for Lore (its bonus comes from
/// LOREBON, handled separately).
fn thief_skill_of(field: EditableField) -> Option<ThiefSkill> {
    Some(match field {
        EditableField::Lockpicking => ThiefSkill::OpenLocks,
        EditableField::FindTraps => ThiefSkill::FindTraps,
        EditableField::SetTraps => ThiefSkill::SetTraps,
        EditableField::PickPockets => ThiefSkill::PickPockets,
        EditableField::DetectIllusion => ThiefSkill::DetectIllusion,
        EditableField::HideInShadows => ThiefSkill::HideInShadows,
        EditableField::MoveSilently => ThiefSkill::MoveSilently,
        _ => return None,
    })
}

// ── Row primitives ───────────────────────────────────────────────────

fn editable_row(
    ui: &mut egui::Ui,
    field: EditableField,
    snap: &CreSnapshot,
    state: &mut AppState,
    editors: &mut KeeperEditors,
    bonus: Option<String>,
) {
    let label = snap.label(field);
    // CRITICAL: bound the row by `allocate_ui_with_layout(vec2(w, 0), …)`
    // so the inner `right_to_left` block can't grow vertically. Without
    // this, the sub-region inherits the card body's full remaining
    // height and each row balloons to hundreds of px tall.
    let avail_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(avail_w, 0.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            if field == EditableField::Attacks {
                show_attacks_dropdown(ui, state, editors);
            } else {
                editors.show_input(ui, field, state, INPUT_WIDTH);
            }
            if let Some(text) = bonus {
                ui.add_space(8.0);
                ui.add(Label::new(text).tone(LabelTone::Muted).size(Size::Small));
            }
            ui.add_space(8.0);
            // Nested left-to-right block claims the leftover space on
            // the left side; the truncating label inside ellipsizes
            // when the column is narrow rather than colliding with
            // the bonus / input on the right.
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let theme = egui_components::theme::Theme::get(ui.ctx());
                let rich = egui::RichText::new(label)
                    .color(theme.colors.muted_foreground)
                    .font(egui::FontId::proportional(theme.metrics.font_size_md));
                ui.add(egui::Label::new(rich).truncate());
            });
        },
    );
}

fn read_only_row(ui: &mut egui::Ui, label: &str, value: &str) {
    let avail_w = ui.available_width();
    ui.allocate_ui_with_layout(
        egui::vec2(avail_w, 0.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.add(Label::new(value).strong());
            ui.add_space(8.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                let theme = egui_components::theme::Theme::get(ui.ctx());
                let rich = egui::RichText::new(label)
                    .color(theme.colors.muted_foreground)
                    .font(egui::FontId::proportional(theme.metrics.font_size_md));
                ui.add(egui::Label::new(rich).truncate());
            });
        },
    );
}

fn show_attacks_dropdown(ui: &mut egui::Ui, state: &mut AppState, editors: &mut KeeperEditors) {
    let labels: Vec<&'static str> = AttacksOption::ALL.iter().map(|o| o.label).collect();
    let previous = editors.attacks_idx;
    Select::new("attacks", &mut editors.attacks_idx)
        .options(labels.iter().copied())
        .placeholder("…")
        .width(INPUT_WIDTH)
        .show(ui);
    if editors.attacks_idx != previous
        && let Some(idx) = editors.attacks_idx
    {
        commit_attacks(idx, state);
    }
}

// ── Helpers for live-bonus computation ───────────────────────────────

fn read_u8(editors: &KeeperEditors, field: EditableField, fallback: u8) -> u8 {
    editors
        .text(field)
        .trim()
        .parse::<u32>()
        .map(|n| n.min(u8::MAX as u32) as u8)
        .unwrap_or(fallback)
}

fn read_i8(editors: &KeeperEditors, field: EditableField, fallback: i8) -> i8 {
    editors
        .text(field)
        .trim()
        .parse::<i32>()
        .map(|n| n.clamp(i8::MIN as i32, i8::MAX as i32) as i8)
        .unwrap_or(fallback)
}

fn read_i16(editors: &KeeperEditors, field: EditableField, fallback: i16) -> i16 {
    editors
        .text(field)
        .trim()
        .parse::<i32>()
        .map(|n| n.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
        .unwrap_or(fallback)
}

fn read_u16(editors: &KeeperEditors, field: EditableField, fallback: u16) -> u16 {
    editors
        .text(field)
        .trim()
        .parse::<u32>()
        .map(|n| n.min(u16::MAX as u32) as u16)
        .unwrap_or(fallback)
}

// ── Bonus formatter ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum BonusKind {
    ToHit,
    Ac,
    HpPerLevel,
}

fn format_bonus(engine: Engine, kind: BonusKind, value: i8) -> String {
    let suffix = match kind {
        BonusKind::ToHit => match engine {
            Engine::Iwd2 => "to hit",
            _ => "THAC0",
        },
        BonusKind::Ac => "AC",
        BonusKind::HpPerLevel => "HP/lvl",
    };
    format!("{value:+} {suffix}")
}

// ── Snapshot — immutable copy of everything we need from the CRE ─────

/// Everything the abilities tab reads from the active CRE / GAM.
/// Captured up-front so the rest of the rendering path can take
/// `&mut state` without conflicting with a long-held `&Cre`.
struct CreSnapshot {
    strength: u8,
    strength_pct: Option<u8>,
    dexterity: u8,
    constitution: u8,
    intelligence: u8,
    wisdom: u8,
    /// Race byte (RACE.IDS index) — drives the SKILLRAC thief-skill
    /// racial adjustment.
    race: u8,
    /// Stored base Lore byte (effective Lore adds the INT/WIS bonus).
    lore: u8,
    thac0_or_bab: i8,
    ac_natural: i16,
    max_hit_points: u16,
    primary_level: u8,
    /// Warrior (Fighter/Paladin/Ranger/Barbarian) — selects the
    /// larger CON→HP table.
    is_warrior: bool,
    /// Highest level that still rolls a Hit Die (and so earns the
    /// CON HP bonus): 9 for warriors/priests, 10 for rogues/wizards.
    hp_roll_cap: u8,
    /// Per-class levels (0 = unused slot). Drives the multi-class CON
    /// HP split.
    class_levels: [u8; 3],
    /// Number of classes the CLASS.IDS byte names (1 single, 2/3 multi)
    /// — the multi-class HP divisor. Robust to Torment storing `1` in
    /// unused level slots (unlike counting non-zero levels).
    class_count: usize,
    /// Dual-classed (vs multi-classed) — they use different HP rules.
    is_dual: bool,
    ability_total: u32,
    labels: HashMap<EditableField, &'static str>,
    visible: HashMap<EditableField, bool>,
    /// Stored base byte for each AD&D thief skill (effective value adds
    /// the SKILLDEX dexterity adjustment on top). Excludes Lore, whose
    /// bonus comes from LOREBON.
    thief_skill_base: HashMap<EditableField, u8>,
    /// IWD2-only "Total levels" + per-class breakdown row.
    iwd2_levels: Option<(u8, String)>,
}

impl CreSnapshot {
    fn capture(state: &AppState) -> Option<Self> {
        let active = state.active();
        let idx = active.selected_party_index?;
        let member = active.save.party_npcs.get(idx)?;
        let cre = match member.cre.as_ref()? {
            NpcCre::Cre(boxed) => boxed.as_ref(),
            NpcCre::Ref(_) => return None,
        };
        let mut labels = HashMap::with_capacity(EditableField::ALL.len());
        let mut visible = HashMap::with_capacity(EditableField::ALL.len());
        let mut thief_skill_base = HashMap::new();
        for &f in EditableField::ALL {
            labels.insert(f, f.label(cre));
            visible.insert(f, f.is_visible(cre));
            if f.section() == Section::ThiefSkills && f != EditableField::Lore {
                thief_skill_base.insert(f, thief_skill_base_value(cre, f));
            }
        }
        let ability_total = u32::from(cre.strength())
            + u32::from(cre.dexterity())
            + u32::from(cre.constitution())
            + u32::from(cre.intelligence())
            + u32::from(cre.wisdom())
            + u32::from(cre.charisma());
        let iwd2_levels = match &cre.header {
            CreHeader::V22(h) => Some((h.total_levels, format_iwd2_class_levels(h))),
            _ => None,
        };
        // Warrior flag + CON HP-roll cap from the raw class byte —
        // EngineCaps owns the IWD2 / EE-data / classic-heuristic
        // decision (and the CLASS.IDS lookup it loaded once at startup).
        let class_id = match &cre.header {
            CreHeader::V10(h) => h.class_class_ids,
            CreHeader::V12(h) => h.class_class_ids,
            CreHeader::V90(h) => h.class_class_ids,
            CreHeader::V22(_) => 0, // IWD2: class byte unused (engine handles it)
        };
        let (is_warrior, hp_roll_cap) = state.engine_caps.class_hp_profile(class_id);
        let class_count = state.engine_caps.class_count(class_id);
        Some(Self {
            strength: cre.strength(),
            strength_pct: cre.strength_bonus(),
            dexterity: cre.dexterity(),
            constitution: cre.constitution(),
            intelligence: cre.intelligence(),
            wisdom: cre.wisdom(),
            race: cre.race(),
            lore: cre.lore().unwrap_or(0),
            thac0_or_bab: cre.thac0_or_bab(),
            ac_natural: cre.ac_natural(),
            max_hit_points: cre.maximum_hit_points(),
            primary_level: cre.primary_level(),
            is_warrior,
            hp_roll_cap,
            class_levels: cre.class_levels(),
            class_count,
            is_dual: cre.is_dual_classed(),
            ability_total,
            labels,
            visible,
            thief_skill_base,
            iwd2_levels,
        })
    }

    fn label(&self, field: EditableField) -> &'static str {
        self.labels.get(&field).copied().unwrap_or("?")
    }

    fn is_visible(&self, field: EditableField) -> bool {
        self.visible.get(&field).copied().unwrap_or(false)
    }

    /// Stored base byte for a thief-skill field (0 if not a tracked
    /// thief skill).
    fn thief_skill_base(&self, field: EditableField) -> u8 {
        self.thief_skill_base.get(&field).copied().unwrap_or(0)
    }
}

/// Read the stored base byte for an AD&D thief-skill field from the CRE.
fn thief_skill_base_value(cre: &Cre, field: EditableField) -> u8 {
    match field {
        EditableField::HideInShadows => cre.hide_in_shadows().unwrap_or(0),
        EditableField::MoveSilently => cre.move_silently(),
        EditableField::Lockpicking => cre.lockpicking().unwrap_or(0),
        EditableField::FindTraps => cre.find_traps().unwrap_or(0),
        EditableField::SetTraps => cre.set_traps().unwrap_or(0),
        EditableField::PickPockets => cre.pick_pockets().unwrap_or(0),
        EditableField::DetectIllusion => cre.detect_illusion().unwrap_or(0),
        _ => 0,
    }
}

// ── IWD2 read-only helpers ───────────────────────────────────────────

fn format_iwd2_class_levels(h: &CreHeaderV22) -> String {
    let entries = [
        ("Barbarian", h.barbarian_levels),
        ("Bard", h.bard_levels),
        ("Cleric", h.cleric_levels),
        ("Druid", h.druid_levels),
        ("Fighter", h.fighter_levels),
        ("Monk", h.monk),
        ("Paladin", h.paladin_levels),
        ("Ranger", h.ranger_levels),
        ("Rogue", h.rogue_levels),
        ("Sorcerer", h.sorcerer_levels),
        ("Wizard", h.wizard_levels),
    ];
    let parts: Vec<String> = entries
        .iter()
        .filter(|(_, lvl)| *lvl > 0)
        .map(|(name, lvl)| format!("{name} {lvl}"))
        .collect();
    if parts.is_empty() {
        "—".into()
    } else {
        parts.join(", ")
    }
}
