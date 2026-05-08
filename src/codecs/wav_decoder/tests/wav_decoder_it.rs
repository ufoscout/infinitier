use std::fs;

use infinitier_datasource::DataSource;
use infinitier_test_utils::get_assets_path;
use infinitier_wav_decoder::{WavDecoder, WavFormat, WavInfo};
use sha2::{Digest, Sha256};

/// Test that can decode WAVC files
#[test]
fn test_decode_wavc() {
    let wavc_path = get_assets_path().join("resources/WAV/1GROMG09.WAVC");
    let data = DataSource::new(wavc_path);
    let mut dec = WavDecoder::open(&data, "1GROMG09").unwrap();

    assert_eq!(dec.format(), WavFormat::Wavc);
    assert_eq!(
        dec.info(),
        &WavInfo {
            channels: 1,
            sample_rate: 22050,
            bits_per_sample: 16,
            total_values: 115168,
        }
    );

    let temp = tempfile::NamedTempFile::new().unwrap();
    dec.decode_to_file(temp.path()).unwrap();

    let created_wav_hash = Sha256::digest(fs::read(temp.path()).unwrap());
    let wav_hash =
        Sha256::digest(fs::read(get_assets_path().join("resources/WAV/1GROMG09.WAV")).unwrap());

    assert_eq!(created_wav_hash, wav_hash);
}

/// Test that can decode WAV files
#[test]
fn test_decode_wav() {
    let wavc_path = get_assets_path().join("resources/WAV/1GROMG09.WAV");
    let data = DataSource::new(wavc_path);
    let mut dec = WavDecoder::open(&data, "1GROMG09").unwrap();

    assert_eq!(dec.format(), WavFormat::Wav);
    assert_eq!(
        dec.info(),
        &WavInfo {
            channels: 1,
            sample_rate: 22050,
            bits_per_sample: 16,
            total_values: 115168,
        }
    );

    let temp = tempfile::NamedTempFile::new().unwrap();
    dec.decode_to_file(temp.path()).unwrap();

    let created_wav_hash = Sha256::digest(fs::read(temp.path()).unwrap());
    let wav_hash =
        Sha256::digest(fs::read(get_assets_path().join("resources/WAV/1GROMG09.WAV")).unwrap());

    assert_eq!(created_wav_hash, wav_hash);
}

/// 8-bit mono PCM (e.g. BG's CHANT.WAV) — gemrb supports it, and so do we.
#[test]
fn test_decode_wav_8bit() {
    let wav_path = get_assets_path().join("resources/WAV/CHANT.WAV");
    let data = DataSource::new(wav_path);
    let mut dec = WavDecoder::open(&data, "CHANT").unwrap();

    assert_eq!(dec.format(), WavFormat::Wav);
    assert_eq!(
        dec.info(),
        &WavInfo {
            channels: 1,
            sample_rate: 22050,
            bits_per_sample: 8,
            total_values: 320213,
        }
    );

    let samples = dec.decode_all().unwrap();
    assert_eq!(samples.len(), 320213);
    // Output must be scaled to the full i16 range — i.e. at least one sample
    // must land outside the 8-bit native [-128, 127] window. Otherwise we'd
    // be playing ~256× too quiet, which is the bug we're guarding against.
    assert!(samples.iter().any(|&s| s > 127 || s < -128));
}

#[test]
fn test_decoded_wavc_infos_match() {
    let mut wavc = {
        let wavc_path = get_assets_path().join("resources/WAV/POQU_22.WAVC");
        let data = DataSource::new(wavc_path);
        let dec = WavDecoder::open(&data, "POQU_22.WAVC").unwrap();
        assert_eq!(dec.format(), WavFormat::Wavc);
        dec
    };

    let mut wav = {
        let wav_path = get_assets_path().join("resources/WAV/POQU_22.WAV");
        let data = DataSource::new(wav_path);
        let dec = WavDecoder::open(&data, "POQU_22.WAV").unwrap();
        assert_eq!(dec.format(), WavFormat::Wav);
        dec
    };

    assert_eq!(wav.info(), wavc.info());
    assert_eq!(wav.decode_all().unwrap(), wavc.decode_all().unwrap());
}

#[test]
fn test_decoded_ogg_infos_match() {
    let mut ogg = {
        let ogg_path = get_assets_path().join("resources/WAV/FIREE05.OGG");
        let data = DataSource::new(ogg_path);
        let dec = WavDecoder::open(&data, "FIREE05.OGG").unwrap();
        assert_eq!(dec.format(), WavFormat::Ogg);
        dec
    };

    let mut wav = {
        let wav_path = get_assets_path().join("resources/WAV/FIREE05.WAV");
        let data = DataSource::new(wav_path);
        let dec = WavDecoder::open(&data, "FIREE05.WAV").unwrap();
        assert_eq!(dec.format(), WavFormat::Wav);
        dec
    };

    assert_eq!(wav.info(), ogg.info());
    assert_eq!(wav.decode_all().unwrap(), ogg.decode_all().unwrap());
}
