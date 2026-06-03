use infinitier_core::fs::{CaseInsensitiveFS, Importer};
use infinitier_core::game::GameDataBuilder;
use infinitier_core::game_detect::detect_game;
use infinitier_core::imported_resource::gam::{ImportedGam, NpcCre};
use infinitier_core::resource::cre::{EffectList, EffectV2, SubSections};
use infinitier_core::resource::gam::GamImporter;
use std::path::PathBuf;
#[test]
#[ignore]
fn anal() {
    let gp = vec![PathBuf::from("/home/ufo/Temp/Games/Baldur's Gate 2 - Enhanced Edition"),
        PathBuf::from("/home/ufo/Temp/Games/Baldur's Gate 2 - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate II - Enhanced Edition")];
    let fs=CaseInsensitiveFS::new(&gp).unwrap();let g=detect_game(&fs).unwrap();
    let gd=GameDataBuilder::new(&gp,g).unwrap().build().unwrap();
    let s=gd.save_games().by_name("000000004-Salvataggio Rapido-4-TOB").cloned().unwrap();
    let gam=GamImporter{name:&s.name,engine:g.engine()}.import(&s.gam).unwrap();
    let im=ImportedGam::load(gam,&gd).unwrap();
    // Collect all op187 records across all party members.
    let mut recs: Vec<Vec<u8>> = Vec::new();
    for npc in &im.party_npcs {
        let Some(NpcCre::Cre(c))=npc.cre.as_ref() else{continue};
        let SubSections::V1(sub)=&c.sub_sections else{continue};
        let EffectList::V2(ef)=&sub.effects else{continue};
        for e in ef { if let EffectV2::LocalVariable(lv)=e { recs.push(lv.record().to_vec()); } }
    }
    println!("total op187 records: {}", recs.len());
    // Which byte offsets are non-zero in ANY record (excluding value 0x14..0x18 and name 0xA0..0xC0)?
    let mut nonzero_off: Vec<usize> = Vec::new();
    for o in 0..264 {
        if (0x14..0x18).contains(&o) || (0xA0..0xC0).contains(&o) { continue; }
        if recs.iter().any(|r| r[o]!=0) { nonzero_off.push(o); }
    }
    println!("non-zero offsets (excl name/value): {:02x?}", nonzero_off);
    // For each such offset, are all records identical there? print distinct values.
    for o in &nonzero_off {
        let mut vals: Vec<u8> = recs.iter().map(|r| r[*o]).collect();
        vals.sort(); vals.dedup();
        if vals.len()>1 || vals[0]!=0 {
            println!("  off 0x{:02x}: distinct={:02x?}", o, vals);
        }
    }
}
