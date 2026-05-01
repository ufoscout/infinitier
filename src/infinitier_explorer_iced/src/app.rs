use iced::widget::{column, horizontal_rule, row, vertical_rule};
use iced::{Element, Length, Task};

use infinitier_core::fs::Importer;
use infinitier_core::game::GameData;
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::key::ResourceType;

use crate::components::resource_tree::ResourceTree;
use crate::components::resource_viewer::ResourceViewer;
use crate::state::{AppState, Message};
use crate::ui;

struct Explorer {
    state: AppState,
    resource_tree: ResourceTree,
    resource_viewer: ResourceViewer,
}

impl Explorer {
    fn new(game_data: GameData) -> (Self, Task<Message>) {
        let resource_tree = ResourceTree::new(&game_data);
        (
            Self {
                state: AppState::new(game_data),
                resource_tree,
                resource_viewer: ResourceViewer,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ToggleGroup(ext) => {
                if self.state.expanded.contains(&ext) {
                    self.state.expanded.remove(&ext);
                } else {
                    self.state.expanded.insert(ext);
                }
            }
            Message::SelectResource(id) => {
                self.state.selected = Some(id);
                if let Some(resource) = self.state.game_data.get_by_id(id) {
                    if resource.r#type == ResourceType::Bmp {
                        let already_cached = self
                            .state
                            .bmp_cache
                            .as_ref()
                            .map(|(cached_id, _)| *cached_id)
                            == Some(id);
                        if !already_cached {
                            let result = resource
                                .datasource
                                .as_ref()
                                .ok_or_else(|| "no datasource available".to_string())
                                .and_then(|ds| {
                                    BmpImporter.import(ds).map_err(|e| e.to_string())
                                })
                                .map(|bmp| {
                                    let w = bmp.image.width();
                                    let h = bmp.image.height();
                                    let pixels = bmp.image.into_raw();
                                    iced::widget::image::Handle::from_rgba(w, h, pixels)
                                });
                            self.state.bmp_cache = Some((id, result));
                        }
                    }
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let main_row = row![
            ui::left_panel::view(&self.state, &self.resource_tree),
            vertical_rule(1),
            ui::central_panel::view(&self.state, &self.resource_viewer),
        ]
        .height(Length::Fill);

        column![main_row, horizontal_rule(1), ui::bottom_panel::view(&self.state)].into()
    }
}

pub fn run(game_data: GameData) -> iced::Result {
    iced::application(
        "Infinitier Explorer (Iced)",
        Explorer::update,
        Explorer::view,
    )
    .run_with(move || Explorer::new(game_data))
}
