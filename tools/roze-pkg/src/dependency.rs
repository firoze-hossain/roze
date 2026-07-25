// src/dependency.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, anyhow};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    pub source: DependencySource,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum DependencySource {
    Registry(String),
    Path(String),
    Git { url: String, tag: Option<String> },
}

pub struct DependencyManager {
    pub dependencies: HashMap<String, Dependency>,
    pub project_dir: PathBuf,
}

impl DependencyManager {
    pub fn new(project_dir: PathBuf) -> Self {
        let mut manager = Self {
            dependencies: HashMap::new(),
            project_dir,
        };
        // Best-effort: if roze.toml doesn't exist yet (a brand new
        // project) or can't be parsed, start empty rather than failing
        // the constructor -- `add` on a fresh project should still work.
        let _ = manager.load_manifest();
        manager
    }

    /// Populates `self.dependencies` from the project's roze.toml, if one
    /// exists. Without this, every command started from an empty map
    /// regardless of what was already recorded on disk: `add` would
    /// silently overwrite any previously-added dependencies (since
    /// `save_manifest` writes out the *entire* in-memory map), and
    /// `remove` could never find anything, since it was always comparing
    /// against a map that had never been told what already existed.
    fn load_manifest(&mut self) -> Result<()> {
        let manifest_path = self.project_dir.join("roze.toml");
        if !manifest_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&manifest_path)?;
        let config: toml::Value = toml::from_str(&content)?;

        if let Some(deps_table) = config.get("dependencies").and_then(|d| d.as_table()) {
            for (name, version_value) in deps_table {
                let version = version_value.as_str().unwrap_or("*").to_string();
                self.dependencies.insert(name.clone(), Dependency {
                    name: name.clone(),
                    version,
                    // The on-disk format only records name+version today
                    // (see save_manifest), so there's no real source to
                    // recover here -- Registry is a reasonable default,
                    // consistent with what `add_dependency` itself sets.
                    source: DependencySource::Registry("https://registry.roze.dev".to_string()),
                });
            }
        }

        Ok(())
    }

    pub fn add_dependency(&mut self, name: &str, version: &str) -> Result<()> {
        let (package, version) = if name.contains("::") {
            let parts: Vec<&str> = name.split("::").collect();
            (parts[1].to_string(), version.to_string())
        } else {
            (name.to_string(), version.to_string())
        };

        let dep = Dependency {
            name: package.clone(),
            version: version.clone(),
            source: DependencySource::Registry("https://registry.roze.dev".to_string()),
        };

        self.dependencies.insert(package, dep);
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
            return Err(anyhow!("Dependency '{}' not found", name));
        }
        self.save_manifest()?;
        Ok(())
    }

    pub fn save_manifest(&self) -> Result<()> {
        let manifest_path = self.project_dir.join("roze.toml");
        if manifest_path.exists() {
            let content = fs::read_to_string(&manifest_path)?;
            let mut config: toml::Value = toml::from_str(&content)?;

            // Ensure dependencies section exists
            if !config.is_table() {
                config = toml::Value::Table(toml::map::Map::new());
            }

            // Always ensure dependencies is a table
            if !config.get("dependencies").is_some() {
                config.as_table_mut().unwrap().insert(
                    "dependencies".to_string(),
                    toml::Value::Table(toml::map::Map::new())
                );
            }

            let deps = self.dependencies.values().map(|dep| {
                (dep.name.clone(), toml::Value::String(dep.version.clone()))
            }).collect::<toml::map::Map<String, toml::Value>>();

            config["dependencies"] = toml::Value::Table(deps);
            fs::write(&manifest_path, toml::to_string_pretty(&config)?)?;
        }
        Ok(())
    }

    pub fn install_all(&self) -> Result<()> {
        println!("📦 Installing dependencies...");

        if self.dependencies.is_empty() {
            println!("  No dependencies to install");
            return Ok(());
        }

        for dep in self.dependencies.values() {
            println!("  Installing {} v{}", dep.name, dep.version);

            let dep_dir = self.project_dir.join("libs").join(&dep.name);
            fs::create_dir_all(&dep_dir)?;

            let stub = format!(
                r#"// Library: {}
// Version: {}
// Auto-generated by Roze package manager

func {}_version() -> string {{
    return "{}";
}}
"#,
                dep.name, dep.version, dep.name, dep.version
            );

            fs::write(dep_dir.join("lib.roze"), stub)?;
        }

        println!("✅ All dependencies installed!");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("roze_pkg_dep_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_second_add_does_not_wipe_out_the_first_dependency() {
        let dir = temp_project("second_add_preserves_first");
        fs::write(dir.join("roze.toml"), "name = \"t\"\nversion = \"0.1.0\"\nmain = \"src/main.roze\"\n").unwrap();

        let mut mgr = DependencyManager::new(dir.clone());
        mgr.add_dependency("foo", "1.0.0").unwrap();

        // A fresh manager, as every real CLI invocation constructs one --
        // this is exactly the bug: without loading existing dependencies
        // first, this second `add` would only ever see "bar" in memory.
        let mut mgr2 = DependencyManager::new(dir.clone());
        mgr2.add_dependency("bar", "2.0.0").unwrap();

        let content = fs::read_to_string(dir.join("roze.toml")).unwrap();
        assert!(content.contains("foo"), "expected 'foo' to survive a second add, got:\n{}", content);
        assert!(content.contains("bar"), "expected 'bar' to be added too, got:\n{}", content);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn remove_finds_a_dependency_added_in_a_previous_invocation() {
        let dir = temp_project("remove_finds_existing");
        fs::write(dir.join("roze.toml"), "name = \"t\"\nversion = \"0.1.0\"\nmain = \"src/main.roze\"\n").unwrap();

        let mut mgr = DependencyManager::new(dir.clone());
        mgr.add_dependency("some_lib", "1.2.3").unwrap();

        // Simulates running `roze-pkg remove some_lib` as a genuinely
        // separate process invocation, the same way the CLI actually
        // works -- a fresh DependencyManager must still know about
        // dependencies recorded by a prior one.
        let mut mgr2 = DependencyManager::new(dir.clone());
        let result = mgr2.remove_dependency("some_lib");
        assert!(result.is_ok(), "expected remove to find the existing dependency, got: {:?}", result);

        let content = fs::read_to_string(dir.join("roze.toml")).unwrap();
        assert!(!content.contains("some_lib"), "expected 'some_lib' to be gone, got:\n{}", content);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn new_project_with_no_manifest_starts_empty_without_erroring() {
        let dir = temp_project("no_manifest_yet");
        // No roze.toml written at all.
        let mgr = DependencyManager::new(dir.clone());
        assert!(mgr.dependencies.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }
}