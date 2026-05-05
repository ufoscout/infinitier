use infinitier_acm_decoder::AcmDecoder;
use infinitier_bam_importer::Bam;
use infinitier_bmp_importer::Bmp;
use infinitier_ids_importer::Ids;
use infinitier_ini_importer::Ini;
use infinitier_pvr_importer::PvrzHeader;
use infinitier_two_da_importer::TwoDA;
use infinitier_wed_importer::Wed;

#[derive(Debug)]
pub enum ImportedResource {
    // Types with importers
    Acm(AcmDecoder),
    Bam(Bam),
    Bmp(Bmp),
    Ids(Ids),
    Ini(Ini),
    Pvrz(PvrzHeader),
    TwoDA(TwoDA),
    Wed(Wed),
    // Types without importers
    Are,
    Bah,
    Bcs,
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
    Mve,
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
    Wav,
    Wbm,
    Wfx,
    Wmp,
    Unknown(u16),
}
