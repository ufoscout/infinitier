//! Abilities tab — mirrors the EEKeeper "Abilities" view.
//!
//! Display is read-only and per-engine: the CRE header variant
//! (`V1.0`, `V1.2`, `V9.0`, `V2.2`) drives which fields exist and
//! how they are labelled. We render straight off `Cre.header` rather
//! than introducing accessors for every byte — the layout matches
//! the on-disk fields, so the source of truth is the parsed header.

use eframe::egui;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::Game;
use infinitier_core::resource::cre::{Cre, CreHeader, CreHeaderV22};
use infinitier_egui_common::theme;

pub struct AbilitiesTab;

impl AbilitiesTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, gam: &ImportedGam, _game: Game) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(3, |cols| {
                section(&mut cols[0], "Ability scores", |ui| ability_scores(ui, cre));
                cols[0].add_space(8.0);
                section(&mut cols[0], "Combat & status", |ui| {
                    combat_stats(ui, cre, gam)
                });

                section(&mut cols[1], "Experience & levels", |ui| {
                    experience_levels(ui, cre)
                });
                cols[1].add_space(8.0);
                section(&mut cols[1], "Morale", |ui| morale(ui, cre));

                section(&mut cols[2], skills_section_title(cre), |ui| {
                    skills(ui, cre)
                });
            });
        });
    }
}

fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    theme::card_frame(ui).show(ui, |ui| {
        ui.label(theme::card_title(title));
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);
        body(ui);
    });
}

fn ability_scores(ui: &mut egui::Ui, cre: &Cre) {
    let str_score = cre.strength();
    row(ui, "Strength", &str_score.to_string());
    // AD&D exceptional-strength % bonus. IWD2 d20 drops it
    // (returns None) — render "—" so the layout stays uniform.
    match cre.strength_bonus() {
        Some(b) => row(ui, "Strength %", &b.to_string()),
        None => row(ui, "Strength %", "—"),
    }
    row(ui, "Dexterity", &cre.dexterity().to_string());
    row(ui, "Constitution", &cre.constitution().to_string());
    row(ui, "Intelligence", &cre.intelligence().to_string());
    row(ui, "Wisdom", &cre.wisdom().to_string());
    row(ui, "Charisma", &cre.charisma().to_string());
    // EEKeeper's "Total" line — sum of the six core scores.
    let total = u32::from(cre.strength())
        + u32::from(cre.dexterity())
        + u32::from(cre.constitution())
        + u32::from(cre.intelligence())
        + u32::from(cre.wisdom())
        + u32::from(cre.charisma());
    row(ui, "Total", &total.to_string());
}

fn combat_stats(ui: &mut egui::Ui, cre: &Cre, gam: &ImportedGam) {
    row(ui, "Current HP", &cre.current_hit_points().to_string());
    row(ui, "Max HP", &cre.maximum_hit_points().to_string());
    // Gold + reputation are party-wide.
    let party_gold = gam.header.party_gold;
    let rep = gam.engine_data.reputation();
    match &cre.header {
        CreHeader::V10(h) => {
            row(ui, "AC (natural)", &h.armor_class_natural.to_string());
            row(ui, "AC (effective)", &h.armor_class_effective.to_string());
            row(ui, "THAC0", &h.thac0.to_string());
            row(ui, "Attacks", &h.number_of_attacks.to_string());
            row(ui, "Reputation (party)", &rep.to_string());
            row(ui, "Gold (party)", &party_gold.to_string());
            row(ui, "Fatigue", &h.fatigue.to_string());
            row(ui, "Intoxication", &h.intoxication.to_string());
            row(ui, "Luck", &h.luck.to_string());
        }
        CreHeader::V12(h) => {
            row(ui, "AC (natural)", &h.armor_class_natural.to_string());
            row(ui, "AC (effective)", &h.armor_class_effective.to_string());
            row(ui, "THAC0", &h.thac0.to_string());
            row(ui, "Attacks", &h.number_of_attacks.to_string());
            row(ui, "Reputation (party)", &rep.to_string());
            row(ui, "Gold (party)", &party_gold.to_string());
            row(ui, "Fatigue", &h.fatigue.to_string());
            row(ui, "Intoxication", &h.intoxication.to_string());
            row(ui, "Luck", &h.luck.to_string());
        }
        CreHeader::V90(h) => {
            row(ui, "AC (natural)", &h.armor_class_natural.to_string());
            row(ui, "AC (effective)", &h.armor_class_effective.to_string());
            row(ui, "THAC0", &h.thac0.to_string());
            row(ui, "Attacks", &h.number_of_attacks.to_string());
            row(ui, "Reputation (party)", &rep.to_string());
            row(ui, "Gold (party)", &party_gold.to_string());
            row(ui, "Fatigue", &h.fatigue.to_string());
            row(ui, "Intoxication", &h.intoxication.to_string());
            row(ui, "Luck", &h.luck.to_string());
        }
        CreHeader::V22(h) => {
            // IWD2: single AC field (no "natural" vs "effective"
            // split); THAC0 is replaced by Base Attack Bonus.
            row(ui, "AC", &h.armor_class.to_string());
            row(
                ui,
                "Base Attack Bonus",
                &h.base_attack_bonus_bab_for_non.to_string(),
            );
            row(ui, "Attacks", &h.number_of_attacks.to_string());
            row(ui, "Reputation (party)", &rep.to_string());
            row(ui, "Gold (party)", &party_gold.to_string());
            row(ui, "Fatigue", &h.fatigue.to_string());
            row(ui, "Intoxication", &h.intoxication.to_string());
            row(ui, "Luck", &h.luck.to_string());
        }
    }
}

fn experience_levels(ui: &mut egui::Ui, cre: &Cre) {
    match &cre.header {
        CreHeader::V10(h) => {
            row(
                ui,
                "Experience",
                &h.creature_power_level_for_summoning_spells.to_string(),
            );
            row(
                ui,
                "Exp for kill",
                &h.xp_gained_for_killing_this_creature.to_string(),
            );
            row(
                ui,
                "Level (1st class)",
                &h.level_first_class_highest_attained_level.to_string(),
            );
            row(
                ui,
                "Level (2nd class)",
                &h.level_second_class_highest_attained_level.to_string(),
            );
            row(
                ui,
                "Level (3rd class)",
                &h.level_third_class_highest_attained_level.to_string(),
            );
        }
        CreHeader::V12(h) => {
            // PST splits XP across primary / secondary / tertiary
            // class pools (the Nameless One's class-switching system).
            row(
                ui,
                "Experience (primary)",
                &h.creature_power_level_for_summoning_spells.to_string(),
            );
            row(
                ui,
                "Experience (2nd class)",
                &h.xp_secondary_class.to_string(),
            );
            row(
                ui,
                "Experience (3rd class)",
                &h.xp_tertiary_class.to_string(),
            );
            row(
                ui,
                "Exp for kill",
                &h.xp_gained_for_killing_this_creature.to_string(),
            );
            row(
                ui,
                "Level (1st class)",
                &h.highest_attained_level_in_class.to_string(),
            );
            row(
                ui,
                "Level (2nd class)",
                &h.highest_attained_level_in_class_2.to_string(),
            );
            row(
                ui,
                "Level (3rd class)",
                &h.highest_attained_level_in_class_3.to_string(),
            );
        }
        CreHeader::V90(h) => {
            row(
                ui,
                "Experience",
                &h.creature_power_level_for_summoning_spells.to_string(),
            );
            row(
                ui,
                "Exp for kill",
                &h.xp_gained_for_killing_this_creature.to_string(),
            );
            row(
                ui,
                "Level (1st class)",
                &h.highest_attained_level_in_class.to_string(),
            );
            row(
                ui,
                "Level (2nd class)",
                &h.highest_attained_level_in_class_2.to_string(),
            );
            row(
                ui,
                "Level (3rd class)",
                &h.highest_attained_level_in_class_3.to_string(),
            );
        }
        CreHeader::V22(h) => {
            // IWD2 uses a single shared XP pool; the per-class level
            // breakdown sits in the dedicated levels section below.
            row(
                ui,
                "Experience",
                &h.creature_power_level_for_summoning_spells.to_string(),
            );
            row(
                ui,
                "Exp for kill",
                &h.xp_gained_for_killing_this_creature.to_string(),
            );
            row(ui, "Total levels", &h.total_levels.to_string());
            row(ui, "Per-class levels", &format_iwd2_class_levels(h));
        }
    }
}

fn morale(ui: &mut egui::Ui, cre: &Cre) {
    match &cre.header {
        CreHeader::V10(h) => {
            row(
                ui,
                "Morale",
                &h.morale_default_value_is_10_capped.to_string(),
            );
            row(
                ui,
                "Morale break",
                &h.morale_break_see_here_for_further.to_string(),
            );
            row(
                ui,
                "Morale recovery",
                &h.morale_recovery_time_see_here_for.to_string(),
            );
        }
        CreHeader::V12(h) => {
            row(ui, "Morale", &h.morale.to_string());
            row(ui, "Morale break", &h.morale_break.to_string());
            row(ui, "Morale recovery", &h.morale_recovery_time.to_string());
        }
        CreHeader::V90(h) => {
            row(ui, "Morale", &h.morale.to_string());
            row(ui, "Morale break", &h.morale_break.to_string());
            row(ui, "Morale recovery", &h.morale_recovery_time.to_string());
        }
        CreHeader::V22(_) => {
            // IWD2 has no morale system on creatures in the same way;
            // the legacy bytes are still in the header but unused by
            // gameplay. Surface a placeholder so the section stays
            // consistent across versions.
            row(ui, "Morale system disabled (d20)", "—");
        }
    }
}

fn skills_section_title(cre: &Cre) -> &'static str {
    match &cre.header {
        CreHeader::V22(_) => "d20 Skills",
        _ => "Thief Skills",
    }
}

fn skills(ui: &mut egui::Ui, cre: &Cre) {
    match &cre.header {
        CreHeader::V10(h) => {
            // V1.0 / EE has both Hide in Shadows and Move Silently as
            // distinct skills (alongside the AD&D thief set).
            row(ui, "Hide in Shadows", &h.hide_in_shadows_base.to_string());
            row(ui, "Move Silently", &h.move_silently.to_string());
            row(ui, "Open Locks", &h.lockpicking.to_string());
            row(ui, "Find Traps", &h.find_disarm_traps.to_string());
            row(ui, "Set Traps", &h.set_traps.to_string());
            row(ui, "Pick Pockets", &h.pick_pockets.to_string());
            row(ui, "Detect Illusions", &h.detect_illusion.to_string());
            row(ui, "Lore", &h.lore.to_string());
        }
        CreHeader::V12(h) => {
            // PST: a single "stealth" skill replaces the
            // Hide/Move-Silently pair.
            row(ui, "Stealth", &h.stealth.to_string());
            row(ui, "Open Locks", &h.lockpicking.to_string());
            row(ui, "Find Traps", &h.find_disarm_traps.to_string());
            row(ui, "Set Traps", &h.set_traps.to_string());
            row(ui, "Pick Pockets", &h.pick_pockets.to_string());
            row(ui, "Detect Illusions", &h.detect_illusion.to_string());
            row(ui, "Lore", &h.lore.to_string());
        }
        CreHeader::V90(h) => {
            row(ui, "Hide in Shadows", &h.hide_in_shadows_base.to_string());
            row(ui, "Stealth", &h.stealth.to_string());
            row(ui, "Open Locks", &h.lockpicking.to_string());
            row(ui, "Find Traps", &h.find_disarm_traps.to_string());
            row(ui, "Set Traps", &h.set_traps.to_string());
            row(ui, "Pick Pockets", &h.pick_pockets.to_string());
            row(ui, "Detect Illusions", &h.detect_illusion.to_string());
            row(ui, "Lore", &h.lore.to_string());
        }
        CreHeader::V22(h) => {
            row(ui, "Alchemy", &h.alchemy.to_string());
            row(ui, "Animal Empathy", &h.animal_empathy.to_string());
            row(ui, "Bluff", &h.bluff.to_string());
            row(ui, "Concentration", &h.concentration.to_string());
            row(ui, "Diplomacy", &h.diplomacy.to_string());
            row(ui, "Disable Device", &h.disable_device.to_string());
            row(ui, "Hide", &h.hide.to_string());
            row(ui, "Intimidate", &h.intimidate.to_string());
            row(ui, "Knowledge (Arcana)", &h.knowledge_arcana.to_string());
            row(ui, "Move Silently", &h.move_silently.to_string());
            row(ui, "Pick Pocket", &h.pick_pocket.to_string());
            row(ui, "Search", &h.search.to_string());
            row(ui, "Spellcraft", &h.spellcraft.to_string());
            row(ui, "Use Magic Device", &h.use_magic_device.to_string());
            row(ui, "Wilderness Lore", &h.wilderness_law.to_string());
        }
    }
}

/// Thin shim so every call in this file uses the same name. The real
/// rendering — left-aligned muted label + right-aligned bold value —
/// lives in `egui_common::theme::row` and matches the Slint
/// `Row` widget exactly.
fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    theme::row(ui, label, value);
}

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
    entries
        .iter()
        .filter(|(_, lvl)| *lvl > 0)
        .map(|(name, lvl)| format!("{name} {lvl}"))
        .collect::<Vec<_>>()
        .join(", ")
}
