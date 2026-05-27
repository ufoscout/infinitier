//! Window lifecycle + callback wiring.

use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};

use crate::MainWindow;
use crate::state::AppState;
use crate::ui;

pub fn run(state: AppState) {
    let state = state.into_rc();
    let window = MainWindow::new().expect("create MainWindow");

    let title = format!(
        "Infinitier Explorer (Slint) — {:?} — {}",
        state.game_data.game(),
        state
            .game_data
            .fs()
            .get_roots()
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
    );
    window.set_window_title(title.into());

    // Static seed: build the tree and the initial empty viewer pane.
    ui::tree::populate(&window, &state);
    ui::info::clear(&window);
    ui::viewer::clear(&window);

    // Click on a tree row — group header or resource leaf.
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_node_clicked(move |idx| {
            if let Some(w) = weak.upgrade() {
                ui::tree::on_node_clicked(&w, &state, idx);
            }
        });
    }

    // BAM viewer cycle / frame change.
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_bam_cycle_changed(move |idx| {
            if let Some(w) = weak.upgrade() {
                ui::viewers::bam::on_cycle_changed(&w, &state, idx);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_bam_frame_changed(move |idx| {
            if let Some(w) = weak.upgrade() {
                ui::viewers::bam::on_frame_changed(&w, &state, idx);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_bam_play_pause_clicked(move || {
            if let Some(w) = weak.upgrade() {
                ui::viewers::bam::on_play_pause_clicked(&w, &state);
            }
        });
    }

    // Sound transport.
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_sound_play_pause_clicked(move || {
            if let Some(w) = weak.upgrade() {
                ui::viewers::sound::on_play_pause_clicked(&w, &state);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_sound_stop_clicked(move || {
            if let Some(w) = weak.upgrade() {
                ui::viewers::sound::on_stop_clicked(&w, &state);
            }
        });
    }

    // TIS controls.
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_tis_columns_changed(move |value| {
            if let Some(w) = weak.upgrade() {
                ui::viewers::tis::on_columns_changed(&w, &state, value);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_tis_show_grid_changed(move |value| {
            if let Some(w) = weak.upgrade() {
                ui::viewers::tis::on_show_grid_changed(&w, &state, value);
            }
        });
    }

    // Movie transport.
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_movie_play_pause_clicked(move || {
            if let Some(w) = weak.upgrade() {
                ui::viewers::movie::on_play_pause_clicked(&w, &state);
            }
        });
    }
    {
        let weak = window.as_weak();
        let state = state.clone();
        window.on_movie_stop_clicked(move || {
            if let Some(w) = weak.upgrade() {
                ui::viewers::movie::on_stop_clicked(&w, &state);
            }
        });
    }

    // ── Playback ticker ──────────────────────────────────────────
    // Slint has no `request_repaint_after` equivalent that the egui
    // viewers used; instead a single 30 fps timer advances the sound
    // progress bar and pumps decoded movie frames into the `image`
    // property. The timer is harmless when neither viewer is active —
    // both per-viewer ticks bail out immediately when their state cell
    // is `None`.
    let playback_timer = Timer::default();
    {
        let weak = window.as_weak();
        let state = state.clone();
        playback_timer.start(TimerMode::Repeated, Duration::from_millis(33), move || {
            if let Some(w) = weak.upgrade() {
                if state.bam_viewer.borrow().is_some() {
                    ui::viewers::bam::tick(&w, &state);
                }
                if state.sound_viewer.borrow().is_some() {
                    ui::viewers::sound::tick(&w, &state);
                }
                if state.movie_viewer.borrow().is_some() {
                    ui::viewers::movie::tick(&w, &state);
                }
                if state.tis_viewer.borrow().is_some() {
                    ui::viewers::tis::tick(&w, &state);
                }
            }
        });
    }

    window.run().expect("event loop");
    // Drop after run so the timer stays alive for the entire window lifetime.
    drop(playback_timer);
}
