use std::collections::BTreeMap;

use eframe::egui;
use egui_ltreeview::{Action, NodeBuilder, TreeView};
use infinitier_core::game::{DataOrigin, GameData};

use crate::state::AppState;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    TypeGroup(&'static str),
    Resource(usize),
}

/// Map from file extension to (type code, entries: label -> resource index)
type Groups = BTreeMap<&'static str, BTreeMap<String, usize>>;

pub struct KeyFileTreeView {
    groups: Groups,
}

impl KeyFileTreeView {
    pub fn new(game_data: &GameData) -> Self {
        let mut groups: Groups = BTreeMap::new();
        for (i, entry) in game_data.resources().iter().enumerate() {
            let ext = entry.r#type.get_extension().unwrap_or("unknown");
            let leaf_label = if matches!(entry.data_origin, DataOrigin::Override { .. }) {
                format!("{} (O)", entry.filename)
            } else {
                entry.filename.clone()
            };
            let entries = groups.entry(ext).or_default();
            entries.insert(leaf_label, i);
        }
        Self { groups }
    }

    /// Renders the tree and returns the label of the newly selected resource leaf, if any.
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        let (_, actions) = TreeView::new(ui.id().with("key_file_tree_view"))
            .allow_drag_and_drop(false)
            .allow_multi_selection(false)
            .show(ui, |builder| {
                for (type_label, entries) in &self.groups {
                    let dir_label = format!("{} ({})", type_label, entries.len());
                    let is_open = builder.node(
                        NodeBuilder::dir(TreeNodeId::TypeGroup(type_label))
                            .default_open(false)
                            .label(dir_label),
                    );
                    if is_open {
                        for (leaf_label, idx) in entries {
                            builder.leaf(TreeNodeId::Resource(*idx), leaf_label.as_str());
                        }
                    }
                    builder.close_dir();
                }
            });

        for action in actions {
            if let Action::SetSelected(ids) = action {
                for id in &ids {
                    if let TreeNodeId::Resource(idx) = id {
                        state.selected_resource = Some(*idx);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_core::{
        game::{DataOrigin, GameData, GameResource},
        resource::{Game, key::ResourceType},
    };

    fn make_game_data(entries: Vec<(&str, ResourceType)>) -> GameData {
        make_game_data_with_origins(
            entries
                .into_iter()
                .map(|(name, r#type)| (name, r#type, DataOrigin::Missing))
                .collect(),
        )
    }

    fn make_game_data_with_origins(entries: Vec<(&str, ResourceType, DataOrigin)>) -> GameData {
        GameData::new(
            entries
                .into_iter()
                .map(|(name, r#type, data_origin)| {
                    let ext = r#type.get_extension().unwrap_or("unknown");
                    GameResource {
                        filename: format!("{}.{}", name, ext),
                        name: name.to_string(),
                        r#type,
                        file_size: None,
                        datasource: None,
                        data_origin,
                    }
                })
                .collect(),
                Game::Bg2,
        )
    }

    #[test]
    fn test_game_data_tree_view() {
        // resources indices:
        //   0: CCHAN05  Bmp
        //   1: MINSCM   Bmp
        //   2: AAATEST  Bmp
        //   3: SPHEART  Bam
        //   4: 1CHELM   Bam
        //   5: AR0072   Wed
        //   6: UNKNOWN  Unknown(0x9999)
        let game_data = make_game_data(vec![
            ("CCHAN05", ResourceType::Bmp),
            ("MINSCM", ResourceType::Bmp),
            ("AAATEST", ResourceType::Bmp),
            ("SPHEART", ResourceType::Bam),
            ("1CHELM", ResourceType::Bam),
            ("AR0072", ResourceType::Wed),
            ("UNKNOWN", ResourceType::Unknown(0x9999)),
        ]);

        let view = KeyFileTreeView::new(&game_data);

        // Groups are keyed by extension, sorted alphabetically: bam < bmp < unknown < wed
        assert_eq!(view.groups.len(), 4);

        // ── bam ──────────────────────────────────────────────────────────────
        let entries = view.groups.get("bam").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries["1CHELM.bam"], 4);
        assert_eq!(entries["SPHEART.bam"], 3);

        // ── bmp ──────────────────────────────────────────────────────────────
        let entries = view.groups.get("bmp").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries["AAATEST.bmp"], 2);
        assert_eq!(entries["CCHAN05.bmp"], 0);
        assert_eq!(entries["MINSCM.bmp"], 1);

        // ── unknown ──────────────────────────────────────────────────────────
        let entries = view.groups.get("unknown").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries["UNKNOWN.unknown"], 6);

        // ── wed ──────────────────────────────────────────────────────────────
        let entries = view.groups.get("wed").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries["AR0072.wed"], 5);
    }

    #[test]
    fn test_override_label() {
        use std::path::PathBuf;
        let game_data = make_game_data_with_origins(vec![
            ("CCHAN05", ResourceType::Bmp, DataOrigin::Missing),
            (
                "MINSCM",
                ResourceType::Bmp,
                DataOrigin::Override {
                    path: PathBuf::from("override/MINSCM.bmp"),
                },
            ),
        ]);

        let view = KeyFileTreeView::new(&game_data);
        let entries = view.groups.get("bmp").unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries["CCHAN05.bmp"], 0);
        assert_eq!(entries["MINSCM.bmp (O)"], 1);
    }
}
