use std::sync::Arc;

use floem::views::label;
use floem::{AnyView, IntoView};
use infinitier_core::game::{GameData, ResourceId};
use infinitier_core::resource::key::ResourceType;

use crate::app::BmpCache;

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

pub fn view(
    selected_id: Option<ResourceId>,
    game_data: &Arc<GameData>,
    bmp_cache: &BmpCache,
) -> AnyView {
    let Some(id) = selected_id else {
        return label(|| "Select a resource from the panel on the left.".to_string()).into_any();
    };

    let Some(resource) = game_data.get_by_id(id) else {
        return label(|| "Resource not found.".to_string()).into_any();
    };

    match resource.r#type {
        ResourceType::Acm => acm::view(),
        ResourceType::Are => are::view(),
        ResourceType::Bah => bah::view(),
        ResourceType::Bam => bam::view(),
        ResourceType::Bcs => bcs::view(),
        ResourceType::Bio => bio::view(),
        ResourceType::Bmp => bmp::view(id, game_data, bmp_cache),
        ResourceType::Bs => bs::view(),
        ResourceType::Chr => chr::view(),
        ResourceType::Chu => chu::view(),
        ResourceType::Cre => cre::view(),
        ResourceType::Dlg => dlg::view(),
        ResourceType::Eff => eff::view(),
        ResourceType::Fnt => fnt::view(),
        ResourceType::Gam => gam::view(),
        ResourceType::Glsl => glsl::view(),
        ResourceType::Gui => gui::view(),
        ResourceType::Ids => ids::view(),
        ResourceType::Ini => ini::view(),
        ResourceType::Itm => itm::view(),
        ResourceType::Lua => lua::view(),
        ResourceType::Maze => maze::view(),
        ResourceType::Menu => menu::view(),
        ResourceType::Mos => mos::view(),
        ResourceType::Mve => mve::view(),
        ResourceType::Mus => mus::view(),
        ResourceType::Plt => plt::view(),
        ResourceType::Png => png::view(),
        ResourceType::Pro => pro::view(),
        ResourceType::Pvrz => pvrz::view(),
        ResourceType::Spl => spl::view(),
        ResourceType::Sql => sql::view(),
        ResourceType::Src => src::view(),
        ResourceType::Sto => sto::view(),
        ResourceType::Tga => tga::view(),
        ResourceType::Tis => tis::view(),
        ResourceType::Toh => toh::view(),
        ResourceType::Tot => tot::view(),
        ResourceType::Ttf => ttf::view(),
        ResourceType::TwoDA => two_da::view(),
        ResourceType::Vef => vef::view(),
        ResourceType::Vvc => vvc::view(),
        ResourceType::Wav => wav::view(),
        ResourceType::Wbm => wbm::view(),
        ResourceType::Wed => wed::view(),
        ResourceType::Wfx => wfx::view(),
        ResourceType::Wmp => wmp::view(),
        ResourceType::Unknown(type_id) => unknown::view(type_id),
    }
}
