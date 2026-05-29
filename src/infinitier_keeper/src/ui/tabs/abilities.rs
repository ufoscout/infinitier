//! Abilities tab — every row is editable.
//!
//! The screen is a 3-column flex of cards (Ability scores + Combat
//! & status / Experience & levels + Morale / Thief Skills). Each
//! card iterates [`EditableField::ALL`] filtered by [`Section`] and
//! [`EditableField::is_visible`], rendering one [`Input`] per visible
//! row plus the read-only "Total" row at the bottom of the Ability
//! scores card.
//!
//! Per-version skill differences (IWD2 d20 skills, V12's combined
//! "Stealth" field) are surfaced via [`EditableField::label`] +
//! [`EditableField::is_visible`]. The V22 (IWD2) d20-skill block is
//! rendered as a separate read-only card because none of those
//! fields are wired through [`crate::cre_fields`] yet.

use gpui::{AnyElement, Context, FontWeight, IntoElement, ParentElement, Styled, div, px};
use gpui_component::input::{Input, InputState};
use gpui_component::select::Select;
use gpui_component::{ActiveTheme, Sizable as _, h_flex, v_flex};
use infinitier_core::engine_caps;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::Engine;
use infinitier_core::resource::cre::{Cre, CreHeader, CreHeaderV22};

use crate::app::KeeperApp;
use crate::cre_fields;
use crate::editable_fields::{EditableField, KeeperEditors, Section};

pub fn render(
    this: &KeeperApp,
    cre: &Cre,
    _gam: &ImportedGam,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    let editors = this
        .editors
        .as_ref()
        .expect("editors lazy-init runs in KeeperApp::render");
    let engine = this.state.game_data.game().engine();

    h_flex()
        .gap_3()
        .items_start()
        .w_full()
        .child(
            v_flex()
                .flex_1()
                .gap_3()
                .child(ability_scores_card(engine, cre, editors, cx))
                .child(combat_status_card(engine, cre, editors, cx)),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_3()
                .child(experience_levels_card(cre, editors, cx))
                .child(morale_card(cre, editors, cx)),
        )
        .child(
            v_flex()
                .flex_1()
                .gap_3()
                .child(skills_card(cre, editors, cx)),
        )
}

// ── Card builders ────────────────────────────────────────────────────

fn ability_scores_card(
    engine: Engine,
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    // Derive bonuses from the *current input text*, not from the
    // committed CRE — so the values refresh on every keystroke
    // (`commit_on_blur_or_enter` calls `cx.notify` on every event).
    // Falling back to the CRE on parse failure keeps the row sane
    // while the user is mid-edit (e.g. while the field is empty).
    let strength = input_value_u8(editors.input(EditableField::Strength), cx, cre.strength());
    let strength_pct = input_value_u8(
        editors.input(EditableField::StrengthPct),
        cx,
        cre.strength_bonus().unwrap_or(0),
    );
    let dexterity = input_value_u8(editors.input(EditableField::Dexterity), cx, cre.dexterity());
    let constitution = input_value_u8(
        editors.input(EditableField::Constitution),
        cx,
        cre.constitution(),
    );
    let bonuses = engine_caps::ability_bonuses(engine, strength, strength_pct, dexterity, constitution);

    let mut rows: Vec<AnyElement> = Vec::with_capacity(EditableField::ALL.len() + 1);
    let show_percentile = EditableField::StrengthPct.is_visible(cre);
    rows.push(editable_row_with_bonus(
        cx,
        EditableField::Strength,
        cre,
        editors,
        Some(format_bonus(engine, BonusKind::ToHit, bonuses.thac0_from_strength)),
    ));
    if show_percentile {
        rows.push(editable_row(cx, EditableField::StrengthPct, cre, editors));
    }
    rows.push(editable_row_with_bonus(
        cx,
        EditableField::Dexterity,
        cre,
        editors,
        Some(format_bonus(engine, BonusKind::Ac, bonuses.ac_from_dexterity)),
    ));
    rows.push(editable_row_with_bonus(
        cx,
        EditableField::Constitution,
        cre,
        editors,
        Some(format_bonus(
            engine,
            BonusKind::HpPerLevel,
            bonuses.hp_per_level_from_constitution,
        )),
    ));
    rows.push(editable_row(cx, EditableField::Intelligence, cre, editors));
    rows.push(editable_row(cx, EditableField::Wisdom, cre, editors));
    rows.push(editable_row(cx, EditableField::Charisma, cre, editors));

    // "Total" — derived, not editable; recomputed live from the CRE
    // so it tracks edits the next paint after a commit.
    let total = u32::from(cre.strength())
        + u32::from(cre.dexterity())
        + u32::from(cre.constitution())
        + u32::from(cre.intelligence())
        + u32::from(cre.wisdom())
        + u32::from(cre.charisma());
    rows.push(read_only_row(cx, "Total", total.to_string()));

    card(cx, "Ability scores", rows)
}

#[derive(Debug, Clone, Copy)]
enum BonusKind {
    ToHit,
    Ac,
    HpPerLevel,
}

/// "+1 THAC0" / "-4 AC" / "+2 HP/lvl" — the muted summary surfaced
/// between an ability-score row's label and its input. The "to-hit"
/// label changes per engine (THAC0 on AD&D, "to hit" on d20).
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

/// Read the current text of an `InputState` and parse it as a `u8`.
/// Returns `fallback` (the CRE's committed value) when the field is
/// empty, mid-edit, or otherwise unparseable — so the bonus stays
/// sensible while the user is still typing.
fn input_value_u8(input: &gpui::Entity<InputState>, cx: &Context<KeeperApp>, fallback: u8) -> u8 {
    let text = input.read(cx).value().to_string();
    let trimmed = text.trim();
    match trimmed.parse::<u32>() {
        Ok(n) => n.min(u8::MAX as u32) as u8,
        Err(_) => fallback,
    }
}

fn input_value_i8(input: &gpui::Entity<InputState>, cx: &Context<KeeperApp>, fallback: i8) -> i8 {
    let text = input.read(cx).value().to_string();
    match text.trim().parse::<i32>() {
        Ok(n) => n.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
        Err(_) => fallback,
    }
}

fn input_value_i16(
    input: &gpui::Entity<InputState>,
    cx: &Context<KeeperApp>,
    fallback: i16,
) -> i16 {
    let text = input.read(cx).value().to_string();
    match text.trim().parse::<i32>() {
        Ok(n) => n.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
        Err(_) => fallback,
    }
}

fn input_value_u16(
    input: &gpui::Entity<InputState>,
    cx: &Context<KeeperApp>,
    fallback: u16,
) -> u16 {
    let text = input.read(cx).value().to_string();
    match text.trim().parse::<u32>() {
        Ok(n) => n.min(u16::MAX as u32) as u16,
        Err(_) => fallback,
    }
}

fn combat_status_card(
    engine: Engine,
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    // Same parsing fallback used in `ability_scores_card`: mid-edit
    // (empty / unparseable) input falls back to the committed CRE
    // value so the derived totals stay sensible.
    let strength = input_value_u8(editors.input(EditableField::Strength), cx, cre.strength());
    let strength_pct = input_value_u8(
        editors.input(EditableField::StrengthPct),
        cx,
        cre.strength_bonus().unwrap_or(0),
    );
    let dexterity = input_value_u8(editors.input(EditableField::Dexterity), cx, cre.dexterity());
    let constitution = input_value_u8(
        editors.input(EditableField::Constitution),
        cx,
        cre.constitution(),
    );
    let bonuses = engine_caps::ability_bonuses(engine, strength, strength_pct, dexterity, constitution);

    // Base values: read from the in-flight input where possible so
    // the totals also update while the user is editing THAC0 / AC /
    // Max HP directly.
    let thac0_base = input_value_i8(
        editors.input(EditableField::Thac0),
        cx,
        cre_fields::thac0_or_bab(cre),
    );
    let ac_base = input_value_i16(
        editors.input(EditableField::AcNatural),
        cx,
        cre_fields::ac_natural(cre),
    );
    let max_hp_base = input_value_u16(
        editors.input(EditableField::MaxHp),
        cx,
        cre_fields::max_hit_points(cre),
    );
    let level = cre_fields::primary_level(cre) as i32;

    // Effective combat values, sign convention per engine. The
    // stored bytes are the un-buffed baselines; the engine adds the
    // ability bonuses (and item / spell effects) at runtime to get
    // the numbers shown on the in-game record sheet.
    let effective_thac0: i32 = match engine {
        // d20: BAB + STR mod (higher = better).
        Engine::Iwd2 => i32::from(thac0_base) + i32::from(bonuses.thac0_from_strength),
        // AD&D: THAC0 - STR bonus (lower = better; positive bonus subtracts).
        _ => i32::from(thac0_base) - i32::from(bonuses.thac0_from_strength),
    };
    // AC: dex_bonus already carries the right sign per engine.
    let effective_ac: i32 = i32::from(ac_base) + i32::from(bonuses.ac_from_dexterity);
    // Max HP: the stored byte is the rolled HP only — CON bonus is
    // applied per HD at runtime, so effective = base + level * bonus.
    let effective_max_hp: i32 =
        i32::from(max_hp_base) + level * i32::from(bonuses.hp_per_level_from_constitution);

    let rows = visible_rows_for_section_with_bonuses(
        Section::CombatStatus,
        cre,
        editors,
        cx,
        |field| match field {
            EditableField::Thac0 => Some(format!("(effective: {effective_thac0})")),
            EditableField::AcNatural => Some(format!("(effective: {effective_ac})")),
            EditableField::MaxHp => Some(format!("(effective: {effective_max_hp})")),
            _ => None,
        },
    );
    card(cx, "Combat & status", rows)
}

fn experience_levels_card(
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    let mut rows = visible_rows_for_section(Section::ExperienceLevels, cre, editors, cx);

    // IWD2 (V22) supplements the editable rows with a read-only
    // "Total levels" + per-class breakdown — none of those are
    // wired through `EditableField` yet, so surface them as
    // computed/read-only rows here.
    if let CreHeader::V22(h) = &cre.header {
        rows.push(read_only_row(cx, "Total levels", h.total_levels.to_string()));
        rows.push(read_only_row(cx, "Per-class levels", format_iwd2_class_levels(h)));
    }

    card(cx, "Experience & levels", rows)
}

fn morale_card(
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    let rows = visible_rows_for_section(Section::Morale, cre, editors, cx);
    // V22 has no morale system — give the empty card a placeholder
    // row so the column doesn't look broken.
    if rows.is_empty() {
        return card(
            cx,
            "Morale",
            vec![read_only_row(cx, "Morale system", "disabled (d20)".to_string())],
        );
    }
    card(cx, "Morale", rows)
}

fn skills_card(
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &mut Context<KeeperApp>,
) -> impl IntoElement {
    // IWD2 d20 skills aren't wired through `EditableField` (no AD&D
    // analogue, different field set). Render them as a read-only
    // dump so the section isn't empty on V22.
    if let CreHeader::V22(h) = &cre.header {
        let rows = iwd2_d20_skills(h)
            .into_iter()
            .map(|(label, value)| read_only_row(cx, label, value))
            .collect::<Vec<_>>();
        return card(cx, "d20 Skills", rows);
    }

    let rows = visible_rows_for_section(Section::ThiefSkills, cre, editors, cx);
    card(cx, "Thief Skills", rows)
}

// ── Row helpers ──────────────────────────────────────────────────────

fn visible_rows_for_section(
    section: Section,
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &Context<KeeperApp>,
) -> Vec<AnyElement> {
    visible_rows_for_section_with_bonuses(section, cre, editors, cx, |_| None)
}

/// Variant of [`visible_rows_for_section`] that lets the caller
/// attach a per-field derived-bonus indicator (e.g. the live
/// "→ effective THAC0" shown next to the THAC0 input).
fn visible_rows_for_section_with_bonuses(
    section: Section,
    cre: &Cre,
    editors: &KeeperEditors,
    cx: &Context<KeeperApp>,
    bonus: impl Fn(EditableField) -> Option<String>,
) -> Vec<AnyElement> {
    EditableField::ALL
        .iter()
        .copied()
        .filter(|field| field.section() == section && field.is_visible(cre))
        .map(|field| editable_row_with_bonus(cx, field, cre, editors, bonus(field)))
        .collect()
}

/// One editable row: muted label left, narrow editor right. Most
/// fields use a text `Input`; `EditableField::Attacks` uses a
/// dropdown of the documented attacks-per-round values so the user
/// sees "1.5" rather than the raw byte `7`.
fn editable_row(
    cx: &Context<KeeperApp>,
    field: EditableField,
    cre: &Cre,
    editors: &KeeperEditors,
) -> AnyElement {
    editable_row_with_bonus(cx, field, cre, editors, None)
}

/// Variant of [`editable_row`] that adds a muted derived-bonus
/// summary between the label and the input. Used for the STR / DEX
/// / CON rows on the Ability scores card.
fn editable_row_with_bonus(
    cx: &Context<KeeperApp>,
    field: EditableField,
    cre: &Cre,
    editors: &KeeperEditors,
    bonus: Option<String>,
) -> AnyElement {
    let theme = cx.theme();
    let editor = if field == EditableField::Attacks {
        div().w(px(110.)).child(Select::new(&editors.attacks).small()).into_any_element()
    } else {
        div().w(px(110.)).child(Input::new(editors.input(field)).small()).into_any_element()
    };
    let mut row = h_flex()
        .py_0p5()
        .gap_3()
        .w_full()
        .items_center()
        .child(
            div()
                .flex_1()
                .text_color(theme.muted_foreground)
                .child(field.label(cre)),
        );
    if let Some(text) = bonus {
        row = row.child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child(text),
        );
    }
    row.child(editor).into_any_element()
}

/// Read-only row used for derived values (Total, per-class breakdown)
/// and d20-skill stubs.
fn read_only_row(cx: &Context<KeeperApp>, label: &str, value: String) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .py_0p5()
        .gap_3()
        .w_full()
        .items_center()
        .child(
            div()
                .flex_1()
                .text_color(theme.muted_foreground)
                .child(label.to_string()),
        )
        .child(div().font_weight(FontWeight::BOLD).child(value))
        .into_any_element()
}

/// One card: bold title, separator, then a list of pre-built rows.
fn card(cx: &Context<KeeperApp>, title: &str, rows: Vec<AnyElement>) -> impl IntoElement {
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
        .children(rows)
}

// ── IWD2 helpers ─────────────────────────────────────────────────────

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
