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
use egui_components::{Card, Label, LabelTone, Select, Size};
use infinitier_core::engine_caps;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::Engine;
use infinitier_core::resource::cre::{Cre, CreHeader, CreHeaderV22};

use crate::components::cre_fields;
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

        egui::ScrollArea::vertical().show(ui, |ui| {
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
            let constitution =
                read_u8(editors, EditableField::Constitution, snap.constitution);
            let bonuses = engine_caps::ability_bonuses(
                engine,
                strength,
                strength_pct,
                dexterity,
                constitution,
            );

            editable_row(
                ui,
                EditableField::Strength,
                snap,
                state,
                editors,
                Some(format_bonus(engine, BonusKind::ToHit, bonuses.thac0_from_strength)),
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
                Some(format_bonus(engine, BonusKind::Ac, bonuses.ac_from_dexterity)),
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
                    bonuses.hp_per_level_from_constitution,
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
            let constitution =
                read_u8(editors, EditableField::Constitution, snap.constitution);
            let bonuses = engine_caps::ability_bonuses(
                engine,
                strength,
                strength_pct,
                dexterity,
                constitution,
            );
            let thac0_base = read_i8(editors, EditableField::Thac0, snap.thac0_or_bab);
            let ac_base = read_i16(editors, EditableField::AcNatural, snap.ac_natural);
            let max_hp_base = read_u16(editors, EditableField::MaxHp, snap.max_hit_points);
            let level = i32::from(snap.primary_level);
            let effective_thac0: i32 = match engine {
                Engine::Iwd2 => i32::from(thac0_base) + i32::from(bonuses.thac0_from_strength),
                _ => i32::from(thac0_base) - i32::from(bonuses.thac0_from_strength),
            };
            let effective_ac: i32 = i32::from(ac_base) + i32::from(bonuses.ac_from_dexterity);
            let effective_max_hp: i32 = i32::from(max_hp_base)
                + level * i32::from(bonuses.hp_per_level_from_constitution);

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
    if let Some(rows) = &snap.iwd2_skills {
        Card::new().title("d20 Skills").divider().show(ui, |ui| {
            for (label, value) in rows {
                read_only_row(ui, label, value);
            }
        });
        return;
    }
    Card::new()
        .title("Thief Skills")
        .divider()
        .show(ui, |ui| {
            for &field in EditableField::ALL {
                if field.section() != Section::ThiefSkills || !snap.is_visible(field) {
                    continue;
                }
                editable_row(ui, field, snap, state, editors, None);
            }
        });
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
                ui.add(
                    Label::new(text)
                        .tone(LabelTone::Muted)
                        .size(Size::Small),
                );
            }
            ui.add_space(8.0);
            // Nested left-to-right block claims the leftover space on
            // the left side; the truncating label inside ellipsizes
            // when the column is narrow rather than colliding with
            // the bonus / input on the right.
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let theme = egui_components::theme::Theme::get(ui.ctx());
                    let rich = egui::RichText::new(label)
                        .color(theme.colors.muted_foreground)
                        .font(egui::FontId::proportional(theme.metrics.font_size_md));
                    ui.add(egui::Label::new(rich).truncate());
                },
            );
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
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let theme = egui_components::theme::Theme::get(ui.ctx());
                    let rich = egui::RichText::new(label)
                        .color(theme.colors.muted_foreground)
                        .font(egui::FontId::proportional(theme.metrics.font_size_md));
                    ui.add(egui::Label::new(rich).truncate());
                },
            );
        },
    );
}

fn show_attacks_dropdown(
    ui: &mut egui::Ui,
    state: &mut AppState,
    editors: &mut KeeperEditors,
) {
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
    thac0_or_bab: i8,
    ac_natural: i16,
    max_hit_points: u16,
    primary_level: u8,
    ability_total: u32,
    labels: HashMap<EditableField, &'static str>,
    visible: HashMap<EditableField, bool>,
    /// IWD2-only read-only block. `None` on AD&D engines.
    iwd2_skills: Option<Vec<(&'static str, String)>>,
    /// IWD2-only "Total levels" + per-class breakdown row.
    iwd2_levels: Option<(u8, String)>,
}

impl CreSnapshot {
    fn capture(state: &AppState) -> Option<Self> {
        let idx = state.selected_party_index?;
        let member = state.save.party_npcs.get(idx)?;
        let cre = match member.cre.as_ref()? {
            NpcCre::Cre(boxed) => boxed.as_ref(),
            NpcCre::Ref(_) => return None,
        };
        let mut labels = HashMap::with_capacity(EditableField::ALL.len());
        let mut visible = HashMap::with_capacity(EditableField::ALL.len());
        for &f in EditableField::ALL {
            labels.insert(f, f.label(cre));
            visible.insert(f, f.is_visible(cre));
        }
        let ability_total = u32::from(cre.strength())
            + u32::from(cre.dexterity())
            + u32::from(cre.constitution())
            + u32::from(cre.intelligence())
            + u32::from(cre.wisdom())
            + u32::from(cre.charisma());
        let (iwd2_skills, iwd2_levels) = match &cre.header {
            CreHeader::V22(h) => (
                Some(iwd2_d20_skills(h)),
                Some((h.total_levels, format_iwd2_class_levels(h))),
            ),
            _ => (None, None),
        };
        Some(Self {
            strength: cre.strength(),
            strength_pct: cre.strength_bonus(),
            dexterity: cre.dexterity(),
            constitution: cre.constitution(),
            thac0_or_bab: cre_fields::thac0_or_bab(cre),
            ac_natural: cre_fields::ac_natural(cre),
            max_hit_points: cre_fields::max_hit_points(cre),
            primary_level: cre_fields::primary_level(cre),
            ability_total,
            labels,
            visible,
            iwd2_skills,
            iwd2_levels,
        })
    }

    fn label(&self, field: EditableField) -> &'static str {
        self.labels.get(&field).copied().unwrap_or("?")
    }

    fn is_visible(&self, field: EditableField) -> bool {
        self.visible.get(&field).copied().unwrap_or(false)
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

fn iwd2_d20_skills(h: &CreHeaderV22) -> Vec<(&'static str, String)> {
    vec![
        ("Alchemy", h.alchemy.to_string()),
        ("Animal Empathy", h.animal_empathy.to_string()),
        ("Bluff", h.bluff.to_string()),
        ("Concentration", h.concentration.to_string()),
        ("Diplomacy", h.diplomacy.to_string()),
        ("Disable Device", h.disable_device.to_string()),
        ("Hide", h.hide.to_string()),
        ("Intimidate", h.intimidate.to_string()),
        ("Knowledge (Arcana)", h.knowledge_arcana.to_string()),
        ("Move Silently", h.move_silently.to_string()),
        ("Pick Pocket", h.pick_pocket.to_string()),
        ("Search", h.search.to_string()),
        ("Spellcraft", h.spellcraft.to_string()),
        ("Use Magic Device", h.use_magic_device.to_string()),
        ("Wilderness Lore", h.wilderness_law.to_string()),
    ]
}

// `Cre` is needed by the snapshot constructor; suppress the unused-import lint.
#[allow(dead_code)]
fn _phantom(_: &Cre) {}
