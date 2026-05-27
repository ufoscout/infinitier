//! One Rust module per Slint panel / tab. Each module owns the
//! property-setter logic for its part of the UI — the Slint side
//! reads, never computes.

pub mod character;
pub mod header;
pub mod party;
pub mod tabs;

/// Build a Slint `ModelRc<KeyValue>` from an arbitrary `(label, value)`
/// row list. Shared by every abilities sub-section.
pub fn key_value_model(rows: Vec<(String, String)>) -> slint::ModelRc<crate::KeyValue> {
    let v: Vec<crate::KeyValue> = rows
        .into_iter()
        .map(|(l, v)| crate::KeyValue {
            label: l.into(),
            value: v.into(),
        })
        .collect();
    slint::ModelRc::new(slint::VecModel::from(v))
}
