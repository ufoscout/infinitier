use eframe::egui;
use egui_ltreeview::{NodeBuilder, TreeView};
use infinitier_key_importer::Key;

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum TreeNodeId {
    TypeGroup(u16),
    Resource(usize),
}

pub struct KeyFileTreeView {
    // (dir_label, type_code, [(resource_index, leaf_label)])
    groups: Vec<(&'static str, u16, Vec<(usize, String)>)>,
}

impl KeyFileTreeView {
    pub fn new(key: &Key) -> Self {
        use std::collections::BTreeMap;

        let mut type_map: BTreeMap<&'static str, (u16, Vec<(usize, String)>)> = BTreeMap::new();
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
