// compiler/src/doc/mod.rs
//
// Automated Documentation Generator for Roze with Live Playground support.
// Parity with rustdoc and javadoc.

#![allow(dead_code, unused_imports)]

pub mod extractor;
pub mod generator;
pub mod indexer;
pub mod markdown;
pub mod model;

pub use extractor::{extract_module, extract_package, extract_single_file};
pub use generator::{DocGenerator, DocGeneratorOptions};
pub use indexer::{build_search_index, export_package_json, generate_search_index_js};
pub use markdown::{markdown_to_html, parse_docstring};
pub use model::*;

use std::path::{Path, PathBuf};
use std::process::Command;
use clap::Parser;
use colored::*;
use anyhow::Result;

/// Convenience helper to document a package or source file into target output directory.
pub fn generate_docs(
    input_path: &Path,
    output_dir: &str,
    enable_playground: bool,
    export_json: bool,
) -> Result<DocPackage> {
    let package = if input_path.is_file() {
        extract_single_file(input_path)?
    } else {
        extract_package(input_path)?
    };

    let generator = DocGenerator::new(DocGeneratorOptions {
        output_dir: output_dir.to_string(),
        enable_playground,
        export_json,
    });

    generator.generate(&package)?;
    Ok(package)
}

#[derive(Parser, Debug)]
#[command(name = "rozedoc")]
#[command(version = "1.0.0")]
#[command(about = "🌹 rozedoc - Automated Documentation Generator for Roze with Live Playground support")]
pub struct DocCli {
    /// Package root directory or single .roze file to document [default: .]
    #[arg(default_value = ".")]
    pub input: PathBuf,

    /// Output directory for generated static HTML documentation [default: target/doc]
    #[arg(short, long, default_value = "target/doc")]
    pub output: String,

    /// Override documented package name
    #[arg(long)]
    pub package_name: Option<String>,

    /// Override documented package version
    #[arg(long)]
    pub package_version: Option<String>,

    /// Disable in-browser live playground simulation buttons
    #[arg(long)]
    pub no_playground: bool,

    /// Export complete machine-readable AST documentation model as JSON
    #[arg(long)]
    pub json: bool,

    /// Automatically open generated documentation in default web browser
    #[arg(long)]
    pub open: bool,
}

pub fn run_cli() -> Result<()> {
    let cli = DocCli::parse();
    run_doc_options(
        &cli.input,
        &cli.output,
        cli.package_name.as_deref(),
        cli.package_version.as_deref(),
        !cli.no_playground,
        cli.json,
        cli.open,
    )
}

pub fn run_doc_options(
    input: &Path,
    output: &str,
    package_name: Option<&str>,
    package_version: Option<&str>,
    enable_playground: bool,
    json: bool,
    open: bool,
) -> Result<()> {
    println!("{}", "🌹 rozedoc v1.0.0 - Generating Roze Documentation...".bold().magenta());

    let mut package = if input.is_file() {
        extract_single_file(input)?
    } else {
        extract_package(input)?
    };

    if let Some(name) = package_name {
        package.name = name.to_string();
    }
    if let Some(ver) = package_version {
        package.version = ver.to_string();
    }

    let generator = DocGenerator::new(DocGeneratorOptions {
        output_dir: output.to_string(),
        enable_playground,
        export_json: json,
    });

    generator.generate(&package)?;

    let total_items = package.total_items();
    let num_modules = package.modules.len();
    let index_file = Path::new(output).join("index.html");

    println!(
        "{} Documented {} items across {} modules for {} v{}.",
        "✨".green(),
        total_items.to_string().bold().yellow(),
        num_modules.to_string().bold().cyan(),
        package.name.bold(),
        package.version
    );
    println!("{} Generated HTML documentation at: {}", "📖".blue(), index_file.display().to_string().bold());

    if json {
        let json_file = Path::new(output).join("doc.json");
        println!("{} Exported machine-readable AST model to: {}", "📊".yellow(), json_file.display().to_string().bold());
    }

    if open {
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
