
# Dry-run publish for every publishable crate (no upload, verifies
# packaging).
publish-dry-run:
    cargo publish --workspace --dry-run

# Publish every publishable crate to crates.io.
publish:
    # cargo publish --workspace
    # use cargo-release because it automatically handles crates.io [too many crates] errors
    PUBLISH_GRACE_SLEEP=10 cargo release --workspace --execute
