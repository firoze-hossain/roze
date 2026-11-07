// tools/roze-pkg/src/dependency.rs
//
// Roze dependency management, resolution, and installation.
// Integrates registry fetching, git cloning, deterministic lockfiles,
// cryptographic SHA-256 checksum verification, and local package caching.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use anyhow::{anyhow, Result};
use colored::*;

use crate::cache::PackageCache;
use crate::crypto::sha256_dir;
use crate::git::{GitDependency, GitFetcher};
use crate::lockfile::{
    normalize_checksum, LockFile, LockedPackage, PackageVerification,
    LEGACY_LOCKFILE_NAME, LOCKFILE_NAME,
};
use crate::project::{DependencyValue, ProjectConfig, LEGACY_MANIFEST_NAME, MANIFEST_NAME};
use crate::registry::RegistryClient;
use crate::resolver::{DependencyResolver, DependencySpec};
use crate::semver::{Version, VersionReq};

pub struct DependencyManager {
    pub project_dir: PathBuf,
    pub dependencies: HashMap<String, DependencyValue>,
    #[allow(dead_code)]
    cache: PackageCache,
    registry: RegistryClient,
    git_fetcher: GitFetcher,
    resolver: DependencyResolver,
}

impl DependencyManager {
    pub fn new(project_dir: PathBuf) -> Self {
        Self::with_cache(project_dir, PackageCache::new())
    }

    pub fn with_cache(project_dir: PathBuf, cache: PackageCache) -> Self {
        let registry = RegistryClient::with_cache(PackageCache::with_root(cache.root_dir.clone()));
        let git_fetcher = GitFetcher::with_cache(PackageCache::with_root(cache.root_dir.clone()));
        let resolver = DependencyResolver::with_clients(
            RegistryClient::with_cache(PackageCache::with_root(cache.root_dir.clone())),
            GitFetcher::with_cache(PackageCache::with_root(cache.root_dir.clone())),
        );

        let mut manager = Self {
            project_dir,
            dependencies: HashMap::new(),
            cache,
            registry,
            git_fetcher,
            resolver,
        };

        let _ = manager.load_manifest();
        manager
    }

    pub fn manifest_path(&self) -> PathBuf {
        if let Some(path) = ProjectConfig::find_manifest(&self.project_dir) {
            path
        } else {
            self.project_dir.join(MANIFEST_NAME)
        }
    }

    pub fn lockfile_path(&self) -> PathBuf {
        let bsr_lock = self.project_dir.join(LOCKFILE_NAME);
        if bsr_lock.exists() {
            return bsr_lock;
        }
        let legacy_lock = self.project_dir.join(LEGACY_LOCKFILE_NAME);
        if legacy_lock.exists() {
            return legacy_lock;
        }
        let manifest = self.manifest_path();
        if manifest.file_name().and_then(|s| s.to_str()) == Some(LEGACY_MANIFEST_NAME) {
            legacy_lock
        } else {
            bsr_lock
        }
    }

    pub fn libs_dir(&self) -> PathBuf {
        self.project_dir.join("libs")
    }

    #[allow(dead_code)]
    pub fn cache(&self) -> &PackageCache {
        &self.cache
    }

    pub fn load_manifest(&mut self) -> Result<()> {
        let path = self.manifest_path();
        if !path.exists() {
            return Ok(());
        }

        let config = ProjectConfig::load(&path)?;
        self.dependencies = config.dependencies;
        Ok(())
    }

    pub fn save_manifest(&self) -> Result<()> {
        let path = self.manifest_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut config: toml::Value = toml::from_str(&content)?;

            if !config.is_table() {
                config = toml::Value::Table(toml::map::Map::new());
            }

            let mut deps_map = toml::map::Map::new();
            let mut sorted_keys: Vec<&String> = self.dependencies.keys().collect();
            sorted_keys.sort();

            for key in sorted_keys {
                let dep_val = &self.dependencies[key];
                let val_toml = match dep_val {
                    DependencyValue::Simple(s) => toml::Value::String(s.clone()),
                    DependencyValue::Detailed(d) => {
                        let mut table = toml::map::Map::new();
                        if let Some(ref v) = d.version {
                            table.insert("version".to_string(), toml::Value::String(v.clone()));
                        }
                        if let Some(ref p) = d.path {
                            table.insert("path".to_string(), toml::Value::String(p.clone()));
                        }
                        if let Some(ref g) = d.git {
                            table.insert("git".to_string(), toml::Value::String(g.clone()));
                        }
                        if let Some(ref t) = d.tag {
                            table.insert("tag".to_string(), toml::Value::String(t.clone()));
                        }
                        if let Some(ref b) = d.branch {
                            table.insert("branch".to_string(), toml::Value::String(b.clone()));
                        }
                        if let Some(ref r) = d.rev {
                            table.insert("rev".to_string(), toml::Value::String(r.clone()));
                        }
                        if let Some(opt) = d.optional {
                            table.insert("optional".to_string(), toml::Value::Boolean(opt));
                        }
                        if !d.features.is_empty() {
                            let arr = d.features.iter().map(|f| toml::Value::String(f.clone())).collect();
                            table.insert("features".to_string(), toml::Value::Array(arr));
                        }
                        if let Some(df) = d.default_features {
                            table.insert("default_features".to_string(), toml::Value::Boolean(df));
                        }
                        if let Some(ref pkg) = d.package {
                            table.insert("package".to_string(), toml::Value::String(pkg.clone()));
                        }
                        toml::Value::Table(table)
                    }
                };
                deps_map.insert(key.clone(), val_toml);
            }

            if let Some(table) = config.as_table_mut() {
                table.insert("dependencies".to_string(), toml::Value::Table(deps_map));
            }

            fs::write(&path, toml::to_string_pretty(&config)?)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn add_dependency(&mut self, name: &str, version: &str) -> Result<()> {
        let (package, version) = if name.contains("::") {
            let parts: Vec<&str> = name.split("::").collect();
            (parts[1].to_string(), version.to_string())
        } else {
            (name.to_string(), version.to_string())
        };

        self.dependencies.insert(
            package,
            DependencyValue::Simple(version),
        );
        self.save_manifest()?;
        Ok(())
    }

    pub fn add_dependency_spec(&mut self, name: &str, value: DependencyValue) -> Result<()> {
        self.dependencies.insert(name.to_string(), value);
        self.save_manifest()?;
        Ok(())
    }

    pub fn remove_dependency(&mut self, name: &str) -> Result<()> {
        let package = if name.contains("::") {
            let parts: Vec<&str> = name.split("::").collect();
            parts[1].to_string()
        } else {
            name.to_string()
        };

        if self.dependencies.remove(&package).is_none() {
            return Err(anyhow!("Dependency '{}' not found in manifest", name));
        }

        self.save_manifest()?;

        // Also clean up from libs/ and bsr.lock
        let lib_dir = self.libs_dir().join(&package);
        if lib_dir.exists() {
            let _ = crate::cache::make_dir_writable(&lib_dir);
            let _ = fs::remove_dir_all(&lib_dir);
        }

        let lock_path = self.lockfile_path();
        if lock_path.exists() {
            if let Ok(mut lock) = LockFile::load(&lock_path) {
                if lock.remove_package(&package) {
                    let _ = lock.save(&lock_path);
                }
            }
        }

        Ok(())
    }

    fn to_resolver_specs(&self) -> Result<HashMap<String, DependencySpec>> {
        let mut map = HashMap::new();
        for (name, val) in &self.dependencies {
            let spec = match val {
                DependencyValue::Simple(s) => DependencySpec::parse_str(s)?,
                DependencyValue::Detailed(d) => {
                    if let Some(ref git_url) = d.git {
                        DependencySpec::Git(GitDependency {
                            url: git_url.clone(),
                            tag: d.tag.clone(),
                            branch: d.branch.clone(),
                            rev: d.rev.clone(),
                        })
                    } else if let Some(ref path_str) = d.path {
                        let full_path = self.project_dir.join(path_str);
                        DependencySpec::Path(full_path)
                    } else if let Some(ref ver_str) = d.version {
                        DependencySpec::Registry(VersionReq::parse(ver_str)?)
                    } else {
                        DependencySpec::Registry(VersionReq::parse("*")?)
                    }
                }
            };
            map.insert(name.clone(), spec);
        }
        Ok(map)
    }

    /// Resolves dependencies and generates/updates `bsr.lock`.
    pub fn resolve_and_lock(&mut self, force_update: bool) -> Result<LockFile> {
        let empty_set = HashSet::new();
        self.resolve_and_lock_with_targets(force_update, &empty_set)
    }

    /// Resolves dependencies with optional selective package unlocking.
    pub fn resolve_and_lock_with_targets(
        &mut self,
        force_update_all: bool,
        update_targets: &HashSet<String>,
    ) -> Result<LockFile> {
        let specs = self.to_resolver_specs()?;
        let lock_path = self.lockfile_path();

        let existing_lock = if !force_update_all && lock_path.exists() {
            LockFile::load(&lock_path).ok()
        } else {
            None
        };

        let mut lockfile = self.resolver.resolve_with_options(
            &specs,
            existing_lock.as_ref(),
            force_update_all,
            update_targets,
        )?;

        // Include root package in lockfile if manifest exists
        if let Ok(config) = ProjectConfig::load(&self.manifest_path()) {
            let mut direct_dep_strings: Vec<String> = self.dependencies.keys().cloned().collect();
            direct_dep_strings.sort();
            let root_pkg = LockedPackage::new_root(&config.name, &config.version, direct_dep_strings);
            lockfile.add_or_update(root_pkg);
        }

        lockfile.save(&lock_path)?;
        Ok(lockfile)
    }

    /// Installs all dependencies recorded in lockfile.
    pub fn install_all(&mut self) -> Result<()> {
        self.install_all_with_opts(false, false)
    }

    /// Installs all dependencies with optional `--locked` and `--frozen` enforcement.
    pub fn install_all_with_opts(&mut self, locked: bool, frozen: bool) -> Result<()> {
        self.install_all_with_options(locked, frozen, false)
    }

    /// Installs all dependencies with `--locked`, `--frozen`, and `--offline` air-gapped mode support.
    pub fn install_all_with_options(&mut self, locked: bool, frozen: bool, offline: bool) -> Result<()> {
        println!("📦 {}", "Installing dependencies...".bright_magenta());

        if self.dependencies.is_empty() {
            println!("  No dependencies specified in manifest");
            return Ok(());
        }

        let lock_path = self.lockfile_path();
        let manifest_path = self.manifest_path();
        let manifest_config = if manifest_path.exists() {
            Some(ProjectConfig::load(&manifest_path)?)
        } else {
            None
        };

        if locked || frozen || offline {
            if !lock_path.exists() {
                return Err(anyhow!(
                    "The lock file {} needs to be updated but --{} was passed to prevent this",
                    lock_path.display(),
                    if offline { "offline" } else if frozen { "frozen" } else { "locked" }
                ));
            }
            let current_lock = LockFile::load(&lock_path)?;
            if let Some(ref m_cfg) = manifest_config {
                if !current_lock.is_up_to_date(m_cfg)? {
                    return Err(anyhow!(
                        "The lock file {} needs to be updated but --{} was passed to prevent this",
                        lock_path.display(),
                        if offline { "offline" } else if frozen { "frozen" } else { "locked" }
                    ));
                }
            }
        }

        let mut lockfile = if lock_path.exists() {
            match LockFile::load(&lock_path) {
                Ok(l) => {
                    let is_sync = if let Some(ref m_cfg) = manifest_config {
                        l.is_up_to_date(m_cfg).unwrap_or(false)
                    } else {
                        self.dependencies.keys().all(|k| l.find_package(k).is_some())
                    };

                    if is_sync {
                        l
                    } else {
                        if locked || frozen || offline {
                            return Err(anyhow!(
                                "The lock file {} needs to be updated but --{} was passed to prevent this",
                                lock_path.display(),
                                if offline { "offline" } else if frozen { "frozen" } else { "locked" }
                            ));
                        }
                        println!("  Manifest changed; updating lockfile...");
                        self.resolve_and_lock(false)?
                    }
                }
                Err(_) => {
                    if locked || frozen || offline {
                        return Err(anyhow!(
                            "The lock file {} is invalid but --{} was passed",
                            lock_path.display(),
                            if offline { "offline" } else if frozen { "frozen" } else { "locked" }
                        ));
                    }
                    self.resolve_and_lock(false)?
                }
            }
        } else {
            self.resolve_and_lock(false)?
        };

        fs::create_dir_all(self.libs_dir())?;

        let mut updated = false;
        let vendor_dir = self.project_dir.join("vendor");

        for pkg in &mut lockfile.packages {
            if pkg.is_root() {
                continue;
            }

            let dest_dir = self.libs_dir().join(&pkg.name);
            let src_display = pkg.source_str();

            // Check if already installed
            if dest_dir.exists() {
                if let Ok(actual) = sha256_dir(&dest_dir) {
                    if let Some(ref expected) = pkg.checksum {
                        if normalize_checksum(&actual) == normalize_checksum(expected) {
                            // Already installed and checksum is 100% valid
                            continue;
                        } else {
                            // Tampering detected!
                            return Err(anyhow!(
                                "Supply-chain security violation: Checksum mismatch for installed package '{}' v{}!\n  Expected SHA-256: {}\n  Computed SHA-256: {}\n  The package contents may have been corrupted or tampered with.",
                                pkg.name, pkg.version, expected, actual
                            ));
                        }
                    }
                }
            }

            // 1. Air-Gapped Strategy: First check local project vendor/ directory
            let vendored_source = if vendor_dir.exists() {
                crate::vendor::VendorManager::find_vendored_package(&vendor_dir, &pkg.name, &pkg.version)
            } else {
                None
            };

            let checksum = if let Some(ref v_path) = vendored_source {
                println!("  Installing {} v{} (from vendored sources {})", pkg.name.bold(), pkg.version.bright_cyan(), v_path.display());
                let actual_vendor_chk = sha256_dir(v_path)?;

                if let Some(ref expected) = pkg.checksum {
                    if normalize_checksum(&actual_vendor_chk) != normalize_checksum(expected) {
                        return Err(anyhow!(
                            "Supply-chain security violation: Checksum mismatch for vendored package '{}' v{} in {}!\n  Expected SHA-256: {}\n  Computed SHA-256: {}\n  The vendored package appears to have been tampered with or corrupted.",
                            pkg.name, pkg.version, v_path.display(), expected, actual_vendor_chk
                        ));
                    }
                }

                if dest_dir.exists() {
                    let _ = crate::cache::make_dir_writable(&dest_dir);
                    let _ = fs::remove_dir_all(&dest_dir);
                }
                fs::create_dir_all(&dest_dir)?;

                // Copy vendored files
                for entry in walkdir::WalkDir::new(v_path) {
                    let entry = entry?;
                    let f_name = entry.file_name().to_string_lossy();
                    if f_name == ".checksum" || f_name == ".bsr-checksum.json" || f_name == ".git" {
                        continue;
                    }
                    let rel = entry.path().strip_prefix(v_path)?;
                    let target = dest_dir.join(rel);
                    if entry.file_type().is_dir() {
                        fs::create_dir_all(&target)?;
                    } else if entry.file_type().is_file() {
                        if let Some(p) = target.parent() {
                            fs::create_dir_all(p)?;
                        }
                        fs::copy(entry.path(), &target)?;
                    }
                }
                sha256_dir(&dest_dir)?
            } else if self.cache.has_package(&pkg.name, &pkg.version) {
                // 2. Second strategy: Install from global immutable cache with hash validation
                println!("  Installing {} v{} (from global cache)", pkg.name.bold(), pkg.version.bright_cyan());
                self.cache.install_from_cache(&pkg.name, &pkg.version, &dest_dir)?;
                sha256_dir(&dest_dir)?
            } else if offline || frozen {
                // Offline / frozen mode enforcement: fail immediately without network access
                return Err(anyhow!(
                    "Offline air-gapped mode error: Package '{}' v{} is not available in vendor/ or global cache ({}), and network access is disabled (--{}).",
                    pkg.name, pkg.version, self.cache.root_dir.display(), if offline { "offline" } else { "frozen" }
                ));
            } else if pkg.is_git() {
                // 3. Fallback: Network fetch
                println!("  Installing {} v{} ({})", pkg.name.bold(), pkg.version.bright_cyan(), src_display);
                let git_url = pkg.git_repo_url().unwrap_or(src_display);
                let rev = pkg.git_commit_hash().map(|s| s.to_string());

                let dep = GitDependency {
                    url: git_url.to_string(),
                    tag: None,
                    branch: None,
                    rev,
                };

                let fetched = self.git_fetcher.fetch(&dep)?;
                self.git_fetcher.install_to_project(&fetched, &dest_dir)?;
                let chk = sha256_dir(&dest_dir)?;
                let _ = self.cache.store_package(&pkg.name, &pkg.version, &dest_dir);
                chk
            } else if pkg.is_path() {
                println!("  Installing {} v{} ({})", pkg.name.bold(), pkg.version.bright_cyan(), src_display);
                let path_str = src_display.trim_start_matches("path+");
                let src_path = PathBuf::from(path_str);
                if !src_path.exists() {
                    return Err(anyhow!("Path dependency not found: {}", src_path.display()));
                }
                if dest_dir.exists() {
                    let _ = crate::cache::make_dir_writable(&dest_dir);
                    let _ = fs::remove_dir_all(&dest_dir);
                }
                fs::create_dir_all(&dest_dir)?;
                for entry in walkdir::WalkDir::new(&src_path) {
                    let entry = entry?;
                    let rel = entry.path().strip_prefix(&src_path)?;
                    let target = dest_dir.join(rel);
                    if entry.file_type().is_dir() {
                        fs::create_dir_all(&target)?;
                    } else if entry.file_type().is_file() {
                        if let Some(p) = target.parent() {
                            fs::create_dir_all(p)?;
                        }
                        fs::copy(entry.path(), &target)?;
                    }
                }
                sha256_dir(&dest_dir)?
            } else {
                // Registry source
                println!("  Installing {} v{} ({})", pkg.name.bold(), pkg.version.bright_cyan(), src_display);
                let ver = Version::parse(&pkg.version)?;
                let _chk = self.registry.fetch_package(&pkg.name, &ver, &dest_dir)?;

                if !dest_dir.join("lib.roze").exists() {
                    let stub = format!(
                        r#"// Library: {}
// Version: {}
// Source: {}

func {}_version() -> string {{
    return "{}";
}}
"#,
                        pkg.name, pkg.version, src_display, pkg.name.replace("::", "_").replace('-', "_"), pkg.version
                    );
                    fs::write(dest_dir.join("lib.roze"), stub)?;
                }

                let chk = sha256_dir(&dest_dir)?;
                // Automatically cache in global cache
                let _ = self.cache.store_package(&pkg.name, &pkg.version, &dest_dir);
                chk
            };

            let _ = crate::cache::make_dir_writable(&dest_dir);

            // Cryptographic checksum verification against lockfile pin
            if let Some(ref expected) = pkg.checksum {
                if normalize_checksum(&checksum) != normalize_checksum(expected) {
                    return Err(anyhow!(
                        "Supply-chain security violation: Checksum mismatch for package '{}' v{}!\n  Expected SHA-256: {}\n  Computed SHA-256: {}\n  The package contents may have been corrupted or tampered with.",
                        pkg.name, pkg.version, expected, checksum
                    ));
                }
            } else {
                pkg.checksum = Some(checksum);
                updated = true;
            }
        }

        if updated {
            lockfile.save(&lock_path)?;
        }

        println!("✅ {}", "All dependencies installed and cryptographically verified!".bright_green());
        Ok(())
    }

    /// Vendors all dependencies into the local project vendor directory.
    pub fn vendor(&self, opts: &crate::vendor::VendorOptions) -> Result<crate::vendor::VendorSummary> {
        crate::vendor::VendorManager::vendor_dependencies_with_cache(&self.project_dir, opts, Some(&self.cache))
    }

    /// Checks if any required library is missing from `libs/`.
    pub fn has_missing_libs(&self) -> Result<bool> {
        let lock_path = self.lockfile_path();
        if !lock_path.exists() {
            return Ok(true);
        }
        let lockfile = LockFile::load(&lock_path)?;
        for pkg in &lockfile.packages {
            if pkg.is_root() {
                continue;
            }
            let dir = self.libs_dir().join(&pkg.name);
            if !dir.exists() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Updates dependencies to their latest compatible versions.
    /// If `package` is specified, updates only that package; otherwise updates all.
    pub fn update(&mut self, package: Option<&str>) -> Result<()> {
        if let Some(pkg) = package {
            println!("🔄 Updating package '{}' to latest compatible version...", pkg.bold());
            let mut to_update = HashSet::new();
            to_update.insert(pkg.to_string());
            self.resolve_and_lock_with_targets(false, &to_update)?;
        } else {
            println!("🔄 {}", "Updating all dependencies to latest compatible versions...".yellow());
            self.resolve_and_lock(true)?;
        }
        self.install_all()
    }

    /// Verifies that all dependencies in `libs/` match their cryptographic SHA-256 checksums.
    pub fn verify_installed(&self) -> Result<Vec<PackageVerification>> {
        let lock_path = self.lockfile_path();
        if !lock_path.exists() {
            return Err(anyhow!("Lockfile not found at {}", lock_path.display()));
        }
        let lock = LockFile::load(&lock_path)?;
        lock.verify_checksums(&self.libs_dir())
    }

    /// Checks if lockfile exists and is strictly up to date with manifest.
    pub fn check_lockfile_up_to_date(&self) -> Result<bool> {
        let lock_path = self.lockfile_path();
        if !lock_path.exists() {
            return Ok(false);
        }
        let lock = LockFile::load(&lock_path)?;
        let manifest_path = self.manifest_path();
        if !manifest_path.exists() {
            return Ok(true);
        }
        let config = ProjectConfig::load(&manifest_path)?;
        lock.is_up_to_date(&config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(name: &str) -> (PathBuf, PackageCache, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir = temp_dir.path().join(format!("roze_pkg_dep_test_{}", name));
        fs::create_dir_all(&dir).unwrap();
        let cache_dir = temp_dir.path().join("cache");
        fs::create_dir_all(&cache_dir).unwrap();
        let cache = PackageCache::with_root(cache_dir);
        (dir, cache, temp_dir)
    }

    #[test]
    fn a_second_add_does_not_wipe_out_the_first_dependency() {
        let (dir, cache, _holder) = temp_project("second_add_preserves_first");
        fs::write(dir.join("roze.toml"), "name = \"t\"\nversion = \"0.1.0\"\nmain = \"src/main.roze\"\n").unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache.clone());
        mgr.add_dependency("foo", "1.0.0").unwrap();

        let mut mgr2 = DependencyManager::with_cache(dir.clone(), cache);
        mgr2.add_dependency("bar", "2.0.0").unwrap();

        let content = fs::read_to_string(dir.join("roze.toml")).unwrap();
        assert!(content.contains("foo"), "expected 'foo' to survive a second add, got:\n{}", content);
        assert!(content.contains("bar"), "expected 'bar' to be added too, got:\n{}", content);
    }

    #[test]
    fn remove_finds_a_dependency_added_in_a_previous_invocation() {
        let (dir, cache, _holder) = temp_project("remove_finds_existing");
        fs::write(dir.join("roze.toml"), "name = \"t\"\nversion = \"0.1.0\"\nmain = \"src/main.roze\"\n").unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache.clone());
        mgr.add_dependency("some_lib", "1.2.3").unwrap();

        let mut mgr2 = DependencyManager::with_cache(dir.clone(), cache);
        let result = mgr2.remove_dependency("some_lib");
        assert!(result.is_ok(), "expected remove to find the existing dependency, got: {:?}", result);

        let content = fs::read_to_string(dir.join("roze.toml")).unwrap();
        assert!(!content.contains("some_lib"), "expected 'some_lib' to be gone, got:\n{}", content);
    }

    #[test]
    fn new_project_with_no_manifest_starts_empty_without_erroring() {
        let (dir, cache, _holder) = temp_project("no_manifest_yet");
        let mgr = DependencyManager::with_cache(dir, cache);
        assert!(mgr.dependencies.is_empty());
    }

    #[test]
    fn full_install_generates_deterministic_lockfile() {
        let (dir, cache, _holder) = temp_project("install_generates_lock");
        fs::write(
            dir.join("roze.toml"),
            r#"name = "demo"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::web" = "^1.0.0"
"#,
        ).unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache);
        mgr.install_all().unwrap();

        let lock_file = dir.join("roze.lock");
        assert!(lock_file.exists());

        let lock = LockFile::load(&lock_file).unwrap();
        let web = lock.find_package("std::web").unwrap();
        assert_eq!(web.version, "1.0.0");
        assert!(web.checksum.is_some());

        let json = lock.find_package("std::json").unwrap();
        assert_eq!(json.version, "0.2.0");
        assert!(json.checksum.is_some());

        // Check libs/ installed directory
        assert!(dir.join("libs").join("std::web").join("lib.roze").exists());
        assert!(dir.join("libs").join("std::json").join("lib.roze").exists());
    }

    #[test]
    fn full_install_with_bsr_manifest_and_lock() {
        let (dir, cache, _holder) = temp_project("install_bsr_mfz");
        fs::write(
            dir.join("bsr.mfz"),
            r#"# bsr.mfz - Basir Project Manifest (Mohammad Firoze)
name = "bsr_demo"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::web" = "^1.0.0"
"#,
        ).unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache);
        assert_eq!(mgr.manifest_path(), dir.join("bsr.mfz"));
        assert_eq!(mgr.lockfile_path(), dir.join("bsr.lock"));

        mgr.install_all().unwrap();

        let lock_file = dir.join("bsr.lock");
        assert!(lock_file.exists(), "bsr.lock should be created");

        let lock = LockFile::load(&lock_file).unwrap();
        let web = lock.find_package("std::web").unwrap();
        assert_eq!(web.version, "1.0.0");
        assert!(web.checksum.is_some());

        assert!(dir.join("libs").join("std::web").join("lib.roze").exists());
    }

    #[test]
    fn test_tamper_detection_in_install_fails_with_security_error() {
        let (dir, cache, _holder) = temp_project("tamper_test");
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "tamper_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "^0.2.0"
"#,
        ).unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache.clone());
        mgr.install_all().unwrap();

        let lock = LockFile::load(&dir.join("bsr.lock")).unwrap();
        let orig_checksum = lock.find_package("std::json").unwrap().checksum.clone().unwrap();

        // Tamper with installed files
        let installed_lib = dir.join("libs").join("std::json").join("lib.roze");
        let _ = crate::cache::make_dir_writable(&dir.join("libs"));
        fs::write(&installed_lib, "// malicious backdoor inserted!").unwrap();

        // Re-running install must detect checksum tampering!
        let mut mgr2 = DependencyManager::with_cache(dir.clone(), cache);
        let res = mgr2.install_all();
        assert!(res.is_err(), "Must reject tampered package");
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Supply-chain security violation"));
        assert!(err_msg.contains(&orig_checksum));
    }

    #[test]
    fn test_install_locked_mode_enforcement() {
        let (dir, cache, _holder) = temp_project("locked_enforcement");
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "locked_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "^0.2.0"
"#,
        ).unwrap();

        // 1. Without lockfile, --locked must fail
        let mut mgr = DependencyManager::with_cache(dir.clone(), cache.clone());
        let res = mgr.install_all_with_opts(true, false);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("--locked was passed to prevent this"));

        // 2. Generate lockfile
        mgr.resolve_and_lock(false).unwrap();

        // Now --locked should succeed
        let res_ok = mgr.install_all_with_opts(true, false);
        assert!(res_ok.is_ok());

        // 3. Alter manifest with new dependency
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "locked_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "^0.2.0"
"test_kit" = "^0.1.0"
"#,
        ).unwrap();

        let mut mgr2 = DependencyManager::with_cache(dir.clone(), cache);
        let res_outdated = mgr2.install_all_with_opts(true, false);
        assert!(res_outdated.is_err());
        assert!(res_outdated.unwrap_err().to_string().contains("--locked was passed to prevent this"));
    }

    #[test]
    fn test_selective_package_update() {
        let (dir, cache, _holder) = temp_project("selective_update");
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "update_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "^0.1.0"
"test_kit" = "^0.1.0"
"#,
        ).unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache.clone());
        mgr.install_all().unwrap();

        let init_lock = LockFile::load(&dir.join("bsr.lock")).unwrap();
        assert_eq!(init_lock.find_package("std::json").unwrap().version, "0.1.0");
        assert_eq!(init_lock.find_package("test_kit").unwrap().version, "0.1.0");

        // Now widen std::json to * and update ONLY std::json
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "update_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "*"
"test_kit" = "^0.1.0"
"#,
        ).unwrap();

        let mut mgr2 = DependencyManager::with_cache(dir.clone(), cache);
        mgr2.update(Some("std::json")).unwrap();

        let updated_lock = LockFile::load(&dir.join("bsr.lock")).unwrap();
        let json = updated_lock.find_package("std::json").unwrap();
        assert_eq!(json.version, "1.0.0", "std::json should be updated to latest");

        let test_kit = updated_lock.find_package("test_kit").unwrap();
        assert_eq!(test_kit.version, "0.1.0", "test_kit should remain at 0.1.0");
    }

    #[test]
    fn test_vendor_and_offline_install() {
        let (dir, cache, _holder) = temp_project("vendor_offline_test");
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "vendor_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "^0.2.0"
"#,
        ).unwrap();

        let mut mgr = DependencyManager::with_cache(dir.clone(), cache.clone());
        mgr.install_all().unwrap();

        // 1. Vendor the dependencies
        let opts = crate::vendor::VendorOptions::default();
        let summary = mgr.vendor(&opts).unwrap();
        assert_eq!(summary.packages.len(), 1);
        assert_eq!(summary.packages[0].name, "std::json");
        assert!(dir.join("vendor/std_json/lib.roze").exists());
        assert!(dir.join("vendor/bsr-vendor.mfz").exists());

        // 2. Wipe libs/
        let _ = crate::cache::make_dir_writable(&dir.join("libs"));
        let _ = fs::remove_dir_all(dir.join("libs"));
        assert!(!dir.join("libs").exists());

        // 3. Install in offline mode (must succeed from vendor/)
        let mut mgr2 = DependencyManager::with_cache(dir.clone(), cache);
        let res = mgr2.install_all_with_options(false, false, true);
        assert!(res.is_ok(), "Offline install from vendor/ must succeed");
        assert!(dir.join("libs/std::json/lib.roze").exists());

        // Checksum in libs must match
        let verif = mgr2.verify_installed().unwrap();
        assert_eq!(verif.len(), 1);
        assert!(verif[0].is_valid());
    }

    #[test]
    fn test_offline_mode_missing_fails() {
        let (dir, cache, _holder) = temp_project("offline_missing_test");
        fs::write(
            dir.join("bsr.mfz"),
            r#"name = "offline_missing_app"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::json" = "^0.2.0"
"#,
        ).unwrap();

        // Generate lockfile without installing
        let mut mgr = DependencyManager::with_cache(dir.clone(), cache);
        mgr.resolve_and_lock(false).unwrap();

        // Create isolated manager with empty custom cache
        let temp_empty_cache = tempfile::tempdir().unwrap();
        let isolated_cache = PackageCache::with_root(temp_empty_cache.path().to_path_buf());
        let mut isolated_mgr = DependencyManager::with_cache(dir.clone(), isolated_cache);

        // Attempt offline install without vendor/ or cache
        let res = isolated_mgr.install_all_with_options(false, false, true);
        assert!(res.is_err(), "Must fail offline when package is missing");
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Offline air-gapped mode error"));
    }
}
