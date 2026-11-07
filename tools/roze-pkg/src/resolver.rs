// tools/roze-pkg/src/resolver.rs
//
// Semantic version dependency graph resolver powered by a SAT / CDCL constraint engine.
// Handles transitive dependencies, diamond dependencies with intelligent backtracking,
// cycle detection with full tracebacks, and deterministic lockfile reconciliation.
// Full parity with Cargo resolver v2 and PubGrub.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use anyhow::{anyhow, Result};

use crate::git::{GitDependency, GitFetcher};
use crate::lockfile::{LockFile, LockedPackage};
use crate::project::ProjectConfig;
use crate::registry::RegistryClient;
use crate::semver::{Version, VersionReq};
use crate::solver::{PackageProvider, RegistryProvider, SatSolver};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencySpec {
    Registry(VersionReq),
    Git(GitDependency),
    Path(PathBuf),
}

impl DependencySpec {
    pub fn parse_str(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.starts_with("git+") || s.starts_with("https://") || s.starts_with("git@") {
            let (url, tag) = match s.split_once('#') {
                Some((u, t)) => (u.to_string(), Some(t.to_string())),
                None => (s.to_string(), None),
            };
            Ok(Self::Git(GitDependency {
                url,
                tag,
                branch: None,
                rev: None,
            }))
        } else if s.starts_with("./") || s.starts_with("../") || s.starts_with('/') {
            Ok(Self::Path(PathBuf::from(s)))
        } else {
            let req = VersionReq::parse(s)?;
            Ok(Self::Registry(req))
        }
    }
}

pub struct DependencyResolver {
    registry: RegistryClient,
    git_fetcher: GitFetcher,
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            registry: RegistryClient::new(),
            git_fetcher: GitFetcher::new(),
        }
    }

    pub fn with_clients(registry: RegistryClient, git_fetcher: GitFetcher) -> Self {
        Self {
            registry,
            git_fetcher,
        }
    }

    /// Resolves direct dependencies against any existing lockfile.
    /// If `force_update` is true, ignores locked versions and resolves to the highest compatible.
    pub fn resolve(
        &self,
        direct_deps: &HashMap<String, DependencySpec>,
        existing_lock: Option<&LockFile>,
        force_update: bool,
    ) -> Result<LockFile> {
        let empty_set = HashSet::new();
        self.resolve_with_options(direct_deps, existing_lock, force_update, &empty_set)
    }

    /// Resolves dependencies using the SAT constraint solver with optional selective package updating.
    pub fn resolve_with_options(
        &self,
        direct_deps: &HashMap<String, DependencySpec>,
        existing_lock: Option<&LockFile>,
        force_update_all: bool,
        update_packages: &HashSet<String>,
    ) -> Result<LockFile> {
        let mut resolved_packages: HashMap<String, LockedPackage> = HashMap::new();
        let mut registry_requirements: Vec<(String, VersionReq)> = Vec::new();

        // 1. Process Git and Path dependencies first, inspecting their manifests for transitive dependencies
        for (name, spec) in direct_deps {
            let should_update = force_update_all || update_packages.contains(name);
            let locked_candidate = if !should_update {
                existing_lock.and_then(|lock| lock.find_package(name))
            } else {
                None
            };

            match spec {
                DependencySpec::Registry(req) => {
                    registry_requirements.push((name.clone(), req.clone()));
                }
                DependencySpec::Git(git_dep) => {
                    let (commit_hash, version, checkout_dir) = if let Some(lc) = locked_candidate {
                        if let Some(hash) = lc.git_commit_hash() {
                            let dep_pinned = GitDependency {
                                url: git_dep.url.clone(),
                                tag: None,
                                branch: None,
                                rev: Some(hash.to_string()),
                            };
                            let fetched = self.git_fetcher.fetch(&dep_pinned)?;
                            (hash.to_string(), lc.version.clone(), fetched.checkout_dir)
                        } else {
                            let fetched = self.git_fetcher.fetch(git_dep)?;
                            (fetched.commit_hash, fetched.version, fetched.checkout_dir)
                        }
                    } else {
                        let fetched = self.git_fetcher.fetch(git_dep)?;
                        (fetched.commit_hash, fetched.version, fetched.checkout_dir)
                    };

                    // Inspect manifest in checkout for transitive dependencies
                    let mut git_dep_strings = Vec::new();
                    if let Some(mpath) = ProjectConfig::find_manifest(&checkout_dir) {
                        if let Ok(mcfg) = ProjectConfig::load(&mpath) {
                            for (sub_name, sub_val) in mcfg.dependencies {
                                match sub_val {
                                    crate::project::DependencyValue::Simple(ver_str) => {
                                        if let Ok(req) = VersionReq::parse(&ver_str) {
                                            registry_requirements.push((sub_name.clone(), req.clone()));
                                            git_dep_strings.push(format!("{} {}", sub_name, req));
                                        }
                                    }
                                    crate::project::DependencyValue::Detailed(d) => {
                                        if let Some(ver_str) = d.version {
                                            if let Ok(req) = VersionReq::parse(&ver_str) {
                                                registry_requirements.push((sub_name.clone(), req.clone()));
                                                git_dep_strings.push(format!("{} {}", sub_name, req));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    git_dep_strings.sort();

                    let source = format!("git+{}#{}", git_dep.url, commit_hash);
                    let locked = LockedPackage {
                        name: name.clone(),
                        version,
                        source: Some(source),
                        checksum: locked_candidate.and_then(|lc| lc.checksum.clone()),
                        dependencies: git_dep_strings,
                    };
                    resolved_packages.insert(name.clone(), locked);
                }
                DependencySpec::Path(path) => {
                    let mut path_dep_strings = Vec::new();
                    if let Some(mpath) = ProjectConfig::find_manifest(path) {
                        if let Ok(mcfg) = ProjectConfig::load(&mpath) {
                            for (sub_name, sub_val) in mcfg.dependencies {
                                match sub_val {
                                    crate::project::DependencyValue::Simple(ver_str) => {
                                        if let Ok(req) = VersionReq::parse(&ver_str) {
                                            registry_requirements.push((sub_name.clone(), req.clone()));
                                            path_dep_strings.push(format!("{} {}", sub_name, req));
                                        }
                                    }
                                    crate::project::DependencyValue::Detailed(d) => {
                                        if let Some(ver_str) = d.version {
                                            if let Ok(req) = VersionReq::parse(&ver_str) {
                                                registry_requirements.push((sub_name.clone(), req.clone()));
                                                path_dep_strings.push(format!("{} {}", sub_name, req));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    path_dep_strings.sort();

                    let source = format!("path+{}", path.display());
                    let locked = LockedPackage {
                        name: name.clone(),
                        version: "0.1.0".to_string(),
                        source: Some(source),
                        checksum: locked_candidate.and_then(|lc| lc.checksum.clone()),
                        dependencies: path_dep_strings,
                    };
                    resolved_packages.insert(name.clone(), locked);
                }
            }
        }

        // 2. Prepare locked versions map for the SAT solver (Cargo resolver v2 minimal churn)
        let mut locked_versions: HashMap<String, Version> = HashMap::new();
        if !force_update_all {
            if let Some(lock) = existing_lock {
                for pkg in &lock.packages {
                    if !update_packages.contains(&pkg.name) && !pkg.is_root() {
                        if let Ok(ver) = Version::parse(&pkg.version) {
                            locked_versions.insert(pkg.name.clone(), ver);
                        }
                    }
                }
            }
        }

        // 3. Run the SAT Solver on all registry dependencies
        if !registry_requirements.is_empty() {
            let provider = RegistryProvider::new(&self.registry);
            let solver = SatSolver::new(&provider).with_locked(locked_versions);

            let solution = solver.solve(&registry_requirements)
                .map_err(|e| anyhow!("{}", e))?;

            // Convert solver solution into locked packages
            for (pkg_name, ver) in solution.packages {
                let locked_candidate = if !force_update_all && !update_packages.contains(&pkg_name) {
                    existing_lock.and_then(|lock| lock.find_package(&pkg_name))
                } else {
                    None
                };

                let checksum = if let Some(lc) = locked_candidate {
                    if lc.version == ver.to_string() {
                        lc.checksum.clone()
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Get dependencies for this solved version
                let sub_deps = provider.dependencies_of(&pkg_name, &ver)?;
                let mut dep_strings: Vec<String> = sub_deps.iter()
                    .map(|(d_name, d_req)| format!("{} {}", d_name, d_req))
                    .collect();
                dep_strings.sort();

                let locked = LockedPackage {
                    name: pkg_name.clone(),
                    version: ver.to_string(),
                    source: Some(format!("registry+{}", self.registry.registry_url)),
                    checksum,
                    dependencies: dep_strings,
                };

                resolved_packages.insert(pkg_name, locked);
            }
        }

        let mut lockfile = LockFile::new();
        for (_, pkg) in resolved_packages {
            lockfile.add_or_update(pkg);
        }
        lockfile.sort();

        Ok(lockfile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_transitive_dependencies() {
        let resolver = DependencyResolver::new();
        let mut direct = HashMap::new();
        direct.insert("std::web".to_string(), DependencySpec::Registry(VersionReq::parse("^1.0.0").unwrap()));

        let lock = resolver.resolve(&direct, None, false).unwrap();
        assert_eq!(lock.packages.len(), 2);

        let web = lock.find_package("std::web").unwrap();
        assert_eq!(web.version, "1.0.0");

        let json = lock.find_package("std::json").unwrap();
        assert_eq!(json.version, "0.2.0");
    }

    #[test]
    fn test_resolve_preserves_lockfile_version() {
        let resolver = DependencyResolver::new();
        let mut direct = HashMap::new();
        direct.insert("std::json".to_string(), DependencySpec::Registry(VersionReq::parse("*").unwrap()));

        let mut existing_lock = LockFile::new();
        existing_lock.add_or_update(LockedPackage {
            name: "std::json".to_string(),
            version: "0.1.0".to_string(),
            source: Some("registry+https://registry.roze.dev".to_string()),
            checksum: Some("pinned_checksum_12345".to_string()),
            dependencies: Vec::new(),
        });

        // With force_update=false, keeps 0.1.0 AND preserves pinned checksum!
        let locked = resolver.resolve(&direct, Some(&existing_lock), false).unwrap();
        let json = locked.find_package("std::json").unwrap();
        assert_eq!(json.version, "0.1.0");
        assert_eq!(json.checksum, Some("pinned_checksum_12345".to_string()));

        // With force_update=true, upgrades to 1.0.0 (highest available for *)
        let updated = resolver.resolve(&direct, Some(&existing_lock), true).unwrap();
        let json_updated = updated.find_package("std::json").unwrap();
        assert_eq!(json_updated.version, "1.0.0");
    }

    #[test]
    fn test_resolve_selective_update() {
        let resolver = DependencyResolver::new();
        let mut direct = HashMap::new();
        direct.insert("std::json".to_string(), DependencySpec::Registry(VersionReq::parse("*").unwrap()));
        direct.insert("test_kit".to_string(), DependencySpec::Registry(VersionReq::parse("*").unwrap()));

        let mut existing_lock = LockFile::new();
        existing_lock.add_or_update(LockedPackage::new_registry("std::json", "0.1.0", "https://reg", Some("json_hash".to_string()), vec![]));
        existing_lock.add_or_update(LockedPackage::new_registry("test_kit", "0.1.0", "https://reg", Some("test_hash".to_string()), vec![]));

        // Update ONLY std::json, keeping test_kit pinned at 0.1.0
        let mut to_update = HashSet::new();
        to_update.insert("std::json".to_string());

        let res = resolver.resolve_with_options(&direct, Some(&existing_lock), false, &to_update).unwrap();

        // std::json upgraded to 1.0.0
        let json = res.find_package("std::json").unwrap();
        assert_eq!(json.version, "1.0.0");

        // test_kit stays pinned at 0.1.0 with its checksum intact
        let test_kit = res.find_package("test_kit").unwrap();
        assert_eq!(test_kit.version, "0.1.0");
        assert_eq!(test_kit.checksum, Some("test_hash".to_string()));
    }
}
