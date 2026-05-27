//! Character-panel orchestration. Owns the tab strip install +
//! the per-(party_idx, tab_idx) refresh that dispatches into
//! `ui::tabs::*`.

use infinitier_core::imported_resource::gam::NpcCre;

use crate::state::AppState;
use crate::ui::tabs;
use crate::{MainWindow, TabLabel};

/// Install the static tab labels into the window. Run once at
/// startup; tab content is refreshed on each `tab-clicked` callback.
pub fn install_tab_strip(window: &MainWindow) {
    let tabs: Vec<TabLabel> = tabs::CharacterTab::ALL
        .iter()
        .map(|t| TabLabel {
            name: t.label().into(),
        })
        .collect();
    window.set_tabs(slint::ModelRc::new(slint::VecModel::from(tabs)));
}

/// Recompute every body property for the (party_idx, tab_idx) pair.
/// Called from the party-clicked and tab-clicked callbacks.
pub fn refresh(window: &MainWindow, state: &AppState, party_idx: i32, tab_idx: i32) {
    let tab = tabs::CharacterTab::ALL
        .get(usize::try_from(tab_idx).unwrap_or(0))
        .copied()
        .unwrap_or(tabs::CharacterTab::Abilities);
    window.set_active_tab_name(tab.label().into());

    let Some(member) = (usize::try_from(party_idx).ok())
        .and_then(|i| state.imported_gam.party_npcs.get(i))
    else {
        window.set_character_title("No party member selected".into());
        window.set_body_message("Pick a party member on the left.".into());
        tabs::clear_abilities(window);
        return;
    };

    window.set_character_title(
        format!("{}. {}", member.index + 1, member.display_name).into(),
    );

    match &member.cre {
        Some(NpcCre::Cre(cre)) => {
            tabs::dispatch(window, tab, cre, &state.imported_gam);
        }
        Some(NpcCre::Ref(resref)) => {
            tabs::clear_abilities(window);
            window.set_body_message(
                format!(
                    "External CRE '{resref}' — embedded record not present in this GAM.",
                )
                .into(),
            );
        }
        None => {
            tabs::clear_abilities(window);
            window.set_body_message(
                "Empty party slot — no creature record to edit.".into(),
            );
        }
    }
}
