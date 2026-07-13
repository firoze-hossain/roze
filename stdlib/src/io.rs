//! Input/Output operations for Roze
//!
//! This module provides console I/O and file operations

use std::io::Write;

pub fn print<T: std::fmt::Display>(value: T) {
    print!("{}", value);
    let _ = std::io::stdout().flush();
}

pub fn println<T: std::fmt::Display>(value: T) {
    println!("{}", value);
}

pub fn read_line() -> String {
    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);
    input.trim().to_string()
}

pub mod file {
    use std::fs;
    use std::path::Path;

    pub fn read(path: &str) -> Result<String, std::io::Error> {
        fs::read_to_string(path)
    }

    pub fn write(path: &str, content: &str) -> Result<(), std::io::Error> {
        fs::write(path, content)
    }

    pub fn exists(path: &str) -> bool {
        Path::new(path).exists()
    }
}