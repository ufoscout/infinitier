use eframe::egui;
use infinitier_key_importer::ResourceType;

use crate::state::AppState;

mod acm;
mod are;
mod bah;
mod bam;
mod bcs;
mod bio;
mod bmp;
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
mod unknown;
mod vef;
mod vvc;
mod wav;
mod wbm;
mod wed;
mod wfx;
mod wmp;

use acm::AcmViewer;
use are::AreViewer;
use bah::BahViewer;
use bam::BamViewer;
use bcs::BcsViewer;
use bio::BioViewer;
use bmp::BmpViewer;
use bs::BsViewer;
use chr::ChrViewer;
use chu::ChuViewer;
use cre::CreViewer;
use dlg::DlgViewer;
use eff::EffViewer;
use fnt::FntViewer;
use gam::GamViewer;
use glsl::GlslViewer;
use gui::GuiViewer;
use ids::IdsViewer;
use ini::IniViewer;
use itm::ItmViewer;
use lua::LuaViewer;
use maze::MazeViewer;
use menu::MenuViewer;
use mos::MosViewer;
use mve::MveViewer;
use mus::MusViewer;
use plt::PltViewer;
use png::PngViewer;
use pro::ProViewer;
use pvrz::PvrzViewer;
use spl::SplViewer;
use sql::SqlViewer;
use src::SrcViewer;
use sto::StoViewer;
use tga::TgaViewer;
use tis::TisViewer;
use toh::TohViewer;
use tot::TotViewer;
use ttf::TtfViewer;
use two_da::TwoDAViewer;
use unknown::UnknownViewer;
use vef::VefViewer;
use vvc::VvcViewer;
use wav::WavViewer;
use wbm::WbmViewer;
use wed::WedViewer;
use wfx::WfxViewer;
use wmp::WmpViewer;

pub struct ResourceViewer;

impl ResourceViewer {
    pub fn show(ui: &mut egui::Ui, state: &AppState) {
        match state.selected_resource {
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a resource from the panel on the left.");
                });
            }
            Some(resource_id) => {
                if let Some(resource) = state.game_data.get_by_id(resource_id) {
                    match resource.r#type {
                        ResourceType::Acm => AcmViewer::show(ui),
                        ResourceType::Are => AreViewer::show(ui),
                        ResourceType::Bah => BahViewer::show(ui),
                        ResourceType::Bam => BamViewer::show(ui),
                        ResourceType::Bcs => BcsViewer::show(ui),
                        ResourceType::Bio => BioViewer::show(ui),
                        ResourceType::Bmp => BmpViewer::show(ui),
                        ResourceType::Bs => BsViewer::show(ui),
                        ResourceType::Chr => ChrViewer::show(ui),
                        ResourceType::Chu => ChuViewer::show(ui),
                        ResourceType::Cre => CreViewer::show(ui),
                        ResourceType::Dlg => DlgViewer::show(ui),
                        ResourceType::Eff => EffViewer::show(ui),
                        ResourceType::Fnt => FntViewer::show(ui),
                        ResourceType::Gam => GamViewer::show(ui),
                        ResourceType::Glsl => GlslViewer::show(ui),
                        ResourceType::Gui => GuiViewer::show(ui),
                        ResourceType::Ids => IdsViewer::show(ui),
                        ResourceType::Ini => IniViewer::show(ui),
                        ResourceType::Itm => ItmViewer::show(ui),
                        ResourceType::Lua => LuaViewer::show(ui),
                        ResourceType::Maze => MazeViewer::show(ui),
                        ResourceType::Menu => MenuViewer::show(ui),
                        ResourceType::Mos => MosViewer::show(ui),
                        ResourceType::Mve => MveViewer::show(ui),
                        ResourceType::Mus => MusViewer::show(ui),
                        ResourceType::Plt => PltViewer::show(ui),
                        ResourceType::Png => PngViewer::show(ui),
                        ResourceType::Pro => ProViewer::show(ui),
                        ResourceType::Pvrz => PvrzViewer::show(ui),
                        ResourceType::Spl => SplViewer::show(ui),
                        ResourceType::Sql => SqlViewer::show(ui),
                        ResourceType::Src => SrcViewer::show(ui),
                        ResourceType::Sto => StoViewer::show(ui),
                        ResourceType::Tga => TgaViewer::show(ui),
                        ResourceType::Tis => TisViewer::show(ui),
                        ResourceType::Toh => TohViewer::show(ui),
                        ResourceType::Tot => TotViewer::show(ui),
                        ResourceType::Ttf => TtfViewer::show(ui),
                        ResourceType::TwoDA => TwoDAViewer::show(ui),
                        ResourceType::Vef => VefViewer::show(ui),
                        ResourceType::Vvc => VvcViewer::show(ui),
                        ResourceType::Wav => WavViewer::show(ui),
                        ResourceType::Wbm => WbmViewer::show(ui),
                        ResourceType::Wed => WedViewer::show(ui),
                        ResourceType::Wfx => WfxViewer::show(ui),
                        ResourceType::Wmp => WmpViewer::show(ui),
                        ResourceType::Unknown(type_id) => UnknownViewer::show(ui, type_id),
                    }
                }
            }
        }
    }
}
