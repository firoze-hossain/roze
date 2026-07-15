use std::fs;
use clap::{Parser as ClapParser, Subcommand};
use colored::*;

mod lexer;
mod parser;
mod codegen;
mod semantic;

use lexer::tokenize;
use parser::parse;
use semantic::check_types;

#[derive(ClapParser)]
#[command(name = "roze")]
#[command(version = "0.1.0")]
#[command(about = "The Roze Programming Language 🌹")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable debug output (shows tokens)
    #[arg(short, long, global = true)]
    debug: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a Roze file
    Build {
        #[arg(value_name = "FILE")]
        file: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Run a Roze file
    Run {
        #[arg(value_name = "FILE")]
        file: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { file, output: _ } => {
            build_file(&file, cli.debug)?;
        }
        Commands::Run { file } => {
            build_file(&file, cli.debug)?;
            run_file(&file)?;
        }
    }

    Ok(())
}

fn build_file(filename: &str, debug: bool) -> anyhow::Result<()> {
    println!("{}", "🌹 Roze Compiler v0.1".bright_magenta());
    println!("{} {}", "📁 Compiling:".cyan(), filename);

    let source = fs::read_to_string(filename)
        .map_err(|e| anyhow::anyhow!("Failed to read file: {}", e))?;

    // Lex
    let tokens = tokenize(&source);
    println!("{} {} tokens", "🔤 Lexer:".green(), tokens.len());

    // Debug output
    if debug {
        println!("\n{}", "🐛 Debug: Tokens".yellow());
        for (i, token) in tokens.iter().enumerate() {
            println!("  Token {:2}: {:?} at line {} col {}",
                     i, token.token, token.line, token.column);
        }
        println!();
    }

    // Parse
    let program = parse(tokens)?;
    println!("{} {} statements", "🌳 Parser:".green(), program.statements.len());

    // Type check
    match check_types(&program) {
        Ok(_) => println!("{}", "✅ Type checking passed!".green()),
        Err(e) => {
            println!("{} {}", "❌ Type error:".red(), e);
            return Err(e);
        }
    }

    // Generate Java code
    codegen::compile_to_java(program, filename)?;

    println!("{} {}", "✅ Build successful!".bright_green(), "🎉");

    Ok(())
}

fn run_file(filename: &str) -> anyhow::Result<()> {
    let class_name = filename
        .split('/')
        .last()
        .unwrap_or(filename)
        .replace(".roze", "");

    println!("{} {}", "🚀 Running:".yellow(), class_name);

    codegen::run_java(&class_name)?;

    Ok(())
}