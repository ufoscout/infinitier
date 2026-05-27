//! BAM viewer. Owns the (cycle, frame) selection state on
//! `AppState::bam_viewer` and re-renders the current frame whenever
//! the user picks a different cycle or moves the frame slider.

use std::rc::Rc;
use std::time::Instant;

use infinitier_core::game::GameResource;
use infinitier_core::imported_resource::bam::ImportedBam;
use infinitier_core::resource::bam::{BamV1, Type as BamType};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::MainWindow;
use crate::state::{AppState, BamPlayback, BamViewerState};
use crate::ui::viewers::common;

pub fn populate(
    window: &MainWindow,
    state: &Rc<AppState>,
    bam: ImportedBam,
    resource: &GameResource,
) {
    let bam_state = BamViewerState {
        bam_type: bam.bam_type,
        file_size_text: common::file_size_text(resource),
        origin_text: common::origin_text(resource),
        selected_cycle: 0,
        selected_frame: 0,
        playback: None,
        bam,
    };
    *state.bam_viewer.borrow_mut() = Some(bam_state);

    window.set_viewer_kind("bam".into());
    refresh(window, state);
}

/// Re-derive every BAM-related property from `AppState::bam_viewer`.
/// Called on initial populate and from the cycle/frame callbacks.
pub fn refresh(window: &MainWindow, state: &Rc<AppState>) {
    let mut guard = state.bam_viewer.borrow_mut();
    let Some(bv) = guard.as_mut() else { return };

    // Clamp the frame to the new cycle's length when the cycle was
    // just switched.
    let frames_in_cycle = bv
        .bam
        .cycles
        .get(bv.selected_cycle)
        .map(|c| c.frame_indices.len())
        .unwrap_or(0);
    if frames_in_cycle == 0 {
        bv.selected_frame = 0;
    } else if bv.selected_frame >= frames_in_cycle {
        bv.selected_frame = frames_in_cycle - 1;
    }

    // Pause playback when there's nothing to animate.
    if frames_in_cycle < 2 && bv.playback.is_some() {
        bv.playback = None;
    }

    // ── Texture ───────────────────────────────────────────────────────
    let image = match bv
        .bam
        .render_frame_centered(bv.selected_cycle, bv.selected_frame)
    {
        Some(buf) => {
            let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                buf.as_raw(),
                buf.width(),
                buf.height(),
            );
            Image::from_rgba8(pixels)
        }
        None => Image::default(),
    };
    window.set_bam_bitmap(image);

    // ── Info-bar text ────────────────────────────────────────────────
    let global_idx = bv
        .bam
        .cycles
        .get(bv.selected_cycle)
        .and_then(|c| c.frame_indices.get(bv.selected_frame).copied());
    let cur_frame = global_idx.and_then(|i| bv.bam.frames.get(i));
    let (fw, fh, cx, cy) = match cur_frame {
        Some(f) => (f.width, f.height, f.center_x, f.center_y),
        None => (0, 0, 0, 0),
    };
    window.set_bam_dims(format!("{fw} × {fh} px").into());
    window.set_bam_frame_center(format!("center ({cx}, {cy})").into());
    window.set_bam_file_size(bv.file_size_text.clone().into());
    window.set_bam_type(
        match bv.bam_type {
            BamType::BamV1 => "BAM V1",
            BamType::BamV2 => "BAM V2",
            BamType::BamC => "BAMC",
        }
        .into(),
    );
    window.set_bam_frames_text(format!("{} frames", bv.bam.frames.len()).into());
    window.set_bam_cycles_text(format!("{} cycles", bv.bam.cycles.len()).into());
    window.set_bam_origin(bv.origin_text.clone().into());

    // ── Selectors ─────────────────────────────────────────────────────
    let cycle_options: Vec<slint::SharedString> = bv
        .bam
        .cycles
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{i} ({} frames)", c.frame_indices.len()).into())
        .collect();
    window.set_bam_cycle_options(slint::ModelRc::new(slint::VecModel::from(cycle_options)));
    window.set_bam_cycle_index(bv.selected_cycle as i32);
    let frame_max = frames_in_cycle.saturating_sub(1) as i32;
    window.set_bam_frame_max(frame_max);
    window.set_bam_frame_index(bv.selected_frame as i32);
    window.set_bam_global_frame_text(
        match global_idx {
            Some(i) => format!("(frame #{i})"),
            None => String::new(),
        }
        .into(),
    );
    window.set_bam_is_playing(bv.playback.is_some());
    window.set_bam_can_play(frames_in_cycle > 1);
}

/// `bam-cycle-changed` callback target. Records the new cycle and
/// triggers a re-render.
pub fn on_cycle_changed(window: &MainWindow, state: &Rc<AppState>, idx: i32) {
    if let Some(bv) = state.bam_viewer.borrow_mut().as_mut() {
        let Ok(i) = usize::try_from(idx) else { return };
        if i >= bv.bam.cycles.len() {
            return;
        }
        bv.selected_cycle = i;
        bv.selected_frame = 0;
        rebase_playback(bv);
    }
    refresh(window, state);
}

/// `bam-frame-changed` callback target.
pub fn on_frame_changed(window: &MainWindow, state: &Rc<AppState>, idx: i32) {
    if let Some(bv) = state.bam_viewer.borrow_mut().as_mut() {
        let Ok(i) = usize::try_from(idx) else { return };
        bv.selected_frame = i;
        rebase_playback(bv);
    }
    refresh(window, state);
}

/// `bam-play-pause-clicked` callback target. Toggles the playback
/// anchor on or off, mirroring the egui BAM viewer's Play button.
pub fn on_play_pause_clicked(window: &MainWindow, state: &Rc<AppState>) {
    if let Some(bv) = state.bam_viewer.borrow_mut().as_mut() {
        let frames_in_cycle = bv
            .bam
            .cycles
            .get(bv.selected_cycle)
            .map(|c| c.frame_indices.len())
            .unwrap_or(0);
        if bv.playback.is_some() {
            bv.playback = None;
        } else if frames_in_cycle > 1 {
            bv.playback = Some(BamPlayback {
                epoch: Instant::now(),
                anchor_frame: bv.selected_frame,
            });
        }
    }
    refresh(window, state);
}

/// Slint Timer tick. Advances `selected_frame` based on the wall-clock
/// elapsed since the playback anchor and pushes a fresh frame to the
/// window. No-op when paused.
pub fn tick(window: &MainWindow, state: &Rc<AppState>) {
    {
        let mut guard = state.bam_viewer.borrow_mut();
        let Some(bv) = guard.as_mut() else { return };
        let Some(p) = bv.playback.as_ref() else { return };
        let len = bv
            .bam
            .cycles
            .get(bv.selected_cycle)
            .map(|c| c.frame_indices.len())
            .unwrap_or(0);
        if len == 0 {
            return;
        }
        let elapsed = p.epoch.elapsed().as_nanos() as u64;
        let ticks = elapsed / BamV1::DEFAULT_FRAME_DURATION.as_nanos() as u64;
        let new_frame = (p.anchor_frame + ticks as usize) % len;
        if new_frame == bv.selected_frame {
            return;
        }
        bv.selected_frame = new_frame;
    }
    refresh(window, state);
}

/// Re-anchor the playback clock to the current selection. Called
/// whenever the user touches the cycle/frame selectors so playback
/// resumes from where they left it instead of jumping.
fn rebase_playback(bv: &mut BamViewerState) {
    if let Some(p) = bv.playback.as_mut() {
        p.epoch = Instant::now();
        p.anchor_frame = bv.selected_frame;
    }
}
