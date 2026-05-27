//! Shared helpers used across multiple viewers.

use bytesize::ByteSize;
use infinitier_core::game::{DataOrigin, GameResource};

/// Format a resource's `data_origin` for the bottom info bar — same
/// three-arm match the egui viewers use.
pub fn origin_text(resource: &GameResource) -> String {
    match &resource.data_origin {
        DataOrigin::Bif { name } => format!("BIF: {name}"),
        DataOrigin::Dir { name, path } => format!("{name}: {}", path.path().display()),
        DataOrigin::Missing => "Missing".to_string(),
    }
}

/// Format a resource's `file_size` or fall back to `"? B"`.
pub fn file_size_text(resource: &GameResource) -> String {
    match resource.file_size {
        Some(s) => ByteSize(s).to_string(),
        None => "? B".to_string(),
    }
}
