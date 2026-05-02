use std::collections::BTreeMap;

use infinitier_core::game::{DataOrigin, GameData, ResourceId};

pub type Groups = BTreeMap<String, Vec<(String, ResourceId)>>;

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
