// tools/roze-pkg/src/generator.rs
//
// Project generator and scaffolding for Basir (`bsr new` and `bsr init`).
// Implements full parity with `cargo new`, `cargo init`, and `go mod init`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{anyhow, Result};
use colored::*;

use super::project::{
    ALTERNATIVE_MANIFEST_NAME, LEGACY_MANIFEST_NAME, MANIFEST_HEADER, MANIFEST_NAME,
};
use super::template::Template;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Bin,
    Lib,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VcsKind {
    Git,
    None,
}

impl VcsKind {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "git" => Ok(VcsKind::Git),
            "none" => Ok(VcsKind::None),
            other => Err(anyhow!("unsupported VCS: '{}'. Supported options: 'git', 'none'", other)),
        }
    }
}

pub use super::project::{validate_package_name, RESERVED_KEYWORDS};

pub fn infer_package_name(path: &Path, explicit_name: Option<&str>) -> Result<String> {
    if let Some(name) = explicit_name {
        validate_package_name(name)?;
        return Ok(name.to_string());
    }

    let resolved_path = if path.as_os_str().is_empty() || path == Path::new(".") {
        std::env::current_dir()?
    } else if path.is_relative() {
        if let Ok(cur) = std::env::current_dir() {
            cur.join(path)
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    let dir_name = resolved_path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Cannot infer package name from path '{}'", path.display()))?;

    validate_package_name(dir_name)?;
    Ok(dir_name.to_string())
}

pub fn is_inside_git_repo(path: &Path) -> bool {
    let abs_path = if path.is_relative() {
        if let Ok(cwd) = std::env::current_dir() {
            cwd.join(path)
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    let mut cur = abs_path.as_path();
    loop {
        if cur.join(".git").exists() {
            return true;
        }
        match cur.parent() {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    false
}

pub fn init_vcs(path: &Path, vcs_opt: Option<VcsKind>) -> Result<()> {
    match vcs_opt {
        Some(VcsKind::None) => Ok(()),
        Some(VcsKind::Git) => {
            let _ = Command::new("git")
                .arg("init")
                .arg(path)
                .output();
            Ok(())
        }
        None => {
            if !is_inside_git_repo(path) {
                let _ = Command::new("git")
                    .arg("init")
                    .arg(path)
                    .output();
            }
            Ok(())
        }
    }
}

pub fn setup_gitignore(path: &Path) -> Result<()> {
    let gitignore_path = path.join(".gitignore");
    if gitignore_path.exists() {
        let content = fs::read_to_string(&gitignore_path)?;
        let mut to_append = Vec::new();
        if !content
            .lines()
            .any(|l| l.trim() == "/target" || l.trim() == "/target/" || l.trim() == "target/")
        {
            to_append.push("/target/");
        }
        if !content.lines().any(|l| l.trim() == "*.class") {
            to_append.push("*.class");
        }
        if !content.lines().any(|l| l.trim() == "*.o") {
            to_append.push("*.o");
        }
        if !to_append.is_empty() {
            let mut new_content = content;
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            for entry in to_append {
                new_content.push_str(entry);
                new_content.push('\n');
            }
            fs::write(&gitignore_path, new_content)?;
        }
    } else {
        let content = [
            "# Basir / Roze build artifacts",
            "/target/",
            "/bin/",
            "*.class",
            "*.o",
            "*.log",
            "__roze_*",
            "",
        ]
        .join("\n");
        fs::write(&gitignore_path, content)?;
    }
    Ok(())
}

pub fn setup_tests_and_benches(path: &Path, name: &str) -> Result<()> {
    let tests_dir = path.join("tests");
    fs::create_dir_all(&tests_dir)?;
    let test_file = tests_dir.join("integration_test.roze");
    if !test_file.exists() {
        let test_content = format!(
            r#"// tests/integration_test.roze - Integration tests for {name}

func assert_eq_int(actual: int, expected: int, msg: string) {{
    if actual != expected {{
        println("❌ Assertion failed: " + msg);
    }} else {{
        println("✅ " + msg);
    }}
}}

func test_basic_arithmetic() {{
    let result = 1 + 1;
    assert_eq_int(result, 2, "1 + 1 == 2");
}}

func main() {{
    println("🧪 Running integration tests for {name}...");
    test_basic_arithmetic();
    println("All integration tests passed!");
}}
"#
        );
        fs::write(&test_file, test_content)?;
    }

    let benches_dir = path.join("benches");
    fs::create_dir_all(&benches_dir)?;
    let bench_file = benches_dir.join("bench_main.roze");
    if !bench_file.exists() {
        let bench_content = format!(
            r#"// benches/bench_main.roze - Benchmark harness for {name}

func run_benchmark() {{
    println("⏱️  Running benchmarks for {name}...");
    let sum = 0;
    let i = 0;
    while i < 100000 {{
        sum = sum + i;
        i = i + 1;
    }}
    if sum > 0 {{
        println("✅ Benchmark iterations complete.");
    }}
}}

func main() {{
    run_benchmark();
}}
"#
        );
        fs::write(&bench_file, bench_content)?;
    }

    Ok(())
}

pub fn setup_readme(path: &Path, name: &str, target: TargetKind) -> Result<()> {
    let readme_path = path.join("README.md");
    if !readme_path.exists() {
        let kind_str = match target {
            TargetKind::Bin => "application",
            TargetKind::Lib => "library",
        };
        let content = format!(
            r#"# {name}

A Roze {kind_str} created with `bsr`.

## Build
```bash
bsr build
```

## Run
```bash
bsr run
```

## Test
```bash
bsr test
```

## Benchmark
```bash
bsr bench
```
"#
        );
        fs::write(&readme_path, content)?;
    }
    Ok(())
}

pub fn setup_manifest(
    path: &Path,
    name: &str,
    target: TargetKind,
    author: &str,
    edition: &str,
    main_file: &str,
) -> Result<()> {
    let manifest_path = path.join(MANIFEST_NAME);
    if manifest_path.exists() {
        return Ok(());
    }

    let desc = match target {
        TargetKind::Bin => "A Roze application",
        TargetKind::Lib => "A Roze library",
    };

    let content = format!(
        r#"{MANIFEST_HEADER}[package]
name = "{name}"
version = "0.1.0"
edition = "{edition}"
authors = ["{author}"]
description = "{desc}"
main = "{main_file}"

[dependencies]
"#
    );

    fs::write(&manifest_path, content)?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct NewOptions {
    pub path: PathBuf,
    pub name: Option<String>,
    pub target: Option<TargetKind>,
    pub vcs: Option<VcsKind>,
    pub edition: String,
    pub author: String,
    pub template: Option<String>,
}

pub fn generate_new(opts: &NewOptions) -> Result<()> {
    let target_path = &opts.path;
    let pkg_name = infer_package_name(target_path, opts.name.as_deref())?;

    // Check collision
    if target_path.exists() {
        if !target_path.is_dir() {
            return Err(anyhow!(
                "destination '{}' already exists and is not a directory",
                target_path.display()
            ));
        }
        let entries = fs::read_dir(target_path)?;
        let has_entries = entries.filter_map(|e| e.ok()).any(|entry| {
            let name = entry.file_name();
            name != ".git" && name != "." && name != ".."
        });
        if has_entries {
            return Err(anyhow!(
                "destination '{}' already exists and is not an empty directory\n\nUse `bsr init` to initialize the directory",
                target_path.display()
            ));
        }
    } else {
        fs::create_dir_all(target_path)?;
    }

    // Backwards compatibility with --template web
    if let Some(tmpl_name) = opts.template.as_deref() {
        if tmpl_name == "web" {
            let tmpl = Template::get_web();
            tmpl.apply(&pkg_name, &opts.author, target_path)?;
            setup_tests_and_benches(target_path, &pkg_name)?;
            setup_gitignore(target_path)?;
            init_vcs(target_path, opts.vcs)?;

            println!(
                "     {} binary (application) `{}` package",
                "Created".green().bold(),
                pkg_name
            );
            return Ok(());
        }
    }

    let target_kind = if let Some(kind) = opts.target {
        kind
    } else if let Some(tmpl) = opts.template.as_deref() {
        if tmpl == "library" {
            TargetKind::Lib
        } else {
            TargetKind::Bin
        }
    } else {
        TargetKind::Bin
    };

    let main_file = match target_kind {
        TargetKind::Bin => "src/main.roze",
        TargetKind::Lib => "src/lib.roze",
    };

    let src_dir = target_path.join("src");
    fs::create_dir_all(&src_dir)?;

    match target_kind {
        TargetKind::Bin => {
            let main_path = target_path.join("src/main.roze");
            if !main_path.exists() {
                let content = format!(
                    "// src/main.roze - Entry point for {pkg_name}\n// Created by {author}\n\nfunc main() {{\n    println(\"🌹 Hello from {pkg_name}!\");\n}}\n",
                    pkg_name = pkg_name,
                    author = opts.author
                );
                fs::write(&main_path, content)?;
            }
        }
        TargetKind::Lib => {
            let lib_path = target_path.join("src/lib.roze");
            if !lib_path.exists() {
                let content = format!(
                    "// src/lib.roze - Library interface for {pkg_name}\n// Created by {author}\n\nfunc add(a: int, b: int) -> int {{\n    return a + b;\n}}\n\nfunc greeting(name: string) -> string {{\n    return \"Hello \" + name + \" from {pkg_name}!\";\n}}\n",
                    pkg_name = pkg_name,
                    author = opts.author
                );
                fs::write(&lib_path, content)?;
            }
        }
    }

    setup_manifest(
        target_path,
        &pkg_name,
        target_kind,
        &opts.author,
        &opts.edition,
        main_file,
    )?;
    setup_tests_and_benches(target_path, &pkg_name)?;
    setup_gitignore(target_path)?;
    setup_readme(target_path, &pkg_name, target_kind)?;
    init_vcs(target_path, opts.vcs)?;

    let kind_msg = match target_kind {
        TargetKind::Bin => "binary (application)",
        TargetKind::Lib => "library",
    };
    println!(
        "     {} {} `{}` package",
        "Created".green().bold(),
        kind_msg,
        pkg_name
    );

    Ok(())
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub path: PathBuf,
    pub name: Option<String>,
    pub target: Option<TargetKind>,
    pub vcs: Option<VcsKind>,
    pub edition: String,
    pub author: String,
}

pub fn generate_init(opts: &InitOptions) -> Result<()> {
    let target_path = &opts.path;
    if !target_path.exists() {
        fs::create_dir_all(target_path)?;
    }

    // Check if manifest already exists
    if target_path.join(MANIFEST_NAME).exists()
        || target_path.join(ALTERNATIVE_MANIFEST_NAME).exists()
        || target_path.join(LEGACY_MANIFEST_NAME).exists()
    {
        return Err(anyhow!(
            "cannot initialize an existing package in '{}' (manifest already exists)",
            target_path.display()
        ));
    }

    let pkg_name = infer_package_name(target_path, opts.name.as_deref())?;

    let target_kind = if let Some(k) = opts.target {
        k
    } else if target_path.join("src/lib.roze").exists() || target_path.join("lib.roze").exists() {
        TargetKind::Lib
    } else {
        TargetKind::Bin
    };

    let main_file = match target_kind {
        TargetKind::Bin => {
            if target_path.join("main.roze").exists() {
                "main.roze"
            } else {
                "src/main.roze"
            }
        }
        TargetKind::Lib => {
            if target_path.join("lib.roze").exists() {
                "lib.roze"
            } else {
                "src/lib.roze"
            }
        }
    };

    match target_kind {
        TargetKind::Bin => {
            let main_path = target_path.join("src/main.roze");
            let root_main = target_path.join("main.roze");
            if !main_path.exists() && !root_main.exists() {
                let src_dir = target_path.join("src");
                fs::create_dir_all(&src_dir)?;
                let content = format!(
                    "// src/main.roze - Entry point for {pkg_name}\n// Created by {author}\n\nfunc main() {{\n    println(\"🌹 Hello from {pkg_name}!\");\n}}\n",
                    pkg_name = pkg_name,
                    author = opts.author
                );
                fs::write(&main_path, content)?;
            }
        }
        TargetKind::Lib => {
            let lib_path = target_path.join("src/lib.roze");
            let root_lib = target_path.join("lib.roze");
            if !lib_path.exists() && !root_lib.exists() {
                let src_dir = target_path.join("src");
                fs::create_dir_all(&src_dir)?;
                let content = format!(
                    "// src/lib.roze - Library interface for {pkg_name}\n// Created by {author}\n\nfunc add(a: int, b: int) -> int {{\n    return a + b;\n}}\n\nfunc greeting(name: string) -> string {{\n    return \"Hello \" + name + \" from {pkg_name}!\";\n}}\n",
                    pkg_name = pkg_name,
                    author = opts.author
                );
                fs::write(&lib_path, content)?;
            }
        }
    }

    setup_manifest(
        target_path,
        &pkg_name,
        target_kind,
        &opts.author,
        &opts.edition,
        main_file,
    )?;
    setup_tests_and_benches(target_path, &pkg_name)?;
    setup_gitignore(target_path)?;
    setup_readme(target_path, &pkg_name, target_kind)?;
    init_vcs(target_path, opts.vcs)?;

    let kind_msg = match target_kind {
        TargetKind::Bin => "binary (application)",
        TargetKind::Lib => "library",
    };
    if opts.name.is_some() {
        println!(
            "     {} {} `{}` package",
            "Created".green().bold(),
            kind_msg,
            pkg_name
        );
    } else {
        println!(
            "     {} {} package",
            "Created".green().bold(),
            kind_msg
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_validate_package_name_valid() {
        assert!(validate_package_name("my_app").is_ok());
        assert!(validate_package_name("my-app").is_ok());
        assert!(validate_package_name("app123").is_ok());
        assert!(validate_package_name("_hidden").is_ok());
        assert!(validate_package_name("roze_core_lib").is_ok());
    }

    #[test]
    fn test_validate_package_name_invalid_chars() {
        assert!(validate_package_name("").is_err());
        assert!(validate_package_name("123app").is_err());
        assert!(validate_package_name("-dash_start").is_err());
        assert!(validate_package_name("has space").is_err());
        assert!(validate_package_name("path/to/thing").is_err());
        assert!(validate_package_name("foo.bar").is_err());
    }

    #[test]
    fn test_validate_package_name_reserved_keywords() {
        assert!(validate_package_name("fn").is_err());
        assert!(validate_package_name("func").is_err());
        assert!(validate_package_name("struct").is_err());
        assert!(validate_package_name("class").is_err());
        assert!(validate_package_name("import").is_err());
        assert!(validate_package_name("let").is_err());
        assert!(validate_package_name("mut").is_err());
        assert!(validate_package_name("while").is_err());
        assert!(validate_package_name("for").is_err());
        assert!(validate_package_name("true").is_err());
        assert!(validate_package_name("null").is_err());
    }

    #[test]
    fn test_generate_new_bin_scaffolding() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("test_bin_app");

        let opts = NewOptions {
            path: project_dir.clone(),
            name: None,
            target: Some(TargetKind::Bin),
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
            template: None,
        };

        assert!(generate_new(&opts).is_ok());

        assert!(project_dir.join("bsr.mfz").exists());
        assert!(project_dir.join("src/main.roze").exists());
        assert!(project_dir.join("tests/integration_test.roze").exists());
        assert!(project_dir.join("benches/bench_main.roze").exists());
        assert!(project_dir.join(".gitignore").exists());
        assert!(project_dir.join("README.md").exists());

        let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
        assert!(manifest.contains("name = \"test_bin_app\""));
        assert!(manifest.contains("edition = \"2026\""));
        assert!(manifest.contains("main = \"src/main.roze\""));
        assert!(manifest.contains("authors = [\"Mohammad Firoze\"]"));

        let gitignore = fs::read_to_string(project_dir.join(".gitignore")).unwrap();
        assert!(gitignore.contains("/target/"));
        assert!(gitignore.contains("*.class"));
    }

    #[test]
    fn test_generate_new_lib_scaffolding() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("test_lib_pkg");

        let opts = NewOptions {
            path: project_dir.clone(),
            name: Some("custom_lib_name".to_string()),
            target: Some(TargetKind::Lib),
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
            template: None,
        };

        assert!(generate_new(&opts).is_ok());

        assert!(project_dir.join("bsr.mfz").exists());
        assert!(project_dir.join("src/lib.roze").exists());
        assert!(!project_dir.join("src/main.roze").exists());
        assert!(project_dir.join("tests/integration_test.roze").exists());
        assert!(project_dir.join("benches/bench_main.roze").exists());

        let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
        assert!(manifest.contains("name = \"custom_lib_name\""));
        assert!(manifest.contains("main = \"src/lib.roze\""));

        let lib_src = fs::read_to_string(project_dir.join("src/lib.roze")).unwrap();
        assert!(lib_src.contains("func add(a: int, b: int) -> int"));
    }

    #[test]
    fn test_generate_new_collision_error() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("existing_nonempty");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("some_file.txt"), "hello").unwrap();

        let opts = NewOptions {
            path: project_dir.clone(),
            name: None,
            target: Some(TargetKind::Bin),
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
            template: None,
        };

        let res = generate_new(&opts);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("already exists and is not an empty directory"));
        assert!(err_msg.contains("Use `bsr init` to initialize the directory"));
    }

    #[test]
    fn test_generate_init_in_empty_dir() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("init_pkg");
        fs::create_dir_all(&project_dir).unwrap();

        let opts = InitOptions {
            path: project_dir.clone(),
            name: None,
            target: None,
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
        };

        assert!(generate_init(&opts).is_ok());
        assert!(project_dir.join("bsr.mfz").exists());
        assert!(project_dir.join("src/main.roze").exists());
        assert!(project_dir.join("tests/integration_test.roze").exists());
        assert!(project_dir.join("benches/bench_main.roze").exists());
    }

    #[test]
    fn test_generate_init_detects_existing_lib() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("init_existing_lib");
        fs::create_dir_all(project_dir.join("src")).unwrap();
        fs::write(
            project_dir.join("src/lib.roze"),
            "func custom_func() -> int { return 42; }\n",
        )
        .unwrap();

        let opts = InitOptions {
            path: project_dir.clone(),
            name: None,
            target: None,
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
        };

        assert!(generate_init(&opts).is_ok());
        assert!(project_dir.join("bsr.mfz").exists());
        let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
        assert!(manifest.contains("main = \"src/lib.roze\""));

        // Ensure custom lib was not overwritten
        let lib_src = fs::read_to_string(project_dir.join("src/lib.roze")).unwrap();
        assert!(lib_src.contains("custom_func"));
    }

    #[test]
    fn test_generate_init_already_initialized_fails() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("already_pkg");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("bsr.mfz"), "name = \"foo\"\n").unwrap();

        let opts = InitOptions {
            path: project_dir.clone(),
            name: None,
            target: None,
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
        };

        let res = generate_init(&opts);
        assert!(res.is_err());
        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("manifest already exists"));
    }

    #[test]
    fn test_generate_init_appends_existing_gitignore() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join("init_gitignore");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join(".gitignore"), "custom_ignore/\n").unwrap();

        let opts = InitOptions {
            path: project_dir.clone(),
            name: None,
            target: None,
            vcs: Some(VcsKind::None),
            edition: "2026".to_string(),
            author: "Mohammad Firoze".to_string(),
        };

        assert!(generate_init(&opts).is_ok());
        let gitignore = fs::read_to_string(project_dir.join(".gitignore")).unwrap();
        assert!(gitignore.contains("custom_ignore/"));
        assert!(gitignore.contains("/target/"));
    }
}
