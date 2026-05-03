use freya::prelude::*;
use infinitier_core::resource::key::ResourceType;

use crate::state::AppState;

mod bmp;
mod unknown;

mod acm;
mod are;
mod bah;
mod bam;
mod bcs;
mod bio;
mod bs;
mod chr;
mod chu;
mod cre;
mod dlg;
mod eff;
mod fnt;
mod gam;
mod glsl;
mod gui;
mod ids;
mod ini;
mod itm;
mod lua;
mod maze;
mod menu;
mod mos;
mod mve;
mod mus;
mod plt;
mod png;
mod pro;
mod pvrz;
mod spl;
mod sql;
mod src;
mod sto;
mod tga;
mod tis;
mod toh;
mod tot;
mod ttf;
mod two_da;
mod vef;
mod vvc;
mod wav;
mod wbm;
mod wed;
mod wfx;
mod wmp;

use bmp::bmp_viewer;
use unknown::unknown_viewer;

use acm::acm_viewer;
use are::are_viewer;
use bah::bah_viewer;
use bam::bam_viewer;
use bcs::bcs_viewer;
use bio::bio_viewer;
use bs::bs_viewer;
use chr::chr_viewer;
use chu::chu_viewer;
use cre::cre_viewer;
use dlg::dlg_viewer;
use eff::eff_viewer;
use fnt::fnt_viewer;
use gam::gam_viewer;
use glsl::glsl_viewer;
use gui::gui_viewer;
use ids::ids_viewer;
use ini::ini_viewer;
use itm::itm_viewer;
use lua::lua_viewer;
use maze::maze_viewer;
use menu::menu_viewer;
use mos::mos_viewer;
use mve::mve_viewer;
use mus::mus_viewer;
use plt::plt_viewer;
use png::png_viewer;
use pro::pro_viewer;
use pvrz::pvrz_viewer;
use spl::spl_viewer;
use sql::sql_viewer;
use src::src_viewer;
use sto::sto_viewer;
use tga::tga_viewer;
use tis::tis_viewer;
use toh::toh_viewer;
use tot::tot_viewer;
use ttf::ttf_viewer;
use two_da::two_da_viewer;
use vef::vef_viewer;
use vvc::vvc_viewer;
use wav::wav_viewer;
use wbm::wbm_viewer;
use wed::wed_viewer;
use wfx::wfx_viewer;
use wmp::wmp_viewer;

#[derive(PartialEq)]
pub struct ResourceViewer;

impl Component for ResourceViewer {
    fn render(&self) -> impl IntoElement {
        let state = use_consume::<State<AppState>>();
        let selected = state.read().selected;

        let content: Element = match selected {
            None => label()
                .text("Select a resource from the panel on the left.")
                .into(),
            Some(resource_id) => {
                let resource_type = state
                    .read()
                    .game_data
                    .get_by_id(resource_id)
                    .map(|r| r.r#type);
                match resource_type {
                    None => label().text("Resource not found.").into(),
                    Some(rt) => match rt {
                        ResourceType::Acm => acm_viewer(),
                        ResourceType::Are => are_viewer(),
                        ResourceType::Bah => bah_viewer(),
                        ResourceType::Bam => bam_viewer(),
                        ResourceType::Bcs => bcs_viewer(),
                        ResourceType::Bio => bio_viewer(),
                        ResourceType::Bmp => bmp_viewer(state, resource_id),
                        ResourceType::Bs => bs_viewer(),
                        ResourceType::Chr => chr_viewer(),
                        ResourceType::Chu => chu_viewer(),
                        ResourceType::Cre => cre_viewer(),
                        ResourceType::Dlg => dlg_viewer(),
                        ResourceType::Eff => eff_viewer(),
                        ResourceType::Fnt => fnt_viewer(),
                        ResourceType::Gam => gam_viewer(),
                        ResourceType::Glsl => glsl_viewer(),
                        ResourceType::Gui => gui_viewer(),
                        ResourceType::Ids => ids_viewer(),
                        ResourceType::Ini => ini_viewer(),
                        ResourceType::Itm => itm_viewer(),
                        ResourceType::Lua => lua_viewer(),
                        ResourceType::Maze => maze_viewer(),
                        ResourceType::Menu => menu_viewer(),
                        ResourceType::Mos => mos_viewer(),
                        ResourceType::Mve => mve_viewer(),
                        ResourceType::Mus => mus_viewer(),
                        ResourceType::Plt => plt_viewer(),
                        ResourceType::Png => png_viewer(),
                        ResourceType::Pro => pro_viewer(),
                        ResourceType::Pvrz => pvrz_viewer(),
                        ResourceType::Spl => spl_viewer(),
                        ResourceType::Sql => sql_viewer(),
                        ResourceType::Src => src_viewer(),
                        ResourceType::Sto => sto_viewer(),
                        ResourceType::Tga => tga_viewer(),
                        ResourceType::Tis => tis_viewer(),
                        ResourceType::Toh => toh_viewer(),
                        ResourceType::Tot => tot_viewer(),
                        ResourceType::Ttf => ttf_viewer(),
                        ResourceType::TwoDA => two_da_viewer(),
                        ResourceType::Vef => vef_viewer(),
                        ResourceType::Vvc => vvc_viewer(),
                        ResourceType::Wav => wav_viewer(),
                        ResourceType::Wbm => wbm_viewer(),
                        ResourceType::Wed => wed_viewer(),
                        ResourceType::Wfx => wfx_viewer(),
                        ResourceType::Wmp => wmp_viewer(),
                        ResourceType::Unknown(type_id) => unknown_viewer(type_id),
                    },
                }
            }
        };

        rect()
            .expanded()
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .child(content)
    }
}
