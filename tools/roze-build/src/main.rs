// src/main.rs
mod build;
mod config;
mod watcher;

use clap::{Parser, Subcommand};
use colored::*;
use anyhow::Result;
use std::path::PathBuf;

use build::Builder;
use config::BuildConfig;
use watcher::Watcher;

#[derive(Parser)]
#[command(name = "roze-build")]
#[command(version = "0.1.0")]
#[command(about = "🌹 Roze Build System")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build the project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,
        /// Clean before building
        #[arg(short, long)]
        clean: bool,
        /// Watch for changes and rebuild
        #[arg(short, long)]
        watch: bool,
    },
    /// Clean build artifacts
    Clean,
    /// Run the project
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
        /// Build in release mode
        #[arg(short, long)]
        release: bool,
    },
    /// Watch for changes and rebuild
    Watch,
    /// Initialize a new project with build configuration
    Init,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { release, clean, watch } => {
            if clean {
                cmd_clean()?;
            }
            if watch {
                cmd_watch(release)?;
            } else {
                cmd_build(release)?;
            }
        }
        Commands::Clean => {
            cmd_clean()?;
        }
        Commands::Run { args, release } => {
            cmd_run(args, release)?;
        }
        Commands::Watch => {
            cmd_watch(false)?;
        }
        Commands::Init => {
            cmd_init()?;
        }
    }

    Ok(())
}

fn cmd_build(release: bool) -> Result<()> {
    println!("🔨 {}", "Building project...".green());

    let config = BuildConfig::load()?;
    let mut builder = Builder::new(config, release);
    builder.build()?;

    println!("✅ {}", "Build successful!".bright_green());
    Ok(())
}

fn cmd_clean() -> Result<()> {
    println!("🧹 {}", "Cleaning build artifacts...".yellow());

    let config = BuildConfig::load()?;
    let mut builder = Builder::new(config, false);
    builder.clean()?;

    println!("✅ {}", "Clean complete!".bright_green());
    Ok(())
}

// src/main.rs - Updated cmd_run function
fn cmd_run(args: Vec<String>, release: bool) -> Result<()> {
    println!("🚀 {}", "Running project...".yellow());

    cmd_build(release)?;

    let config = BuildConfig::load()?;
    let options = config.get_options(release);
    let output_dir = &options.output_dir;

    // Extract class name from main file path
    let class_name = PathBuf::from(&config.main)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    println!("📦 Running class: {}", class_name);

    let status = std::process::Command::new("java")
        .arg("-cp")
        .arg(output_dir)
        .arg(&class_name)
        .args(&args)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Run failed"))
    }
}

fn cmd_watch(release: bool) -> Result<()> {
    println!("👁️ {}", "Watching for changes...".yellow());

    let config = BuildConfig::load()?;
    let builder = Builder::new(config, release);
    let watcher = Watcher::new(builder);
    watcher.watch()?;

    Ok(())
}

fn cmd_init() -> Result<()> {
    println!("📁 {}", "Initializing build configuration...".green());

    let config = BuildConfig::default();
    config.save()?;

    println!("✅ {}", "Created build.roze.toml".bright_green());
    Ok(())
}