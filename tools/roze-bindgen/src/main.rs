// ==============================================================================
// 🌹 Roze Bindgen CLI (M6-T07)
// ==============================================================================

use std::path::PathBuf;
use clap::Parser;
use colored::*;

#[derive(Parser, Debug)]
#[command(name = "roze-bindgen")]
#[command(version = "1.0.0")]
#[command(about = "WebAssembly JS/TS Interface Binding Generator for Roze 🌹")]
struct Cli {
    /// WebAssembly binary file (.wasm) to generate bindings for
    #[arg(value_name = "WASM_FILE")]
    wasm_file: PathBuf,

    /// Output directory for generated JS/TS bindings and metadata
    #[arg(short, long, default_value = "pkg")]
    out_dir: PathBuf,
}

fn main() {
    let cli = Cli::parse();

    if !cli.wasm_file.exists() {
        eprintln!("{} File not found: {}", "❌ Error:".bright_red().bold(), cli.wasm_file.display());
        std::process::exit(1);
    }

    println!("🌹 {}", "Roze WebAssembly Bindgen Generator v1.0.0".bright_cyan().bold());
    println!("   Analyzing: {}", cli.wasm_file.display());
    println!("   Output directory: {}", cli.out_dir.display());

    match roze_bindgen::generate_bindings(&cli.wasm_file, &cli.out_dir) {
        Ok(_) => {
            println!("\n{} 🎉", "Binding generation complete!".bright_green().bold());
        }
        Err(e) => {
            eprintln!("\n{} {}", "❌ Error:".bright_red().bold(), e);
            std::process::exit(1);
        }
    }
}
