// src/project.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub main: String,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, String>,
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
    pub fn new(name: &str) -> Self {
        let mut config = Self::default();
        config.name = name.to_string();
        config
    }

    pub fn load(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), anyhow::Error> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn add_dependency(&mut self, name: &str, version: &str) {
        self.dependencies.insert(name.to_string(), version.to_string());
    }

    pub fn remove_dependency(&mut self, name: &str) {
        self.dependencies.remove(name);
    }

    pub fn has_dependency(&self, name: &str) -> bool {
        self.dependencies.contains_key(name)
    }
}