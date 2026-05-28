//! Abilities tab — the rich one. Per-CRE-version dispatch produces
//! `(label, value)` row lists, then `section` renders each list as a
//! card identical in layout to the Slint spike (title + separator +
//! right-aligned values).

use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, px};
use gpui_component::{ActiveTheme, h_flex, v_flex};
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::cre::{Cre, CreHeader, CreHeaderV22};

use crate::app::KeeperApp;

pub fn render(cre: &Cre, gam: &ImportedGam, cx: &mut Context<KeeperApp>) -> impl IntoElement {
    h_flex()
        .gap_3()
        .items_start()
        .w_full()
        .child(
            v_flex()
                .flex_1()
                .gap_3()
                .child(section(cx, "Ability scores", ability_scores(cre)))
                .child(section(cx, "Combat & status", combat_stats(cre, gam))),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_3()
                .child(section(cx, "Experience & levels", experience_levels(cre)))
                .child(section(cx, "Morale", morale_rows(cre))),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_3()
                .child(section(cx, skills_title(cre), skills(cre))),
        )
}

/// One card: bold title, separator, then a list of label/value rows.
fn section(cx: &Context<KeeperApp>, title: &str, rows: Vec<(String, String)>) -> impl IntoElement {
    let theme = cx.theme();
    v_flex()
        .p_3()
        .gap_1()
        .w_full()
        .rounded(theme.radius)
        .bg(theme.secondary)
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(14.))
                .child(title.to_string()),
        )
        .child(div().h_px().mt_1().mb_1().bg(theme.border))
        .children(
            rows.into_iter()
                .map(|(label, value)| key_value_row(cx, label, value)),
        )
}

/// One field row: muted label left, bold value right-aligned to the
/// card's inner edge — same shape Slint's `Row` component paints.
fn key_value_row(cx: &Context<KeeperApp>, label: String, value: String) -> impl IntoElement {
    let theme = cx.theme();
    h_flex()
        .py_0p5()
        .gap_3()
        .w_full()
        .child(
            div()
                .flex_1()
                .text_color(theme.muted_foreground)
                .child(label),
        )
        .child(div().font_weight(FontWeight::BOLD).child(value))
}

// ── Per-section builders (ported verbatim from the Slint spike) ──────

fn ability_scores(cre: &Cre) -> Vec<(String, String)> {
    let str_score = cre.strength();
    let str_bonus = match cre.strength_bonus() {
        Some(b) => b.to_string(),
        None => "—".into(),
    };
    let total = u32::from(cre.strength())
        + u32::from(cre.dexterity())
        + u32::from(cre.constitution())
        + u32::from(cre.intelligence())
        + u32::from(cre.wisdom())
        + u32::from(cre.charisma());
    vec![
        ("Strength".into(), str_score.to_string()),
        ("Strength %".into(), str_bonus),
        ("Dexterity".into(), cre.dexterity().to_string()),
        ("Constitution".into(), cre.constitution().to_string()),
        ("Intelligence".into(), cre.intelligence().to_string()),
        ("Wisdom".into(), cre.wisdom().to_string()),
        ("Charisma".into(), cre.charisma().to_string()),
        ("Total".into(), total.to_string()),
    ]
}

fn combat_stats(cre: &Cre, gam: &ImportedGam) -> Vec<(String, String)> {
    let mut rows = vec![
        ("Current HP".into(), cre.current_hit_points().to_string()),
        ("Max HP".into(), cre.maximum_hit_points().to_string()),
    ];
    let party_gold = gam.header.party_gold.to_string();
    let rep = gam.engine_data.reputation().to_string();
    match &cre.header {
        CreHeader::V10(h) => {
            rows.push(("AC (natural)".into(), h.armor_class_natural.to_string()));
            rows.push(("AC (effective)".into(), h.armor_class_effective.to_string()));
            rows.push(("THAC0".into(), h.thac0.to_string()));
            rows.push(("Attacks".into(), h.number_of_attacks.to_string()));
            rows.push(("Reputation (party)".into(), rep));
            rows.push(("Gold (party)".into(), party_gold));
            rows.push(("Fatigue".into(), h.fatigue.to_string()));
            rows.push(("Intoxication".into(), h.intoxication.to_string()));
            rows.push(("Luck".into(), h.luck.to_string()));
        }
        CreHeader::V12(h) => {
            rows.push(("AC (natural)".into(), h.armor_class_natural.to_string()));
            rows.push(("AC (effective)".into(), h.armor_class_effective.to_string()));
            rows.push(("THAC0".into(), h.thac0.to_string()));
            rows.push(("Attacks".into(), h.number_of_attacks.to_string()));
            rows.push(("Reputation (party)".into(), rep));
            rows.push(("Gold (party)".into(), party_gold));
            rows.push(("Fatigue".into(), h.fatigue.to_string()));
            rows.push(("Intoxication".into(), h.intoxication.to_string()));
            rows.push(("Luck".into(), h.luck.to_string()));
        }
        CreHeader::V90(h) => {
            rows.push(("AC (natural)".into(), h.armor_class_natural.to_string()));
            rows.push(("AC (effective)".into(), h.armor_class_effective.to_string()));
            rows.push(("THAC0".into(), h.thac0.to_string()));
            rows.push(("Attacks".into(), h.number_of_attacks.to_string()));
            rows.push(("Reputation (party)".into(), rep));
            rows.push(("Gold (party)".into(), party_gold));
            rows.push(("Fatigue".into(), h.fatigue.to_string()));
            rows.push(("Intoxication".into(), h.intoxication.to_string()));
            rows.push(("Luck".into(), h.luck.to_string()));
        }
        CreHeader::V22(h) => {
            rows.push(("AC".into(), h.armor_class.to_string()));
            rows.push((
                "Base Attack Bonus".into(),
                h.base_attack_bonus_bab_for_non.to_string(),
            ));
            rows.push(("Attacks".into(), h.number_of_attacks.to_string()));
            rows.push(("Reputation (party)".into(), rep));
            rows.push(("Gold (party)".into(), party_gold));
            rows.push(("Fatigue".into(), h.fatigue.to_string()));
            rows.push(("Intoxication".into(), h.intoxication.to_string()));
            rows.push(("Luck".into(), h.luck.to_string()));
        }
    }
    rows
}

fn experience_levels(cre: &Cre) -> Vec<(String, String)> {
    match &cre.header {
        CreHeader::V10(h) => vec![
            (
                "Experience".into(),
                h.creature_power_level_for_summoning_spells.to_string(),
            ),
            (
                "Exp for kill".into(),
                h.xp_gained_for_killing_this_creature.to_string(),
            ),
            (
                "Level (1st class)".into(),
                h.level_first_class_highest_attained_level.to_string(),
            ),
            (
                "Level (2nd class)".into(),
                h.level_second_class_highest_attained_level.to_string(),
            ),
            (
                "Level (3rd class)".into(),
                h.level_third_class_highest_attained_level.to_string(),
            ),
        ],
        CreHeader::V12(h) => vec![
            (
                "Experience (primary)".into(),
                h.creature_power_level_for_summoning_spells.to_string(),
            ),
            (
                "Experience (2nd class)".into(),
                h.xp_secondary_class.to_string(),
            ),
            (
                "Experience (3rd class)".into(),
                h.xp_tertiary_class.to_string(),
            ),
            (
                "Exp for kill".into(),
                h.xp_gained_for_killing_this_creature.to_string(),
            ),
            (
                "Level (1st class)".into(),
                h.highest_attained_level_in_class.to_string(),
            ),
            (
                "Level (2nd class)".into(),
                h.highest_attained_level_in_class_2.to_string(),
            ),
            (
                "Level (3rd class)".into(),
                h.highest_attained_level_in_class_3.to_string(),
            ),
        ],
        CreHeader::V90(h) => vec![
            (
                "Experience".into(),
                h.creature_power_level_for_summoning_spells.to_string(),
            ),
            (
                "Exp for kill".into(),
                h.xp_gained_for_killing_this_creature.to_string(),
            ),
            (
                "Level (1st class)".into(),
                h.highest_attained_level_in_class.to_string(),
            ),
            (
                "Level (2nd class)".into(),
                h.highest_attained_level_in_class_2.to_string(),
            ),
            (
                "Level (3rd class)".into(),
                h.highest_attained_level_in_class_3.to_string(),
            ),
        ],
        CreHeader::V22(h) => vec![
            (
                "Experience".into(),
                h.creature_power_level_for_summoning_spells.to_string(),
            ),
            (
                "Exp for kill".into(),
                h.xp_gained_for_killing_this_creature.to_string(),
            ),
            ("Total levels".into(), h.total_levels.to_string()),
            ("Per-class levels".into(), format_iwd2_class_levels(h)),
        ],
    }
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

fn morale_rows(cre: &Cre) -> Vec<(String, String)> {
    match &cre.header {
        CreHeader::V10(h) => vec![
            (
                "Morale".into(),
                h.morale_default_value_is_10_capped.to_string(),
            ),
            (
                "Morale break".into(),
                h.morale_break_see_here_for_further.to_string(),
            ),
            (
                "Morale recovery".into(),
                h.morale_recovery_time_see_here_for.to_string(),
            ),
        ],
        CreHeader::V12(h) => vec![
            ("Morale".into(), h.morale.to_string()),
            ("Morale break".into(), h.morale_break.to_string()),
            ("Morale recovery".into(), h.morale_recovery_time.to_string()),
        ],
        CreHeader::V90(h) => vec![
            ("Morale".into(), h.morale.to_string()),
            ("Morale break".into(), h.morale_break.to_string()),
            ("Morale recovery".into(), h.morale_recovery_time.to_string()),
        ],
        CreHeader::V22(_) => vec![("Morale system".into(), "disabled (d20)".into())],
    }
}

fn skills_title(cre: &Cre) -> &'static str {
    match &cre.header {
        CreHeader::V22(_) => "d20 Skills",
        _ => "Thief Skills",
    }
}

fn skills(cre: &Cre) -> Vec<(String, String)> {
    match &cre.header {
        CreHeader::V10(h) => vec![
            ("Hide in Shadows".into(), h.hide_in_shadows_base.to_string()),
            ("Move Silently".into(), h.move_silently.to_string()),
            ("Open Locks".into(), h.lockpicking.to_string()),
            ("Find Traps".into(), h.find_disarm_traps.to_string()),
            ("Set Traps".into(), h.set_traps.to_string()),
            ("Pick Pockets".into(), h.pick_pockets.to_string()),
            ("Detect Illusions".into(), h.detect_illusion.to_string()),
            ("Lore".into(), h.lore.to_string()),
        ],
        CreHeader::V12(h) => vec![
            ("Stealth".into(), h.stealth.to_string()),
            ("Open Locks".into(), h.lockpicking.to_string()),
            ("Find Traps".into(), h.find_disarm_traps.to_string()),
            ("Set Traps".into(), h.set_traps.to_string()),
            ("Pick Pockets".into(), h.pick_pockets.to_string()),
            ("Detect Illusions".into(), h.detect_illusion.to_string()),
            ("Lore".into(), h.lore.to_string()),
        ],
        CreHeader::V90(h) => vec![
            ("Hide in Shadows".into(), h.hide_in_shadows_base.to_string()),
            ("Stealth".into(), h.stealth.to_string()),
            ("Open Locks".into(), h.lockpicking.to_string()),
            ("Find Traps".into(), h.find_disarm_traps.to_string()),
            ("Set Traps".into(), h.set_traps.to_string()),
            ("Pick Pockets".into(), h.pick_pockets.to_string()),
            ("Detect Illusions".into(), h.detect_illusion.to_string()),
            ("Lore".into(), h.lore.to_string()),
        ],
        CreHeader::V22(h) => vec![
            ("Alchemy".into(), h.alchemy.to_string()),
            ("Animal Empathy".into(), h.animal_empathy.to_string()),
            ("Bluff".into(), h.bluff.to_string()),
            ("Concentration".into(), h.concentration.to_string()),
            ("Diplomacy".into(), h.diplomacy.to_string()),
            ("Disable Device".into(), h.disable_device.to_string()),
            ("Hide".into(), h.hide.to_string()),
            ("Intimidate".into(), h.intimidate.to_string()),
            ("Knowledge (Arcana)".into(), h.knowledge_arcana.to_string()),
            ("Move Silently".into(), h.move_silently.to_string()),
            ("Pick Pocket".into(), h.pick_pocket.to_string()),
            ("Search".into(), h.search.to_string()),
            ("Spellcraft".into(), h.spellcraft.to_string()),
            ("Use Magic Device".into(), h.use_magic_device.to_string()),
            ("Wilderness Lore".into(), h.wilderness_law.to_string()),
        ],
    }
}
