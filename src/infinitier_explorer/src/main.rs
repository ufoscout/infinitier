use eframe::egui;
use egui_ltreeview::TreeView;
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};
use infinitier_key_importer::{Key, KeyImporter, ResourceType};
use std::{collections::BTreeMap, path::PathBuf};

fn main() {
    let game_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!(
                "Usage: infinitier_explorer <game_folder>\n\
                 Supported games: bg, bg2, bgee, bg2ee, idw, idwee, idw2, pst, pstee"
            );
            std::process::exit(1);
        });

    let key = load_key(&game_path).unwrap_or_else(|e| {
        eprintln!("Failed to load key file from '{}': {e}", game_path.display());
        std::process::exit(1);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Infinitier Explorer")
            .with_inner_size([1024.0, 768.0]),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Infinitier Explorer",
        options,
        Box::new(move |_cc| Ok(Box::new(ExplorerApp::new(key)))),
    ) {
        eprintln!("Failed to run explorer: {e}");
    }
}

fn load_key(game_path: &std::path::Path) -> std::io::Result<Key> {
    let fs = CaseInsensitiveFS::new(game_path)?;
    let key_path = fs.get_path(&CaseInsensitivePath::new("CHITIN.KEY"))?;
    KeyImporter::import(&DataSource::new(key_path.as_path()))
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum TreeNodeId {
    TypeGroup(u16),
    Resource(usize),
}

struct ExplorerApp {
    key: Key,
    // (display_name, type_code, resource_indices_sorted_by_name)
    resource_groups: Vec<(String, u16, Vec<usize>)>,
}

impl ExplorerApp {
    fn new(key: Key) -> Self {
        let mut type_map: BTreeMap<String, (u16, Vec<usize>)> = BTreeMap::new();
        for (i, entry) in key.resource_entries.iter().enumerate() {
            let label = resource_type_label(&entry.r#type);
            let code = entry.r#type.to_u16();
            type_map
                .entry(label)
                .or_insert_with(|| (code, Vec::new()))
                .1
                .push(i);
        }

        let resource_groups = type_map
            .into_iter()
            .map(|(name, (code, mut indices))| {
                indices.sort_by_key(|&i| &key.resource_entries[i].resource_name);
                (name, code, indices)
            })
            .collect();

        Self { key, resource_groups }
    }
}

fn resource_type_label(rt: &ResourceType) -> String {
    match rt.get_extension() {
        Some(ext) => ext.to_uppercase(),
        None => format!("Unknown ({:#05x})", rt.to_u16()),
    }
}

impl eframe::App for ExplorerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::left("resource_panel")
            .resizable(true)
            .default_size(260.0)
            .show_inside(ui, |ui| {
                ui.heading("Resources");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    TreeView::new(ui.id().with("key_tree"))
                        .allow_drag_and_drop(false)
                        .show(ui, |builder| {
                            for (type_name, type_code, indices) in &self.resource_groups {
                                let dir_label =
                                    format!("{} ({})", type_name, indices.len());
                                let is_open = builder
                                    .dir(TreeNodeId::TypeGroup(*type_code), dir_label);
                                if is_open {
                                    for &idx in indices {
                                        let entry = &self.key.resource_entries[idx];
                                        let ext = entry
                                            .r#type
                                            .get_extension()
                                            .unwrap_or("???");
                                        let label = format!(
                                            "{}.{}",
                                            entry.resource_name, ext
                                        );
                                        builder.leaf(TreeNodeId::Resource(idx), label);
                                    }
                                }
                                builder.close_dir();
                            }
                        });
                });
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.label("Select a resource from the panel on the left.");
            });
        });
    }
}
