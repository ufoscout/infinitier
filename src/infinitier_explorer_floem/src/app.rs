use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::rc::Rc;
use std::sync::Arc;

use floem::prelude::*;
use floem::reactive::{RwSignal, SignalGet, SignalUpdate};
use floem::window::WindowConfig;
use floem::{AnyView, Application, IntoView};

use infinitier_core::game::{DataOrigin, GameData, ResourceId};

use crate::components::resource_viewer;
use crate::state::{Groups, build_groups};

pub type BmpCache = Rc<RefCell<HashMap<ResourceId, Result<Vec<u8>, String>>>>;

#[derive(Clone)]
enum TreeItem {
    Group {
        ext: String,
        count: usize,
        is_expanded: bool,
    },
    Resource {
        label: String,
        id: ResourceId,
    },
}

impl TreeItem {
}

fn build_flat_tree(groups: &Groups, expanded: &BTreeSet<String>) -> Vec<TreeItem> {
    let mut items = Vec::new();
    for (ext, entries) in groups {
        let is_expanded = expanded.contains(ext);
        items.push(TreeItem::Group {
            ext: ext.clone(),
            count: entries.len(),
            is_expanded,
        });
        if is_expanded {
            for (lbl, id) in entries {
                items.push(TreeItem::Resource {
                    label: lbl.clone(),
                    id: *id,
                });
            }
        }
    }
    items
}

fn status_text(selected: Option<ResourceId>, game_data: &GameData) -> String {
    match selected {
        None => "No file selected".to_string(),
        Some(id) => match game_data.get_by_id(id) {
            None => "Resource not found".to_string(),
            Some(resource) => {
                let origin = match &resource.data_origin {
                    DataOrigin::Bif { name } => format!("BIF: {name}"),
                    DataOrigin::Override { path } => format!("Override: {}", path.display()),
                    DataOrigin::Missing => "Missing".to_string(),
                };
                format!("Resource: {} — Source: {}", resource.name, origin)
            }
        },
    }
}

pub fn run(game_data: GameData) {
    let groups = build_groups(&game_data);
    let game_data = Arc::new(game_data);
    let groups = Arc::new(groups);

    Application::new()
        .window(
            move |_| {
                let expanded = RwSignal::new(BTreeSet::<String>::new());
                let selected = RwSignal::new(None::<ResourceId>);
                let bmp_cache: BmpCache = Rc::new(RefCell::new(HashMap::new()));

                let gd = Arc::clone(&game_data);
                let gr = Arc::clone(&groups);

                // Left panel: collapsible tree
                let tree = {
                    let gr_tree = Arc::clone(&gr);
                    dyn_container(
                        move || (expanded.get(), selected.get()),
                        move |(exp, sel)| -> AnyView {
                            let items = build_flat_tree(&gr_tree, &exp);
                            v_stack_from_iter(items.into_iter().map(move |item| -> AnyView {
                                match item {
                                    TreeItem::Group { ext, count, is_expanded } => {
                                        let arrow = if is_expanded { "▼" } else { "▶" };
                                        let lbl = format!("{arrow} {ext} ({count})");
                                        let ext_c = ext;
                                        button(label(move || lbl.clone()))
                                            .action(move || {
                                                expanded.update(|e| {
                                                    if e.contains(&ext_c) {
                                                        e.remove(&ext_c);
                                                    } else {
                                                        e.insert(ext_c.clone());
                                                    }
                                                });
                                            })
                                            .style(|s| s.width_full())
                                            .into_any()
                                    }
                                    TreeItem::Resource { label: res_lbl, id } => {
                                        let prefix = if sel == Some(id) { "> " } else { "  " };
                                        let lbl = format!("{prefix}{res_lbl}");
                                        button(label(move || lbl.clone()))
                                            .action(move || selected.set(Some(id)))
                                            .style(|s| s.width_full())
                                            .into_any()
                                    }
                                }
                            }))
                            .style(|s| s.width_full())
                            .into_any()
                        },
                    )
                };

                let left_panel = v_stack((
                    label(|| "Resources".to_string()),
                    scroll(tree).style(|s| s.flex_grow(1.0).width_full()),
                ))
                .style(|s| s.width(280.0).height_full().flex_col());

                // Center panel: resource viewer
                let viewer = {
                    let gd_view = Arc::clone(&gd);
                    let cache_view = Rc::clone(&bmp_cache);
                    dyn_container(
                        move || selected.get(),
                        move |sel| {
                            resource_viewer::view(sel, &gd_view, &cache_view)
                        },
                    )
                    .style(|s| s.flex_grow(1.0).height_full())
                };

                // Bottom status bar
                let status_bar = {
                    let gd_status = Arc::clone(&gd);
                    label(move || status_text(selected.get(), &gd_status))
                        .style(|s| s.padding(4.0).width_full())
                };

                v_stack((
                    h_stack((left_panel, viewer))
                        .style(|s| s.flex_grow(1.0).width_full()),
                    status_bar,
                ))
                .style(|s| s.width_full().height_full().flex_col())
            },
            Some(
                WindowConfig::default()
                    .title("Infinitier Explorer (Floem)"),
            ),
        )
        .run()
}
