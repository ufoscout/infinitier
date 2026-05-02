use infinitier_core::fs::Importer;
use infinitier_core::game::{DataOrigin, ResourceId};
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::key::ResourceType;
use std::sync::Arc;

use xilem::masonry::peniko::{ImageAlphaType, ImageData, ImageFormat};
use xilem::view::{
    AnyFlexChild, FlexExt, MainAxisAlignment, flex_col, flex_row, label, portal, prose, split,
    text_button,
};
use xilem::{Blob, EventLoop, WidgetView, WindowOptions, Xilem};

use crate::components::resource_viewer;
use crate::state::AppState;

fn select_resource(state: &mut AppState, id: ResourceId) {
    state.selected = Some(id);

    let is_bmp = state
        .game_data
        .get_by_id(id)
        .map(|r| r.r#type == ResourceType::Bmp)
        .unwrap_or(false);

    let already_cached = state.bmp_cache.as_ref().map(|(cid, _)| *cid) == Some(id);

    if is_bmp && !already_cached {
        let result = {
            let ds_opt = state
                .game_data
                .get_by_id(id)
                .and_then(|r| r.datasource.as_ref());

            match ds_opt {
                None => Err("no datasource available".to_string()),
                Some(ds) => BmpImporter
                    .import(ds)
                    .map_err(|e| e.to_string())
                    .map(|bmp| {
                        let w = bmp.image.width();
                        let h = bmp.image.height();
                        let pixels = bmp.image.into_raw();
                        ImageData {
                            data: Blob::new(Arc::new(pixels)),
                            format: ImageFormat::Rgba8,
                            alpha_type: ImageAlphaType::Alpha,
                            width: w,
                            height: h,
                        }
                    }),
            }
        };
        state.bmp_cache = Some((id, result));
    }
}

fn status_text(state: &AppState) -> String {
    match state.selected {
        None => "No file selected".to_string(),
        Some(id) => match state.game_data.get_by_id(id) {
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

fn app_logic(state: &mut AppState) -> impl WidgetView<AppState> + use<> {
    let mut tree_items: Vec<AnyFlexChild<AppState>> = Vec::new();

    for (ext, entries) in &state.groups {
        let is_expanded = state.expanded.contains(ext);
        let arrow = if is_expanded { "▼" } else { "▶" };
        let group_label = format!("{arrow} {ext} ({})", entries.len());
        let group_ext = ext.clone();

        tree_items.push(
            text_button(group_label, move |state: &mut AppState| {
                if state.expanded.contains(&group_ext) {
                    state.expanded.remove(&group_ext);
                } else {
                    state.expanded.insert(group_ext.clone());
                }
            })
            .into_any_flex(),
        );

        if is_expanded {
            for (res_label, resource_id) in entries {
                let resource_id = *resource_id;
                let prefix = if state.selected == Some(resource_id) {
                    "> "
                } else {
                    "  "
                };
                let item_label = format!("{prefix}{res_label}");

                tree_items.push(
                    text_button(item_label, move |state: &mut AppState| {
                        select_resource(state, resource_id);
                    })
                    .into_any_flex(),
                );
            }
        }
    }

    let tree = flex_col(tree_items).main_axis_alignment(MainAxisAlignment::Start);

    let left_panel = flex_col((
        prose("Resources"),
        portal(tree).flex(1.0),
    ))
    .must_fill_major_axis(true);

    let center_panel = portal(resource_viewer::view(state));

    let status = flex_row((label(status_text(state)),));

    flex_col((
        split(left_panel, center_panel)
            .split_point(0.28)
            .flex(1.0),
        status,
    ))
    .must_fill_major_axis(true)
}

pub fn run(state: AppState) {
    let app = Xilem::new_simple(
        state,
        app_logic,
        WindowOptions::new("Infinitier Explorer (Xilem)"),
    );
    app.run_in(EventLoop::with_user_event())
        .expect("Failed to run xilem app");
}
