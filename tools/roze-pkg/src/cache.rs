// tools/roze-pkg/src/cache.rs
//
// Global package and git checkout caching.
// Avoids repeated network downloads and speeds up compilation by maintaining a local
// store of downloaded and built package artifacts.

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::crypto::{sha256_dir, sha256_hex};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CachedPackageInfo {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub checksum: String,
    pub size_bytes: u64,
}

pub struct PackageCache {
    pub root_dir: PathBuf,
}

impl Default for PackageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageCache {
    pub fn new() -> Self {
        let root = match std::env::var("ROZE_CACHE_DIR") {
            Ok(custom) => PathBuf::from(custom),
            Err(_) => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".roze").join("cache")
            }
        };
        Self { root_dir: root }
    }

    pub fn with_root(root: PathBuf) -> Self {
        Self { root_dir: root }
    }

    pub fn packages_dir(&self) -> PathBuf {
        self.root_dir.join("packages")
    }

    pub fn git_dir(&self) -> PathBuf {
        self.root_dir.join("git")
    }

    pub fn index_dir(&self) -> PathBuf {
        self.root_dir.join("index")
    }

    pub fn package_dir(&self, name: &str, version: &str) -> PathBuf {
        // Sanitize name for filesystem (replace '::' or '/' with '_')
        let safe_name = name.replace("::", "_").replace('/', "_");
        self.packages_dir().join(safe_name).join(version)
    }

    pub fn has_package(&self, name: &str, version: &str) -> bool {
        let dir = self.package_dir(name, version);
        dir.exists() && dir.is_dir()
    }

    #[allow(dead_code)]
    pub fn verify_package(&self, name: &str, version: &str, expected_checksum: &str) -> bool {
        let dir = self.package_dir(name, version);
        if !dir.exists() {
            return false;
        }
        match sha256_dir(&dir) {
            Ok(actual) => actual == expected_checksum,
            Err(_) => false,
        }
    }

    /// Stores the contents of `source_dir` into the cache under `<name>/<version>`.
    /// Returns the SHA-256 tree hash of the stored package.
    pub fn store_package(&self, name: &str, version: &str, source_dir: &Path) -> Result<String> {
        let target_dir = self.package_dir(name, version);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)?;
        }
        fs::create_dir_all(&target_dir)?;

        copy_dir_recursive(source_dir, &target_dir)?;

        let checksum = sha256_dir(&target_dir)?;
        // Write .checksum metadata file
        fs::write(target_dir.join(".checksum"), &checksum)?;

        Ok(checksum)
    }

    /// Installs a cached package into `dest_dir`.
    pub fn install_from_cache(&self, name: &str, version: &str, dest_dir: &Path) -> Result<()> {
        let cache_dir = self.package_dir(name, version);
        if !cache_dir.exists() {
            return Err(anyhow!("Package {}@{} is not cached", name, version));
        }

        if dest_dir.exists() {
            fs::remove_dir_all(dest_dir)?;
        }
        fs::create_dir_all(dest_dir)?;

        copy_dir_recursive(&cache_dir, dest_dir)?;

        // Remove cache-internal metadata if present
        let meta = dest_dir.join(".checksum");
        if meta.exists() {
            let _ = fs::remove_file(meta);
        }

        Ok(())
    }

    /// Returns a list of all cached packages.
    pub fn list_cached_packages(&self) -> Result<Vec<CachedPackageInfo>> {
        let mut list = Vec::new();
        let pkg_root = self.packages_dir();
        if !pkg_root.exists() {
            return Ok(list);
        }

        for pkg_entry in fs::read_dir(&pkg_root)? {
            let pkg_entry = pkg_entry?;
            if !pkg_entry.file_type()?.is_dir() {
                continue;
            }
            let name = pkg_entry.file_name().to_string_lossy().to_string();

            for ver_entry in fs::read_dir(pkg_entry.path())? {
                let ver_entry = ver_entry?;
                if !ver_entry.file_type()?.is_dir() {
                    continue;
                }
                let version = ver_entry.file_name().to_string_lossy().to_string();
                let path = ver_entry.path();

                let checksum = match fs::read_to_string(path.join(".checksum")) {
                    Ok(c) => c.trim().to_string(),
                    Err(_) => sha256_dir(&path).unwrap_or_default(),
                };

                let size_bytes = dir_size(&path).unwrap_or(0);

                list.push(CachedPackageInfo {
                    name: name.clone(),
                    version,
                    path,
                    checksum,
                    size_bytes,
                });
            }
        }

        list.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));
        Ok(list)
    }

    /// Deletes all cached packages and checkouts. Returns total bytes cleared.
    pub fn clean_cache(&self) -> Result<u64> {
        let total_size = dir_size(&self.root_dir).unwrap_or(0);
        if self.root_dir.exists() {
            fs::remove_dir_all(&self.root_dir)?;
        }
        fs::create_dir_all(self.packages_dir())?;
        fs::create_dir_all(self.git_dir())?;
        fs::create_dir_all(self.index_dir())?;
        Ok(total_size)
    }

    pub fn git_checkout_dir(&self, url: &str, rev_or_tag: Option<&str>) -> PathBuf {
        let url_hash = sha256_hex(url.as_bytes());
        let safe_rev = rev_or_tag.unwrap_or("HEAD").replace('/', "_");
        self.git_dir().join(&url_hash[..16]).join(safe_rev)
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);

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

fn dir_size(path: &Path) -> Result<u64> {
    let mut total = 0;
    if !path.exists() {
        return Ok(0);
    }
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_store_and_install() {
        let temp_root = tempfile::tempdir().unwrap();
        let cache = PackageCache::with_root(temp_root.path().join("cache"));

        let src = tempfile::tempdir().unwrap();
        fs::write(src.path().join("lib.roze"), b"func cached_fn() -> int { return 42; }").unwrap();

        let checksum = cache.store_package("my_pkg", "1.0.0", src.path()).unwrap();
        assert!(!checksum.is_empty());
        assert!(cache.has_package("my_pkg", "1.0.0"));
        assert!(cache.verify_package("my_pkg", "1.0.0", &checksum));

        let dest = tempfile::tempdir().unwrap();
        let install_target = dest.path().join("installed");
        cache.install_from_cache("my_pkg", "1.0.0", &install_target).unwrap();

        let installed_content = fs::read_to_string(install_target.join("lib.roze")).unwrap();
        assert_eq!(installed_content, "func cached_fn() -> int { return 42; }");

        let cached_list = cache.list_cached_packages().unwrap();
        assert_eq!(cached_list.len(), 1);
        assert_eq!(cached_list[0].name, "my_pkg");
        assert_eq!(cached_list[0].version, "1.0.0");

        let freed = cache.clean_cache().unwrap();
        assert!(freed > 0);
        assert!(!cache.has_package("my_pkg", "1.0.0"));
    }
}
