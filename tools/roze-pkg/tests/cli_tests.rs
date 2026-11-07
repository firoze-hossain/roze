// tools/roze-pkg/tests/cli_tests.rs
//
// Integration tests for the Basir (`bsr`) CLI binary.

use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn bsr_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // pop test binary name
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("bsr")
}

#[test]
fn test_cli_bsr_new_bin() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("my_bin_app");

    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--bin")
        .status()
        .expect("failed to execute bsr");

    assert!(status.success());
    assert!(project_dir.join("bsr.mfz").exists());
    assert!(project_dir.join("src/main.roze").exists());
    assert!(project_dir.join("tests/integration_test.roze").exists());
    assert!(project_dir.join("benches/bench_main.roze").exists());
    assert!(project_dir.join(".gitignore").exists());
    assert!(project_dir.join("README.md").exists());

    let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
    assert!(manifest.contains("name = \"my_bin_app\""));
    assert!(manifest.contains("main = \"src/main.roze\""));
    assert!(manifest.contains("edition = \"2026\""));

    let main_src = fs::read_to_string(project_dir.join("src/main.roze")).unwrap();
    assert!(main_src.contains("func main()"));
}

#[test]
fn test_cli_bsr_new_lib() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("my_lib_pkg");

    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--lib")
        .arg("--name")
        .arg("custom_library")
        .status()
        .expect("failed to execute bsr");

    assert!(status.success());
    assert!(project_dir.join("bsr.mfz").exists());
    assert!(project_dir.join("src/lib.roze").exists());
    assert!(!project_dir.join("src/main.roze").exists());
    assert!(project_dir.join("tests/integration_test.roze").exists());
    assert!(project_dir.join("benches/bench_main.roze").exists());

    let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
    assert!(manifest.contains("name = \"custom_library\""));
    assert!(manifest.contains("main = \"src/lib.roze\""));

    let lib_src = fs::read_to_string(project_dir.join("src/lib.roze")).unwrap();
    assert!(lib_src.contains("func add(a: int, b: int) -> int"));
    assert!(lib_src.contains("func greeting(name: string) -> string"));
}

#[test]
fn test_cli_bsr_init_current_dir() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("init_project");
    fs::create_dir_all(&project_dir).unwrap();

    let status = Command::new(bsr_bin())
        .current_dir(&project_dir)
        .arg("init")
        .status()
        .expect("failed to execute bsr");

    assert!(status.success());
    assert!(project_dir.join("bsr.mfz").exists());
    assert!(project_dir.join("src/main.roze").exists());
    assert!(project_dir.join("tests/integration_test.roze").exists());
    assert!(project_dir.join("benches/bench_main.roze").exists());

    let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
    assert!(manifest.contains("name = \"init_project\""));
}

#[test]
fn test_cli_bsr_init_lib_flag() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("init_lib_project");
    fs::create_dir_all(&project_dir).unwrap();

    let status = Command::new(bsr_bin())
        .current_dir(&project_dir)
        .arg("init")
        .arg("--lib")
        .status()
        .expect("failed to execute bsr");

    assert!(status.success());
    assert!(project_dir.join("bsr.mfz").exists());
    assert!(project_dir.join("src/lib.roze").exists());
    assert!(!project_dir.join("src/main.roze").exists());

    let manifest = fs::read_to_string(project_dir.join("bsr.mfz")).unwrap();
    assert!(manifest.contains("main = \"src/lib.roze\""));
}

#[test]
fn test_cli_bsr_new_collision_error() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("collision_pkg");

    // First creation succeeds
    let s1 = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .status()
        .expect("failed to execute bsr");
    assert!(s1.success());

    // Second creation fails
    let output = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .output()
        .expect("failed to execute bsr");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists and is not an empty directory"));
    assert!(stderr.contains("Use `bsr init` to initialize the directory"));
}

#[test]
fn test_cli_bsr_init_existing_package_error() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("already_init");
    fs::create_dir_all(&project_dir).unwrap();

    let s1 = Command::new(bsr_bin())
        .current_dir(&project_dir)
        .arg("init")
        .status()
        .expect("failed to execute bsr");
    assert!(s1.success());

    // Second init fails
    let output = Command::new(bsr_bin())
        .current_dir(&project_dir)
        .arg("init")
        .output()
        .expect("failed to execute bsr");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("manifest already exists"));
}

#[test]
fn test_cli_bsr_new_keyword_error() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("keyword_pkg");

    let output = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--name")
        .arg("struct")
        .output()
        .expect("failed to execute bsr");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("it is a Roze keyword"));
}

#[test]
fn test_cli_bsr_new_vcs_none() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("no_vcs_pkg");

    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--vcs")
        .arg("none")
        .status()
        .expect("failed to execute bsr");

    assert!(status.success());
    assert!(!project_dir.join(".git").exists());
}
