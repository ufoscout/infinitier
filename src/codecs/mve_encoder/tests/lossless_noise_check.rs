//! Spot-check: try to encode the noise asset *without* lossy
//! downsampling. Phase 6 added 0x8/0xa, so blocks that previously
//! failed every lossless mode in the chooser should now mostly fit.
//! Any blocks that still need 0xb (raw 8×8 = 64 B) are reported.

use std::fs;
use std::path::PathBuf;

use infinitier_datasource::DataSource;
use infinitier_mve_decoder::MveDecoder;
use infinitier_mve_encoder::{FromAssetsOptions, encode_from_assets};
use infinitier_test_utils::{get_assets_path, get_target_path};

#[test]
fn noise_encodes_losslessly_with_phase_6() {
    let asset = get_assets_path().join("mve_encoder/320x240_15fps_3s_noise");
    if !asset.is_dir() {
        eprintln!("noise asset missing — skipping");
        return;
    }
    let mut pngs: Vec<PathBuf> = fs::read_dir(&asset)
        .unwrap()
        .filter_map(|r| r.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("png"))
        .collect();
    pngs.sort();
    let wav = asset.join("audio.wav");
    let out = get_target_path().join("mve_encoder");
    fs::create_dir_all(&out).unwrap();

    let opts = FromAssetsOptions {
        frame_duration_us: 66_667,
        lossy_downsample: false,
        strict_palette: false,
        output_name: "noise_lossless".into(),
    };
    let path =
        encode_from_assets(&pngs, &wav, &opts, &out).expect("lossless noise encode should succeed");
    let size = fs::metadata(&path).unwrap().len();

    let bytes = fs::read(&path).unwrap();
    let ds = DataSource::new(bytes);
    let mut dec = MveDecoder::new(ds.reader().unwrap(), "noise_lossless").unwrap();
    while dec.next_frame().unwrap().is_some() {}
    let stats = dec.block_mode_stats();
    let total: u64 = stats.video8.iter().sum();
    let raw = stats.video8[0xb];
    eprintln!("noise lossless: {size} B, {raw}/{total} blocks fell to 0xb");
    eprintln!("histogram: {:?}", stats.video8);
}
