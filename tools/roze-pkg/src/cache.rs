// tools/roze-pkg/src/cache.rs
//
// Global immutable package cache (~/.bsr/cache/).
// Avoids repeated network downloads and provides cryptographic integrity verification,
// hash validation, and tamper detection across air-gapped enterprise environments.

use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::crypto::{sha256_dir, sha256_hex};
use crate::lockfile::normalize_checksum;

#[derive(Debug, Clone)]
pub struct CachedPackageInfo {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub checksum: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheStatus {
    Valid,
    Corrupted { expected: String, actual: String },
    Unchecksummed { actual: String },
}

#[derive(Debug, Clone)]
pub struct CacheVerificationReport {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub status: CacheStatus,
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
        let root = match std::env::var("BSR_CACHE_DIR") {
            Ok(custom) => PathBuf::from(custom),
            Err(_) => match std::env::var("ROZE_CACHE_DIR") {
                Ok(custom) => PathBuf::from(custom),
                Err(_) => {
                    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                    let bsr_cache = home.join(".bsr").join("cache");
                    let roze_cache = home.join(".roze").join("cache");
                    if !bsr_cache.exists() && roze_cache.exists() {
                        roze_cache
                    } else {
                        bsr_cache
                    }
                }
            },
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

    pub fn verify_package(&self, name: &str, version: &str, expected_checksum: &str) -> bool {
        let dir = self.package_dir(name, version);
        if !dir.exists() {
            return false;
        }
        match sha256_dir(&dir) {
            Ok(actual) => normalize_checksum(&actual) == normalize_checksum(expected_checksum),
            Err(_) => false,
        }
    }

    /// Stores the contents of `source_dir` into the cache under `<name>/<version>`.
    /// Enforces immutability and returns the SHA-256 tree hash of the stored package.
    pub fn store_package(&self, name: &str, version: &str, source_dir: &Path) -> Result<String> {
        let target_dir = self.package_dir(name, version);
        if target_dir.exists() {
            make_dir_writable(&target_dir)?;
            fs::remove_dir_all(&target_dir)?;
        }
        fs::create_dir_all(&target_dir)?;

        copy_dir_recursive(source_dir, &target_dir)?;

        let checksum = sha256_dir(&target_dir)?;
        // Write .checksum metadata file
        fs::write(target_dir.join(".checksum"), &checksum)?;

        // Enforce cache immutability: mark files read-only
        set_dir_immutable(&target_dir)?;

        Ok(checksum)
    }

    /// Installs a cached package into `dest_dir`, strictly verifying its cryptographic hash before copying.
    /// Fails with a supply-chain security error if the cached package is corrupted or has been tampered with.
    pub fn install_from_cache(&self, name: &str, version: &str, dest_dir: &Path) -> Result<String> {
        let cache_dir = self.package_dir(name, version);
        if !cache_dir.exists() {
            return Err(anyhow!(
                "Package {}@{} is not cached in global cache ({})",
                name, version, self.root_dir.display()
            ));
        }

        // Hash validation: verify cache integrity against stored .checksum metadata
        let actual_checksum = sha256_dir(&cache_dir)?;
        let checksum_file = cache_dir.join(".checksum");
        if checksum_file.exists() {
            let expected_checksum = fs::read_to_string(&checksum_file)?.trim().to_string();
            if normalize_checksum(&actual_checksum) != normalize_checksum(&expected_checksum) {
                return Err(anyhow!(
                    "Supply-chain security violation: Global cache integrity check failed for '{}' v{} in {}!\n  Expected SHA-256: {}\n  Computed SHA-256: {}\n  The cached package files appear to have been tampered with or corrupted.",
                    name, version, self.root_dir.display(), expected_checksum, actual_checksum
                ));
            }
        }

        if dest_dir.exists() {
            make_dir_writable(dest_dir)?;
            fs::remove_dir_all(dest_dir)?;
        }
        fs::create_dir_all(dest_dir)?;

        copy_dir_recursive(&cache_dir, dest_dir)?;

        // Ensure installed package files are writable for the project
        make_dir_writable(dest_dir)?;

        // Remove cache-internal metadata if present in target
        let meta = dest_dir.join(".checksum");
        if meta.exists() {
            let _ = fs::remove_file(meta);
        }

        Ok(actual_checksum)
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

    /// Cryptographically verifies all cached packages in the global cache.
    pub fn verify_all_packages(&self) -> Result<Vec<CacheVerificationReport>> {
        let mut reports = Vec::new();
        let pkg_root = self.packages_dir();
        if !pkg_root.exists() {
            return Ok(reports);
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

                let actual = sha256_dir(&path)?;
                let status = match fs::read_to_string(path.join(".checksum")) {
                    Ok(expected) => {
                        let exp = expected.trim().to_string();
                        if normalize_checksum(&actual) == normalize_checksum(&exp) {
                            CacheStatus::Valid
                        } else {
                            CacheStatus::Corrupted {
                                expected: exp,
                                actual,
                            }
                        }
                    }
                    Err(_) => CacheStatus::Unchecksummed { actual },
                };

                reports.push(CacheVerificationReport {
                    name: name.clone(),
                    version,
                    path,
                    status,
                });
            }
        }

        reports.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.version.cmp(&b.version)));
        Ok(reports)
    }

    /// Deletes all cached packages and checkouts. Returns total bytes cleared.
    pub fn clean_cache(&self) -> Result<u64> {
        let total_size = dir_size(&self.root_dir).unwrap_or(0);
        if self.root_dir.exists() {
            make_dir_writable(&self.root_dir)?;
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

/// Recursively sets all files in directory to read-only to guarantee immutability.
pub fn set_dir_immutable(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let metadata = entry.metadata()?;
            let mut perms = metadata.permissions();
            perms.set_readonly(true);
            let _ = fs::set_permissions(entry.path(), perms);
        }
    }
    Ok(())
}

/// Recursively restores write permissions to files in directory for modification/deletion.
pub fn make_dir_writable(dir: &Path) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let mut perms = metadata.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = fs::set_permissions(entry.path(), perms);
    }
    Ok(())
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
        let installed_checksum = cache.install_from_cache("my_pkg", "1.0.0", &install_target).unwrap();
        assert_eq!(normalize_checksum(&installed_checksum), normalize_checksum(&checksum));

        let installed_content = fs::read_to_string(install_target.join("lib.roze")).unwrap();
        assert_eq!(installed_content, "func cached_fn() -> int { return 42; }");

        let cached_list = cache.list_cached_packages().unwrap();
        assert_eq!(cached_list.len(), 1);
        assert_eq!(cached_list[0].name, "my_pkg");
        assert_eq!(cached_list[0].version, "1.0.0");

        // Verify all packages
        let reports = cache.verify_all_packages().unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].status, CacheStatus::Valid);

        let freed = cache.clean_cache().unwrap();
        assert!(freed > 0);
        assert!(!cache.has_package("my_pkg", "1.0.0"));
    }

    #[test]
    fn test_cache_tamper_detection_in_cache() {
        let temp_root = tempfile::tempdir().unwrap();
        let cache = PackageCache::with_root(temp_root.path().join("cache"));

        let src = tempfile::tempdir().unwrap();
        fs::write(src.path().join("lib.roze"), b"func safe() {}").unwrap();

        cache.store_package("tampered_pkg", "1.0.0", src.path()).unwrap();

        // Tamper with the package inside the cache
        let pkg_dir = cache.package_dir("tampered_pkg", "1.0.0");
        make_dir_writable(&pkg_dir).unwrap();
        fs::write(pkg_dir.join("lib.roze"), b"// MALICIOUS CACHE INJECTION").unwrap();

        // 1. verify_all_packages detects corruption
        let reports = cache.verify_all_packages().unwrap();
        assert_eq!(reports.len(), 1);
        match &reports[0].status {
            CacheStatus::Corrupted { .. } => {}
            other => panic!("Expected Corrupted, got {:?}", other),
        }

        // 2. install_from_cache aborts with security violation
        let dest = tempfile::tempdir().unwrap();
        let res = cache.install_from_cache("tampered_pkg", "1.0.0", &dest.path().join("target"));
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Supply-chain security violation"));
    }
}
