//! Loaded application state. Owned by `main()`, borrowed by every
//! property-population function across `ui/*`.

use std::rc::Rc;

use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

/// The fully-loaded keeper state. Constructed once by `load::load`
/// and then handed off to `app::run`; closures registered on the
/// `MainWindow` callbacks hold an `Rc<AppState>` so they can re-read
/// the (immutable) ImportedGam across callback firings.
pub struct AppState {
    pub game_data: GameData,
    pub save_name: String,
    pub imported_gam: ImportedGam,
}

impl AppState {
    /// Wrap in `Rc` for sharing across callback closures. The
    /// keeper never needs interior mutability — selection state
    /// lives on the Slint window via `set_selected_party` /
    /// `set_selected_tab` rather than on the Rust side.
    pub fn into_rc(self) -> Rc<Self> {
        Rc::new(self)
    }
}
