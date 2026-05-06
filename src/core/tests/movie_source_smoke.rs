use infinitier_core::movie::MovieSource;
use infinitier_datasource::DataSource;
use infinitier_test_utils::get_assets_path;

#[test]
fn movie_source_opens_bundled_mve() {
    let path = get_assets_path().join("resources/MVE/8_bits/BILOGO.MVE");
    let ds = DataSource::new(path.clone());
    let src = MovieSource::new(ds, "BILOGO.MVE");
    let mut dec = src
        .open()
        .unwrap_or_else(|e| panic!("open failed: {e} (path={})", path.display()));
    // Pull the first frame so the timer chunk gets parsed.
    let _ = dec.next_frame();
    println!(
        "BILOGO: {}x{}, frame_dur={}us",
        dec.width(),
        dec.height(),
        dec.frame_duration_us()
    );
    assert!(dec.width() > 0, "width must be non-zero");
    assert!(dec.height() > 0, "height must be non-zero");
    assert!(dec.frame_duration_us() > 0, "frame_dur must be non-zero");
}
