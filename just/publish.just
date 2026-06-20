
# Dry-run publish for every publishable crate (no upload, verifies
# packaging).
publish-dry-run:
    cargo publish --workspace --dry-run

# Publish every publishable crate to crates.io.
publish:
    cargo publish --workspace
