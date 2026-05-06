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
    let mut dec = WavDecoder::open(&data).unwrap();

    assert_eq!(dec.format(), WavFormat::Wavc);
    assert_eq!(dec.info(), &WavInfo {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 16,
        total_values: 115168,
    });

    let temp = tempfile::NamedTempFile::new().unwrap();
    dec.decode_to_file(temp.path()).unwrap();

    let created_wav_hash = Sha256::digest(fs::read(temp.path()).unwrap());
    let wav_hash = Sha256::digest(fs::read(&get_assets_path().join("resources/WAV/1GROMG09.WAV")).unwrap());

    assert_eq!(created_wav_hash, wav_hash);
}

/// Test that can decode WAV files
#[test]
fn test_decode_wav() {
    let wavc_path = get_assets_path().join("resources/WAV/1GROMG09.WAV");
    let data = DataSource::new(wavc_path);
    let mut dec = WavDecoder::open(&data).unwrap();

    assert_eq!(dec.format(), WavFormat::Wav);
    assert_eq!(dec.info(), &WavInfo {
        channels: 1,
        sample_rate: 22050,
        bits_per_sample: 16,
        total_values: 115168,
    });

    let temp = tempfile::NamedTempFile::new().unwrap();
    dec.decode_to_file(temp.path()).unwrap();

    let created_wav_hash = Sha256::digest(fs::read(temp.path()).unwrap());
    let wav_hash = Sha256::digest(fs::read(&get_assets_path().join("resources/WAV/1GROMG09.WAV")).unwrap());

    assert_eq!(created_wav_hash, wav_hash);
}

#[test]
fn test_decoded_infos_match() {
    let mut wavc = {
        let wavc_path = get_assets_path().join("resources/WAV/1GROMG09.WAVC");
        let data = DataSource::new(wavc_path);
        let dec = WavDecoder::open(&data).unwrap();
        assert_eq!(dec.format(), WavFormat::Wavc);
        dec
    };

    let mut wav = {
        let wav_path = get_assets_path().join("resources/WAV/1GROMG09.WAV");
        let data = DataSource::new(wav_path);
        let dec = WavDecoder::open(&data).unwrap();
        assert_eq!(dec.format(), WavFormat::Wav);
        dec
    };

    assert_eq!(wav.info(), wavc.info());
    assert_eq!(wav.decode_all().unwrap(), wavc.decode_all().unwrap());

}