pub mod jvm;

use crate::parser::ast::Program;
use anyhow::{Result, anyhow};
use std::fs;
use std::process::Command;

pub fn compile_to_java(program: Program, input_file: &str) -> Result<()> {
    let class_name = input_file
        .split('/')
        .last()
        .unwrap_or(input_file)
        .replace(".roze", "");

    let generator = jvm::JavaSourceGenerator::new(program, class_name.clone());
    let source_code = generator.generate()?;

    // Write Java source file
    let java_file = format!("{}.java", class_name);
    fs::write(&java_file, source_code)?;

    println!("📝 Generated Java source: {}", java_file);

    // Compile with javac
    let status = Command::new("javac")
        .arg(&java_file)
        .status()?;

    if status.success() {
        println!("✅ Compiled to Java bytecode: {}.class", class_name);
        Ok(())
    } else {
        Err(anyhow!("Failed to compile Java source"))
    }
}

pub fn run_java(class_name: &str) -> Result<()> {
    let status = Command::new("java")
        .arg(class_name)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to run Java class"))
    }
}