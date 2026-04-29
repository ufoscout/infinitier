
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
