use std::collections::{BTreeMap, BTreeSet};

use iced::widget::image::Handle as ImageHandle;
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, vertical_rule, Column, Image,
};
use iced::{ContentFit, Element, Length, Task};

use infinitier_core::fs::Importer;
use infinitier_core::game::{DataOrigin, GameData, GameResource, ResourceId};
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::key::ResourceType;

type Groups = BTreeMap<String, Vec<(String, ResourceId)>>;

struct Explorer {
    game_data: GameData,
    groups: Groups,
    expanded: BTreeSet<String>,
    selected: Option<ResourceId>,
    bmp_cache: Option<(ResourceId, Result<ImageHandle, String>)>,
}

#[derive(Debug, Clone)]
enum Message {
    ToggleGroup(String),
    SelectResource(ResourceId),
}

impl Explorer {
    fn new(game_data: GameData) -> (Self, Task<Message>) {
        let groups = build_groups(&game_data);
        (
            Self {
                game_data,
                groups,
                expanded: BTreeSet::new(),
                selected: None,
                bmp_cache: None,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ToggleGroup(ext) => {
                if self.expanded.contains(&ext) {
                    self.expanded.remove(&ext);
                } else {
                    self.expanded.insert(ext);
                }
            }
            Message::SelectResource(id) => {
                self.selected = Some(id);
                if let Some(resource) = self.game_data.get_by_id(id) {
                    if resource.r#type == ResourceType::Bmp {
                        if self.bmp_cache.as_ref().map(|(cached_id, _)| *cached_id) != Some(id) {
                            let result = load_bmp(resource);
                            self.bmp_cache = Some((id, result));
                        }
                    }
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let main_row = row![
            self.left_panel(),
            vertical_rule(1),
            self.central_panel(),
        ]
        .height(Length::Fill);

        column![main_row, horizontal_rule(1), self.bottom_panel()].into()
    }

    fn left_panel(&self) -> Element<'_, Message> {
        let mut col: Column<Message> = column![].spacing(0);

        for (ext, entries) in &self.groups {
            let is_open = self.expanded.contains(ext);
            let arrow = if is_open { "▼" } else { "▶" };
            let header_label = format!("{} {} ({})", arrow, ext, entries.len());

            let header_btn = button(text(header_label))
                .on_press(Message::ToggleGroup(ext.clone()))
                .width(Length::Fill);
            col = col.push(header_btn);

            if is_open {
                for (label, resource_id) in entries {
                    let id = *resource_id;
                    let item_text = format!("  {}", label);
                    let mut item_btn = button(text(item_text))
                        .on_press(Message::SelectResource(id))
                        .width(Length::Fill);
                    if self.selected == Some(id) {
                        item_btn = item_btn.style(button::primary);
                    } else {
                        item_btn = item_btn.style(button::secondary);
                    }
                    col = col.push(item_btn);
                }
            }
        }

        container(scrollable(col))
            .width(Length::Fixed(280.0))
            .height(Length::Fill)
            .into()
    }

    fn central_panel(&self) -> Element<'_, Message> {
        let content: Element<Message> = match self.selected {
            None => container(text("Select a resource from the panel on the left."))
                .center(Length::Fill)
                .into(),
            Some(resource_id) => match self.game_data.get_by_id(resource_id) {
                Some(resource) => self.resource_view(resource_id, resource),
                None => container(text("Resource not found."))
                    .center(Length::Fill)
                    .into(),
            },
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn resource_view(&self, resource_id: ResourceId, resource: &GameResource) -> Element<'_, Message> {
        match resource.r#type {
            ResourceType::Bmp => {
                if let Some((cached_id, result)) = &self.bmp_cache {
                    if *cached_id == resource_id {
                        return match result {
                            Err(msg) => container(text(format!("Error loading BMP: {msg}")))
                                .center(Length::Fill)
                                .into(),
                            Ok(handle) => container(
                                Image::new(handle.clone())
                                    .content_fit(ContentFit::Contain),
                            )
                            .center(Length::Fill)
                            .into(),
                        };
                    }
                }
                container(text("Loading...")).center(Length::Fill).into()
            }
            other => {
                let label = format!("{:?} Viewer", other);
                container(text(label)).center(Length::Fill).into()
            }
        }
    }

    fn bottom_panel(&self) -> Element<'_, Message> {
        let content = self
            .selected
            .and_then(|id| self.game_data.get_by_id(id))
            .map(|resource| {
                let origin = match &resource.data_origin {
                    DataOrigin::Bif { name } => format!("BIF: {name}"),
                    DataOrigin::Override { path } => {
                        format!("Override: {}", path.display())
                    }
                    DataOrigin::Missing => "Missing".to_string(),
                };
                format!("{} — {}", resource.filename, origin)
            })
            .unwrap_or_default();

        container(text(content))
            .padding(4)
            .width(Length::Fill)
            .into()
    }
}

fn load_bmp(resource: &GameResource) -> Result<ImageHandle, String> {
    let ds = resource
        .datasource
        .as_ref()
        .ok_or_else(|| "no datasource available".to_string())?;
    let bmp = BmpImporter.import(ds).map_err(|e| e.to_string())?;
    let width = bmp.image.width();
    let height = bmp.image.height();
    let pixels = bmp.image.into_raw();
    Ok(ImageHandle::from_rgba(width, height, pixels))
}

fn build_groups(game_data: &GameData) -> Groups {
    let mut groups: Groups = BTreeMap::new();
    for (i, entry) in game_data.resources().iter().enumerate() {
        let ext = entry.r#type.get_extension().unwrap_or("unknown").to_string();
        let label = if matches!(entry.data_origin, DataOrigin::Override { .. }) {
            format!("{} (O)", entry.filename)
        } else {
            entry.filename.clone()
        };
        groups.entry(ext).or_default().push((label, i));
    }
    for entries in groups.values_mut() {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }
    groups
}

pub fn run(game_data: GameData) -> iced::Result {
    iced::application(
        "Infinitier Explorer (Iced)",
        Explorer::update,
        Explorer::view,
    )
    .run_with(move || Explorer::new(game_data))
}
