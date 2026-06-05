#![doc = include_str!("../readme.md")]

use std::collections::HashMap;

mod exporter;
mod importer;

pub use exporter::TwoDAExporter;
pub use importer::TwoDAImporter;

/// Represents a 2DA file.
#[derive(Debug, Clone)]
pub struct TwoDA {
    pub headers: Vec<String>,
    pub default: String,
    pub rows: HashMap<String, Vec<String>>,
}
