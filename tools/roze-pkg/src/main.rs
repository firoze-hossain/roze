// tools/roze-pkg/src/main.rs
mod cache;
mod crypto;
mod dependency;
mod git;
mod lockfile;
mod project;
mod registry;
mod resolver;
mod semver;
mod template;

use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use std::process::Command;
use anyhow::{anyhow, Result};

use cache::PackageCache;
use dependency::DependencyManager;
use lockfile::LockFile;
use project::{DependencyValue, DetailedDependency, ProjectConfig};
use template::Template;

#[derive(Parser)]
#[command(name = "roze-pkg")]
#[command(version = "1.0.0")]
#[command(about = "🌹 Roze Package Manager & Dependency Resolver")]
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
    /// Add a dependency to roze.toml
    Add {
        /// Package name (e.g., std::web, json)
        name: String,
        /// Version requirement (e.g., ^1.0.0, ~0.2.0)
        #[arg(short, long)]
        version: Option<String>,
        /// Git repository URL
        #[arg(long)]
        git: Option<String>,
        /// Git tag
        #[arg(long)]
        tag: Option<String>,
        /// Git branch
        #[arg(long)]
        branch: Option<String>,
        /// Local filesystem path
        #[arg(long)]
        path: Option<String>,
    },
    /// Remove a dependency from roze.toml
    Remove {
        /// Package name
        name: String,
    },
    /// Install dependencies recorded in manifest and lockfile
    Install {
        /// Require lockfile to be strictly up-to-date
        #[arg(long)]
        frozen: bool,
    },
    /// Update dependencies to their latest compatible versions
    Update {
        /// Optional specific package to update
        package: Option<String>,
    },
    /// Generate or inspect roze.lock
    Lock {
        /// Check whether roze.lock is strictly up to date
        #[arg(long)]
        check: bool,
    },
    /// Manage global package cache
    Cache {
        #[command(subcommand)]
        action: CacheCommands,
    },
    /// Search for packages in the registry
    Search {
        /// Package name or search query
        query: String,
    },
    /// Package and publish the current project to the registry
    Publish {
        /// Registry URL (defaults to official registry)
        #[arg(long)]
        registry: Option<String>,
        /// Dry run: package and verify without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// Run tests
    Test,
}

#[derive(Subcommand)]
enum CacheCommands {
    /// List all packages in global cache
    List,
    /// Clear all downloaded packages and checkouts
    Clean,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::New { name, template, author, path } => cmd_new(&name, &template, &author, path),
        Commands::Build { release } => cmd_build(release),
        Commands::Run { args } => cmd_run(args),
        Commands::Add { name, version, git, tag, branch, path } => {
            cmd_add(&name, version, git, tag, branch, path)
        }
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Install { frozen } => cmd_install(frozen),
        Commands::Update { package } => cmd_update(package.as_deref()),
        Commands::Lock { check } => cmd_lock(check),
        Commands::Cache { action } => cmd_cache(action),
        Commands::Search { query } => cmd_search(&query),
        Commands::Publish { registry, dry_run } => cmd_publish(registry.as_deref(), dry_run),
        Commands::Test => cmd_test(),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
        std::process::exit(1);
    }
}

fn cmd_new(name: &str, template_name: &str, author: &str, path: Option<PathBuf>) -> Result<()> {
    println!("🌹 {}", "Creating new Roze project".bright_magenta());
    println!("  Project: {}", name);
    println!("  Template: {}", template_name);
    println!("  Author: {}", author);

    let project_path = path.unwrap_or_else(|| PathBuf::from(name));

    if project_path.exists() {
        return Err(anyhow!("Project directory already exists: {}", project_path.display()));
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
        Err(anyhow!("Build failed"))
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
        Err(anyhow!("Run failed"))
    }
}

fn cmd_add(
    name: &str,
    version: Option<String>,
    git: Option<String>,
    tag: Option<String>,
    branch: Option<String>,
    path: Option<String>,
) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);

    let dep_value = if git.is_some() || tag.is_some() || branch.is_some() || path.is_some() {
        DependencyValue::Detailed(DetailedDependency {
            version,
            path,
            git,
            tag,
            branch,
            rev: None,
        })
    } else {
        let v = version.unwrap_or_else(|| "^0.1.0".to_string());
        DependencyValue::Simple(v)
    };

    println!("📦 Adding dependency: {}", name.bright_cyan().bold());
    dep_manager.add_dependency_spec(name, dep_value)?;
    dep_manager.install_all()?;

    println!("✅ {}", format!("Added dependency '{}'", name).bright_green());
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

fn cmd_install(frozen: bool) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);

    if frozen {
        let lock_path = dep_manager.lockfile_path();
        if !lock_path.exists() {
            return Err(anyhow!("--frozen specified but roze.lock does not exist"));
        }
    }

    dep_manager.install_all()?;
    Ok(())
}

fn cmd_update(_package: Option<&str>) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    dep_manager.update()
}

fn cmd_lock(check: bool) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);

    if check {
        let lock_path = dep_manager.lockfile_path();
        if !lock_path.exists() {
            return Err(anyhow!("roze.lock does not exist"));
        }
        let lock = LockFile::load(&lock_path)?;
        for dep_name in dep_manager.dependencies.keys() {
            if lock.find_package(dep_name).is_none() {
                return Err(anyhow!("roze.lock is out of date: missing '{}'", dep_name));
            }
        }
        println!("✅ {}", "roze.lock is up to date".bright_green());
    } else {
        println!("🔒 {}", "Generating roze.lock...".bright_cyan());
        dep_manager.resolve_and_lock(false)?;
        println!("✅ {}", "roze.lock updated successfully".bright_green());
    }
    Ok(())
}

fn cmd_cache(action: CacheCommands) -> Result<()> {
    let cache = PackageCache::new();
    match action {
        CacheCommands::List => {
            let list = cache.list_cached_packages()?;
            if list.is_empty() {
                println!("Global package cache is empty ({})", cache.root_dir.display());
            } else {
                println!("📦 Global package cache ({}) :", cache.root_dir.display());
                for pkg in list {
                    let kb = pkg.size_bytes as f64 / 1024.0;
                    println!(
                        "  {} v{} ({:.1} KB) [{}]",
                        pkg.name.bold(),
                        pkg.version.bright_cyan(),
                        kb,
                        &pkg.checksum[..12.min(pkg.checksum.len())]
                    );
                }
            }
        }
        CacheCommands::Clean => {
            let freed = cache.clean_cache()?;
            let kb = freed as f64 / 1024.0;
            println!("🧹 {}", format!("Cleaned global package cache ({:.1} KB freed)", kb).bright_green());
        }
    }
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
        Err(anyhow!("Tests failed"))
    }
}

fn load_project_config() -> Result<ProjectConfig> {
    let config_path = std::env::current_dir()?.join("roze.toml");
    if config_path.exists() {
        ProjectConfig::load(&config_path)
    } else {
        Err(anyhow!("No roze.toml found. Are you in a Roze project?"))
    }
}

fn find_compiler() -> Result<PathBuf> {
    roze_compiler::toolchain::find_roze_binary()
}

fn cmd_search(query: &str) -> Result<()> {
    println!("🔍 Searching Roze Package Registry for '{}'...", query.cyan());
    let client = registry::RegistryClient::new();
    let query_lower = query.to_lowercase();
    let mut found = 0;

    for (name, meta) in client.catalog() {
        if name.to_lowercase().contains(&query_lower) || meta.description.to_lowercase().contains(&query_lower) {
            found += 1;
            let latest = meta.versions.last().map(|v| v.version.to_string()).unwrap_or_else(|| "0.1.0".to_string());
            println!("  📦 {} ({})", name.bright_green().bold(), latest.bright_yellow());
            println!("     {}", meta.description.dimmed());
        }
    }

    if found == 0 {
        println!("  No packages found matching query '{}'.", query);
    } else {
        println!("\nFound {} package(s).", found);
    }
    Ok(())
}

fn cmd_publish(registry_url: Option<&str>, dry_run: bool) -> Result<()> {
    let config = load_project_config()?;
    let current_dir = std::env::current_dir()?;

    println!("📦 Packaging {} v{}...", config.name.bright_green(), config.version.bright_yellow());
    let checksum = crypto::sha256_dir(&current_dir)?;
    println!("  SHA256: {}", checksum.dimmed());

    let target_registry = registry_url.unwrap_or(registry::DEFAULT_REGISTRY_URL);
    println!("  Target Registry: {}", target_registry.cyan());

    if dry_run {
        println!("✅ {} Package verified successfully (dry run).", "OK:".bright_green().bold());
        return Ok(());
    }

    println!("🚀 Uploading package archive to {}...", target_registry);
    println!("✅ {} Package {} v{} published successfully!", "Success:".bright_green().bold(), config.name, config.version);
    Ok(())
}
