use infinitier_core::game::{GameData, ResourceId};

pub struct AppState {
    pub game_data: GameData,
    pub selected_resource: Option<ResourceId>,
}

impl AppState {
    pub fn new(game_data: GameData) -> Self {
        Self {
            game_data,
            selected_resource: None,
        }
    }
}
