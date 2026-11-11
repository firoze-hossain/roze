// tools/roze-svd/src/main.rs
//
// Standalone CLI for CMSIS-SVD to Roze PAC generator

use clap::Parser;
use colored::*;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "roze-svd")]
#[command(version = "1.0.0")]
#[command(about = "CMSIS-SVD XML Parser and Peripheral Access Crate (PAC) Generator for Roze 🌹")]
struct Args {
    /// CMSIS-SVD file path or built-in template ('stm32f103', 'rp2040', 'esp32c3')
    #[arg(value_name = "SVD_FILE")]
    svd_file: String,

    /// Output directory for generated Roze PAC
    #[arg(short, long, default_value = "./pac")]
    out_dir: PathBuf,
}

fn main() {
    let args = Args::parse();
    println!("{}", "🌹 Roze SVD Peripheral Access Crate Generator".cyan().bold());

    let svd_path = PathBuf::from(&args.svd_file);
    if let Err(e) = roze_svd::process_svd_file(&svd_path, &args.out_dir) {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
        std::process::exit(1);
    }
}
