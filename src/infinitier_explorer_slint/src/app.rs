use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use infinitier_core::fs::Importer;
use infinitier_core::game::{GameData, ResourceId};
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::key::ResourceType;

use crate::MainWindow;
use crate::TreeItem;
use crate::components::resource_viewer::{ResourceViewer, ViewerData};
use crate::state::{AppState, Groups, build_groups};

pub fn run(game_data: GameData) -> Result<(), slint::PlatformError> {
    let groups = build_groups(&game_data);
    let state = Rc::new(RefCell::new(AppState::new(game_data, groups)));

    let window = MainWindow::new()?;

    {
        let st = state.borrow();
        let items = build_flat_items(&st.groups, &st.expanded);
        window.set_tree_items(ModelRc::new(VecModel::from(items)));
    }

    window.on_item_clicked({
        let window_weak = window.as_weak();
        let state = state.clone();
        move |idx| {
            let Some(window) = window_weak.upgrade() else {
                return;
            };

            let (is_group, group_key, resource_index) = {
                let model = window.get_tree_items();
                let item = model.row_data(idx as usize).unwrap();
                (item.is_group, item.group_key.to_string(), item.resource_index as usize)
            };

            if is_group {
                let new_items = {
                    let mut st = state.borrow_mut();
                    if st.expanded.contains(&group_key) {
                        st.expanded.remove(&group_key);
                    } else {
                        st.expanded.insert(group_key);
                    }
                    build_flat_items(&st.groups, &st.expanded)
                };
                window.set_tree_items(ModelRc::new(VecModel::from(new_items)));
            } else {
                {
                    let mut st = state.borrow_mut();
                    st.selected = Some(resource_index);
                    load_bmp_if_needed(&mut st, resource_index);
                }
                {
                    let st = state.borrow();
                    update_viewer(&window, &st);
                    update_info(&window, &st);
                    window.set_selected_resource(resource_index as i32);
                }
            }
        }
    });

    window.run()
}

fn build_flat_items(groups: &Groups, expanded: &BTreeSet<String>) -> Vec<TreeItem> {
    println!("building flat items");
    let mut items = Vec::new();
    for (ext, entries) in groups {
        let is_expanded = expanded.contains(ext);
        items.push(TreeItem {
            label: format!("{} ({})", ext, entries.len()).into(),
            is_group: true,
            is_expanded,
            resource_index: -1,
            group_key: ext.clone().into(),
        });
        if is_expanded {
            for (label, resource_id) in entries {
                items.push(TreeItem {
                    label: label.clone().into(),
                    is_group: false,
                    is_expanded: false,
                    resource_index: *resource_id as i32,
                    group_key: "".into(),
                });
            }
        }
    }
    items
}

fn load_bmp_if_needed(state: &mut AppState, resource_id: ResourceId) {
    if state.bmp_cache.as_ref().map(|(id, _)| *id) == Some(resource_id) {
        return;
    }

    let result: Option<Result<slint::Image, String>> = {
        let resource = state.game_data.get_by_id(resource_id);
        match resource {
            Some(r) if r.r#type == ResourceType::Bmp => Some(
                r.datasource
                    .as_ref()
                    .ok_or_else(|| "no datasource available".to_string())
                    .and_then(|ds| BmpImporter.import(ds).map_err(|e| e.to_string()))
                    .map(|bmp| {
                        let w = bmp.image.width();
                        let h = bmp.image.height();
                        let mut buffer =
                            slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(w, h);
                        buffer.make_mut_bytes().copy_from_slice(bmp.image.as_raw());
                        slint::Image::from_rgba8(buffer)
                    }),
            ),
            _ => None,
        }
    };

    if let Some(result) = result {
        state.bmp_cache = Some((resource_id, result));
    }
}

fn update_viewer(window: &MainWindow, state: &AppState) {
    let Some(resource_id) = state.selected else {
        window.set_show_image(false);
        window.set_viewer_text("Select a resource from the panel on the left.".into());
        return;
    };

    match ResourceViewer::get_data(state, resource_id) {
        ViewerData::Text(text) => {
            window.set_show_image(false);
            window.set_viewer_text(text);
        }
        ViewerData::Image(image) => {
            window.set_preview_image(image);
            window.set_show_image(true);
        }
    }
}

fn update_info(window: &MainWindow, state: &AppState) {
    let text = match state.selected {
        None => "No file selected".to_string(),
        Some(resource_id) => match state.game_data.get_by_id(resource_id) {
            Some(resource) => {
                format!("Resource: {} - Source: {:?}", resource.name, resource.data_origin)
            }
            None => "Resource not found".to_string(),
        },
    };
    window.set_info_text(text.into());
}
