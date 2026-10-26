// tools/roze-pkg/src/dependency.rs
//
// Roze dependency management, resolution, and installation.
// Integrates registry fetching, git cloning, deterministic lockfiles,
// and local package caching.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use anyhow::{anyhow, Result};
use colored::*;

use crate::cache::PackageCache;
use crate::crypto::sha256_dir;
use crate::git::{GitDependency, GitFetcher};
use crate::lockfile::{LockFile, LOCKFILE_NAME};
use crate::project::{DependencyValue, ProjectConfig};
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
        let cache = PackageCache::new();
        let registry = RegistryClient::with_cache(PackageCache::new());
        let git_fetcher = GitFetcher::with_cache(PackageCache::new());
        let resolver = DependencyResolver::with_clients(
            RegistryClient::with_cache(PackageCache::new()),
            GitFetcher::with_cache(PackageCache::new()),
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

    #[allow(dead_code)]
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
        self.project_dir.join("roze.toml")
    }

    pub fn lockfile_path(&self) -> PathBuf {
        self.project_dir.join(LOCKFILE_NAME)
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

        // Also clean up from libs/ and roze.lock
        let lib_dir = self.libs_dir().join(&package);
        if lib_dir.exists() {
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

    /// Resolves dependencies and generates/updates `roze.lock`.
    pub fn resolve_and_lock(&mut self, force_update: bool) -> Result<LockFile> {
        let specs = self.to_resolver_specs()?;
        let lock_path = self.lockfile_path();

        let existing_lock = if !force_update && lock_path.exists() {
            LockFile::load(&lock_path).ok()
        } else {
            None
        };

        let mut lockfile = self.resolver.resolve(&specs, existing_lock.as_ref(), force_update)?;
        lockfile.save(&lock_path)?;
        Ok(lockfile)
    }

    /// Installs all dependencies recorded in `roze.lock` (resolving first if lockfile is missing).
    pub fn install_all(&mut self) -> Result<()> {
        println!("📦 {}", "Installing dependencies...".bright_magenta());

        if self.dependencies.is_empty() {
            println!("  No dependencies specified in manifest");
            return Ok(());
        }

        let lock_path = self.lockfile_path();
        let mut lockfile = if lock_path.exists() {
            match LockFile::load(&lock_path) {
                Ok(l) => {
                    // Check that all manifest dependencies are present in lockfile
                    let all_present = self.dependencies.keys().all(|k| l.find_package(k).is_some());
                    if all_present {
                        l
                    } else {
                        println!("  Manifest changed; updating lockfile...");
                        self.resolve_and_lock(false)?
                    }
                }
                Err(_) => self.resolve_and_lock(false)?,
            }
        } else {
            self.resolve_and_lock(false)?
        };

        fs::create_dir_all(self.libs_dir())?;

        let mut updated = false;
        for pkg in &mut lockfile.packages {
            let dest_dir = self.libs_dir().join(&pkg.name);
            println!("  Installing {} v{} ({})", pkg.name.bold(), pkg.version.bright_cyan(), pkg.source);

            let checksum = if pkg.source.starts_with("git+") {
                let git_url = pkg.source.trim_start_matches("git+").split('#').next().unwrap_or(&pkg.source);
                let rev = pkg.source.split('#').nth(1);

                let dep = GitDependency {
                    url: git_url.to_string(),
                    tag: None,
                    branch: None,
                    rev: rev.map(|r| r.to_string()),
                };

                let fetched = self.git_fetcher.fetch(&dep)?;
                self.git_fetcher.install_to_project(&fetched, &dest_dir)?;
                sha256_dir(&dest_dir)?
            } else if pkg.source.starts_with("path+") {
                let path_str = pkg.source.trim_start_matches("path+");
                let src_path = PathBuf::from(path_str);
                if !src_path.exists() {
                    return Err(anyhow!("Path dependency not found: {}", src_path.display()));
                }
                if dest_dir.exists() {
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
                let ver = Version::parse(&pkg.version)?;
                let chk = self.registry.fetch_package(&pkg.name, &ver, &dest_dir)?;

                // If no lib.roze exists, generate standard export stub
                if !dest_dir.join("lib.roze").exists() {
                    let stub = format!(
                        r#"// Library: {}
// Version: {}
// Source: {}

func {}_version() -> string {{
    return "{}";
}}
"#,
                        pkg.name, pkg.version, pkg.source, pkg.name.replace("::", "_").replace('-', "_"), pkg.version
                    );
                    fs::write(dest_dir.join("lib.roze"), stub)?;
                }

                chk
            };

            if pkg.checksum.as_deref() != Some(&checksum) {
                pkg.checksum = Some(checksum);
                updated = true;
            }
        }

        if updated {
            lockfile.save(&lock_path)?;
        }

        println!("✅ {}", "All dependencies installed successfully!".bright_green());
        Ok(())
    }

    /// Updates dependencies to their latest compatible versions.
    pub fn update(&mut self) -> Result<()> {
        println!("🔄 {}", "Updating dependencies to latest compatible versions...".yellow());
        self.resolve_and_lock(true)?;
        self.install_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(name: &str) -> (PathBuf, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir = temp_dir.path().join(format!("roze_pkg_dep_test_{}", name));
        fs::create_dir_all(&dir).unwrap();
        (dir, temp_dir)
    }

    #[test]
    fn a_second_add_does_not_wipe_out_the_first_dependency() {
        let (dir, _holder) = temp_project("second_add_preserves_first");
        fs::write(dir.join("roze.toml"), "name = \"t\"\nversion = \"0.1.0\"\nmain = \"src/main.roze\"\n").unwrap();

        let mut mgr = DependencyManager::new(dir.clone());
        mgr.add_dependency("foo", "1.0.0").unwrap();

        let mut mgr2 = DependencyManager::new(dir.clone());
        mgr2.add_dependency("bar", "2.0.0").unwrap();

        let content = fs::read_to_string(dir.join("roze.toml")).unwrap();
        assert!(content.contains("foo"), "expected 'foo' to survive a second add, got:\n{}", content);
        assert!(content.contains("bar"), "expected 'bar' to be added too, got:\n{}", content);
    }

    #[test]
    fn remove_finds_a_dependency_added_in_a_previous_invocation() {
        let (dir, _holder) = temp_project("remove_finds_existing");
        fs::write(dir.join("roze.toml"), "name = \"t\"\nversion = \"0.1.0\"\nmain = \"src/main.roze\"\n").unwrap();

        let mut mgr = DependencyManager::new(dir.clone());
        mgr.add_dependency("some_lib", "1.2.3").unwrap();

        let mut mgr2 = DependencyManager::new(dir.clone());
        let result = mgr2.remove_dependency("some_lib");
        assert!(result.is_ok(), "expected remove to find the existing dependency, got: {:?}", result);

        let content = fs::read_to_string(dir.join("roze.toml")).unwrap();
        assert!(!content.contains("some_lib"), "expected 'some_lib' to be gone, got:\n{}", content);
    }

    #[test]
    fn new_project_with_no_manifest_starts_empty_without_erroring() {
        let (dir, _holder) = temp_project("no_manifest_yet");
        let mgr = DependencyManager::new(dir);
        assert!(mgr.dependencies.is_empty());
    }

    #[test]
    fn full_install_generates_deterministic_lockfile() {
        let (dir, _holder) = temp_project("install_generates_lock");
        fs::write(
            dir.join("roze.toml"),
            r#"name = "demo"
version = "0.1.0"
main = "src/main.roze"

[dependencies]
"std::web" = "^1.0.0"
"#,
        ).unwrap();

        let mut mgr = DependencyManager::new(dir.clone());
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
}
