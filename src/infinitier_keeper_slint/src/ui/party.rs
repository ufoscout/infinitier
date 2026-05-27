//! Builds the left-rail party list and pushes it onto the Slint
//! window's `party` ModelRc. Run once at startup; the selection
//! state lives on the Slint side via `selected-party`.

use infinitier_core::imported_resource::gam::NpcCre;

use crate::state::AppState;
use crate::{MainWindow, PartyMember};

pub fn populate(window: &MainWindow, state: &AppState) {
    let party: Vec<PartyMember> = state
        .imported_gam
        .party_npcs
        .iter()
        .map(|n| PartyMember {
            display: n.display_name.clone().into(),
            has_cre: matches!(n.cre, Some(NpcCre::Cre(_))),
        })
        .collect();
    window.set_party(slint::ModelRc::new(slint::VecModel::from(party)));
}
