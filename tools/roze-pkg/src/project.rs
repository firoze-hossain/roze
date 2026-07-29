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
    pub fn load(path: &PathBuf) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config: ProjectConfig = toml::from_str(&content)?;
        Ok(config)
    }
}