/// `git describe --tags --dirty` of this build, baked in at compile time by
/// `build.rs` (e.g. `v0.0.14` on a release tag). Empty for untagged builds.
/// `option_env!` keeps the crate compiling even if `build.rs` couldn't run.
const GIT_DESCRIBE: &str = match option_env!("VERGEN_GIT_DESCRIBE") {
    Some(v) => v,
    None => "",
};

/// Git branch of this build. Empty on a detached/tag checkout (the normal
/// state for a release build).
const GIT_BRANCH: &str = match option_env!("VERGEN_GIT_BRANCH") {
    Some(v) => v,
    None => "",
};

/// Short commit hash — fallback label when there's no tag to describe.
const GIT_SHA: &str = match option_env!("VERGEN_GIT_SHA") {
    Some(v) => v,
    None => "",
};

/// Human-facing build identifier: the tag/describe when available, otherwise
/// the branch + short SHA for untagged dev builds, else `"unknown"`.
pub fn build_version() -> String {
    if !GIT_DESCRIBE.is_empty() {
        GIT_DESCRIBE.to_owned()
    } else if !GIT_SHA.is_empty() && !GIT_BRANCH.is_empty() {
        format!("{GIT_BRANCH}-{GIT_SHA}")
    } else if !GIT_SHA.is_empty() {
        GIT_SHA.to_owned()
    } else {
        "unknown".to_owned()
    }
}
