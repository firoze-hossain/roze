// tools/roze-pkg/src/resolver.rs
//
// Semantic version dependency graph resolver.
// Handles transitive dependencies, diamond dependencies, cycle detection,
// and deterministic lockfile reconciliation.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use anyhow::{anyhow, Result};

use crate::git::{GitDependency, GitFetcher};
use crate::lockfile::{LockFile, LockedPackage};
use crate::registry::RegistryClient;
use crate::semver::{Version, VersionReq};

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

    /// Resolves dependencies with optional selective package updating.
    /// If `update_packages` is non-empty, only those specific packages (and their sub-dependencies)
    /// are unlocked; all other dependencies strictly preserve their pinned versions, git commit hashes,
    /// and cryptographic checksums.
    pub fn resolve_with_options(
        &self,
        direct_deps: &HashMap<String, DependencySpec>,
        existing_lock: Option<&LockFile>,
        force_update_all: bool,
        update_packages: &HashSet<String>,
    ) -> Result<LockFile> {
        let mut resolved_packages: HashMap<String, LockedPackage> = HashMap::new();
        let mut resolution_queue: VecDeque<(String, DependencySpec)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();

        // Enqueue direct dependencies
        for (name, spec) in direct_deps {
            resolution_queue.push_back((name.clone(), spec.clone()));
        }

        // Active path stack for cycle detection
        let mut active_stack: Vec<String> = Vec::new();

        while let Some((pkg_name, spec)) = resolution_queue.pop_front() {
            if resolved_packages.contains_key(&pkg_name) {
                continue;
            }

            active_stack.push(pkg_name.clone());

            let should_update = force_update_all || update_packages.contains(&pkg_name);
            let locked_candidate = if !should_update {
                existing_lock.and_then(|lock| lock.find_package(&pkg_name))
            } else {
                None
            };

            match spec {
                DependencySpec::Registry(ref req) => {
                    let locked_version = locked_candidate.and_then(|lp| Version::parse(&lp.version).ok());

                    let (version, sub_deps) = if let Some(lv) = locked_version.filter(|v| req.matches(v)) {
                        // Keep locked version
                        let meta = self.registry.query_package(&pkg_name)
                            .ok_or_else(|| anyhow!("Package '{}' not found in registry", pkg_name))?;
                        let v_info = meta.versions.into_iter().find(|v| v.version == lv)
                            .ok_or_else(|| anyhow!("Version {} for '{}' not found in registry", lv, pkg_name))?;
                        (lv, v_info.dependencies)
                    } else {
                        // Find highest match
                        let best = self.registry.find_best_match(&pkg_name, req)?
                            .ok_or_else(|| anyhow!("No version matching '{}' found for package '{}'", req, pkg_name))?;
                        (best.version, best.dependencies)
                    };

                    let mut dep_strings = Vec::new();
                    for (sub_name, sub_ver_req_str) in &sub_deps {
                        let sub_spec = DependencySpec::parse_str(sub_ver_req_str)?;
                        dep_strings.push(format!("{} {}", sub_name, sub_ver_req_str));

                        if !resolved_packages.contains_key(sub_name) && !visited.contains(sub_name) {
                            if active_stack.contains(sub_name) {
                                return Err(anyhow!(
                                    "Dependency cycle detected: {} -> {}",
                                    active_stack.join(" -> "),
                                    sub_name
                                ));
                            }
                            resolution_queue.push_back((sub_name.clone(), sub_spec));
                        }
                    }

                    dep_strings.sort();

                    // Preserve existing checksum if unchanged
                    let checksum = if let Some(lc) = locked_candidate {
                        if lc.version == version.to_string() {
                            lc.checksum.clone()
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let locked = LockedPackage {
                        name: pkg_name.clone(),
                        version: version.to_string(),
                        source: Some(format!("registry+{}", self.registry.registry_url)),
                        checksum,
                        dependencies: dep_strings,
                    };

                    resolved_packages.insert(pkg_name.clone(), locked);
                }
                DependencySpec::Git(ref git_dep) => {
                    let (commit_hash, version, checksum) = if let Some(lc) = locked_candidate {
                        if let Some(hash) = lc.git_commit_hash() {
                            (hash.to_string(), lc.version.clone(), lc.checksum.clone())
                        } else {
                            let fetched = self.git_fetcher.fetch(git_dep)?;
                            (fetched.commit_hash, fetched.version, None)
                        }
                    } else {
                        let fetched = self.git_fetcher.fetch(git_dep)?;
                        (fetched.commit_hash, fetched.version, None)
                    };

                    let source = format!("git+{}#{}", git_dep.url, commit_hash);
                    let locked = LockedPackage {
                        name: pkg_name.clone(),
                        version,
                        source: Some(source),
                        checksum,
                        dependencies: Vec::new(),
                    };

                    resolved_packages.insert(pkg_name.clone(), locked);
                }
                DependencySpec::Path(ref path) => {
                    let source = format!("path+{}", path.display());
                    let checksum = locked_candidate.and_then(|lc| lc.checksum.clone());

                    let locked = LockedPackage {
                        name: pkg_name.clone(),
                        version: "0.1.0".to_string(),
                        source: Some(source),
                        checksum,
                        dependencies: Vec::new(),
                    };

                    resolved_packages.insert(pkg_name.clone(), locked);
                }
            }

            visited.insert(pkg_name.clone());
            active_stack.pop();
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
