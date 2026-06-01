//! The Xilem view tree — the read-only equivalent of the egui keeper's
//! `app.rs` + `ui/*` panels. `app_logic` is the root component: it reads
//! `&mut AppState` and rebuilds the whole tree each pass. Navigation
//! (active save tab, selected party slot, active character tab, theme
//! toggle) mutates `AppState` from button callbacks; the data itself is
//! displayed read-only.

use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
use infinitier_core::resource::cre::Cre;
use xilem::masonry::layout::Length;
use xilem::style::Style as _;
use xilem::view::{
    CrossAxisAlignment, FlexExt as _, FlexSpacer, MainAxisAlignment, flex_col, flex_row, portal,
    sized_box,
};
use xilem::{AnyWidgetView, WidgetView};
use xilem_components::Theme;
use xilem_components::view as xc;

use crate::fields::{EditableField, Section};
use crate::state::AppState;
use crate::tabs::CharacterTab;

/// Boxed, type-erased view over the keeper state. Action is `()` — all
/// interaction is in-place state mutation.
type View = Box<AnyWidgetView<AppState>>;

fn px(v: f32) -> Length {
    Length::px(v as f64)
}

const RAIL_WIDTH: f32 = 240.0;

/// Root component.
pub fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    let theme = if state.dark {
        Theme::dark()
    } else {
        Theme::light()
    };

    flex_col((
        header(theme),
        save_tab_strip(theme, state),
        main_area(theme, state).flex(1.0),
    ))
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .main_axis_alignment(MainAxisAlignment::Start)
}

// ── Header bar (Load / Save / theme toggle) ───────────────────────────

fn header(theme: Theme) -> impl WidgetView<AppState> + use<> {
    xc::bar(
        theme,
        flex_row((
            xc::button_primary(theme, "Load", |_: &mut AppState| {
                log::info!("[load] not implemented in the read-only Xilem port");
            }),
            xc::button_primary(theme, "Save", |_: &mut AppState| {
                log::info!("[save] read-only build — nothing to write");
            }),
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
            xc::tab_button(theme, t.save_name.clone(), i == active, move |s: &mut AppState| {
                s.active_tab = i;
            })
            .boxed()
        })
        .collect();
    xc::bar(theme, xc::h_stack(theme, tabs))
}

// ── Main area: party rail | character panel ───────────────────────────

fn main_area(theme: Theme, state: &AppState) -> View {
    flex_row((party_rail(theme, state), character_panel(theme, state).flex(1.0)))
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
            xc::tab_button(theme, t.label(), t == selected_tab, move |s: &mut AppState| {
                s.active_mut().selected_tab = t;
            })
            .boxed()
        })
        .collect();

    let content: View = if selected_tab == CharacterTab::Abilities {
        abilities_view(theme, cre, gam).boxed()
    } else {
        centered_message(theme, format!("{} — not implemented yet.", selected_tab.label()))
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

fn abilities_view(theme: Theme, cre: &Cre, gam: &ImportedGam) -> impl WidgetView<AppState> + use<> {
    // Column 0: ability scores (+ Total) and combat & status.
    let mut ability_rows = section_rows(theme, cre, gam, Section::AbilityScores);
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
                section_rows(theme, cre, gam, Section::CombatStatus),
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
                section_rows(theme, cre, gam, Section::ExperienceLevels),
            )
            .boxed(),
            xc::card(theme, "Morale", or_placeholder(theme, section_rows(theme, cre, gam, Section::Morale), "disabled (d20)")).boxed(),
        ],
    );

    // Column 2: thief skills.
    let col2 = xc::v_stack(
        theme,
        vec![
            xc::card(
                theme,
                "Thief Skills",
                or_placeholder(theme, section_rows(theme, cre, gam, Section::ThiefSkills), "d20 skills — not shown in this build"),
            )
            .boxed(),
        ],
    );

    flex_row((col0.flex(1.0), col1.flex(1.0), col2.flex(1.0)))
        .cross_axis_alignment(CrossAxisAlignment::Start)
        .gap(px(theme.gap * 2.0))
}

/// All visible rows of a section, as `label : value` rows.
fn section_rows(theme: Theme, cre: &Cre, gam: &ImportedGam, section: Section) -> Vec<View> {
    EditableField::ALL
        .iter()
        .copied()
        .filter(|f| f.section() == section && f.is_visible(cre))
        .map(|f| xc::value_row(theme, f.label(cre), f.read_text(cre, gam)).boxed())
        .collect()
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
