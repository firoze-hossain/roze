// tools/rozedoc/src/main.rs
//
// Standalone CLI binary for rozedoc - Roze Automated Documentation Engine.

use std::path::{Path, PathBuf};
use std::process::Command;
use clap::Parser;
use colored::*;
use anyhow::Result;

#[derive(Parser, Debug)]
#[command(name = "rozedoc")]
#[command(version = "1.0.0")]
#[command(about = "🌹 rozedoc - Automated Documentation Generator for Roze with Live Playground support")]
struct Cli {
    /// Package root directory or single .roze file to document [default: .]
    #[arg(default_value = ".")]
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("{}", "🌹 rozedoc v1.0.0 - Generating Roze Documentation...".bold().magenta());

    let mut package = if cli.input.is_file() {
        rozedoc::extract_single_file(&cli.input)?
    } else {
        rozedoc::extract_package(&cli.input)?
    };

    if let Some(ref name) = cli.package_name {
        package.name = name.clone();
    }
    if let Some(ref ver) = cli.package_version {
        package.version = ver.clone();
    }

    let generator = rozedoc::DocGenerator::new(rozedoc::DocGeneratorOptions {
        output_dir: cli.output.clone(),
        enable_playground: !cli.no_playground,
        export_json: cli.json,
    });

    generator.generate(&package)?;

    let total_items = package.total_items();
    let num_modules = package.modules.len();
    let index_file = Path::new(&cli.output).join("index.html");

    println!(
        "{} Documented {} items across {} modules for {} v{}.",
        "✨".green(),
        total_items.to_string().bold().yellow(),
        num_modules.to_string().bold().cyan(),
        package.name.bold(),
        package.version
    );
    println!("{} Generated HTML documentation at: {}", "📖".blue(), index_file.display().to_string().bold());

    if cli.json {
        let json_file = Path::new(&cli.output).join("doc.json");
        println!("{} Exported machine-readable AST model to: {}", "📊".yellow(), json_file.display().to_string().bold());
    }

    if cli.open {
        println!("🌐 Opening documentation in web browser...");
        #[cfg(target_os = "linux")]
        let _ = Command::new("xdg-open").arg(&index_file).spawn();
        #[cfg(target_os = "macos")]
        let _ = Command::new("open").arg(&index_file).spawn();
        #[cfg(target_os = "windows")]
        let _ = Command::new("cmd").args(["/C", "start", &index_file.to_string_lossy()]).spawn();
    }

    Ok(())
}
