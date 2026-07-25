// compiler/src/toolchain.rs
//
// Finding the `roze` compiler binary from one of the other tools
// (roze-build, roze-pkg) turned out to be implemented twice, independently,
// each with its own bug: roze-build's version moved to the *parent*
// directory before ever checking anything (so it could never find a
// compiler sitting in the current directory's own target/release), and
// roze-pkg's version walked up a hardcoded number of directories from its
// own executable's location, landing one level too high for a normal
// `cargo build --release` layout. One shared, tested implementation here
// means a fix only has to happen once.
use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};

/// Locates the `roze` compiler binary, trying (in order):
/// 1. Next to whichever binary is currently running (the layout for an
///    installed toolchain, where all `roze*` binaries ship side by side).
/// 2. On `PATH`.
/// 3. Walking up from (and including) the current directory, looking for
///    `target/release/roze` or `target/debug/roze` -- useful when running
///    a tool from inside a checkout of this repo during development,
///    before anything's been installed anywhere.
pub fn find_roze_binary() -> Result<PathBuf> {
    let exe_name = if cfg!(windows) { "roze.exe" } else { "roze" };

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(exe_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }

    if let Ok(path) = which::which("roze") {
        return Ok(path);
    }

    let current_dir = std::env::current_dir()?;
    if let Some(found) = search_upward_for_binary(&current_dir, exe_name) {
        return Ok(found);
    }

    Err(anyhow!(
        "Could not find the Roze compiler ('{}'). Make sure it's on your PATH, \
         installed alongside this tool, or built at target/release/{} \
         somewhere at or above the current directory.",
        exe_name, exe_name
    ))
}

/// Walks up from (and including) `start`, looking for
/// `target/release/<exe_name>` or `target/debug/<exe_name>`. Pure and
/// side-effect-free (aside from filesystem existence checks), separated
/// out specifically so it's unit-testable without needing to mutate the
/// process's actual current directory (unsafe to do from parallel tests).
pub fn search_upward_for_binary(start: &Path, exe_name: &str) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        for profile in ["release", "debug"] {
            let candidate = dir.join("target").join(profile).join(exe_name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_binary_in_the_starting_directory_itself() {
        let base = std::env::temp_dir().join("roze_toolchain_test_current_dir");
        let _ = fs::remove_dir_all(&base);
        let release_dir = base.join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        let exe = release_dir.join("roze");
        fs::write(&exe, "fake").unwrap();

        // The bug this guards against: an earlier search in this spot
        // moved to the parent before checking anything, so it could
        // never find a binary sitting in the starting directory itself.
        assert_eq!(search_upward_for_binary(&base, "roze"), Some(exe));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn finds_binary_in_an_ancestor_directory() {
        let base = std::env::temp_dir().join("roze_toolchain_test_ancestor");
        let _ = fs::remove_dir_all(&base);
        let nested = base.join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        let release_dir = base.join("target").join("release");
        fs::create_dir_all(&release_dir).unwrap();
        let exe = release_dir.join("roze");
        fs::write(&exe, "fake").unwrap();

        assert_eq!(search_upward_for_binary(&nested, "roze"), Some(exe));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn returns_none_when_nothing_found() {
        let base = std::env::temp_dir().join("roze_toolchain_test_nothing_found");
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();

        assert_eq!(search_upward_for_binary(&base, "definitely_not_a_real_compiler"), None);

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn prefers_release_over_debug() {
        let base = std::env::temp_dir().join("roze_toolchain_test_prefers_release");
        let _ = fs::remove_dir_all(&base);
        let release_dir = base.join("target").join("release");
        let debug_dir = base.join("target").join("debug");
        fs::create_dir_all(&release_dir).unwrap();
        fs::create_dir_all(&debug_dir).unwrap();
        fs::write(release_dir.join("roze"), "fake").unwrap();
        fs::write(debug_dir.join("roze"), "fake").unwrap();

        assert_eq!(search_upward_for_binary(&base, "roze"), Some(release_dir.join("roze")));

        fs::remove_dir_all(&base).unwrap();
    }
}
