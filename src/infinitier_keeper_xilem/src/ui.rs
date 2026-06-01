//! The Xilem view tree — the read-only equivalent of the egui keeper's
//! `app.rs` + `ui/*` panels. `app_logic` is the root component: it reads
//! `&mut AppState` and rebuilds the whole tree each pass. Navigation
//! (active save tab, selected party slot, active character tab, theme
//! toggle) mutates `AppState` from button callbacks; the data itself is
//! displayed read-only.

use std::collections::HashMap;

use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
use infinitier_core::resource::cre::Cre;
use xilem::masonry::layout::Length;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt as _, FlexSpacer, MainAxisAlignment, flex_col, flex_row, portal,
    sized_box, text_input,
};
use xilem::{AnyWidgetView, WidgetView};
use xilem_components::Theme;
use xilem_components::view as xc;

use crate::fields::{AttacksOption, EditableField, Section};
use crate::state::AppState;
use crate::tabs::CharacterTab;

/// Width (logical px) of each editable value input.
const INPUT_WIDTH: f32 = 96.0;

/// Boxed, type-erased view over the keeper state. Action is `()` — all
/// interaction is in-place state mutation.
type View = Box<AnyWidgetView<AppState>>;

fn px(v: f32) -> Length {
    Length::px(v as f64)
}

const RAIL_WIDTH: f32 = 240.0;

/// Root component.
pub fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    // Refill the in-flight edit buffers whenever the selected save/slot
    // changed since they were last bound (mirrors the egui keeper's
    // `KeeperEditors::prepare`).
    ensure_editors(state);

    let theme = if state.dark {
        Theme::dark()
    } else {
        Theme::light()
    };

    flex_col((
        header(theme, state),
        save_tab_strip(theme, state),
        main_area(theme, state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .main_axis_alignment(MainAxisAlignment::Start)
}

// ── Editing: in-flight buffers + commit ───────────────────────────────

/// Refresh `state.editors` from the active CRE/GAM if the bound
/// `(save tab, party slot)` changed.
fn ensure_editors(state: &mut AppState) {
    let key = (state.active_tab, state.active().selected_party_index);
    if state.editors_bound_to == Some(key) {
        return;
    }
    refresh_editors(state);
    state.editors_bound_to = Some(key);
}

/// Fill every editable field's buffer with its current displayed value.
fn refresh_editors(state: &mut AppState) {
    let mut map = HashMap::new();
    {
        let active = state.active();
        if let Some(idx) = active.selected_party_index
            && let Some(npc) = active.save.party_npcs.get(idx)
            && let Some(NpcCre::Cre(cre)) = npc.cre.as_ref()
        {
            let gam = &active.save;
            for &field in EditableField::ALL {
                map.insert(field, field.read_text(cre, gam));
            }
        }
    }
    state.editors = map;
}

/// Handle a keystroke in an editable field. Masonry's text input has no
/// blur event (only per-keystroke `Changed` and `Enter`), so we clamp
/// *live*: as soon as the text is a complete integer we commit it (which
/// writes the clamped value back and reflects it in the box). Transient,
/// not-yet-numeric input — empty, a lone `-`, etc. — is kept verbatim so
/// the user can finish typing.
fn edit_field(state: &mut AppState, field: EditableField, new: String) {
    if new.trim().parse::<i64>().is_ok() {
        commit_field(state, field, &new);
    } else {
        state.editors.insert(field, new);
    }
}

/// Parse + clamp + write `raw` for `field`, then refresh that field's
/// buffer with the committed (clamped) value so the input shows it.
fn commit_field(state: &mut AppState, field: EditableField, raw: &str) {
    {
        let AppState {
            engine_caps,
            tabs,
            active_tab,
            ..
        } = &mut *state;
        let active = &mut tabs[*active_tab];
        if field.is_gam_field() {
            field.write_clamped_gam(&mut active.save, raw, engine_caps);
        } else if let Some(idx) = active.selected_party_index
            && let Some(npc) = active.save.party_npcs.get_mut(idx)
            && let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut()
        {
            field.write_clamped_cre(boxed, raw, engine_caps);
        }
    }
    let committed = current_field_text(state, field);
    state.editors.insert(field, committed);
}

/// The current displayed value of `field` for the active CRE/GAM.
fn current_field_text(state: &AppState, field: EditableField) -> String {
    let active = state.active();
    if let Some(idx) = active.selected_party_index
        && let Some(npc) = active.save.party_npcs.get(idx)
        && let Some(NpcCre::Cre(cre)) = npc.cre.as_ref()
    {
        return field.read_text(cre, &active.save);
    }
    String::new()
}

// ── Header bar (Load / Save / theme toggle) ───────────────────────────

fn header(theme: Theme, state: &AppState) -> impl WidgetView<AppState> + use<> {
    let status = state.status.clone().unwrap_or_default();
    xc::bar(
        theme,
        flex_row((
            xc::button_primary(theme, "Load", |_: &mut AppState| {
                log::info!("[load] not implemented in the Xilem port");
            }),
            xc::button_primary(theme, "Save", |s: &mut AppState| {
                s.status = Some(match crate::save::save_active(s) {
                    Ok(dest) => format!("Saved → {}", dest.display()),
                    Err(e) => format!("Save failed: {e}"),
                });
            }),
            xc::muted::<AppState, ()>(theme, status),
            FlexSpacer::Flex(1.0),
            xc::tab_button(theme, "Theme", false, |s: &mut AppState| s.dark = !s.dark),
        ))
        .cross_axis_alignment(CrossAxisAlignment::Center)
        .gap(px(theme.gap)),
    )
}

// ── Save tab strip (one tab per open save) ────────────────────────────

fn save_tab_strip(theme: Theme, state: &AppState) -> impl WidgetView<AppState> + use<> {
    let active = state.active_tab;
    let tabs: Vec<View> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            xc::tab_button(
                theme,
                t.save_name.clone(),
                i == active,
                move |s: &mut AppState| {
                    s.active_tab = i;
                },
            )
            .boxed()
        })
        .collect();
    xc::bar(theme, xc::h_stack(theme, tabs))
}

// ── Main area: party rail | character panel ───────────────────────────

fn main_area(theme: Theme, state: &AppState) -> View {
    flex_row((
        party_rail(theme, state),
        character_panel(theme, state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .gap(px(theme.gap))
    .boxed()
}

fn party_rail(theme: Theme, state: &AppState) -> impl WidgetView<AppState> + use<> {
    let active = state.active();
    let selected = active.selected_party_index;
    let count = active.save.party_npcs.len();

    let mut children: Vec<View> = vec![xc::title(theme, format!("Party ({count})")).boxed()];
    for (i, npc) in active.save.party_npcs.iter().enumerate() {
        let name = if npc.display_name.is_empty() {
            format!("Slot {i}")
        } else {
            npc.display_name.clone()
        };
        children.push(
            xc::tab_button(theme, name, selected == Some(i), move |s: &mut AppState| {
                s.active_mut().selected_party_index = Some(i);
            })
            .boxed(),
        );
    }

    sized_box(portal(xc::v_stack(theme, children)))
        .width(px(RAIL_WIDTH))
        .padding(px(theme.padding * 0.6))
        .background_color(theme.surface)
}

// ── Character panel: tab strip + active tab content ───────────────────

fn character_panel(theme: Theme, state: &AppState) -> View {
    let active = state.active();
    let Some(idx) = active.selected_party_index else {
        return centered_message(theme, "Select a party member on the left.");
    };
    let Some(npc) = active.save.party_npcs.get(idx) else {
        return centered_message(theme, "Stale selection — party member not found.");
    };
    let cre: &Cre = match npc.cre.as_ref() {
        Some(NpcCre::Cre(boxed)) => boxed.as_ref(),
        Some(NpcCre::Ref(name)) => {
            return centered_message(theme, format!("External CRE '{name}' — not embedded."));
        }
        None => return centered_message(theme, "Empty party slot."),
    };
    let gam = &active.save;
    let selected_tab = active.selected_tab;

    // Tab strip — one button per character tab.
    let strip: Vec<View> = CharacterTab::ALL
        .iter()
        .copied()
        .map(|t| {
            xc::tab_button(
                theme,
                t.label(),
                t == selected_tab,
                move |s: &mut AppState| {
                    s.active_mut().selected_tab = t;
                },
            )
            .boxed()
        })
        .collect();

    let content: View = if selected_tab == CharacterTab::Abilities {
        abilities_view(theme, cre, gam, &state.editors).boxed()
    } else {
        centered_message(
            theme,
            format!("{} — not implemented yet.", selected_tab.label()),
        )
    };

    flex_col((
        xc::bar(theme, portal(xc::h_stack(theme, strip))),
        portal(content).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .boxed()
}

fn centered_message(theme: Theme, msg: impl Into<String>) -> View {
    sized_box(xc::muted::<AppState, ()>(theme, msg))
        .padding(px(theme.padding))
        .boxed()
}

// ── Abilities tab (read-only, 3-column cards) ─────────────────────────

fn abilities_view(
    theme: Theme,
    cre: &Cre,
    gam: &ImportedGam,
    editors: &HashMap<EditableField, String>,
) -> impl WidgetView<AppState> + use<> {
    // Column 0: ability scores (+ Total) and combat & status.
    let mut ability_rows = section_rows(theme, cre, gam, editors, Section::AbilityScores);
    let total = u32::from(cre.strength())
        + u32::from(cre.dexterity())
        + u32::from(cre.constitution())
        + u32::from(cre.intelligence())
        + u32::from(cre.wisdom())
        + u32::from(cre.charisma());
    ability_rows.push(xc::value_row(theme, "Total", total.to_string()).boxed());

    let col0 = xc::v_stack(
        theme,
        vec![
            xc::card(theme, "Ability scores", ability_rows).boxed(),
            xc::card(
                theme,
                "Combat & status",
                section_rows(theme, cre, gam, editors, Section::CombatStatus),
            )
            .boxed(),
        ],
    );

    // Column 1: experience & levels, morale.
    let col1 = xc::v_stack(
        theme,
        vec![
            xc::card(
                theme,
                "Experience & levels",
                section_rows(theme, cre, gam, editors, Section::ExperienceLevels),
            )
            .boxed(),
            xc::card(
                theme,
                "Morale",
                or_placeholder(
                    theme,
                    section_rows(theme, cre, gam, editors, Section::Morale),
                    "disabled (d20)",
                ),
            )
            .boxed(),
        ],
    );

    // Column 2: thief skills.
    let col2 = xc::v_stack(
        theme,
        vec![
            xc::card(
                theme,
                "Thief Skills",
                or_placeholder(
                    theme,
                    section_rows(theme, cre, gam, editors, Section::ThiefSkills),
                    "d20 skills — not shown in this build",
                ),
            )
            .boxed(),
        ],
    );

    flex_row((col0.flex(1.0), col1.flex(1.0), col2.flex(1.0)))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(px(theme.gap * 2.0))
}

/// Every visible editable row of a section.
fn section_rows(
    theme: Theme,
    cre: &Cre,
    gam: &ImportedGam,
    editors: &HashMap<EditableField, String>,
    section: Section,
) -> Vec<View> {
    EditableField::ALL
        .iter()
        .copied()
        .filter(|f| f.section() == section && f.is_visible(cre))
        .map(|f| {
            if f == EditableField::Attacks {
                attacks_row(theme, cre)
            } else {
                editable_row(theme, f, cre, gam, editors)
            }
        })
        .collect()
}

/// The Attacks row uses a combo box (the value is one of a few documented
/// bytes shown as a per-round label like "1.5"), not a free text field.
fn attacks_row(theme: Theme, cre: &Cre) -> View {
    let selected =
        AttacksOption::index_for_byte(crate::cre_fields::attacks_byte(cre)).unwrap_or(0);
    flex_row((
        xc::muted::<AppState, ()>(theme, "Attacks".to_string()),
        FlexSpacer::Flex(1.0),
        sized_box(xc::select(
            theme,
            AttacksOption::labels(),
            selected,
            |s: &mut AppState, idx| commit_attacks(s, idx),
        ))
        .width(px(INPUT_WIDTH)),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .boxed()
}

/// Write the picked attacks option's byte to the active CRE.
fn commit_attacks(state: &mut AppState, idx: usize) {
    let Some(byte) = AttacksOption::byte_for_index(idx) else {
        return;
    };
    let active = state.active_mut();
    if let Some(i) = active.selected_party_index
        && let Some(npc) = active.save.party_npcs.get_mut(i)
        && let Some(NpcCre::Cre(boxed)) = npc.cre.as_mut()
    {
        crate::cre_fields::set_attacks_byte(boxed, byte);
    }
}

/// One editable `label : input` row. The input shows the in-flight
/// buffer; each keystroke updates the buffer, Enter commits (parse +
/// clamp + write-back to the CRE/GAM, then the buffer is refreshed to the
/// clamped value).
fn editable_row(
    theme: Theme,
    field: EditableField,
    cre: &Cre,
    gam: &ImportedGam,
    editors: &HashMap<EditableField, String>,
) -> View {
    let label = field.label(cre).to_string();
    let buf = editors
        .get(&field)
        .cloned()
        .unwrap_or_else(|| field.read_text(cre, gam));
    flex_row((
        xc::muted::<AppState, ()>(theme, label),
        FlexSpacer::Flex(1.0),
        sized_box(
            text_input(buf, move |s: &mut AppState, new: String| {
                edit_field(s, field, new);
            })
            .on_enter(move |s: &mut AppState, new: String| {
                commit_field(s, field, &new);
            })
            .text_size(theme.font_size),
        )
        .width(px(INPUT_WIDTH)),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Center)
    .boxed()
}

/// Return `rows` unless empty, in which case a single muted placeholder
/// row (keeps empty cards from looking broken).
fn or_placeholder(theme: Theme, rows: Vec<View>, placeholder: &str) -> Vec<View> {
    if rows.is_empty() {
        vec![xc::muted::<AppState, ()>(theme, placeholder.to_string()).boxed()]
    } else {
        rows
    }
}
