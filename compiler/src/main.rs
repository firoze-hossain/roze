// compiler/src/main.rs
//
// 🌹 Roze Monolithic Developer Toolchain CLI (M6-T09)
// Unifies Compiler, Build System, Test Runner, Formatter, Doc Engine, REPL,
// and 'bsr' Package Manager into a single hyper-fast monolithic executable.

use std::fs;
use std::path::{Path, PathBuf};
use clap::{Parser as ClapParser, Subcommand};
use colored::*;

mod lexer;
mod parser;
mod codegen;
mod semantic;
mod error;
mod imports;
mod ir;
mod target;
mod doc;
mod pkg;
mod fmt;
mod repl;
mod test_runner;
mod toolchain;

use lexer::tokenize;
use parser::parse;
use semantic::check_and_lower_with_options;
use error::RozeError;
pub use target::Target;

#[derive(ClapParser)]
#[command(name = "roze")]
#[command(version = "1.0.0")]
#[command(about = "🌹 Roze Monolithic Developer Toolchain & Compiler Suite")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable debug output (shows tokens and compiler details)
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a Roze file or whole project package
    Build {
        /// Input file (optional, defaults to project build or src/main.roze)
        #[arg(value_name = "FILE", default_value = "src/main.roze")]
        file: String,
        /// Output directory
        #[arg(short, long)]
        output: Option<String>,
        /// Extra classpath entries for javac (for --target jvm)
        #[arg(long)]
        classpath: Option<String>,
        /// Which backend to compile with
        #[arg(long, value_enum, default_value_t = Target::Jvm)]
        target: Target,
        /// Extra shared/static libraries to link against (for --target native)
        #[arg(long = "link", value_name = "LIB", action = clap::ArgAction::Append)]
        link: Vec<String>,
        /// Custom linker script (for --target freestanding)
        #[arg(short = 'T', long = "linker-script", value_name = "SCRIPT")]
        linker_script: Option<String>,
        /// Custom entry point symbol (for --target freestanding, defaults to _start)
        #[arg(long = "entry", value_name = "ENTRY")]
        entry: Option<String>,
        /// Disable dynamic heap allocation (for --target freestanding)
        #[arg(long = "no-heap")]
        no_heap: bool,
        /// Disable floating-point and vector registers for ISR bare-metal safety
        #[arg(long = "disable-float-registers", alias = "no-float")]
        disable_float_registers: bool,
        /// Target microcontroller chip (stm32, rp2040, esp32)
        #[arg(long = "chip", value_name = "CHIP")]
        chip: Option<String>,
        /// Build in release mode
        #[arg(short, long)]
        release: bool,
    },
    /// Compile and run a Roze file or project package
    Run {
        /// Input file (optional, defaults to project run or src/main.roze)
        #[arg(value_name = "FILE", default_value = "src/main.roze")]
        file: String,
        /// Extra classpath entries for javac/java
        #[arg(long)]
        classpath: Option<String>,
        /// Which backend to compile and run with
        #[arg(long, value_enum, default_value_t = Target::Jvm)]
        target: Target,
        /// Extra shared/static libraries to link against (for --target native)
        #[arg(long = "link", value_name = "LIB", action = clap::ArgAction::Append)]
        link: Vec<String>,
        /// Custom linker script (for --target freestanding)
        #[arg(short = 'T', long = "linker-script", value_name = "SCRIPT")]
        linker_script: Option<String>,
        /// Custom entry point symbol (for --target freestanding)
        #[arg(long = "entry", value_name = "ENTRY")]
        entry: Option<String>,
        /// Disable dynamic heap allocation (for --target freestanding)
        #[arg(long = "no-heap")]
        no_heap: bool,
        /// Disable floating-point and vector registers for ISR bare-metal safety
        #[arg(long = "disable-float-registers", alias = "no-float")]
        disable_float_registers: bool,
        /// Target microcontroller chip (stm32, rp2040, esp32)
        #[arg(long = "chip", value_name = "CHIP")]
        chip: Option<String>,
        /// Arguments to pass to the running program
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Run unit and integration tests across files or package
    Test {
        /// Target test file or directory path [default: tests]
        #[arg(value_name = "PATH", default_value = "tests")]
        path: PathBuf,
        /// Filter tests by name substring
        #[arg(short, long)]
        filter: Option<String>,
        /// Print stdout from passing tests
        #[arg(long)]
        nocapture: bool,
        /// Verbose test output
        #[arg(short, long)]
        verbose: bool,
        /// Which backend to run tests with
        #[arg(long, value_enum, default_value_t = Target::Jvm)]
        target: Target,
    },
    /// Format Roze source code files
    Fmt {
        /// Files or directories to format [default: .]
        #[arg(value_name = "PATHS")]
        paths: Vec<PathBuf>,
        /// Check formatting without writing changes to disk
        #[arg(long)]
        check: bool,
        /// Indentation width in spaces [default: 4]
        #[arg(short, long, default_value_t = 4)]
        indent: usize,
        /// Verbose formatting output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Launch interactive Read-Eval-Print Loop (REPL) console
    Repl,
    /// Generate static HTML documentation with interactive playground
    Doc {
        /// Package root directory or single .roze file to document [default: .]
        #[arg(value_name = "INPUT", default_value = ".")]
        input: PathBuf,
        /// Output directory for generated static HTML documentation [default: target/doc]
        #[arg(short, long, default_value = "target/doc")]
        output: String,
        /// Override documented package name
        #[arg(long)]
        package_name: Option<String>,
        /// Override documented package version
        #[arg(long)]
        package_version: Option<String>,
        /// Disable in-browser live playground simulation buttons
        #[arg(long)]
        no_playground: bool,
        /// Export complete machine-readable AST documentation model as JSON
        #[arg(long)]
        json: bool,
        /// Automatically open generated documentation in default web browser
        #[arg(long)]
        open: bool,
    },
    /// Fast syntax and semantic type check without generating output files
    Check {
        /// Input file to verify [default: src/main.roze]
        #[arg(value_name = "FILE", default_value = "src/main.roze")]
        file: String,
    },
    /// Lower AST to Typed SSA IR and dump textual intermediate representation
    Ssa {
        /// Input file
        #[arg(value_name = "FILE", default_value = "src/main.roze")]
        file: String,
    },
    /// Create a new Roze package with manifest, src/, and tests/
    New {
        /// Package path/name
        path: PathBuf,
        /// Use a binary (application) template [default]
        #[arg(long, conflicts_with = "lib")]
        bin: bool,
        /// Initialize as a library package instead of an application binary
        #[arg(long, conflicts_with = "bin")]
        lib: bool,
        /// Set the resulting package name
        #[arg(long)]
        name: Option<String>,
        /// Initialize a new VCS repository [possible values: git, none]
        #[arg(long)]
        vcs: Option<String>,
        /// Edition to set in manifest [default: 2026]
        #[arg(long, default_value = "2026")]
        edition: String,
        /// Author name
        #[arg(short, long, default_value = "Mohammad Firoze")]
        author: String,
        /// Template to scaffold from
        #[arg(long)]
        template: Option<String>,
    },
    /// Initialize an existing directory as a Roze package
    Init {
        /// Optional path to directory [default: .]
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
        /// Use a binary (application) template [default]
        #[arg(long, conflicts_with = "lib")]
        bin: bool,
        /// Initialize as a library package instead of an application binary
        #[arg(long, conflicts_with = "bin")]
        lib: bool,
        /// Set the resulting package name
        #[arg(long)]
        name: Option<String>,
        /// Initialize a new VCS repository [possible values: git, none]
        #[arg(long)]
        vcs: Option<String>,
        /// Edition to set in manifest [default: 2026]
        #[arg(long, default_value = "2026")]
        edition: String,
        /// Author name
        #[arg(short, long, default_value = "Mohammad Firoze")]
        author: String,
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
    /// Package and publish the current project to the registry
    Publish {
        /// Registry URL
        #[arg(long)]
        registry: Option<String>,
        /// Hex-encoded Ed25519 32-byte secret key seed for signing
        #[arg(long)]
        key: Option<String>,
        /// Dry run: package and verify without uploading
        #[arg(long)]
        dry_run: bool,
    },
    /// Search for packages in the registry
    Search {
        /// Package name or search query
        query: String,
    },
    /// Run native microbenchmarks with nanosecond timing and throughput analysis
    Bench {
        /// Optional name filter
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
        /// Save benchmark results as a named baseline
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
        /// Throughput bytes per iteration
        #[arg(long)]
        throughput_bytes: Option<u64>,
    },
    /// Clean build artifacts (target/, .roze/, .bsr/)
    Clean,
    /// Package manager subcommands ('bsr' namespace)
    Pkg {
        #[command(subcommand)]
        action: pkg::Commands,
    },
    /// Generate JavaScript & TypeScript bindings from a WebAssembly module
    Bindgen {
        /// WebAssembly file (.wasm)
        #[arg(value_name = "WASM_FILE")]
        wasm_file: String,
        /// Output directory for generated JS/TS bindings
        #[arg(short, long, default_value = "pkg")]
        out_dir: String,
    },
    /// Program firmware image directly onto target microcontroller flash
    Flash {
        /// Source file (.roze) or compiled firmware (.bin, .hex, .elf)
        #[arg(value_name = "FILE")]
        file: String,
        /// Target chip family (stm32, rp2040, esp32)
        #[arg(short, long, default_value = "stm32")]
        chip: String,
        /// Programmer communication serial port
        #[arg(short, long)]
        port: Option<String>,
        /// Debug probe interface (stlink, jlink, cmsis-dap)
        #[arg(short, long)]
        interface: Option<String>,
    },
    /// Parse CMSIS-SVD hardware description and generate Roze PAC
    Svd {
        /// SVD XML file path or template name ('stm32f103', 'rp2040', 'esp32c3')
        #[arg(value_name = "SVD_FILE")]
        svd_file: String,
        /// Output directory for generated Roze PAC
        #[arg(short, long, default_value = "pac")]
        out_dir: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build { file, output: _, classpath, target, link, linker_script, entry, no_heap, disable_float_registers, chip, release } => {
            // Smart build: if file is default src/main.roze and bsr manifest exists, run package build
            if file == "src/main.roze" && (Path::new("bsr.mfz").exists() || Path::new("roze.mfz").exists() || Path::new("roze.toml").exists()) {
                pkg::cmd_build(release, false, false, false).map_err(|e| {
                    eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                    ()
                })
            } else {
                build_file(&file, cli.debug, classpath.as_deref(), target, &link, linker_script.as_deref(), entry.as_deref(), no_heap, disable_float_registers, chip.as_deref())
            }
        }
        Commands::Run { file, classpath, target, link, linker_script, entry, no_heap, disable_float_registers, chip, args } => {
            // Smart run: if file is default src/main.roze and bsr manifest exists, run package run
            if file == "src/main.roze" && (Path::new("bsr.mfz").exists() || Path::new("roze.mfz").exists() || Path::new("roze.toml").exists()) {
                pkg::cmd_run(args).map_err(|e| {
                    eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                    ()
                })
            } else {
                build_file(&file, cli.debug, classpath.as_deref(), target, &link, linker_script.as_deref(), entry.as_deref(), no_heap, disable_float_registers, chip.as_deref())
                    .and_then(|_| run_file(&file, classpath.as_deref(), target))
            }
        }
        Commands::Test { path, filter, nocapture, verbose, target } => {
            test_runner::run_test_suite(&path, filter.as_deref(), nocapture, verbose, target).map(|_| ()).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Fmt { paths, check, indent, verbose } => {
            fmt::run_fmt_cli(&paths, check, indent, verbose).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Repl => {
            repl::run_repl_cli().map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Doc { input, output, package_name, package_version, no_playground, json, open } => {
            doc::run_doc_options(&input, &output, package_name.as_deref(), package_version.as_deref(), !no_playground, json, open).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Check { file } => {
            check_file(&file)
        }
        Commands::Ssa { file } => {
            dump_ssa(&file)
        }
        Commands::New { path, bin, lib, name, vcs, edition, author, template } => {
            pkg::cmd_new(path, bin, lib, name, vcs, edition, author, template).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Init { path, bin, lib, name, vcs, edition, author } => {
            pkg::cmd_init(path, bin, lib, name, vcs, edition, author).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Add { name, version, git, tag, branch, path } => {
            pkg::cmd_add(&name, version, git, tag, branch, path).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Remove { name } => {
            pkg::cmd_remove(&name).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Install { frozen, locked, offline } => {
            pkg::cmd_install(frozen, locked, offline).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Update { package } => {
            pkg::cmd_update(package.as_deref()).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Lock { check, verify, package } => {
            pkg::cmd_lock(check, verify, package.as_deref()).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Vendor { path, versioned_dirs, no_delete, verify } => {
            pkg::cmd_vendor(path, versioned_dirs, no_delete, verify).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Publish { registry, key, dry_run } => {
            pkg::cmd_publish(registry.as_deref(), key.as_deref(), dry_run).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Search { query } => {
            pkg::cmd_search(&query).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Bench { filter, warmup, measurement_time, samples, save_baseline, baseline, json, profile, native, jvm, verbose, throughput_bytes } => {
            pkg::cmd_bench(filter, warmup, measurement_time, samples, save_baseline, baseline, json, profile, native, jvm, verbose, throughput_bytes).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Clean => {
            clean_project().map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Pkg { action } => {
            pkg::run_subcommand(action).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Bindgen { wasm_file, out_dir } => {
            let wasm_p = Path::new(&wasm_file);
            let out_p = Path::new(&out_dir);
            roze_bindgen::generate_bindings(wasm_p, out_p).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Flash { file, chip, port, interface } => {
            let bin_path = if file.ends_with(".roze") {
                let target = if chip.contains("rp2040") || chip.contains("pico") {
                    Target::Thumbv6mNoneEabi
                } else if chip.contains("esp32") {
                    Target::Riscv32imc
                } else {
                    Target::Thumbv7mNoneEabi
                };
                if let Err(()) = build_file(&file, false, None, target, &[], None, None, false, false, Some(&chip)) {
                    eprintln!("{} Failed to compile firmware for flashing", "❌ Error:".bright_red().bold());
                    std::process::exit(1);
                }
                let stem = codegen::class_name_from_path(&file);
                let cand1 = PathBuf::from(format!("{}.bin", stem));
                let cand2 = Path::new(&file).with_extension("bin");
                if cand1.exists() {
                    cand1
                } else if cand2.exists() {
                    cand2
                } else {
                    cand1
                }
            } else {
                PathBuf::from(&file)
            };

            codegen::embedded::flash_firmware(&bin_path, &chip, port.as_deref(), interface.as_deref()).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
        Commands::Svd { svd_file, out_dir } => {
            let svd_p = Path::new(&svd_file);
            let out_p = Path::new(&out_dir);
            roze_svd::process_svd_file(svd_p, out_p).map_err(|e| {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                ()
            })
        }
    };

    if result.is_err() {
        std::process::exit(1);
    }
}

fn check_file(filename: &str) -> Result<(), ()> {
    println!("{}", "🔍 [Roze Check] Validating syntax and semantics...".cyan().bold());
    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} couldn't read '{}': {}", "❌ Error:".bright_red().bold(), filename, e);
            return Err(());
        }
    };
    let tokens = tokenize(&source);
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };
    let base_dir = Path::new(filename).parent().unwrap_or_else(|| Path::new("."));
    let resolved_program = match imports::resolve_imports(program, base_dir) {
        Ok(res) => res,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };
    if let Err(e) = check_and_lower_with_options(&resolved_program, false, false) {
        report_error(&e, filename, &source);
        return Err(());
    }
    println!("{} Semantic check passed for {}", "✅ [Check Passed]".bright_green().bold(), filename.bold());
    Ok(())
}

fn dump_ssa(filename: &str) -> Result<(), ()> {
    println!("{} {}", "⚡ [Roze SSA] Lowering to Typed SSA IR:".cyan().bold(), filename.bold());
    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} couldn't read '{}': {}", "❌ Error:".bright_red().bold(), filename, e);
            return Err(());
        }
    };
    let tokens = tokenize(&source);
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };
    let base_dir = Path::new(filename).parent().unwrap_or_else(|| Path::new("."));
    let resolved_program = match imports::resolve_imports(program, base_dir) {
        Ok(res) => res,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };
    let typed_program = match check_and_lower_with_options(&resolved_program, false, false) {
        Ok(tp) => tp,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };

    println!("\n==================================================================");
    println!("🌹 Typed SSA Intermediate Representation: {}", filename);
    println!("==================================================================");
    for stmt in &typed_program.statements {
        println!("  {:?}", stmt);
    }
    println!("==================================================================\n");
    Ok(())
}

fn clean_project() -> Result<(), anyhow::Error> {
    println!("🧹 {}", "Cleaning build artifacts...".yellow());
    let mut cleaned = 0;
    for dir in &["target", ".roze", ".bsr"] {
        let p = Path::new(dir);
        if p.exists() {
            fs::remove_dir_all(p)?;
            println!("  {} Removed {}", "✓".green(), dir);
            cleaned += 1;
        }
    }
    if cleaned == 0 {
        println!("  No build artifacts found.");
    }
    println!("✅ {}", "Clean complete!".bright_green());
    Ok(())
}

fn build_file(
    filename: &str,
    debug: bool,
    classpath: Option<&str>,
    target: Target,
    link_libs: &[String],
    linker_script: Option<&str>,
    entry: Option<&str>,
    no_heap: bool,
    disable_float_registers: bool,
    chip: Option<&str>,
) -> Result<(), ()> {
    println!("{}", "🌹 Roze Compiler v1.0.0".bright_magenta());
    println!("{} {}", "📁 Compiling:".cyan(), filename);

    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{} couldn't read '{}': {}", "❌ Error:".bright_red().bold(), filename, e);
            return Err(());
        }
    };

    // Lex
    let tokens = tokenize(&source);
    println!("{} {} tokens", "🔤 Lexer:".green(), tokens.len());

    // Debug output
    if debug {
        println!("\n{}", "🐛 Debug: Tokens".yellow());
        for (i, token) in tokens.iter().enumerate() {
            println!("  Token {:2}: {:?} at line {} col {}", i, token.token, token.line, token.column);
        }
        println!();
    }

    // Parse
    let program = match parse(tokens) {
        Ok(p) => p,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };
    println!("{} {} statements", "🌳 Parser:".green(), program.statements.len());

    // Resolve imports
    let base_dir = Path::new(filename).parent().unwrap_or_else(|| Path::new("."));
    let resolved_program = match imports::resolve_imports(program, base_dir) {
        Ok(result) => result,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };

    // Type check
    let typed_program = match check_and_lower_with_options(&resolved_program, no_heap, disable_float_registers) {
        Ok(tp) => {
            println!("{}", "✅ Type checking passed!".bright_green());
            tp
        }
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };

    // Code generation
    let codegen_result = match target {
        Target::Jvm => codegen::compile_to_java(typed_program, filename, classpath),
        Target::Native => codegen::native::compile_to_native(typed_program, filename, link_libs),
        Target::Freestanding => codegen::native::compile_to_freestanding(
            typed_program,
            filename,
            linker_script,
            entry,
            no_heap,
            disable_float_registers,
        ),
        Target::Wasm32 => codegen::wasm::compile_to_wasm(typed_program, filename, false).map(|_| ()),
        Target::Wasm32Wasi => codegen::wasm::compile_to_wasm(typed_program, filename, true).map(|_| ()),
        Target::Thumbv7mNoneEabi => codegen::embedded::compile_embedded_target(
            typed_program,
            filename,
            codegen::embedded::EmbeddedArch::CortexM3M4,
            chip,
        ).map(|_| ()),
        Target::Thumbv6mNoneEabi => codegen::embedded::compile_embedded_target(
            typed_program,
            filename,
            codegen::embedded::EmbeddedArch::CortexM0Plus,
            chip,
        ).map(|_| ()),
        Target::Riscv32imc => codegen::embedded::compile_embedded_target(
            typed_program,
            filename,
            codegen::embedded::EmbeddedArch::Riscv32Imc,
            chip,
        ).map(|_| ()),
        Target::XtensaEsp32NoneElf => codegen::embedded::compile_embedded_target(
            typed_program,
            filename,
            codegen::embedded::EmbeddedArch::XtensaEsp32,
            chip,
        ).map(|_| ()),
    };
    if let Err(e) = codegen_result {
        report_error(&e, filename, &source);
        return Err(());
    }

    println!("{} {}", "✅ Build successful!".bright_green(), "🎉");
    Ok(())
}

fn run_file(filename: &str, classpath: Option<&str>, target: Target) -> Result<(), ()> {
    let class_name = codegen::class_name_from_path(filename);
    println!("{} {}", "🚀 Running:".yellow(), class_name);

    let run_result = match target {
        Target::Jvm => codegen::run_java(&class_name, classpath),
        Target::Native => codegen::native::run_native(&class_name),
        Target::Freestanding => codegen::native::run_freestanding(&class_name),
        Target::Wasm32 | Target::Wasm32Wasi => {
            let wasm_file = if filename.ends_with(".roze") {
                format!("{}.wasm", &filename[..filename.len() - 5])
            } else {
                format!("{}.wasm", filename)
            };
            codegen::wasm::run_wasm(&wasm_file)
        }
        Target::Thumbv7mNoneEabi | Target::Thumbv6mNoneEabi | Target::Riscv32imc | Target::XtensaEsp32NoneElf => {
            println!("{} Microcontroller firmware binary ready for hardware deployment ('roze flash')", "✅ [Embedded Firmware]".bright_green().bold());
            Ok(())
        }
    };
    if let Err(e) = run_result {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
        return Err(());
    }
    Ok(())
}

fn report_error(err: &anyhow::Error, filename: &str, source: &str) {
    if err.downcast_ref::<error::AlreadyReported>().is_some() {
        return;
    }
    if let Some(roze_err) = err.downcast_ref::<RozeError>() {
        eprintln!("{}", roze_err.report(filename, source));
    } else {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), err);
    }
}
