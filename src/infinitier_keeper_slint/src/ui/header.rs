//! Populates the header-strip properties on the `MainWindow` from
//! `AppState`. Set once at startup — none of these change after the
//! save is loaded.

use crate::state::AppState;
use crate::MainWindow;

pub fn populate(window: &MainWindow, state: &AppState) {
    window.set_game_label(format!("{:?}", state.game_data.game()).into());
    window.set_game_folder(
        state
            .game_data
            .fs()
            .get_roots()
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
            .into(),
    );
    window.set_save_name(state.save_name.clone().into());
    window.set_gam_version(format!("{:?}", state.imported_gam.version).into());
}
