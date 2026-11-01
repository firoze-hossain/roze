// tools/roze-pkg/src/project.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;

pub const MANIFEST_NAME: &str = "bsr.mfz";
pub const ALTERNATIVE_MANIFEST_NAME: &str = "roze.mfz";
pub const LEGACY_MANIFEST_NAME: &str = "roze.toml";
pub const MANIFEST_HEADER: &str = "# bsr.mfz - Basir Project Manifest (Mohammad Firoze)\n\n";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum DependencyValue {
    Simple(String),
    Detailed(DetailedDependency),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Default)]
pub struct DetailedDependency {
    pub version: Option<String>,
    pub path: Option<String>,
    pub git: Option<String>,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub rev: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub main: String,
    #[serde(default)]
    pub dependencies: HashMap<String, DependencyValue>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, DependencyValue>,
    #[serde(default)]
    pub target: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "my_project".to_string(),
            version: "0.1.0".to_string(),
            description: Some("A Roze project".to_string()),
            authors: vec!["Your Name".to_string()],
            main: "src/main.roze".to_string(),
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
            target: Some("jvm".to_string()),
        }
    }
}

impl ProjectConfig {
    pub fn find_manifest(dir: &Path) -> Option<PathBuf> {
        let bsr = dir.join(MANIFEST_NAME);
        if bsr.exists() {
            return Some(bsr);
        }
        let rz_mfz = dir.join(ALTERNATIVE_MANIFEST_NAME);
        if rz_mfz.exists() {
            return Some(rz_mfz);
        }
        let legacy = dir.join(LEGACY_MANIFEST_NAME);
        if legacy.exists() {
            return Some(legacy);
        }
        None
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        Ok(config)
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        let mut final_content = String::new();
        if path.file_name().and_then(|s| s.to_str()) == Some(MANIFEST_NAME) {
            final_content.push_str(MANIFEST_HEADER);
        }
        final_content.push_str(&content);
        fs::write(path, final_content)?;
        Ok(())
    }
}
