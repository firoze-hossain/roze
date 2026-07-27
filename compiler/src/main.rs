// compiler/src/main.rs
use std::fs;
use clap::{Parser as ClapParser, Subcommand};
use colored::*;

mod lexer;
mod parser;
mod codegen;
mod semantic;
mod error;
mod imports;
mod ir;

use lexer::tokenize;
use parser::parse;
use semantic::check_and_lower;
use error::RozeError;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum Target {
    /// Compile to JVM bytecode via javac (the default; supports the
    /// full language, including Core/Collections/IO/Web/Database).
    Jvm,
    /// Compile to a native executable via Cranelift, with no JVM/JDK
    /// involved. An intentionally minimal SPIKE (see
    /// compiler/src/codegen/native.rs): int/bool functions, arithmetic,
    /// if/while/for, and println of an int/bool/string-literal only --
    /// no strings-as-values, no list/map, no intrinsics. See
    /// docs/MEMORY_MODEL_DECISION.md for why.
    Native,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a Roze file
    Build {
        /// Input file (optional, defaults to main.roze)
        #[arg(value_name = "FILE", default_value = "src/main.roze")]
        file: String,
        /// Output directory
        #[arg(short, long)]
        output: Option<String>,
        /// Extra classpath entries for javac (e.g. a JDBC driver jar for
        /// sql_connect/sql_query/sql_execute -- see stdlib/src/sql.roze).
        /// Use your platform's path separator (';' on Windows, ':'
        /// elsewhere) to list more than one. Ignored for --target native.
        #[arg(long)]
        classpath: Option<String>,
        /// Which backend to compile with
        #[arg(long, value_enum, default_value_t = Target::Jvm)]
        target: Target,
    },
    /// Run a Roze file
    Run {
        /// Input file (optional, defaults to main.roze)
        #[arg(value_name = "FILE", default_value = "src/main.roze")]
        file: String,
        /// Extra classpath entries for javac/java (see `build --classpath`)
        #[arg(long)]
        classpath: Option<String>,
        /// Which backend to compile and run with
        #[arg(long, value_enum, default_value_t = Target::Jvm)]
        target: Target,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Build { file, output: _, classpath, target } => build_file(&file, cli.debug, classpath.as_deref(), target),
        Commands::Run { file, classpath, target } => build_file(&file, cli.debug, classpath.as_deref(), target)
            .and_then(|_| run_file(&file, classpath.as_deref(), target)),
    };

    // Every error is already reported (with a full source snippet, when
    // we have position info) by the time we get here -- main() just needs
    // to exit non-zero. We deliberately never let an error value reach
    // Rust's default `Termination` printer (which is what previously
    // produced raw `anyhow` Debug output complete with a Rust panic-style
    // stack backtrace for what were really just user syntax/type errors).
    if result.is_err() {
        std::process::exit(1);
    }
}

fn build_file(filename: &str, debug: bool, classpath: Option<&str>, target: Target) -> Result<(), ()> {
    println!("{}", "🌹 Roze Compiler v0.1".bright_magenta());
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
            println!("  Token {:2}: {:?} at line {} col {}",
                     i, token.token, token.line, token.column);
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

    // Resolve imports (pulls in another file's/the bundled Core module's
    // top-level functions -- see imports.rs)
    let base_dir = std::path::Path::new(filename).parent().unwrap_or_else(|| std::path::Path::new("."));
    let program = match imports::resolve_imports(program, base_dir) {
        Ok(p) => p,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };

    // Type check, producing the fully type-annotated IR that codegen
    // consumes directly (see ir.rs) -- this is also what guarantees
    // codegen never has to re-derive a type from scratch itself.
    let typed_program = match check_and_lower(&program) {
        Ok(p) => p,
        Err(e) => {
            report_error(&e, filename, &source);
            return Err(());
        }
    };
    println!("{}", "✅ Type checking passed!".green());

    // Generate code with the selected backend
    let codegen_result = match target {
        Target::Jvm => codegen::compile_to_java(typed_program, filename, classpath),
        Target::Native => codegen::native::compile_to_native(typed_program, filename),
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
    };
    if let Err(e) = run_result {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
        return Err(());
    }

    Ok(())
}

/// Prints a clean, final report for a compiler failure. `RozeError`s (from
/// the lexer/parser/type checker) get the full treatment: message, a
/// `-->` pointer at file:line:column, the offending source line, and a
/// `^^^` underline. Anything else (a javac/java subprocess failure, an
/// internal error) gets a plain one-line message -- never a raw Rust
/// Debug dump or backtrace.
fn report_error(err: &anyhow::Error, filename: &str, source: &str) {
    if err.downcast_ref::<error::AlreadyReported>().is_some() {
        // Already printed in full, against its own file/source (e.g. a
        // syntax error inside an imported module) -- nothing more to do.
        return;
    }
    if let Some(roze_err) = err.downcast_ref::<RozeError>() {
        eprintln!("{}", roze_err.report(filename, source));
    } else {
        eprintln!("{} {}", "❌ Error:".bright_red().bold(), err);
    }
}
