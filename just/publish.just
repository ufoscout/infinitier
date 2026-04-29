
# Dry-run publish for all public crates (no upload, verifies packaging)
publish-dry-run:
    cargo publish --dry-run -p infinitier_datasource
    cargo publish --dry-run -p infinitier_acm_decoder
    cargo publish --dry-run -p infinitier_mve_decoder

# Publish all public crates to crates.io in dependency order
publish:
    cargo publish -p infinitier_datasource
    cargo publish -p infinitier_fs

    cargo publish -p infinitier_acm_decoder
    cargo publish -p infinitier_mve_decoder

    cargo publish -p infinitier_bam_importer
    cargo publish -p infinitier_bif_importer
    cargo publish -p infinitier_bmp_importer
    cargo publish -p infinitier_ids_importer
    cargo publish -p infinitier_key_importer
    cargo publish -p infinitier_pvr_importer
    cargo publish -p infinitier_two_da_importer
    cargo publish -p infinitier_wed_importer
