use iced::widget::{column, row, rule};
use iced::{Element, Length, Subscription, Task};

use infinitier_core::fs::Importer;
use infinitier_core::game::{GameData, ResourceId};
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
                self.select_resource(id);
            }
            Message::NavigateUp => {
                let visible = self.resource_tree.visible_items();
                if let Some(id) = navigate(&visible, self.state.selected, Direction::Up) {
                    self.select_resource(id);
                }
            }
            Message::NavigateDown => {
                let visible = self.resource_tree.visible_items();
                if let Some(id) = navigate(&visible, self.state.selected, Direction::Down) {
                    self.select_resource(id);
                }
            }
            Message::Ignored => {}
        }
        Task::none()
    }

    fn select_resource(&mut self, id: ResourceId) {
        self.state.selected = Some(id);
        if let Some(resource) = self.state.game_data.get_by_id(id) {
            if resource.r#type == ResourceType::Bmp {
                let already_cached =
                    self.state.bmp_cache.as_ref().map(|(cid, _)| *cid) == Some(id);
                if !already_cached {
                    let result = resource
                        .datasource
                        .as_ref()
                        .ok_or_else(|| "no datasource available".to_string())
                        .and_then(|ds| BmpImporter.import(ds).map_err(|e| e.to_string()))
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

    fn view(&self) -> Element<'_, Message> {
        let main_row = row![
            ui::left_panel::view(&self.state, &self.resource_tree),
            rule::vertical(1),
            ui::central_panel::view(&self.state, &self.resource_viewer),
        ]
        .height(Length::Fill);

        column![main_row, rule::horizontal(1), ui::bottom_panel::view(&self.state)].into()
    }

    fn subscription(_state: &Explorer) -> Subscription<Message> {
        use iced::keyboard::{Event, Key, key::Named};
        iced::keyboard::listen().map(|event| match event {
            Event::KeyPressed { key: Key::Named(Named::ArrowUp), .. } => Message::NavigateUp,
            Event::KeyPressed { key: Key::Named(Named::ArrowDown), .. } => Message::NavigateDown,
            _ => Message::Ignored,
        })
    }
}

enum Direction {
    Up,
    Down,
}

fn navigate(visible: &[ResourceId], selected: Option<ResourceId>, dir: Direction) -> Option<ResourceId> {
    if visible.is_empty() {
        return None;
    }
    let pos = selected.and_then(|id| visible.iter().position(|&v| v == id));
    let next = match dir {
        Direction::Up => match pos {
            None | Some(0) => 0,
            Some(p) => p - 1,
        },
        Direction::Down => match pos {
            None => 0,
            Some(p) => (p + 1).min(visible.len() - 1),
        },
    };
    Some(visible[next])
}

pub fn run(game_data: GameData) -> iced::Result {
    let game_data = std::sync::Mutex::new(Some(game_data));
    iced::application(
        move || {
            let data = game_data
                .lock()
                .unwrap()
                .take()
                .expect("boot called more than once");
            Explorer::new(data)
        },
        Explorer::update,
        Explorer::view,
    )
    .subscription(Explorer::subscription)
    .title("Infinitier Explorer (Iced)")
    // .theme(|_| iced::Theme::Light)
    .run()
}
