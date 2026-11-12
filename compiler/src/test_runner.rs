// compiler/src/test_runner.rs
//
// 🌹 Roze Unified Test Runner (roze-test)
// Automated test discovery, execution, assertion tracking, and result reporting.
// Matches 'cargo test' and 'go test'.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use colored::*;
use anyhow::{Result, anyhow};
use walkdir::WalkDir;

/// Configuration options for running Roze test suites.
#[derive(Debug, Clone)]
pub struct TestOptions {
    pub filter: Option<String>,
    pub nocapture: bool,
    pub verbose: bool,
    pub target: crate::target::Target,
}

/// Status of an individual test execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestStatus {
    Passed,
    Failed(String),
    Ignored,
}

/// Summary of an entire test suite run.
#[derive(Debug, Default, Clone)]
pub struct TestSuiteSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
    pub failures: Vec<(String, String)>,
    pub elapsed_ms: u128,
}

/// Discovers all candidate Roze test files in the specified path or directory.
pub fn discover_test_files(base_path: &Path, filter: Option<&str>) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if base_path.is_file() {
        if base_path.extension().map_or(false, |ext| ext == "roze") {
            let name = base_path.to_string_lossy();
            if filter.map_or(true, |f| name.contains(f)) {
                files.push(base_path.to_path_buf());
            }
        }
        return files;
    }

    let search_dir = if base_path.join("tests").is_dir() {
        base_path.join("tests")
    } else {
        base_path.to_path_buf()
    };

    for entry in WalkDir::new(&search_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            !p.components().any(|c| {
                let s = c.as_os_str().to_string_lossy();
                s.starts_with('.') || s == "target" || s == "vendor"
            })
        })
    {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "roze") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let full = path.to_string_lossy();
            // Match files in tests/ or ending in _test.roze or starting with test_
            if full.contains("tests") || name.starts_with("test_") || name.ends_with("_test.roze") {
                if filter.map_or(true, |f| full.contains(f) || name.contains(f)) {
                    files.push(path.to_path_buf());
                }
            }
        }
    }

    files.sort();
    files
}

/// Runs a single test file using the Roze compiler toolchain.
pub fn run_single_test_file(
    file_path: &Path,
    roze_bin: &Path,
    target: crate::target::Target,
    nocapture: bool,
) -> (TestStatus, u128) {
    let start = Instant::now();

    let target_str = match target {
        crate::target::Target::Jvm => "jvm",
        crate::target::Target::Native => "native",
        crate::target::Target::Freestanding => "freestanding",
        crate::target::Target::Wasm32 => "wasm32",
        crate::target::Target::Wasm32Wasi => "wasm32-wasi",
        crate::target::Target::Thumbv7mNoneEabi => "thumbv7m",
        crate::target::Target::Thumbv6mNoneEabi => "thumbv6m",
        crate::target::Target::Riscv32imc => "riscv32imc",
        crate::target::Target::XtensaEsp32NoneElf => "esp32",
    };

    let mut cmd = Command::new(roze_bin);
    cmd.arg("run")
        .arg(file_path)
        .arg("--target")
        .arg(target_str);

    let output_result = if nocapture {
        cmd.status().map(|s| {
            if s.success() {
                Ok(())
            } else {
                Err(format!("Exit code: {:?}", s.code()))
            }
        })
    } else {
        cmd.output().map(|out| {
            if out.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                Err(format!("{}\n{}", stdout, stderr).trim().to_string())
            }
        })
    };

    let elapsed = start.elapsed().as_millis();

    match output_result {
        Ok(Ok(())) => (TestStatus::Passed, elapsed),
        Ok(Err(err_msg)) => (TestStatus::Failed(err_msg), elapsed),
        Err(e) => (TestStatus::Failed(format!("Failed to execute roze binary: {}", e)), elapsed),
    }
}

/// Runs the complete Roze test suite across discovered test files.
pub fn run_test_suite(
    target_path: &Path,
    filter: Option<&str>,
    nocapture: bool,
    _verbose: bool,
    target: crate::target::Target,
) -> Result<TestSuiteSummary> {
    // If inside a bsr package project and no specific file given, check if bsr test should run
    if target_path.is_dir() && (target_path.join("bsr.mfz").exists() || target_path.join("roze.mfz").exists() || target_path.join("roze.toml").exists()) {
        println!("{}", "📦 Running project test suite via Package Manager...".cyan());
        if let Err(e) = crate::pkg::cmd_test() {
            return Err(e);
        }
        return Ok(TestSuiteSummary {
            total: 1,
            passed: 1,
            failed: 0,
            ignored: 0,
            failures: Vec::new(),
            elapsed_ms: 10,
        });
    }

    let files = discover_test_files(target_path, filter);
    if files.is_empty() {
        println!("{}", "No tests found to run.".yellow());
        return Ok(TestSuiteSummary::default());
    }

    let roze_bin = crate::toolchain::find_roze_binary()
        .unwrap_or_else(|_| PathBuf::from("roze"));

    println!("\nrunning {} tests", files.len());
    let suite_start = Instant::now();

    let mut summary = TestSuiteSummary {
        total: files.len(),
        ..Default::default()
    };

    for file in &files {
        let display_name = file.file_name().unwrap_or_default().to_string_lossy();
        print!("test {} ... ", display_name);
        std::io::Write::flush(&mut std::io::stdout())?;

        let (status, elapsed) = run_single_test_file(file, &roze_bin, target, nocapture);

        match status {
            TestStatus::Passed => {
                println!("{} ({}ms)", "ok".bright_green().bold(), elapsed);
                summary.passed += 1;
            }
            TestStatus::Failed(err) => {
                println!("{} ({}ms)", "FAILED".bright_red().bold(), elapsed);
                summary.failed += 1;
                summary.failures.push((display_name.to_string(), err));
            }
            TestStatus::Ignored => {
                println!("{}", "ignored".yellow());
                summary.ignored += 1;
            }
        }
    }

    summary.elapsed_ms = suite_start.elapsed().as_millis();

    if !summary.failures.is_empty() {
        println!("\nfailures:\n");
        for (name, err) in &summary.failures {
            println!("---- {} stdout/stderr ----", name.bold().red());
            println!("{}\n", err);
        }
    }

    let res_color = if summary.failed == 0 {
        "ok".bright_green().bold()
    } else {
        "FAILED".bright_red().bold()
    };

    println!(
        "\ntest result: {}. {} passed; {} failed; {} ignored; finished in {}ms\n",
        res_color,
        summary.passed.to_string().bright_green().bold(),
        summary.failed.to_string().bright_red().bold(),
        summary.ignored.to_string().yellow(),
        summary.elapsed_ms
    );

    if summary.failed > 0 {
        Err(anyhow!("Test suite failed with {} failure(s)", summary.failed))
    } else {
        Ok(summary)
    }
}
