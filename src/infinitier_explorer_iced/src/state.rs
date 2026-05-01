use std::collections::{BTreeMap, BTreeSet};

use iced::widget::image::Handle as ImageHandle;

use infinitier_core::game::{DataOrigin, GameData, ResourceId};

pub type Groups = BTreeMap<String, Vec<(String, ResourceId)>>;

pub struct AppState {
    pub game_data: GameData,
    pub expanded: BTreeSet<String>,
    pub selected: Option<ResourceId>,
    pub bmp_cache: Option<(ResourceId, Result<ImageHandle, String>)>,
}

impl AppState {
    pub fn new(game_data: GameData) -> Self {
        Self {
            game_data,
            expanded: BTreeSet::new(),
            selected: None,
            bmp_cache: None,
        }
    }
}

pub fn build_groups(game_data: &GameData) -> Groups {
    let mut groups: Groups = BTreeMap::new();
    for (i, entry) in game_data.resources().iter().enumerate() {
        let ext = entry.r#type.get_extension().unwrap_or("unknown").to_string();
        let label = if matches!(entry.data_origin, DataOrigin::Override { .. }) {
            format!("{} (O)", entry.filename)
        } else {
            entry.filename.clone()
        };
        groups.entry(ext).or_default().push((label, i));
    }
    for entries in groups.values_mut() {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }
    groups
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleGroup(String),
    SelectResource(ResourceId),
    NavigateUp,
    NavigateDown,
    Ignored,
}
