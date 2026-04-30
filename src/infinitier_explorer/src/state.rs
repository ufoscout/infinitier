use infinitier_key_importer::{Key, ResourceEntry};

pub struct AppState {
    pub key_file: Key,
    pub selected_resource: Option<ResourceEntry>,
}

impl AppState {
    pub fn new(key: Key) -> Self {
        Self {
            key_file: key,
            selected_resource: None,
        }
    }
}
