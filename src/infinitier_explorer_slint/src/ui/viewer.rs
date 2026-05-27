//! Central viewer dispatcher.
//!
//! Imports the selected resource and routes to the matching populator
//! under `ui::viewers::*`. Mirrors the egui explorer's
//! `ResourceViewer::show` match exactly — same arms, same fallbacks.

use std::rc::Rc;

use infinitier_core::imported_resource::ImportedResource;
use log::error;

use crate::MainWindow;
use crate::state::AppState;
use crate::ui::viewers;

/// Reset every viewer-related property to the empty-state defaults.
/// Called at startup and whenever a group toggle clears the selection.
pub fn clear(window: &MainWindow) {
    window.set_viewer_kind("empty".into());
    window.set_message_text(slint::SharedString::default());
}

/// Populate the viewer pane for the resource at `resource_idx`.
pub fn show(window: &MainWindow, state: &Rc<AppState>, resource_idx: usize) {
    // Drop any per-viewer state from a previous selection so audio
    // playback threads / GPU textures get released before we spawn
    // their successors.
    state.bam_viewer.borrow_mut().take();
    state.sound_viewer.borrow_mut().take();
    state.movie_viewer.borrow_mut().take();
    state.tis_viewer.borrow_mut().take();

    let Some(resource) = state.game_data.get_by_id(resource_idx) else {
        error!("Resource not found: {resource_idx}");
        viewers::message::populate(
            window,
            &format!("Resource not found: id={resource_idx}"),
        );
        return;
    };

    let imported = match resource.import(&state.game_data) {
        Ok(i) => i,
        Err(e) => {
            error!("Error importing resource {resource_idx}: {e}");
            viewers::message::populate(
                window,
                &format!("Error importing resource {resource_idx}: {e}"),
            );
            return;
        }
    };

    match imported {
        ImportedResource::Image(img) => viewers::image::populate(window, img, resource),
        ImportedResource::Bam(bam) => viewers::bam::populate(window, state, bam, resource),
        ImportedResource::Bcs(bcs) => viewers::bcs::populate(window, bcs, resource),
        ImportedResource::Fnt(fnt) => viewers::fnt::populate(window, fnt, resource),
        ImportedResource::TwoDA(t) => viewers::two_da::populate(window, t),
        ImportedResource::Ids(ids) => viewers::ids::populate(window, ids),
        ImportedResource::Ini(ini) => viewers::ini::populate(window, ini),

        ImportedResource::Sound(sound) => viewers::sound::populate(window, state, sound, resource),
        ImportedResource::Mve(src) | ImportedResource::Wbm(src) => {
            viewers::movie::populate(window, state, src)
        }

        ImportedResource::Wed(_) => viewers::stub::label(window, "WED Viewer"),
        ImportedResource::Cre(_) => viewers::stub::label(window, "CRE Viewer"),
        ImportedResource::Gam(_) => viewers::stub::label(window, "GAM Viewer"),
        ImportedResource::Itm(_) => viewers::stub::label(window, "ITM Viewer"),
        ImportedResource::Spl(_) => viewers::stub::label(window, "SPL Viewer"),
        ImportedResource::Tis(tis) => viewers::tis::populate(window, state, tis, resource),
        ImportedResource::Ttf(_) => viewers::stub::label(window, "TTF Viewer"),

        ImportedResource::Are => viewers::stub::label(window, "ARE Viewer"),
        ImportedResource::Bah => viewers::stub::label(window, "BAH Viewer"),
        ImportedResource::Bio => viewers::stub::label(window, "BIO Viewer"),
        ImportedResource::Chr => viewers::stub::label(window, "CHR Viewer"),
        ImportedResource::Chu => viewers::stub::label(window, "CHU Viewer"),
        ImportedResource::Dlg => viewers::stub::label(window, "DLG Viewer"),
        ImportedResource::Eff => viewers::stub::label(window, "EFF Viewer"),
        ImportedResource::Glsl => viewers::stub::label(window, "GLSL Viewer"),
        ImportedResource::Gui => viewers::stub::label(window, "GUI Viewer"),
        ImportedResource::Lua => viewers::stub::label(window, "LUA Viewer"),
        ImportedResource::Maze => viewers::stub::label(window, "MAZE Viewer"),
        ImportedResource::Menu => viewers::stub::label(window, "MENU Viewer"),
        ImportedResource::Mus => viewers::stub::label(window, "MUS Viewer"),
        ImportedResource::Plt => viewers::stub::label(window, "PLT Viewer"),
        ImportedResource::Pro => viewers::stub::label(window, "PRO Viewer"),
        ImportedResource::Sql => viewers::stub::label(window, "SQL Viewer"),
        ImportedResource::Src => viewers::stub::label(window, "SRC Viewer"),
        ImportedResource::Sto => viewers::stub::label(window, "STO Viewer"),
        ImportedResource::Tga => viewers::stub::label(window, "TGA Viewer"),
        ImportedResource::Toh => viewers::stub::label(window, "TOH Viewer"),
        ImportedResource::Tot => viewers::stub::label(window, "TOT Viewer"),
        ImportedResource::Vef => viewers::stub::label(window, "VEF Viewer"),
        ImportedResource::Vvc => viewers::stub::label(window, "VVC Viewer"),
        ImportedResource::Wfx => viewers::stub::label(window, "WFX Viewer"),
        ImportedResource::Wmp => viewers::stub::label(window, "WMP Viewer"),

        // `Sav` / `Tlk` have no `ResourceType` entry yet, so the import
        // dispatcher never produces them here. Treat as Unknown for
        // safety until dedicated viewers exist.
        ImportedResource::Sav(_) | ImportedResource::Tlk(_) | ImportedResource::Unknown(_) => {
            viewers::stub::unknown(window, resource)
        }
    }
}
