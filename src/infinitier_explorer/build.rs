//! Bakes git build info into the binary so the running explorer can report
//! which build it is (startup log + window title). The released binary has
//! no `.git` directory, so this has to happen at build time.
//!
//! Emits three `cargo:rustc-env` values, read via `option_env!` in
//! `build_data.rs`:
//!   - `VERGEN_GIT_DESCRIBE` — `git describe --tags --dirty` (e.g. `v0.0.14`
//!     on a release tag, `v0.0.14-3-gabc1234-dirty` on a dev build). EMPTY
//!     when the repo has no tags reachable from HEAD.
//!   - `VERGEN_GIT_BRANCH`   — the current branch (empty on a detached/tag
//!     checkout, which is the normal state for a release build)
//!   - `VERGEN_GIT_SHA`      — short commit hash, used as the fallback label
//!     for untagged dev builds where DESCRIBE is empty
//!
//! `Emitter` defaults to non-fatal: if git info can't be read (no repo, etc.)
//! it warns and falls back to an idempotent placeholder rather than failing
//! the build.

use vergen_gix::{Emitter, Gix};

fn main() {
    let gix = Gix::builder()
        .describe(true, true, None)
        .branch(true)
        .sha(true)
        .build();
    let result = Emitter::default()
        .add_instructions(&gix)
        .and_then(|emitter| emitter.emit());

    if let Err(e) = result {
        // Don't break local builds outside a git checkout — env!() still
        // resolves because vergen emits idempotent placeholders on failure.
        println!("cargo:warning=git version info unavailable: {e}");
    }
}
