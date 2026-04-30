use std::collections::BTreeMap;

use eframe::egui;
use egui_ltreeview::{Action, NodeBuilder, TreeView};
use infinitier_key_importer::Key;

use crate::state::AppState;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    TypeGroup(u16),
    Resource(usize),
}

/// Map from file extension to (type code, entries)
type Groups = BTreeMap<&'static str, (u16, BTreeMap<String, usize>)>;

pub struct KeyFileTreeView {
    groups: Groups,
}

impl KeyFileTreeView {
    pub fn new(key: &Key) -> Self {
        let mut groups = BTreeMap::new();
        for (i, entry) in key.resource_entries.iter().enumerate() {
            let type_code = entry.r#type.to_u16();
            let ext = entry.r#type.get_extension().unwrap_or("unknown");
            let leaf_label = format!("{}.{}", entry.resource_name, ext);
            let (_, entries) = groups
                .entry(ext)
                .or_insert_with(|| (type_code, BTreeMap::new()));
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
                for (type_label, (type_code, entries)) in &self.groups {
                    let dir_label = format!("{} ({})", type_label, entries.len());
                    let is_open = builder.node(
                        NodeBuilder::dir(TreeNodeId::TypeGroup(*type_code))
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
                        state.key_file.resource_entries.get(*idx).map(|resource| {
                            state.selected_resource = Some(resource.clone());
                        });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infinitier_key_importer::{Key, ResourceEntry, ResourceType};

    fn make_key(entries: Vec<(&str, ResourceType)>) -> Key {
        Key {
            signature: "KEY".to_string(),
            version: "V1".to_string(),
            resources_offset: 0,
            bif_offset: 0,
            bif_entries: vec![],
            resource_entries: entries
                .into_iter()
                .map(|(name, r#type)| ResourceEntry {
                    resource_name: name.to_string(),
                    r#type,
                    bif_entries_index: 0,
                    index_into_bif_file: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn test_key_file_tree_view() {
        // resource_entries indices:
        //   0: CCHAN05  Bmp
        //   1: MINSCM   Bmp
        //   2: AAATEST  Bmp
        //   3: SPHEART  Bam
        //   4: 1CHELM   Bam
        //   5: AR0072   Wed
        //   6: UNKNOWN  Unknown(0x9999)
        let key = make_key(vec![
            ("CCHAN05", ResourceType::Bmp),
            ("MINSCM", ResourceType::Bmp),
            ("AAATEST", ResourceType::Bmp),
            ("SPHEART", ResourceType::Bam),
            ("1CHELM", ResourceType::Bam),
            ("AR0072", ResourceType::Wed),
            ("UNKNOWN", ResourceType::Unknown(0x9999)),
        ]);

        let view = KeyFileTreeView::new(&key);

        // Groups are keyed by extension, sorted alphabetically: bam < bmp < unknown < wed
        assert_eq!(view.groups.len(), 4);

        // ── bam ──────────────────────────────────────────────────────────────
        let (code, entries) = view.groups.get("bam").unwrap();
        assert_eq!(*code, ResourceType::Bam.to_u16());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries["1CHELM.bam"], 4);
        assert_eq!(entries["SPHEART.bam"], 3);

        // ── bmp ──────────────────────────────────────────────────────────────
        let (code, entries) = view.groups.get("bmp").unwrap();
        assert_eq!(*code, ResourceType::Bmp.to_u16());
        assert_eq!(entries.len(), 3);
        assert_eq!(entries["AAATEST.bmp"], 2);
        assert_eq!(entries["CCHAN05.bmp"], 0);
        assert_eq!(entries["MINSCM.bmp"], 1);

        // ── unknown ──────────────────────────────────────────────────────────
        let (code, entries) = view.groups.get("unknown").unwrap();
        assert_eq!(*code, 0x9999);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries["UNKNOWN.unknown"], 6);

        // ── wed ──────────────────────────────────────────────────────────────
        let (code, entries) = view.groups.get("wed").unwrap();
        assert_eq!(*code, ResourceType::Wed.to_u16());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries["AR0072.wed"], 5);
    }
}
