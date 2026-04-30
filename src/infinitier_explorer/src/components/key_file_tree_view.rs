use std::collections::BTreeMap;

use eframe::egui;
use egui_ltreeview::{NodeBuilder, TreeView};
use infinitier_key_importer::Key;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    TypeGroup(u16),
    Resource(usize),
}

type Groups = Vec<(&'static str, u16, Vec<(usize, String)>)>;
type GroupsMap = BTreeMap<&'static str, (u16, Vec<(usize, String)>)>;

pub struct KeyFileTreeView {
    groups: Groups,
}

impl KeyFileTreeView {
    pub fn new(key: &Key) -> Self {
        let mut type_map: GroupsMap = BTreeMap::new();
        for (i, entry) in key.resource_entries.iter().enumerate() {
            let type_code = entry.r#type.to_u16();
            let ext = entry.r#type.get_extension().unwrap_or("unknown");
            let leaf_label = format!("{}.{}", entry.resource_name, ext);
            type_map
                .entry(ext)
                .or_insert_with(|| (type_code, Vec::new()))
                .1
                .push((i, leaf_label));
        }

        let groups = type_map
            .into_iter()
            .map(|(type_label, (code, mut entries))| {
                entries.sort_by(|a, b| a.1.cmp(&b.1));
                (type_label, code, entries)
            })
            .collect();

        Self { groups }
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        TreeView::new(ui.id().with("key_file_tree_view"))
            .allow_drag_and_drop(false)
            .show(ui, |builder| {
                for (type_label, type_code, entries) in &self.groups {
                    let dir_label = format!("{} ({})", type_label, entries.len());
                    let is_open = builder.node(
                        NodeBuilder::dir(TreeNodeId::TypeGroup(*type_code))
                            .default_open(false)
                            .label(dir_label),
                    );
                    if is_open {
                        for (idx, leaf_label) in entries {
                            builder.leaf(TreeNodeId::Resource(*idx), leaf_label.as_str());
                        }
                    }
                    builder.close_dir();
                }
            });
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

        // Groups are keyed by extension and sorted alphabetically (BTreeMap order):
        // "bam" < "bmp" < "unknown" < "wed"
        assert_eq!(view.groups.len(), 4);

        // ── bam ──────────────────────────────────────────────────────────────
        let (label, code, entries) = &view.groups[0];
        assert_eq!(*label, "bam");
        assert_eq!(*code, ResourceType::Bam.to_u16());
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], (4, "1CHELM.bam".to_string()));
        assert_eq!(entries[1], (3, "SPHEART.bam".to_string()));

        // ── bmp ──────────────────────────────────────────────────────────────
        let (label, code, entries) = &view.groups[1];
        assert_eq!(*label, "bmp");
        assert_eq!(*code, ResourceType::Bmp.to_u16());
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], (2, "AAATEST.bmp".to_string()));
        assert_eq!(entries[1], (0, "CCHAN05.bmp".to_string()));
        assert_eq!(entries[2], (1, "MINSCM.bmp".to_string()));

        // ── unknown ──────────────────────────────────────────────────────────
        let (label, code, entries) = &view.groups[2];
        assert_eq!(*label, "unknown");
        assert_eq!(*code, 0x9999);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], (6, "UNKNOWN.unknown".to_string()));

        // ── wed ──────────────────────────────────────────────────────────────
        let (label, code, entries) = &view.groups[3];
        assert_eq!(*label, "wed");
        assert_eq!(*code, ResourceType::Wed.to_u16());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], (5, "AR0072.wed".to_string()));
    }
}
