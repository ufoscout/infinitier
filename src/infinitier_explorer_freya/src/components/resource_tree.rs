use freya::prelude::*;
use image::ImageFormat;
use infinitier_core::{fs::Importer, resource::bmp::BmpImporter, resource::key::ResourceType};

use crate::state::AppState;

#[derive(PartialEq)]
pub struct ResourceTree;

impl Component for ResourceTree {
    fn render(&self) -> impl IntoElement {
        let mut state = use_consume::<State<AppState>>();

        let groups: Vec<(String, Vec<(String, usize)>)> = state
            .read()
            .groups
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        let rows: Vec<Element> = groups
            .into_iter()
            .map(|(ext, entries)| {
                let is_open = state.read().expanded.contains(&ext);
                let arrow = if is_open { "▼" } else { "▶" };
                let header_text = format!("{} {} ({})", arrow, ext, entries.len());
                let ext_for_click = ext.clone();

                let mut group_rect = rect()
                    .width(Size::fill())
                    .direction(Direction::Vertical)
                    .child(
                        rect()
                            .width(Size::fill())
                            .padding(Gaps::new(2., 4., 2., 4.))
                            .background((232u8, 232u8, 232u8))
                            .on_press(move |_| {
                                let mut s = state.write();
                                if s.expanded.contains(&ext_for_click) {
                                    s.expanded.remove(&ext_for_click);
                                } else {
                                    s.expanded.insert(ext_for_click.clone());
                                }
                            })
                            .child(label().text(header_text).font_size(13.)),
                    );

                if is_open {
                    let leaf_elements: Vec<Element> = entries
                        .into_iter()
                        .map(|(label_text, resource_id)| {
                            let is_selected = state.read().selected == Some(resource_id);
                            let bg = if is_selected {
                                (74u8, 144u8, 217u8)
                            } else {
                                (255u8, 255u8, 255u8)
                            };
                            let text_color = if is_selected {
                                (255u8, 255u8, 255u8)
                            } else {
                                (0u8, 0u8, 0u8)
                            };
                            rect()
                                .width(Size::fill())
                                .padding(Gaps::new(2., 4., 2., 16.))
                                .background(bg)
                                .on_press(move |_| {
                                    let mut s = state.write();
                                    s.selected = Some(resource_id);
                                    let resource_type = s
                                        .game_data
                                        .get_by_id(resource_id)
                                        .map(|r| r.r#type);
                                    if resource_type == Some(ResourceType::Bmp) {
                                        let already_cached = s
                                            .bmp_cache
                                            .as_ref()
                                            .map(|(cid, _)| *cid)
                                            == Some(resource_id);
                                        if !already_cached {
                                            let datasource = s
                                                .game_data
                                                .get_by_id(resource_id)
                                                .and_then(|r| r.datasource.clone());
                                            let result = datasource
                                                .ok_or_else(|| "no datasource".to_string())
                                                .and_then(|ds| {
                                                    BmpImporter
                                                        .import(&ds)
                                                        .map_err(|e| e.to_string())
                                                })
                                                .and_then(|bmp| {
                                                    let mut png_bytes: Vec<u8> = Vec::new();
                                                    bmp.image
                                                        .write_to(
                                                            &mut std::io::Cursor::new(
                                                                &mut png_bytes,
                                                            ),
                                                            ImageFormat::Png,
                                                        )
                                                        .map_err(|e| e.to_string())?;
                                                    Ok(Bytes::from(png_bytes))
                                                });
                                            s.bmp_cache = Some((resource_id, result));
                                        }
                                    }
                                })
                                .child(label().text(label_text).font_size(12.).color(text_color))
                                .into()
                        })
                        .collect();
                    group_rect = group_rect.children(leaf_elements);
                }

                group_rect.into()
            })
            .collect();

        ScrollView::new()
            .width(Size::fill())
            .height(Size::fill())
            .child(
                rect()
                    .width(Size::fill())
                    .direction(Direction::Vertical)
                    .children(rows),
            )
    }
}
