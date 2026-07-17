// src/build.rs - Updated to avoid double compilation
use crate::config::BuildConfig;
use glob::glob;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use anyhow::{Result, anyhow};

#[derive(Clone)]
pub struct Builder {
    config: BuildConfig,
    release: bool,
    compiler_path: PathBuf,
}

impl Builder {
    pub fn new(config: BuildConfig, release: bool) -> Self {
        Self {
            config,
            release,
            compiler_path: Self::find_compiler().unwrap_or_else(|_| PathBuf::from("roze")),
        }
    }

    pub fn build(&mut self) -> Result<()> {
        let options = self.config.get_options(self.release);
        let output_dir = PathBuf::from(&options.output_dir);

        // Create output directory
        fs::create_dir_all(&output_dir)?;

        // Find all source files
        let source_files = self.find_source_files()?;
        println!("📁 Found {} source files", source_files.len());

        // Compile each source file
        for file in &source_files {
            // Skip if it's the main file (we'll compile it separately)
            if file.to_string_lossy() == self.config.main {
                continue;
            }
            self.compile_file(file, &output_dir)?;
        }

        // Compile the main file last
        self.compile_file(&PathBuf::from(&self.config.main), &output_dir)?;

        Ok(())
    }

    pub fn clean(&mut self) -> Result<()> {
        let options = self.config.get_options(self.release);
        let output_dir = PathBuf::from(&options.output_dir);

        if output_dir.exists() {
            fs::remove_dir_all(&output_dir)?;
            println!("🗑️ Removed: {}", output_dir.display());
        }

        Ok(())
    }

    fn find_source_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for pattern in &self.config.sources {
            let entries = glob(pattern)?;
            for entry in entries {
                if let Ok(path) = entry {
                    if path.extension().map_or(false, |ext| ext == "roze") {
                        files.push(path);
                    }
                }
            }
        }

        Ok(files)
    }

    fn compile_file(&self, file: &PathBuf, output_dir: &PathBuf) -> Result<()> {
        println!("  Compiling: {}", file.display());

        let status = Command::new(&self.compiler_path)
            .arg("build")
            .arg(file)
            .status()?;

        if !status.success() {
            return Err(anyhow!("Failed to compile: {}", file.display()));
        }

        // Move compiled class file to output directory
        let class_name = file
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let class_file = PathBuf::from(format!("{}.class", class_name));
        if class_file.exists() {
            let dest = output_dir.join(&class_file);
            fs::rename(&class_file, &dest)?;
        }

        Ok(())
    }

    fn find_compiler() -> Result<PathBuf> {
        // Check if there's a roze binary in PATH
        if let Ok(path) = which::which("roze") {
            return Ok(path);
        }

        // Check in the project root
        let current_dir = std::env::current_dir()?;
        let mut path = current_dir.clone();
        for _ in 0..4 {
            path = path.parent().unwrap_or(&path).to_path_buf();
            let compiler = path.join("target/release/roze");
            if compiler.exists() {
                return Ok(compiler);
            }
        }

        Err(anyhow!("Could not find Roze compiler"))
    }
}