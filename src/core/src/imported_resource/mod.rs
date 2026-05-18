use infinitier_bmp_importer::Bmp;
use infinitier_ids_importer::Ids;
use infinitier_ini_importer::Ini;
use infinitier_pvr_importer::PvrzHeader;
use infinitier_two_da_importer::TwoDA;
use infinitier_wed_importer::Wed;

use crate::sound::SoundDecoder;
use bam::ImportedBam;
use bcs::ImportedBcs;
use movie::MovieSource;

pub mod bam;
pub mod bcs;
pub mod movie;

#[derive(Debug)]
pub enum ImportedResource {
    // Types with importers
    Bam(ImportedBam),
    Bcs(ImportedBcs),
    Bmp(Bmp),
    Ids(Ids),
    Ini(Ini),
    Pvrz(PvrzHeader),
    /// ACM and WAV/WAVC resources both decode to PCM via the unified
    /// streaming [`SoundDecoder`].
    Sound(SoundDecoder),
    TwoDA(TwoDA),
    Wed(Wed),
    // Types without importers
    Are,
    Bah,
    Bio,
    Bs,
    Chr,
    Chu,
    Cre,
    Dlg,
    Eff,
    Fnt,
    Gam,
    Glsl,
    Gui,
    Itm,
    Lua,
    Maze,
    Menu,
    Mos,
    Mus,
    Mve(MovieSource),
    Plt,
    Png,
    Pro,
    Spl,
    Sql,
    Src,
    Sto,
    Tga,
    Tis,
    Toh,
    Tot,
    Ttf,
    Vef,
    Vvc,
    Wbm(MovieSource),
    Wfx,
    Wmp,
    Unknown(u16),
}
