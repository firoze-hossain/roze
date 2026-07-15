// src/main.rs
mod project;
mod template;
mod dependency;

use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use std::process::Command;
use anyhow::Result;

use project::ProjectConfig;
use template::Template;
use dependency::DependencyManager;

#[derive(Parser)]
#[command(name = "roze-pkg")]
#[command(version = "0.1.0")]
#[command(about = "🌹 Roze Package Manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Roze project
    New {
        /// Project name
        name: String,
        /// Template to use (default, web, library)
        #[arg(short, long, default_value = "default")]
        template: String,
        /// Author name
        #[arg(short, long, default_value = "Firoze")]
        author: String,
        /// Path to create project in
        #[arg(short, long)]
        path: Option<PathBuf>,
    },
    /// Build the current project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,
    },
    /// Run the current project
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
    },
    /// Add a dependency
    Add {
        /// Package name (e.g., std::web)
        name: String,
        /// Version (e.g., 0.1.0)
        #[arg(short, long, default_value = "0.1.0")]
        version: String,
    },
    /// Remove a dependency
    Remove {
        /// Package name
        name: String,
    },
    /// Install dependencies
    Install,
    /// Run tests
    Test,
    /// Update dependencies
    Update,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, template, author, path } => {
            cmd_new(&name, &template, &author, path)?;
        }
        Commands::Build { release } => {
            cmd_build(release)?;
        }
        Commands::Run { args } => {
            cmd_run(args)?;
        }
        Commands::Add { name, version } => {
            cmd_add(&name, &version)?;
        }
        Commands::Remove { name } => {
            cmd_remove(&name)?;
        }
        Commands::Install => {
            cmd_install()?;
        }
        Commands::Test => {
            cmd_test()?;
        }
        Commands::Update => {
            cmd_update()?;
        }
    }

    Ok(())
}

fn cmd_new(name: &str, template_name: &str, author: &str, path: Option<PathBuf>) -> Result<()> {
    println!("🌹 {}", "Creating new Roze project".bright_magenta());
    println!("  Project: {}", name);
    println!("  Template: {}", template_name);
    println!("  Author: {}", author);

    let project_path = path.unwrap_or_else(|| PathBuf::from(name));

    if project_path.exists() {
        return Err(anyhow::anyhow!("Project directory already exists: {}", project_path.display()));
    }

    let template = match template_name {
        "web" => Template::get_web(),
        "library" => Template::get_library(),
        _ => Template::get_default(),
    };

    template.apply(name, author, &project_path)?;

    println!("✅ Project created at: {}", project_path.display());
    println!("\nNext steps:");
    println!("  cd {}", name);
    println!("  roze-pkg install  # Install dependencies");
    println!("  roze-pkg run      # Run the project");

    Ok(())
}

fn cmd_build(_release: bool) -> Result<()> {
    println!("🔨 {}", "Building project...".green());

    let config = load_project_config()?;
    let main_file = config.main;

    let compiler_path = find_compiler()?;
    let status = Command::new(compiler_path)
        .arg("build")
        .arg(&main_file)
        .status()?;

    if status.success() {
        println!("✅ {}", "Build successful!".bright_green());
        Ok(())
    } else {
        Err(anyhow::anyhow!("Build failed"))
    }
}

fn cmd_run(args: Vec<String>) -> Result<()> {
    println!("🚀 {}", "Running project...".yellow());

    let config = load_project_config()?;
    let main_file = config.main;

    let compiler_path = find_compiler()?;

    let status = Command::new(&compiler_path)
        .arg("run")
        .arg(&main_file)
        .args(&args)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Run failed"))
    }
}

fn cmd_add(name: &str, version: &str) -> Result<()> {
    println!("📦 {}", format!("Adding dependency: {}@{}", name, version).green());

    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    dep_manager.add_dependency(name, version)?;
    dep_manager.install_all()?;

    println!("✅ {}", format!("Added {}@{}", name, version).bright_green());
    Ok(())
}

fn cmd_remove(name: &str) -> Result<()> {
    println!("🗑️ {}", format!("Removing dependency: {}", name).red());

    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    dep_manager.remove_dependency(name)?;

    println!("✅ {}", format!("Removed {}", name).bright_green());
    Ok(())
}

fn cmd_install() -> Result<()> {
    println!("📦 {}", "Installing dependencies...".green());

    let project_dir = std::env::current_dir()?;
    let dep_manager = DependencyManager::new(project_dir);
    dep_manager.install_all()?;

    Ok(())
}

fn cmd_test() -> Result<()> {
    println!("🧪 {}", "Running tests...".yellow());

    let config = load_project_config()?;
    let main_file = config.main;

    let compiler_path = find_compiler()?;
    let status = Command::new(compiler_path)
        .arg("run")
        .arg(&main_file)
        .status()?;

    if status.success() {
        println!("✅ {}", "All tests passed!".bright_green());
        Ok(())
    } else {
        Err(anyhow::anyhow!("Tests failed"))
    }
}

fn cmd_update() -> Result<()> {
    println!("🔄 {}", "Updating dependencies...".yellow());
    cmd_install()
}

fn load_project_config() -> Result<ProjectConfig> {
    let config_path = std::env::current_dir()?.join("roze.toml");
    if config_path.exists() {
        ProjectConfig::load(&config_path)
    } else {
        Err(anyhow::anyhow!("No roze.toml found. Are you in a Roze project?"))
    }
}

fn find_compiler() -> Result<PathBuf> {
    // Get the current executable's directory
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));

    // Go up from the package manager: tools/roze-pkg -> tools -> project_root
    let mut current = exe_dir.clone();
    for _ in 0..3 {
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }
    let project_root = current;

    // 1. Check in the main target/release FIRST (this is where it actually is!)
    let project_compiler = project_root.join("target/release/roze");
    if project_compiler.exists() {
        return Ok(project_compiler);
    }

    // 2. Check in the compiler's target/release
    let compiler_dir = project_root.join("compiler/target/release/roze");
    if compiler_dir.exists() {
        return Ok(compiler_dir);
    }

    // 3. Check in current working directory
    let cwd = std::env::current_dir()?;
    let cwd_compiler = cwd.join("target/release/roze");
    if cwd_compiler.exists() {
        return Ok(cwd_compiler);
    }

    // 4. Check if there's a roze binary in PATH
    if let Ok(path) = which::which("roze") {
        return Ok(path);
    }

    // 5. Check one level up from current directory
    if let Some(parent) = cwd.parent() {
        let parent_compiler = parent.join("target/release/roze");
        if parent_compiler.exists() {
            return Ok(parent_compiler);
        }
    }

    Err(anyhow::anyhow!(
        "Could not find Roze compiler.\n\
         Tried locations:\n\
         - {}\n\
         - {}\n\
         - {}\n\
         Please ensure it's built with: cargo build --release -p roze-compiler",
        project_compiler.display(),
        compiler_dir.display(),
        cwd_compiler.display()
    ))
}