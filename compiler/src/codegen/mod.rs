pub mod jvm;

use crate::parser::ast::Program;
use anyhow::{Result, anyhow};
use std::fs;
use std::process::Command;

/// Extracts a bare Java-safe class name from a source file path: the
/// last component after either `/` or `\`, with a trailing `.roze`
/// extension stripped.
///
/// Deliberately does not use `std::path::Path` for this: `Path`'s
/// separator handling is platform-conditional (only `/` on Unix, both
/// `/` and `\` on Windows), which would make a Windows-specific path bug
/// impossible to catch by running tests on a non-Windows machine.
/// Treating both characters as separators unconditionally, regardless of
/// which OS actually compiles/runs this code, is what actually fixes
/// (and lets us test) the real bug: a path like `tests\test.roze` was
/// previously passed straight through unsplit, embedding a literal
/// backslash into the generated `public class tests\test { ... }` --
/// which fails to compile ("illegal character: '\'").
///
/// Also uses `strip_suffix` rather than `.replace(".roze", "")`: the
/// latter would incorrectly strip *every* occurrence of that substring
/// anywhere in the path, not just a trailing extension (e.g.
/// "my.roze.thing.roze" should become "my.roze.thing", not "my.thing").
pub fn class_name_from_path(path: &str) -> String {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file_name.strip_suffix(".roze").unwrap_or(file_name).to_string()
}

pub fn compile_to_java(program: Program, input_file: &str) -> Result<()> {
    let class_name = class_name_from_path(input_file);

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
        .arg("-Dstdout.encoding=UTF-8")
        .arg("-Dstderr.encoding=UTF-8")
        .arg(class_name)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Failed to run Java class"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_a_unix_style_directory() {
        assert_eq!(class_name_from_path("examples/core_demo.roze"), "core_demo");
    }

    #[test]
    fn strips_a_windows_style_directory() {
        // The exact bug: this used to come out as "tests\test" (backslash
        // and all), which isn't a valid Java identifier.
        assert_eq!(class_name_from_path("tests\\test.roze"), "test");
    }

    #[test]
    fn strips_a_deeply_nested_windows_path() {
        assert_eq!(class_name_from_path("D:\\Projects\\roze\\tests\\test.roze"), "test");
    }

    #[test]
    fn strips_a_mixed_separator_path() {
        assert_eq!(class_name_from_path("some/dir\\test.roze"), "test");
    }

    #[test]
    fn handles_a_bare_filename_with_no_directory() {
        assert_eq!(class_name_from_path("test.roze"), "test");
    }

    #[test]
    fn only_strips_a_trailing_roze_extension_not_every_occurrence() {
        // A naive `.replace(".roze", "")` would have mangled this.
        assert_eq!(class_name_from_path("my.roze.thing.roze"), "my.roze.thing");
    }
}