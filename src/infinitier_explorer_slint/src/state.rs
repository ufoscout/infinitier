//! Loaded explorer state. Owned by `main()`, shared with callback
//! closures via `Rc<RefCell<…>>`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use infinitier_core::game::{DataOrigin, GameData};
use infinitier_core::imported_resource::bam::ImportedBam;
use infinitier_core::resource::bam::Type as BamType;

/// One ext-grouped tree shape, precomputed at startup so click handlers
/// only have to scan a `Vec`. Matches the egui tree exactly: keyed by
/// extension, with leaves ordered alphabetically by display label.
pub type Groups = BTreeMap<&'static str, BTreeMap<String, usize>>;

/// Mutable state for the currently-shown BAM viewer. Held by the
/// dispatcher across `bam-cycle-changed`, `bam-frame-changed`, and
/// `bam-play-pause-clicked` callbacks so they don't have to re-import
/// the resource.
pub struct BamViewerState {
    pub bam: ImportedBam,
    pub bam_type: BamType,
    pub file_size_text: String,
    pub origin_text: String,
    pub selected_cycle: usize,
    pub selected_frame: usize,
    /// `Some` while looping; carries the wall-clock anchor used to
    /// derive which frame is due. Cleared when paused/stopped.
    pub playback: Option<BamPlayback>,
}

/// Wall-clock anchor for BAM playback. The current frame is derived
/// as `(anchor_frame + elapsed / DEFAULT_FRAME_DURATION) % len`.
pub struct BamPlayback {
    pub epoch: Instant,
    pub anchor_frame: usize,
}

pub struct AppState {
    pub game_data: GameData,
    pub groups: Groups,
    /// Display order of the group headers in the flat tree.
    pub group_order: Vec<&'static str>,
    /// Whether each group is currently expanded. Mutated by clicks on
    /// the group rows.
    pub group_expanded: RefCell<Vec<bool>>,
    /// `Some` while the BAM viewer is active. `None` for any other
    /// viewer.
    pub bam_viewer: RefCell<Option<BamViewerState>>,
    /// `Some` while a Sound viewer is active. Owns the decoder + a
    /// rodio sink; spawning the decoder thread is deferred to Play.
    pub sound_viewer: RefCell<Option<crate::ui::viewers::sound::SoundViewerState>>,
    /// `Some` while a Movie viewer is active.
    pub movie_viewer: RefCell<Option<crate::ui::viewers::movie::MovieViewerState>>,
    /// `Some` while a TIS viewer is active.
    pub tis_viewer: RefCell<Option<crate::ui::viewers::tis::TisViewerState>>,
}

impl AppState {
    pub fn new(game_data: GameData) -> Self {
        let mut groups: Groups = BTreeMap::new();
        for (i, entry) in game_data.resources().iter().enumerate() {
            let ext = entry.r#type.get_extension().unwrap_or("unknown");
            let leaf_label = if matches!(entry.data_origin, DataOrigin::Dir { .. }) {
                format!("{} (O)", entry.resource_name_with_extension())
            } else {
                entry.resource_name_with_extension()
            };
            groups.entry(ext).or_default().insert(leaf_label, i);
        }
        let group_order: Vec<&'static str> = groups.keys().copied().collect();
        let group_expanded = RefCell::new(vec![false; group_order.len()]);
        Self {
            game_data,
            groups,
            group_order,
            group_expanded,
            bam_viewer: RefCell::new(None),
            sound_viewer: RefCell::new(None),
            movie_viewer: RefCell::new(None),
            tis_viewer: RefCell::new(None),
        }
    }

    pub fn into_rc(self) -> Rc<Self> {
        Rc::new(self)
    }
}
