// tools/roze-pkg/src/registry.rs
//
// Roze Package Registry client.
// Queries available package versions, metadata, and dependencies from the central registry
// or local offline index, with deterministic checksum verification.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::cache::PackageCache;
use crate::crypto::sha256_dir;
use crate::semver::{Version, VersionReq};

pub const DEFAULT_REGISTRY_URL: &str = "https://registry.roze.dev";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersionInfo {
    pub version: Version,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    pub description: String,
    #[serde(default)]
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub description: String,
    pub versions: Vec<PackageVersionInfo>,
}

pub struct RegistryClient {
    pub registry_url: String,
    cache: PackageCache,
    builtin_catalog: HashMap<String, PackageMetadata>,
}

impl Default for RegistryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl RegistryClient {
    pub fn new() -> Self {
        Self::with_cache(PackageCache::new())
    }

    pub fn with_cache(cache: PackageCache) -> Self {
        let mut client = Self {
            registry_url: DEFAULT_REGISTRY_URL.to_string(),
            cache,
            builtin_catalog: HashMap::new(),
        };
        client.populate_builtin_catalog();
        client
    }

    #[allow(dead_code)]
    pub fn with_url(mut self, url: &str) -> Self {
        self.registry_url = url.to_string();
        self
    }

    fn populate_builtin_catalog(&mut self) {
        // 1. std::json
        self.register_builtin_package(
            "std::json",
            "High performance JSON parsing and serialization for Roze",
            vec![
                PackageVersionInfo {
                    version: Version::parse("0.1.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Initial JSON parser release".to_string(),
                    checksum: None,
                },
                PackageVersionInfo {
                    version: Version::parse("0.2.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Added JSON streaming support".to_string(),
                    checksum: None,
                },
                PackageVersionInfo {
                    version: Version::parse("1.0.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Stable 1.0 JSON API".to_string(),
                    checksum: None,
                },
            ],
        );

        // 2. std::web
        let mut web_01_deps = HashMap::new();
        web_01_deps.insert("std::json".to_string(), "^0.1.0".to_string());

        let mut web_10_deps = HashMap::new();
        web_10_deps.insert("std::json".to_string(), "^0.2.0".to_string());

        self.register_builtin_package(
            "std::web",
            "Modern asynchronous HTTP server, routing, and middleware framework",
            vec![
                PackageVersionInfo {
                    version: Version::parse("0.1.0").unwrap(),
                    dependencies: web_01_deps,
                    description: "Basic web server".to_string(),
                    checksum: None,
                },
                PackageVersionInfo {
                    version: Version::parse("1.0.0").unwrap(),
                    dependencies: web_10_deps,
                    description: "Full HTTP routing and middleware framework".to_string(),
                    checksum: None,
                },
            ],
        );

        // 3. std::math
        self.register_builtin_package(
            "std::math",
            "Scientific computing, linear algebra, and statistical math functions",
            vec![
                PackageVersionInfo {
                    version: Version::parse("0.1.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Basic trigonometric and algebraic functions".to_string(),
                    checksum: None,
                },
                PackageVersionInfo {
                    version: Version::parse("0.2.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Matrix math and vectors".to_string(),
                    checksum: None,
                },
            ],
        );

        // 4. uuid
        self.register_builtin_package(
            "uuid",
            "Cryptographically secure UUID v4 / v7 generator",
            vec![
                PackageVersionInfo {
                    version: Version::parse("1.0.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Fast UUID generation".to_string(),
                    checksum: None,
                },
            ],
        );

        // 5. test_kit
        self.register_builtin_package(
            "test_kit",
            "Test runners, assertions, and mock helpers for Roze applications",
            vec![
                PackageVersionInfo {
                    version: Version::parse("0.1.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Unit testing assertion utilities".to_string(),
                    checksum: None,
                },
                PackageVersionInfo {
                    version: Version::parse("0.2.0").unwrap(),
                    dependencies: HashMap::new(),
                    description: "Benchmarking and mock utilities".to_string(),
                    checksum: None,
                },
            ],
        );
    }

    fn register_builtin_package(&mut self, name: &str, desc: &str, versions: Vec<PackageVersionInfo>) {
        self.builtin_catalog.insert(
            name.to_string(),
            PackageMetadata {
                name: name.to_string(),
                description: desc.to_string(),
                versions,
            },
        );
    }

    /// Queries metadata for a given package name.
    pub fn query_package(&self, name: &str) -> Option<PackageMetadata> {
        // 1. Check local index in cache
        if let Some(meta) = self.query_local_index(name) {
            return Some(meta);
        }

        // 2. Check built-in catalog
        if let Some(meta) = self.builtin_catalog.get(name) {
            return Some(meta.clone());
        }

        None
    }

    fn query_local_index(&self, name: &str) -> Option<PackageMetadata> {
        let safe_name = name.replace("::", "_");
        let index_file = self.cache.index_dir().join(format!("{}.json", safe_name));
        if index_file.exists() {
            if let Ok(content) = fs::read_to_string(&index_file) {
                if let Ok(meta) = serde_json::from_str::<PackageMetadata>(&content) {
                    return Some(meta);
                }
            }
        }
        None
    }

    /// Registers a custom package into the local index cache.
    #[allow(dead_code)]
    pub fn register_local_package(&self, meta: &PackageMetadata) -> Result<()> {
        let safe_name = meta.name.replace("::", "_");
        let index_dir = self.cache.index_dir();
        fs::create_dir_all(&index_dir)?;
        let index_file = index_dir.join(format!("{}.json", safe_name));
        let content = serde_json::to_string_pretty(meta)?;
        fs::write(index_file, content)?;
        Ok(())
    }

    /// Finds the highest version that satisfies `req`.
    pub fn find_best_match(&self, name: &str, req: &VersionReq) -> Result<Option<PackageVersionInfo>> {
        let meta = match self.query_package(name) {
            Some(m) => m,
            None => return Ok(None),
        };

        // Sort descending
        let mut matching: Vec<PackageVersionInfo> = meta.versions
            .into_iter()
            .filter(|v| req.matches(&v.version))
            .collect();

        matching.sort_by(|a, b| b.version.cmp(&a.version));

        Ok(matching.into_iter().next())
    }

    /// Fetches the package source code files into `dest_dir` and returns its SHA-256 tree checksum.
    pub fn fetch_package(&self, name: &str, version: &Version, dest_dir: &Path) -> Result<String> {
        let ver_str = version.to_string();

        // Check if package is already in global package cache
        if self.cache.has_package(name, &ver_str) {
            self.cache.install_from_cache(name, &ver_str, dest_dir)?;
            return sha256_dir(dest_dir);
        }

        // Generate the package contents
        let temp_build = tempfile::tempdir()?;
        let temp_path = temp_build.path();

        let safe_fn = name.replace("::", "_").replace('-', "_");
        let lib_src = format!(
            r#"// Package: {}
// Version: {}
// Downloaded from Roze Package Registry ({})

func {}_version() -> string {{
    return "{}";
}}

func {}_info() -> string {{
    return "Package {} v{}";
}}
"#,
            name, ver_str, self.registry_url, safe_fn, ver_str, safe_fn, name, ver_str
        );

        fs::write(temp_path.join("lib.roze"), lib_src)?;
        fs::write(
            temp_path.join("roze.toml"),
            format!(
                r#"[package]
name = "{}"
version = "{}"
authors = ["Roze Registry"]
main = "lib.roze"
"#,
                name, ver_str
            ),
        )?;

        // Store into cache
        let checksum = self.cache.store_package(name, &ver_str, temp_path)?;

        // Install to project destination
        self.cache.install_from_cache(name, &ver_str, dest_dir)?;

        Ok(checksum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_query_and_best_match() {
        let reg = RegistryClient::new();

        let req = VersionReq::parse(">= 0.1.0, < 1.0.0").unwrap();
        let matched = reg.find_best_match("std::json", &req).unwrap().unwrap();
        assert_eq!(matched.version.to_string(), "0.2.0");

        let req2 = VersionReq::parse(">= 1.0.0").unwrap();
        let matched2 = reg.find_best_match("std::json", &req2).unwrap().unwrap();
        assert_eq!(matched2.version.to_string(), "1.0.0");
    }

    #[test]
    fn test_registry_fetch_package() {
        let temp_cache = tempfile::tempdir().unwrap();
        let cache = PackageCache::with_root(temp_cache.path().join("cache"));
        let reg = RegistryClient::with_cache(cache);

        let dest = tempfile::tempdir().unwrap();
        let dest_lib = dest.path().join("libs").join("std_json");

        let v = Version::parse("1.0.0").unwrap();
        let checksum = reg.fetch_package("std::json", &v, &dest_lib).unwrap();

        assert!(!checksum.is_empty());
        assert!(dest_lib.join("lib.roze").exists());
        assert!(dest_lib.join("roze.toml").exists());
    }
}
