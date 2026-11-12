// compiler/src/pkg/mod.rs
#![allow(dead_code, unused_imports)]

pub mod cache;
pub mod crypto;
pub mod dependency;
pub mod generator;
pub mod git;
pub mod lockfile;
pub mod project;
pub mod registry;
pub mod resolver;
pub mod semver;
pub mod solver;
pub mod template;
pub mod vendor;
pub mod bench;

use clap::{Parser, Subcommand};
use colored::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;
use anyhow::{anyhow, Result};

use cache::PackageCache;
use dependency::DependencyManager;
use lockfile::ChecksumStatus;
use project::{DependencyValue, DetailedDependency, ProjectConfig};

#[derive(Parser)]
#[command(name = "bsr")]
#[command(version = "1.0.0")]
#[command(about = "🌹 Basir (bsr) - Roze Package Manager & Dependency Resolver (.mfz)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Create a new Roze project in a new directory
    New {
        /// Path to create the project in
        path: PathBuf,
        /// Use a binary (application) template [default]
        #[arg(long, conflicts_with = "lib")]
        bin: bool,
        /// Use a library template
        #[arg(long, conflicts_with = "bin")]
        lib: bool,
        /// Set the resulting package name, defaults to the directory name
        #[arg(long)]
        name: Option<String>,
        /// Initialize a new VCS repository [possible values: git, none]
        #[arg(long)]
        vcs: Option<String>,
        /// Edition to set in manifest (e.g. 2026)
        #[arg(long, default_value = "2026")]
        edition: String,
        /// Author name
        #[arg(short, long, default_value = "Mohammad Firoze")]
        author: String,
        /// Template to use (default, web, library)
        #[arg(short, long)]
        template: Option<String>,
    },
    /// Initialize a new Roze project in an existing directory
    Init {
        /// Directory to initialize [default: current directory]
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Use a binary (application) template
        #[arg(long, conflicts_with = "lib")]
        bin: bool,
        /// Use a library template
        #[arg(long, conflicts_with = "bin")]
        lib: bool,
        /// Set the resulting package name, defaults to the directory name
        #[arg(long)]
        name: Option<String>,
        /// Initialize a new VCS repository [possible values: git, none]
        #[arg(long)]
        vcs: Option<String>,
        /// Edition to set in manifest (e.g. 2026)
        #[arg(long, default_value = "2026")]
        edition: String,
        /// Author name
        #[arg(short, long, default_value = "Mohammad Firoze")]
        author: String,
    },
    /// Build the current project
    Build {
        /// Build in release mode
        #[arg(short, long)]
        release: bool,
        /// Require bsr.lock to be strictly up-to-date
        #[arg(long)]
        locked: bool,
        /// Require bsr.lock to be up-to-date and forbid network access
        #[arg(long)]
        frozen: bool,
        /// Build in 100% offline air-gapped mode using local cache or vendor/
        #[arg(long)]
        offline: bool,
    },
    /// Run the current project
    Run {
        /// Arguments to pass to the program
        args: Vec<String>,
    },
    /// Add a dependency to bsr.mfz
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
    /// Remove a dependency from bsr.mfz
    Remove {
        /// Package name
        name: String,
    },
    /// Install dependencies recorded in manifest and lockfile
    Install {
        /// Require lockfile to be strictly up-to-date and forbid network access
        #[arg(long)]
        frozen: bool,
        /// Require lockfile to be strictly up-to-date
        #[arg(long)]
        locked: bool,
        /// Install in 100% offline air-gapped mode using local cache or vendor/
        #[arg(long)]
        offline: bool,
    },
    /// Update dependencies to their latest compatible versions
    Update {
        /// Optional specific package to update
        package: Option<String>,
    },
    /// Generate, inspect, or cryptographically verify bsr.lock
    Lock {
        /// Check whether bsr.lock is strictly up to date with the manifest
        #[arg(long)]
        check: bool,
        /// Cryptographically verify SHA-256 checksums of installed packages in libs/
        #[arg(long)]
        verify: bool,
        /// Selectively update a specific package in bsr.lock
        #[arg(long)]
        package: Option<String>,
    },
    /// Vendor all dependencies into a local directory for offline air-gapped enterprise builds
    Vendor {
        /// Directory to vendor packages into [default: vendor]
        #[arg(default_value = "vendor")]
        path: PathBuf,
        /// Always include version numbers in directory names (e.g. vendor/foo-1.0.0)
        #[arg(long)]
        versioned_dirs: bool,
        /// Don't delete existing files in vendor directory before vendoring
        #[arg(long)]
        no_delete: bool,
        /// Cryptographically verify SHA-256 integrity of vendored packages
        #[arg(long)]
        verify: bool,
    },
    /// Manage global package cache (~/.bsr/cache/)
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
        /// Hex-encoded Ed25519 32-byte secret key seed for signing
        #[arg(long)]
        key: Option<String>,
        /// Dry run: package and verify without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate a new Ed25519 signing keypair for package releases
    Keygen,
    /// Run tests
    Test,
    /// Run native microbenchmarks with nanosecond timing, statistical warm-up, and throughput analysis
    Bench {
        /// Optional name filter (runs only benchmarks matching filter)
        filter: Option<String>,
        /// Warm-up duration in milliseconds [default: 500]
        #[arg(long, default_value = "500")]
        warmup: u64,
        /// Measurement duration in milliseconds [default: 2000]
        #[arg(long, default_value = "2000")]
        measurement_time: u64,
        /// Number of samples to collect [default: 50]
        #[arg(long, default_value = "50")]
        samples: usize,
        /// Save benchmark results as a named baseline for regression comparison
        #[arg(long)]
        save_baseline: Option<String>,
        /// Compare benchmark results against a previously saved baseline
        #[arg(long)]
        baseline: Option<String>,
        /// Output benchmark results as structured JSON
        #[arg(long)]
        json: bool,
        /// Enable memory allocation and peak RSS tracking
        #[arg(long)]
        profile: bool,
        /// Compile and execute benchmarks using native backend instead of JVM
        #[arg(long)]
        native: bool,
        /// Compile and execute benchmarks using JVM backend instead of native binary
        #[arg(long)]
        jvm: bool,
        /// Print detailed sample distribution percentiles and histogram
        #[arg(short, long)]
        verbose: bool,
        /// Throughput bytes per iteration for bandwidth calculation (e.g., 1048576)
        #[arg(long)]
        throughput_bytes: Option<u64>,
    },
    /// Inspect or validate the project manifest (bsr.mfz)
    Manifest {
        /// Validate manifest syntax and dependencies without modifying
        #[arg(long)]
        check: bool,
    },
    /// Build a package's documentation using rozedoc
    Doc {
        /// Opens the docs in a browser after the operation
        #[arg(long)]
        open: bool,
        /// Disable in-browser live playground simulation buttons
        #[arg(long)]
        no_playground: bool,
        /// Output directory for generated static HTML documentation [default: target/doc]
        #[arg(short, long, default_value = "target/doc")]
        output: String,
        /// Export complete machine-readable AST documentation model as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CacheCommands {
    /// List all packages in global cache
    List,
    /// Clear all downloaded packages and checkouts
    Clean,
    /// Cryptographically verify SHA-256 integrity of all cached packages
    Verify,
}

pub fn run_cli() {
    let cli = Cli::parse();
    let result = run_subcommand(cli.command);
    if let Err(e) = result {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
        std::process::exit(1);
    }
}

pub fn run_subcommand(command: Commands) -> Result<()> {
    match command {
        Commands::New {
            path,
            bin,
            lib,
            name,
            vcs,
            edition,
            author,
            template,
        } => cmd_new(path, bin, lib, name, vcs, edition, author, template),
        Commands::Init {
            path,
            bin,
            lib,
            name,
            vcs,
            edition,
            author,
        } => cmd_init(path, bin, lib, name, vcs, edition, author),
        Commands::Build { release, locked, frozen, offline } => cmd_build(release, locked, frozen, offline),
        Commands::Run { args } => cmd_run(args),
        Commands::Add { name, version, git, tag, branch, path } => {
            cmd_add(&name, version, git, tag, branch, path)
        }
        Commands::Remove { name } => cmd_remove(&name),
        Commands::Install { frozen, locked, offline } => cmd_install(frozen, locked, offline),
        Commands::Update { package } => cmd_update(package.as_deref()),
        Commands::Lock { check, verify, package } => cmd_lock(check, verify, package.as_deref()),
        Commands::Vendor { path, versioned_dirs, no_delete, verify } => {
            cmd_vendor(path, versioned_dirs, no_delete, verify)
        }
        Commands::Cache { action } => cmd_cache(action),
        Commands::Search { query } => cmd_search(&query),
        Commands::Publish { registry, key, dry_run } => cmd_publish(registry.as_deref(), key.as_deref(), dry_run),
        Commands::Keygen => cmd_keygen(),
        Commands::Test => cmd_test(),
        Commands::Bench {
            filter,
            warmup,
            measurement_time,
            samples,
            save_baseline,
            baseline,
            json,
            profile,
            native,
            jvm,
            verbose,
            throughput_bytes,
        } => cmd_bench(
            filter,
            warmup,
            measurement_time,
            samples,
            save_baseline,
            baseline,
            json,
            profile,
            native,
            jvm,
            verbose,
            throughput_bytes,
        ),
        Commands::Manifest { check } => cmd_manifest(check),
        Commands::Doc { open, no_playground, output, json } => cmd_doc(open, no_playground, output, json),
    }
}

pub fn cmd_new(
    path: PathBuf,
    bin: bool,
    lib: bool,
    name: Option<String>,
    vcs: Option<String>,
    edition: String,
    author: String,
    template: Option<String>,
) -> Result<()> {
    let target = if lib {
        Some(generator::TargetKind::Lib)
    } else if bin {
        Some(generator::TargetKind::Bin)
    } else {
        None
    };

    let vcs_kind = match vcs.as_deref() {
        Some(s) => Some(generator::VcsKind::parse(s)?),
        None => None,
    };

    let opts = generator::NewOptions {
        path,
        name,
        target,
        vcs: vcs_kind,
        edition,
        author,
        template,
    };

    generator::generate_new(&opts)
}

pub fn cmd_init(
    path: PathBuf,
    bin: bool,
    lib: bool,
    name: Option<String>,
    vcs: Option<String>,
    edition: String,
    author: String,
) -> Result<()> {
    let target = if lib {
        Some(generator::TargetKind::Lib)
    } else if bin {
        Some(generator::TargetKind::Bin)
    } else {
        None
    };

    let vcs_kind = match vcs.as_deref() {
        Some(s) => Some(generator::VcsKind::parse(s)?),
        None => None,
    };

    let opts = generator::InitOptions {
        path,
        name,
        target,
        vcs: vcs_kind,
        edition,
        author,
    };

    generator::generate_init(&opts)
}

pub fn cmd_build(_release: bool, locked: bool, frozen: bool, offline: bool) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    let lock_path = dep_manager.lockfile_path();
    let lock_filename = lock_path.file_name().and_then(|s| s.to_str()).unwrap_or("bsr.lock");

    if locked || frozen || offline {
        if !lock_path.exists() {
            return Err(anyhow!(
                "the lock file {} needs to be updated but --{} was passed to prevent this",
                lock_filename,
                if offline { "offline" } else if frozen { "frozen" } else { "locked" }
            ));
        }
        if !dep_manager.check_lockfile_up_to_date()? {
            return Err(anyhow!(
                "the lock file {} needs to be updated but --{} was passed to prevent this",
                lock_filename,
                if offline { "offline" } else if frozen { "frozen" } else { "locked" }
            ));
        }
    }

    // Auto-install dependencies if missing from libs/ or if in offline mode
    if dep_manager.has_missing_libs()? || offline {
        dep_manager.install_all_with_options(locked, frozen, offline)?;
    }

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

pub fn cmd_run(args: Vec<String>) -> Result<()> {
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

pub fn cmd_add(
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
            ..Default::default()
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

pub fn cmd_remove(name: &str) -> Result<()> {
    println!("🗑️ {}", format!("Removing dependency: {}", name).red());

    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    dep_manager.remove_dependency(name)?;

    println!("✅ {}", format!("Removed {}", name).bright_green());
    Ok(())
}

pub fn cmd_install(frozen: bool, locked: bool, offline: bool) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    dep_manager.install_all_with_options(locked, frozen, offline)
}

pub fn cmd_vendor(path: PathBuf, versioned_dirs: bool, no_delete: bool, verify: bool) -> Result<()> {
    let project_dir = std::env::current_dir()?;

    if verify {
        println!("🔍 {}", format!("Verifying cryptographic checksums of vendored packages in {}...", path.display()).cyan());
        let results = vendor::VendorManager::verify_vendor(&project_dir, &path)?;
        if results.is_empty() {
            println!("  No vendored packages found to verify in {}", path.display());
            return Ok(());
        }

        let mut any_mismatch = false;
        let mut any_missing = false;

        for r in results {
            match r.status {
                vendor::VendorStatus::Ok => {
                    println!("  {} {} v{} (checksums verified)", "✓".green(), r.name.bold(), r.version.cyan());
                }
                vendor::VendorStatus::ChecksumMismatch { expected, actual } => {
                    any_mismatch = true;
                    println!("  {} {} v{} CHECKSUM MISMATCH!", "✗".red().bold(), r.name.bold(), r.version);
                    println!("    Expected: {}", expected.yellow());
                    println!("    Actual:   {}", actual.red());
                }
                vendor::VendorStatus::FileTampered { file, expected, actual } => {
                    any_mismatch = true;
                    println!("  {} {} v{} file '{}' TAMPERED!", "✗".red().bold(), r.name.bold(), r.version, file);
                    println!("    Expected: {}", expected.yellow());
                    println!("    Actual:   {}", actual.red());
                }
                vendor::VendorStatus::FileMissing { file } => {
                    any_missing = true;
                    println!("  {} {} v{} file '{}' is MISSING!", "✗".red().bold(), r.name.bold(), r.version, file);
                }
                vendor::VendorStatus::MissingChecksumFile => {
                    any_missing = true;
                    println!("  {} {} v{} missing .bsr-checksum.json!", "✗".red().bold(), r.name.bold(), r.version);
                }
                vendor::VendorStatus::MissingDirectory => {
                    any_missing = true;
                    println!("  {} {} v{} directory is MISSING!", "✗".red().bold(), r.name.bold(), r.version);
                }
            }
        }

        if any_mismatch {
            return Err(anyhow!("Supply-chain security violation: one or more vendored package checksums do not match!"));
        }
        if any_missing {
            return Err(anyhow!("One or more vendored packages or files are missing: run 'bsr vendor' to re-vendor"));
        }

        println!("✅ {}", "All vendored packages cryptographically verified with 100% integrity!".bright_green());
        return Ok(());
    }

    println!("📦 {}", format!("Vendoring dependencies into {}...", path.display()).bright_magenta());
    let opts = vendor::VendorOptions {
        output_dir: path.clone(),
        versioned_dirs,
        no_delete,
    };
    let summary = vendor::VendorManager::vendor_dependencies(&project_dir, &opts)?;
    for pkg in &summary.packages {
        let kb = pkg.total_bytes as f64 / 1024.0;
        let preview = if pkg.checksum.len() >= 12 { &pkg.checksum[..12] } else { &pkg.checksum };
        println!("  {} {} v{} ({:.1} KB, {} files) [{}]", "✓".green(), pkg.name.bold(), pkg.version.bright_cyan(), kb, pkg.file_count, preview);
    }
    let total_kb = summary.total_bytes as f64 / 1024.0;
    println!("✅ {}", format!("Vendoring complete! {} packages vendored into {}/ ({:.1} KB)", summary.packages.len(), path.display(), total_kb).bright_green());
    println!("💡 {}", "To build using vendored dependencies in offline air-gapped environments, run:".yellow());
    println!("     bsr build --offline");
    Ok(())
}

pub fn cmd_update(package: Option<&str>) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    dep_manager.update(package)
}

pub fn cmd_lock(check: bool, verify: bool, package: Option<&str>) -> Result<()> {
    let project_dir = std::env::current_dir()?;
    let mut dep_manager = DependencyManager::new(project_dir);
    let lock_path = dep_manager.lockfile_path();
    let lock_filename = lock_path.file_name().and_then(|s| s.to_str()).unwrap_or("bsr.lock");

    if verify {
        println!("🔍 {}", "Verifying cryptographic checksums of installed packages...".cyan());
        let reports = dep_manager.verify_installed()?;
        if reports.is_empty() {
            println!("  No dependencies to verify");
            return Ok(());
        }

        let mut any_mismatch = false;
        let mut any_missing = false;

        for r in reports {
            match r.status {
                ChecksumStatus::Valid => {
                    println!("  {} {} v{} (checksum verified)", "✓".green(), r.name.bold(), r.version.cyan());
                }
                ChecksumStatus::Mismatch { expected, actual } => {
                    any_mismatch = true;
                    println!("  {} {} v{} CHECKSUM MISMATCH!", "✗".red().bold(), r.name.bold(), r.version);
                    println!("    Expected: {}", expected.yellow());
                    println!("    Actual:   {}", actual.red());
                }
                ChecksumStatus::Missing => {
                    any_missing = true;
                    println!("  {} {} v{} is MISSING from libs/", "✗".red().bold(), r.name.bold(), r.version);
                }
                ChecksumStatus::Unpinned { computed } => {
                    let preview = if computed.len() >= 12 { &computed[..12] } else { &computed };
                    println!("  {} {} v{} has no pinned checksum in lockfile (computed: {})", "!".yellow(), r.name, r.version, preview);
                }
            }
        }

        if any_mismatch {
            return Err(anyhow!("Supply-chain security violation: one or more package checksums do not match {}!", lock_filename));
        }
        if any_missing {
            return Err(anyhow!("One or more locked packages are missing from libs/: run 'bsr install'"));
        }

        println!("✅ {}", "All package checksums verified with 100% cryptographic integrity!".bright_green());
        return Ok(());
    }

    if check {
        if !lock_path.exists() {
            return Err(anyhow!("{} does not exist: run 'bsr lock' to create it", lock_filename));
        }
        let is_up_to_date = dep_manager.check_lockfile_up_to_date()?;
        if !is_up_to_date {
            return Err(anyhow!("{} is out of date: run 'bsr lock' or 'bsr install' to update it", lock_filename));
        }
        println!("✅ {}", format!("{} is strictly up to date with manifest", lock_filename).bright_green());
        return Ok(());
    }

    if let Some(pkg) = package {
        println!("🔒 Updating package '{}' in {}...", pkg.bold(), lock_filename);
        let mut to_update = HashSet::new();
        to_update.insert(pkg.to_string());
        dep_manager.resolve_and_lock_with_targets(false, &to_update)?;
        println!("✅ {} updated successfully in {}", pkg.green(), lock_filename);
        return Ok(());
    }

    println!("🔒 {}", format!("Generating {}...", lock_filename).bright_cyan());
    dep_manager.resolve_and_lock(false)?;
    println!("✅ {}", format!("{} generated successfully with cryptographic checksums", lock_filename).bright_green());
    Ok(())
}

pub fn cmd_cache(action: CacheCommands) -> Result<()> {
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
        CacheCommands::Verify => {
            println!("🔍 {}", format!("Cryptographically verifying global package cache ({})...", cache.root_dir.display()).cyan());
            let reports = cache.verify_all_packages()?;
            if reports.is_empty() {
                println!("  Global package cache is empty ({})", cache.root_dir.display());
                return Ok(());
            }

            let mut any_corrupted = false;
            for r in reports {
                match r.status {
                    cache::CacheStatus::Valid => {
                        println!("  {} {} v{} (hash verified)", "✓".green(), r.name.bold(), r.version.cyan());
                    }
                    cache::CacheStatus::Corrupted { expected, actual } => {
                        any_corrupted = true;
                        println!("  {} {} v{} CORRUPTED CHECKSUM MISMATCH!", "✗".red().bold(), r.name.bold(), r.version);
                        println!("    Expected: {}", expected.yellow());
                        println!("    Actual:   {}", actual.red());
                    }
                    cache::CacheStatus::Unchecksummed { actual } => {
                        let preview = if actual.len() >= 12 { &actual[..12] } else { &actual };
                        println!("  {} {} v{} (unpinned, computed: {})", "!".yellow(), r.name.bold(), r.version, preview);
                    }
                }
            }

            if any_corrupted {
                return Err(anyhow!("Supply-chain security violation: one or more packages in global cache are corrupted!"));
            }

            println!("✅ {}", "All cached packages cryptographically verified with 100% integrity!".bright_green());
        }
    }
    Ok(())
}

pub fn cmd_test() -> Result<()> {
    println!("🧪 {}", "Running tests...".yellow());

    let compiler_path = find_compiler()?;
    let cur = std::env::current_dir()?;
    let tests_dir = cur.join("tests");
    let mut ran_any = false;

    if tests_dir.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(&tests_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("roze"))
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            ran_any = true;
            println!("  Running test fixture: {}", path.file_name().unwrap().to_string_lossy().cyan());
            let status = Command::new(&compiler_path)
                .arg("run")
                .arg(&path)
                .status()?;
            if !status.success() {
                return Err(anyhow!("Test failed: {}", path.display()));
            }
        }
    }

    if !ran_any {
        let config = load_project_config()?;
        let main_file = config.main;

        let status = Command::new(&compiler_path)
            .arg("run")
            .arg(&main_file)
            .status()?;

        if !status.success() {
            return Err(anyhow!("Tests failed"));
        }
    }

    println!("✅ {}", "All tests passed!".bright_green());
    Ok(())
}

pub fn cmd_bench(
    filter: Option<String>,
    warmup: u64,
    measurement_time: u64,
    samples: usize,
    save_baseline: Option<String>,
    baseline: Option<String>,
    json: bool,
    profile: bool,
    native: bool,
    jvm: bool,
    verbose: bool,
    throughput_bytes: Option<u64>,
) -> Result<()> {
    let cur = std::env::current_dir()?;
    let config = bench::BenchmarkConfig {
        filter,
        warmup_ms: warmup,
        measurement_time_ms: measurement_time,
        samples,
        save_baseline,
        baseline,
        json,
        profile,
        native,
        jvm,
        verbose,
        throughput_bytes,
    };

    if !json {
        println!("⏱️  {}", "Roze Native Microbenchmark & Profiler Engine ('bsr bench')".yellow().bold());
    }
    let _ = bench::run_benchmarks(&config, &cur)?;
    if !json {
        println!("\n✅ {}", "Benchmark suite finished successfully!".bright_green());
    }
    Ok(())
}

fn load_project_config() -> Result<ProjectConfig> {
    let cur = std::env::current_dir()?;
    if let Some(config_path) = ProjectConfig::find_manifest(&cur) {
        ProjectConfig::load(&config_path)
    } else {
        Err(anyhow!("No bsr.mfz (or roze.mfz / roze.toml) found. Are you in a Roze project?"))
    }
}

fn find_compiler() -> Result<PathBuf> {
    crate::toolchain::find_roze_binary()
}

pub fn cmd_search(query: &str) -> Result<()> {
    println!("🔍 Searching Roze Package Registry for '{}'...", query.cyan());
    let client = registry::RegistryClient::new();
    let results = client.search_packages(query);

    if results.is_empty() {
        println!("  No packages found matching query '{}'.", query);
    } else {
        println!("\nFound {} package(s):", results.len());
        for pkg in results {
            println!("  📦 {} ({}) - {} downloads", pkg.name.bright_green().bold(), pkg.version.bright_yellow(), pkg.downloads);
            println!("     {}", pkg.description.dimmed());
            if let Some(ref repo) = pkg.repository {
                println!("     repo: {}", repo.cyan());
            }
        }
    }
    Ok(())
}

pub fn cmd_keygen() -> Result<()> {
    let kp = crypto::ed25519_generate_keypair();
    println!("🌹 {}", "Ed25519 Digital Signing Keypair Generated".bright_magenta().bold());
    println!("  Public Key:  {}", kp.public_key_hex().bright_green().bold());
    println!("  Secret Seed: {}", kp.seed_hex().bright_yellow());
    println!("\n  {}", "Keep your secret seed secure and private.".dimmed());
    println!("  {}", "To sign a package release, pass: --key <seed_hex> to 'bsr publish'.".dimmed());
    Ok(())
}

pub fn cmd_publish(registry_url: Option<&str>, key_hex: Option<&str>, dry_run: bool) -> Result<()> {
    let config = load_project_config()?;
    let current_dir = std::env::current_dir()?;

    println!("📦 Packaging {} v{}...", config.name.bright_green(), config.version.bright_yellow());
    let checksum = crypto::sha256_dir(&current_dir)?;
    println!("  SHA256: {}", checksum.dimmed());

    // Ed25519 cryptographic signing
    let (signature_opt, pubkey_opt) = if let Some(hex_str) = key_hex {
        let bytes = crypto::hex_to_bytes(hex_str)?;
        if bytes.len() != 32 {
            return Err(anyhow!("Ed25519 secret key must be 32 bytes (64 hex characters), got {} bytes", bytes.len()));
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        let envelope = registry::SignedPackageEnvelope::new(&config.name, &config.version, &checksum, &seed);
        if !envelope.verify()? {
            return Err(anyhow!("Internal Ed25519 self-verification failed"));
        }
        println!("  Ed25519 Signature:  {}", envelope.signature.cyan());
        println!("  Ed25519 Public Key: {}", envelope.public_key.green());
        println!("  Cryptographic Envelope: {}", "VERIFIED (RFC 8032)".bright_green().bold());
        (Some(envelope.signature), Some(envelope.public_key))
    } else {
        println!("  Signing: {}", "unsigned (pass --key to sign with Ed25519)".dimmed());
        (None, None)
    };

    let target_registry = registry_url.unwrap_or(registry::DEFAULT_REGISTRY_URL);
    println!("  Target Registry: {}", target_registry.cyan());

    if dry_run {
        println!("✅ {} Package verified successfully (dry run).", "OK:".bright_green().bold());
        return Ok(());
    }

    println!("🚀 Uploading package archive to {}...", target_registry);
    let mut reg_client = registry::RegistryClient::new().with_url(target_registry);
    let pub_req = registry::PublishRequest {
        name: config.name.clone(),
        version: config.version.clone(),
        description: config.description.unwrap_or_else(|| format!("Package {}", config.name)),
        authors: config.authors,
        license: config.license,
        checksum,
        dependencies: HashMap::new(),
        signature: signature_opt,
        public_key: pubkey_opt,
    };
    reg_client.publish_package(&pub_req)?;
    println!("✅ {} Package {} v{} published successfully!", "Success:".bright_green().bold(), config.name, config.version);
    Ok(())
}

pub fn cmd_manifest(check: bool) -> Result<()> {
    let config = load_project_config()?;
    config.validate()?;

    if check {
        println!("✅ {}", "Manifest syntax, dependencies, and feature flags are valid!".bright_green());
        return Ok(());
    }

    println!("🌹 {}", "Basir Project Manifest (bsr.mfz)".bright_magenta().bold());
    println!("  Package:     {} v{}", config.name.bright_green().bold(), config.version.bright_yellow());
    if let Some(ref ed) = config.edition {
        println!("  Edition:     {}", ed.cyan());
    }
    if !config.authors.is_empty() {
        println!("  Authors:     {}", config.authors.join(", "));
    }
    if let Some(ref lic) = config.license {
        println!("  License:     {}", lic);
    }
    if let Some(ref desc) = config.description {
        println!("  Description: {}", desc);
    }
    println!("  Main:        {}", config.main);
    if let Some(ref tgt) = config.target {
        println!("  Target:      {}", tgt);
    }

    if !config.features.is_empty() {
        println!("\n  {}", "Feature Flags:".bright_cyan().bold());
        for (feat, deps) in &config.features {
            if deps.is_empty() {
                println!("    • {}", feat.bold());
            } else {
                println!("    • {}: [{}]", feat.bold(), deps.join(", ").dimmed());
            }
        }
    }

    if !config.dependencies.is_empty() {
        println!("\n  {} ({})", "Dependencies:".bright_cyan().bold(), config.dependencies.len());
        for (name, dep) in &config.dependencies {
            let opt_badge = if dep.is_optional() { " (optional)".dimmed().to_string() } else { "".to_string() };
            match dep {
                DependencyValue::Simple(ver) => {
                    println!("    • {} {}{}", name.green(), ver.yellow(), opt_badge);
                }
                DependencyValue::Detailed(d) => {
                    let mut details = Vec::new();
                    if let Some(ref v) = d.version { details.push(format!("v{}", v)); }
                    if let Some(ref p) = d.path { details.push(format!("path: {}", p)); }
                    if let Some(ref g) = d.git { details.push(format!("git: {}", g)); }
                    if !d.features.is_empty() { details.push(format!("features: {:?}", d.features)); }
                    println!("    • {} [{}{}]", name.green(), details.join(", "), opt_badge);
                }
            }
        }
    }

    if !config.targets.is_empty() {
        println!("\n  {}", "Target Configurations:".bright_cyan().bold());
        for (tname, tcfg) in &config.targets {
            println!("    • [target.{}]: {} deps, {} flags", tname.yellow(), tcfg.dependencies.len(), tcfg.flags.len());
        }
    }

    Ok(())
}

pub fn cmd_doc(open: bool, no_playground: bool, output: String, json: bool) -> Result<()> {
    let current_dir = std::env::current_dir()?;
    let config = load_project_config()?;
    println!(
        "{} Building documentation for {} v{}...",
        "📚".magenta(),
        config.name.bold(),
        config.version
    );

    let generator = crate::doc::DocGenerator::new(crate::doc::DocGeneratorOptions {
        output_dir: output.clone(),
        enable_playground: !no_playground,
        export_json: json,
    });

    let package = crate::doc::extract_package(&current_dir)?;
    generator.generate(&package)?;

    let index_path = PathBuf::from(&output).join("index.html");
    println!(
        "{} Documentation generated at: {}",
        "✨".green(),
        index_path.display().to_string().bold()
    );

    if open {
        println!("🌐 Opening documentation in web browser...");
        #[cfg(target_os = "linux")]
        let _ = Command::new("xdg-open").arg(&index_path).spawn();
        #[cfg(target_os = "macos")]
        let _ = Command::new("open").arg(&index_path).spawn();
        #[cfg(target_os = "windows")]
        let _ = Command::new("cmd").args(["/C", "start", &index_path.to_string_lossy()]).spawn();
    }

    Ok(())
}

