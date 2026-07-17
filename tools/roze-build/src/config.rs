// src/config.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildConfig {
    pub name: String,
    pub version: String,
    pub main: String,
    pub sources: Vec<String>,
    pub dependencies: HashMap<String, String>,
    pub dev_dependencies: HashMap<String, String>,
    pub build: BuildOptions,
    pub release: BuildOptions,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BuildOptions {
    pub optimize: bool,
    pub debug: bool,
    pub output_dir: String,
    pub target: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            name: "my_project".to_string(),
            version: "0.1.0".to_string(),
            main: "src/main.roze".to_string(),
            sources: vec!["src/**/*.roze".to_string()],
            dependencies: HashMap::new(),
            dev_dependencies: HashMap::new(),
            build: BuildOptions {
                optimize: false,
                debug: true,
                output_dir: "target/debug".to_string(),
                target: "jvm".to_string(),
            },
            release: BuildOptions {
                optimize: true,
                debug: false,
                output_dir: "target/release".to_string(),
                target: "jvm".to_string(),
            },
        }
    }
}

impl BuildConfig {
    pub fn load() -> Result<Self, anyhow::Error> {
        let config_path = std::env::current_dir()?.join("build.roze.toml");
        if config_path.exists() {
            let content = fs::read_to_string(config_path)?;
            let config: BuildConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), anyhow::Error> {
        let config_path = std::env::current_dir()?.join("build.roze.toml");
        let content = toml::to_string_pretty(self)?;
        fs::write(config_path, content)?;
        Ok(())
    }

    pub fn get_options(&self, release: bool) -> &BuildOptions {
        if release {
            &self.release
        } else {
            &self.build
        }
    }
}