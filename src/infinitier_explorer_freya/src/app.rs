use freya::prelude::*;

use crate::state::{AppState, INIT_GAME_DATA};
use crate::ui::{bottom_panel::BottomPanel, central_panel::CentralPanel, left_panel::LeftPanel};

pub fn app() -> impl IntoElement {
    let state = use_state(|| {
        let game_data = INIT_GAME_DATA
            .get()
            .expect("GameData not initialized")
            .lock()
            .unwrap()
            .take()
            .expect("GameData already taken");
        AppState::new(game_data)
    });
    use_provide_context(|| state);

    rect()
        .expanded()
        .direction(Direction::Vertical)
        .background((255u8, 255u8, 255u8))
        .child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .direction(Direction::Horizontal)
                .child(LeftPanel)
                .child(CentralPanel),
        )
        .child(BottomPanel)
}
