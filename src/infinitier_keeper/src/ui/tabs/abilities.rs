//! Abilities tab — mirrors the EEKeeper "Abilities" view.
//!
//! Display is read-only and per-engine: the CRE header variant
//! (`V1.0`, `V1.2`, `V9.0`, `V2.2`) drives which fields exist and
//! how they are labelled. We render straight off `Cre.header` rather
//! than introducing accessors for every byte — the layout matches
//! the on-disk fields, so the source of truth is the parsed header.

use eframe::egui;
use infinitier_common::Game;
use infinitier_cre_resource::{Cre, CreHeader};
use infinitier_gam_resource::{Gam, GamEngineData};

pub struct AbilitiesTab;

impl AbilitiesTab {
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, gam: &Gam, _game: Game) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            // Three even columns spanning the available width — using
            // `ui.columns` (rather than `horizontal_top` + nested
            // `vertical`s) is what actually constrains each column to
            // 1/N of the width; otherwise the first vertical greedily
            // takes everything.
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

/// Party-wide reputation lives on the GAM, in `reputation × 10`
/// units. Returns the player-facing value (0..=20 typically).
fn party_reputation(gam: &Gam) -> u32 {
    let raw = match &gam.engine_data {
        GamEngineData::Bg(d) => d.reputation,
        GamEngineData::Bg2(d) => d.reputation,
        GamEngineData::Ee(d) => d.reputation,
        GamEngineData::Iwd(d) => d.reputation,
        GamEngineData::Iwd2(d) => d.reputation,
        GamEngineData::Pst(d) => d.reputation,
    };
    raw / 10
}

fn section(ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.strong(title);
        ui.separator();
        body(ui);
    });
}

fn ability_scores(ui: &mut egui::Ui, cre: &Cre) {
    egui::Grid::new("abilities_scores")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            let str_score = cre.strength();
            row(ui, "Strength", &str_score.to_string());
            // AD&D exceptional-strength % bonus. IWD2 d20 drops it
            // (returns None) — render "—" so the grid stays uniform.
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
        });
}

fn combat_stats(ui: &mut egui::Ui, cre: &Cre, gam: &Gam) {
    egui::Grid::new("abilities_combat")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
            row(ui, "Current HP", &cre.current_hit_points().to_string());
            row(ui, "Max HP", &cre.maximum_hit_points().to_string());
            // Gold + reputation are party-wide and live on the GAM,
            // not the per-character CRE (the CRE fields are typically
            // 0 for party members). We pull from the GAM so the
            // numbers match what the engine actually uses.
            let party_gold = gam.header.party_gold;
            let rep = party_reputation(gam);
            match &cre.header {
                CreHeader::V10(h) => {
                    row(ui, "AC (natural)", &h.armor_class_natural.to_string());
                    row(ui, "AC (effective)", &h.armor_class_effective.to_string());
                    row(ui, "THAC0", &h.thac0.to_string());
                    row(ui, "Attacks", &format_attacks(h.number_of_attacks));
                    row(ui, "Reputation", &rep.to_string());
                    row(ui, "Gold (party)", &party_gold.to_string());
                    row(ui, "Fatigue", &h.fatigue.to_string());
                    row(ui, "Intoxication", &h.intoxication.to_string());
                    row(ui, "Luck", &h.luck.to_string());
                }
                CreHeader::V12(h) => {
                    row(ui, "AC (natural)", &h.armor_class_natural.to_string());
                    row(ui, "AC (effective)", &h.armor_class_effective.to_string());
                    row(ui, "THAC0", &h.thac0.to_string());
                    row(ui, "Attacks", &format_attacks(h.number_of_attacks));
                    row(ui, "Reputation", &rep.to_string());
                    row(ui, "Gold (party)", &party_gold.to_string());
                    row(ui, "Fatigue", &h.fatigue.to_string());
                    row(ui, "Intoxication", &h.intoxication.to_string());
                    row(ui, "Luck", &h.luck.to_string());
                }
                CreHeader::V90(h) => {
                    row(ui, "AC (natural)", &h.armor_class_natural.to_string());
                    row(ui, "AC (effective)", &h.armor_class_effective.to_string());
                    row(ui, "THAC0", &h.thac0.to_string());
                    row(ui, "Attacks", &format_attacks(h.number_of_attacks));
                    row(ui, "Reputation", &rep.to_string());
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
                    row(ui, "Attacks", &format_attacks(h.number_of_attacks));
                    row(ui, "Reputation", &rep.to_string());
                    row(ui, "Gold (party)", &party_gold.to_string());
                    row(ui, "Fatigue", &h.fatigue.to_string());
                    row(ui, "Intoxication", &h.intoxication.to_string());
                    row(ui, "Luck", &h.luck.to_string());
                }
            }
        });
}

fn experience_levels(ui: &mut egui::Ui, cre: &Cre) {
    egui::Grid::new("abilities_xp")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .striped(true)
        .show(ui, |ui| {
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
                    // PST splits XP across primary / secondary /
                    // tertiary class pools (the Nameless One's
                    // class-switching system).
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
                    // IWD2 uses a single shared XP pool; the per-class
                    // level breakdown sits in the dedicated levels
                    // section below.
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
                    ui.label("Per-class levels");
                    ui.label(format_iwd2_class_levels(h));
                    ui.end_row();
                }
            }
        });
}

fn morale(ui: &mut egui::Ui, cre: &Cre) {
    egui::Grid::new("abilities_morale")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .striped(true)
        .show(ui, |ui| match &cre.header {
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
                // IWD2 has no morale system on creatures in the same
                // way; the legacy bytes are still in the header but
                // unused by gameplay. Surface a placeholder so the
                // section stays consistent across versions.
                ui.label("Morale system disabled (d20)");
                ui.label("—");
                ui.end_row();
            }
        });
}

fn skills_section_title(cre: &Cre) -> &'static str {
    match &cre.header {
        CreHeader::V22(_) => "d20 Skills",
        _ => "Thief Skills",
    }
}

fn skills(ui: &mut egui::Ui, cre: &Cre) {
    egui::Grid::new("abilities_skills")
        .num_columns(2)
        .spacing([16.0, 4.0])
        .striped(true)
        .show(ui, |ui| match &cre.header {
            CreHeader::V10(h) => {
                // V1.0 / EE has both Hide in Shadows and Move Silently
                // as distinct skills (alongside the AD&D thief set).
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
        });
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(label);
    ui.strong(value);
    ui.end_row();
}

/// Decode the CRE `number_of_attacks` byte into the player-facing
/// attacks-per-round string. Per IESDP: 0..=5 are literal counts,
/// 6..=10 encode the half-attack offsets (6 = ½, 7 = 3/2, 8 = 5/2,
/// 9 = 7/2, 10 = 9/2). Anything else falls through as a raw count.
fn format_attacks(raw: u8) -> String {
    match raw {
        0..=5 => raw.to_string(),
        6 => "0.5".to_string(),
        7 => "1.5".to_string(),
        8 => "2.5".to_string(),
        9 => "3.5".to_string(),
        10 => "4.5".to_string(),
        _ => raw.to_string(),
    }
}

fn format_iwd2_class_levels(h: &infinitier_cre_resource::CreHeaderV22) -> String {
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
        "—".to_string()
    } else {
        parts.join(", ")
    }
}
