// tools/roze-pkg/src/git.rs
//
// Git repository dependency fetcher and checkout manager.
// Supports branch, tag, and exact revision pinning with local checkout caching.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Result};
use crate::cache::PackageCache;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDependency {
    pub url: String,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FetchedGitPackage {
    #[allow(dead_code)]
    pub name: String,
    pub version: String,
    pub commit_hash: String,
    pub checkout_dir: PathBuf,
}

pub struct GitFetcher {
    cache: PackageCache,
}

impl Default for GitFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl GitFetcher {
    pub fn new() -> Self {
        Self {
            cache: PackageCache::new(),
        }
    }

    pub fn with_cache(cache: PackageCache) -> Self {
        Self { cache }
    }

    /// Fetches the specified git dependency into the cache and returns its details.
    pub fn fetch(&self, dep: &GitDependency) -> Result<FetchedGitPackage> {
        let pin = dep.rev.as_deref()
            .or(dep.tag.as_deref())
            .or(dep.branch.as_deref())
            .unwrap_or("HEAD");

        let target_dir = self.cache.git_checkout_dir(&dep.url, Some(pin));

        if !target_dir.exists() || !target_dir.join(".git").exists() {
            if target_dir.exists() {
                let _ = fs::remove_dir_all(&target_dir);
            }
            fs::create_dir_all(&target_dir)?;

            // Clone repository
            println!("  Cloning {} ...", dep.url);
            let mut clone_cmd = Command::new("git");
            clone_cmd.arg("clone").arg(&dep.url).arg(&target_dir);
            let status = clone_cmd.status().map_err(|e| {
                anyhow!("Failed to run git command (is git installed?): {}", e)
            })?;

            if !status.success() {
                return Err(anyhow!("Failed to clone git repository from {}", dep.url));
            }

            // Checkout specific tag, branch, or rev if requested
            if let Some(ref rev) = dep.rev {
                Self::run_git_checkout(&target_dir, rev)?;
            } else if let Some(ref tag) = dep.tag {
                Self::run_git_checkout(&target_dir, &format!("tags/{}", tag))?;
            } else if let Some(ref branch) = dep.branch {
                Self::run_git_checkout(&target_dir, branch)?;
            }
        }

        // Get exact commit hash
        let commit_hash = Self::get_commit_hash(&target_dir)?;

        // Inspect roze.toml inside the repo if present
        let manifest_path = target_dir.join("roze.toml");
        let (name, version) = if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path)?;
            let doc: toml::Value = toml::from_str(&content)?;
            let n = doc.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed").to_string();
            let v = doc.get("version").and_then(|v| v.as_str()).unwrap_or("0.1.0").to_string();
            (n, v)
        } else {
            let repo_name = dep.url
                .trim_end_matches('/')
                .trim_end_matches(".git")
                .rsplit('/')
                .next()
                .unwrap_or("pkg")
                .to_string();
            let v = dep.tag.as_deref().unwrap_or("0.1.0").trim_start_matches('v').to_string();
            (repo_name, v)
        };

        Ok(FetchedGitPackage {
            name,
            version,
            commit_hash,
            checkout_dir: target_dir,
        })
    }

    fn run_git_checkout(repo_dir: &Path, target: &str) -> Result<()> {
        let status = Command::new("git")
            .current_dir(repo_dir)
            .args(["checkout", target])
            .status()
            .map_err(|e| anyhow!("Failed to execute git checkout: {}", e))?;

        if !status.success() {
            return Err(anyhow!("Failed to checkout '{}' in repository {}", target, repo_dir.display()));
        }
        Ok(())
    }

    pub fn get_commit_hash(repo_dir: &Path) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .map_err(|e| anyhow!("Failed to get commit hash: {}", e))?;

        if !output.status.success() {
            return Err(anyhow!("git rev-parse HEAD failed"));
        }
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(hash)
    }

    /// Copies the package files from the checked out git repo into the project's destination dir
    pub fn install_to_project(&self, fetched: &FetchedGitPackage, dest_dir: &Path) -> Result<()> {
        if dest_dir.exists() {
            fs::remove_dir_all(dest_dir)?;
        }
        fs::create_dir_all(dest_dir)?;

        for entry in walkdir::WalkDir::new(&fetched.checkout_dir) {
            let entry = entry?;
            let rel = entry.path().strip_prefix(&fetched.checkout_dir)?;

            // Skip .git directory
            if rel.starts_with(".git") {
                continue;
            }

            let target = dest_dir.join(rel);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target)?;
            } else if entry.file_type().is_file() {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(entry.path(), &target)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_local_repo_fetch() {
        // Initialize a small local git repo to test git fetching without network
        let temp_repo = tempfile::tempdir().unwrap();
        let repo_path = temp_repo.path();

        let _ = Command::new("git").current_dir(repo_path).args(["init"]).status();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.email", "roze@dev"]).status();
        let _ = Command::new("git").current_dir(repo_path).args(["config", "user.name", "Roze Test"]).status();

        fs::write(repo_path.join("roze.toml"), "name = \"git_test_pkg\"\nversion = \"1.2.3\"\nmain = \"lib.roze\"\n").unwrap();
        fs::write(repo_path.join("lib.roze"), "func git_hello() -> string { return \"git!\"; }").unwrap();

        let _ = Command::new("git").current_dir(repo_path).args(["add", "."]).status();
        let _ = Command::new("git").current_dir(repo_path).args(["commit", "-m", "Initial commit"]).status();
        let _ = Command::new("git").current_dir(repo_path).args(["tag", "v1.2.3"]).status();

        let temp_cache_dir = tempfile::tempdir().unwrap();
        let cache = PackageCache::with_root(temp_cache_dir.path().join("cache"));
        let fetcher = GitFetcher::with_cache(cache);

        let dep = GitDependency {
            url: repo_path.display().to_string(),
            tag: Some("v1.2.3".to_string()),
            branch: None,
            rev: None,
        };

        let fetched = fetcher.fetch(&dep).unwrap();
        assert_eq!(fetched.name, "git_test_pkg");
        assert_eq!(fetched.version, "1.2.3");
        assert_eq!(fetched.commit_hash.len(), 40);

        let dest = tempfile::tempdir().unwrap();
        let dest_lib = dest.path().join("libs").join("git_test_pkg");
        fetcher.install_to_project(&fetched, &dest_lib).unwrap();

        assert!(dest_lib.join("lib.roze").exists());
        let content = fs::read_to_string(dest_lib.join("lib.roze")).unwrap();
        assert_eq!(content, "func git_hello() -> string { return \"git!\"; }");
    }
}
