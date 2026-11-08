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

#[test]
fn test_cli_bsr_manifest_display_and_check() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("manifest_pkg");

    // Create a new package
    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--bin")
        .status()
        .expect("failed to execute bsr new");
    assert!(status.success());

    // Append some features and target configs to bsr.mfz
    let mfz_extra = r#"
[features]
default = ["logging"]
logging = []

[target.native]
flags = ["-O3"]
opt_level = 3
"#;
    let mfz_path = project_dir.join("bsr.mfz");
    let mut content = fs::read_to_string(&mfz_path).unwrap();
    content.push_str(mfz_extra);
    fs::write(&mfz_path, content).unwrap();

    // Run bsr manifest (display mode)
    let output = Command::new(bsr_bin())
        .arg("manifest")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr manifest");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Basir Project Manifest"));
    assert!(stdout.contains("manifest_pkg"));
    assert!(stdout.contains("Feature Flags:"));
    assert!(stdout.contains("Target Configurations:"));

    // Run bsr manifest --check
    let check_output = Command::new(bsr_bin())
        .arg("manifest")
        .arg("--check")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr manifest --check");

    assert!(check_output.status.success());
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(check_stdout.contains("valid"));
}

#[test]
fn test_cli_bsr_lock_and_verify_workflow() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("lock_pkg");

    // 1. Create a new package
    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--bin")
        .status()
        .expect("failed to execute bsr new");
    assert!(status.success());

    // 2. Add dependencies under existing [dependencies] in bsr.mfz
    let mfz_path = project_dir.join("bsr.mfz");
    let mut content = fs::read_to_string(&mfz_path).unwrap();
    content.push_str("\"std::json\" = \"^0.2.0\"\n");
    fs::write(&mfz_path, content).unwrap();

    // 3. Run bsr lock
    let output = Command::new(bsr_bin())
        .arg("lock")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr lock");
    assert!(output.status.success());
    assert!(project_dir.join("bsr.lock").exists());

    let lock_content = fs::read_to_string(project_dir.join("bsr.lock")).unwrap();
    assert!(lock_content.contains("name = \"std::json\""));
    assert!(lock_content.contains("version = \"0.2.0\""));

    // 4. bsr lock --check should succeed
    let check_out = Command::new(bsr_bin())
        .arg("lock")
        .arg("--check")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr lock --check");
    assert!(check_out.status.success());
    let stdout = String::from_utf8_lossy(&check_out.stdout);
    assert!(stdout.contains("strictly up to date"));

    // 5. bsr install --locked should succeed
    let install_out = Command::new(bsr_bin())
        .arg("install")
        .arg("--locked")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr install --locked");
    assert!(install_out.status.success());
    assert!(project_dir.join("libs/std::json/lib.roze").exists());

    // 6. bsr lock --verify should succeed
    let verify_out = Command::new(bsr_bin())
        .arg("lock")
        .arg("--verify")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr lock --verify");
    assert!(verify_out.status.success());
    let v_stdout = String::from_utf8_lossy(&verify_out.stdout);
    assert!(v_stdout.contains("cryptographic integrity"));

    // 7. Tamper with installed library: bsr lock --verify must fail!
    fs::write(project_dir.join("libs/std::json/lib.roze"), "// tampered malicious code").unwrap();
    let tamper_verify = Command::new(bsr_bin())
        .arg("lock")
        .arg("--verify")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr lock --verify");
    assert!(!tamper_verify.status.success(), "Tampered package must fail lock verification");
    let err_str = String::from_utf8_lossy(&tamper_verify.stderr);
    let out_str = String::from_utf8_lossy(&tamper_verify.stdout);
    assert!(err_str.contains("Supply-chain security violation") || out_str.contains("CHECKSUM MISMATCH"));

    // 8. Manifest desync: add new dependency, bsr lock --check must fail
    let mut content2 = fs::read_to_string(&mfz_path).unwrap();
    content2.push_str("\"test_kit\" = \"^0.1.0\"\n");
    fs::write(&mfz_path, content2).unwrap();

    let check_fail = Command::new(bsr_bin())
        .arg("lock")
        .arg("--check")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr lock --check");
    assert!(!check_fail.status.success(), "Outdated lockfile must fail check");
}

#[test]
fn test_cli_bsr_vendor_and_offline_build() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("vendor_cli_app");

    // 1. Create project
    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--bin")
        .status()
        .expect("failed to execute bsr new");
    assert!(status.success());

    // 2. Add dependency to bsr.mfz
    let mfz_path = project_dir.join("bsr.mfz");
    let mut content = fs::read_to_string(&mfz_path).unwrap();
    content.push_str("\"std::json\" = \"^0.2.0\"\n");
    fs::write(&mfz_path, content).unwrap();

    // 3. bsr install
    let install_status = Command::new(bsr_bin())
        .arg("install")
        .current_dir(&project_dir)
        .status()
        .expect("failed to execute bsr install");
    assert!(install_status.success());

    // 4. bsr vendor
    let vendor_status = Command::new(bsr_bin())
        .arg("vendor")
        .current_dir(&project_dir)
        .status()
        .expect("failed to execute bsr vendor");
    assert!(vendor_status.success());

    let vendor_dir = project_dir.join("vendor");
    assert!(vendor_dir.exists());
    assert!(vendor_dir.join("bsr-vendor.mfz").exists());
    assert!(vendor_dir.join(".bsr-checksum").exists());
    assert!(vendor_dir.join("std_json").exists());
    assert!(vendor_dir.join("std_json/.bsr-checksum.json").exists());

    // 5. bsr vendor --verify
    let verify_out = Command::new(bsr_bin())
        .arg("vendor")
        .arg("--verify")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr vendor --verify");
    assert!(verify_out.status.success());
    let stdout = String::from_utf8_lossy(&verify_out.stdout);
    assert!(stdout.contains("cryptographically verified"));

    // 6. Tamper with vendored file and verify detection
    fs::write(vendor_dir.join("std_json/lib.roze"), "// TAMPERED VENDOR CODE").unwrap();
    let tamper_out = Command::new(bsr_bin())
        .arg("vendor")
        .arg("--verify")
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr vendor --verify");
    assert!(!tamper_out.status.success());

    // 7. Re-vendor cleanly
    let revendor_status = Command::new(bsr_bin())
        .arg("vendor")
        .current_dir(&project_dir)
        .status()
        .expect("failed to execute bsr vendor");
    assert!(revendor_status.success());

    // 8. Wipe libs/ and install with --offline
    let _ = fs::remove_dir_all(project_dir.join("libs"));
    assert!(!project_dir.join("libs").exists());

    let offline_install = Command::new(bsr_bin())
        .arg("install")
        .arg("--offline")
        .current_dir(&project_dir)
        .status()
        .expect("failed to execute bsr install --offline");
    assert!(offline_install.success());
    assert!(project_dir.join("libs/std::json/lib.roze").exists());
}

#[test]
fn test_cli_bsr_cache_management() {
    let temp_cache = tempdir().unwrap();
    let cache_dir = temp_cache.path().join("cache");

    // 1. bsr cache list on empty cache
    let list_out = Command::new(bsr_bin())
        .env("BSR_CACHE_DIR", &cache_dir)
        .arg("cache")
        .arg("list")
        .output()
        .expect("failed to execute bsr cache list");
    assert!(list_out.status.success());
    let stdout = String::from_utf8_lossy(&list_out.stdout);
    assert!(stdout.contains("empty") || stdout.contains("cache"));

    // 2. bsr cache verify
    let verify_out = Command::new(bsr_bin())
        .env("BSR_CACHE_DIR", &cache_dir)
        .arg("cache")
        .arg("verify")
        .output()
        .expect("failed to execute bsr cache verify");
    assert!(verify_out.status.success());

    // 3. bsr cache clean
    let clean_out = Command::new(bsr_bin())
        .env("BSR_CACHE_DIR", &cache_dir)
        .arg("cache")
        .arg("clean")
        .output()
        .expect("failed to execute bsr cache clean");
    assert!(clean_out.status.success());
    let clean_stdout = String::from_utf8_lossy(&clean_out.stdout);
    assert!(clean_stdout.contains("Cleaned global package cache"));
}

#[test]
fn test_cli_bsr_keygen() {
    let output = Command::new(bsr_bin())
        .arg("keygen")
        .output()
        .expect("failed to execute bsr keygen");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Ed25519 Digital Signing Keypair Generated"));
    assert!(stdout.contains("Public Key:"));
    assert!(stdout.contains("Secret Seed:"));
}

#[test]
fn test_cli_bsr_search() {
    let output = Command::new(bsr_bin())
        .arg("search")
        .arg("json")
        .output()
        .expect("failed to execute bsr search json");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("std::json"));
    assert!(stdout.contains("JSON parsing"));
}

#[test]
fn test_cli_bsr_publish_dry_run_with_ed25519() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("secure_crypto_pkg");

    // 1. Create project
    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--lib")
        .status()
        .expect("failed to execute bsr new");
    assert!(status.success());

    // 2. Generate keypair
    let keygen_out = Command::new(bsr_bin())
        .arg("keygen")
        .output()
        .expect("failed to execute bsr keygen");
    assert!(keygen_out.status.success());
    let stdout = String::from_utf8_lossy(&keygen_out.stdout);

    // Extract seed
    let seed_line = stdout.lines().find(|l| l.contains("Secret Seed:")).unwrap();
    let seed = seed_line.split("Secret Seed:").nth(1).unwrap().trim();

    // 3. Dry-run publish with Ed25519 signature
    let pub_out = Command::new(bsr_bin())
        .arg("publish")
        .arg("--dry-run")
        .arg("--key")
        .arg(seed)
        .current_dir(&project_dir)
        .output()
        .expect("failed to execute bsr publish");

    assert!(pub_out.status.success());
    let pub_stdout = String::from_utf8_lossy(&pub_out.stdout);
    assert!(pub_stdout.contains("Packaging secure_crypto_pkg"));
    assert!(pub_stdout.contains("Ed25519 Signature:"));
    assert!(pub_stdout.contains("VERIFIED (RFC 8032)"));
    assert!(pub_stdout.contains("Package verified successfully (dry run)"));
}

#[test]
fn test_cli_bsr_git_tag_dependency() {
    // 1. Create local git repo with tag v1.0.0
    let temp_git = tempdir().unwrap();
    let git_repo = temp_git.path().join("remote_pkg");
    fs::create_dir_all(&git_repo).unwrap();

    let _ = Command::new("git").current_dir(&git_repo).args(["init"]).status();
    let _ = Command::new("git").current_dir(&git_repo).args(["config", "user.email", "git@roze.dev"]).status();
    let _ = Command::new("git").current_dir(&git_repo).args(["config", "user.name", "Git User"]).status();

    fs::write(git_repo.join("bsr.mfz"), "[package]\nname = \"remote_pkg\"\nversion = \"1.0.0\"\nmain = \"src/lib.roze\"\n").unwrap();
    fs::create_dir_all(git_repo.join("src")).unwrap();
    fs::write(git_repo.join("src/lib.roze"), "func remote_answer() -> int { return 42; }\n").unwrap();

    let _ = Command::new("git").current_dir(&git_repo).args(["add", "."]).status();
    let _ = Command::new("git").current_dir(&git_repo).args(["commit", "-m", "Release v1.0.0"]).status();
    let _ = Command::new("git").current_dir(&git_repo).args(["tag", "v1.0.0"]).status();

    // 2. Create consumer project
    let temp_proj = tempdir().unwrap();
    let consumer_dir = temp_proj.path().join("consumer_app");
    let status = Command::new(bsr_bin())
        .arg("new")
        .arg(&consumer_dir)
        .arg("--bin")
        .status()
        .expect("failed to create consumer");
    assert!(status.success());

    // 3. Add git dependency with tag
    let add_status = Command::new(bsr_bin())
        .arg("add")
        .arg("remote_pkg")
        .arg("--git")
        .arg(git_repo.display().to_string())
        .arg("--tag")
        .arg("v1.0.0")
        .current_dir(&consumer_dir)
        .status()
        .expect("failed to add git dep");
    assert!(add_status.success());

    // 4. Install dependencies
    let temp_cache = tempdir().unwrap();
    let cache_dir = temp_cache.path().join("cache");
    let install_status = Command::new(bsr_bin())
        .env("BSR_CACHE_DIR", &cache_dir)
        .arg("install")
        .current_dir(&consumer_dir)
        .status()
        .expect("failed to install git dep");
    assert!(install_status.success());

    // 5. Verify lockfile and installed files
    let lock_content = fs::read_to_string(consumer_dir.join("bsr.lock")).unwrap();
    assert!(lock_content.contains("name = \"remote_pkg\""));
    assert!(lock_content.contains("git+"));

    assert!(consumer_dir.join("libs/remote_pkg/src/lib.roze").exists());
}

#[test]
fn test_cli_bsr_doc_generation() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("doc_test_app");

    // 1. Create a new library project
    let new_status = Command::new(bsr_bin())
        .arg("new")
        .arg(&project_dir)
        .arg("--lib")
        .status()
        .expect("failed to run bsr new");
    assert!(new_status.success());

    // 2. Add rich docstrings to src/lib.roze
    let lib_code = r#"
//! Core documentation test module.

/// Computes the square of an integer.
///
/// @param n Integer to square
/// @return Square value
/// @example
/// let res = square(5);
func square(n: int) -> int {
    return n * n;
}

/// A 2D point representation.
class Point {
    x: float,
    y: float,
}
"#;
    fs::write(project_dir.join("src/lib.roze"), lib_code).unwrap();

    // 3. Run bsr doc --output target/doc --json
    let doc_status = Command::new(bsr_bin())
        .arg("doc")
        .arg("--output")
        .arg(project_dir.join("target/doc").display().to_string())
        .arg("--json")
        .current_dir(&project_dir)
        .status()
        .expect("failed to run bsr doc");
    assert!(doc_status.success());

    // 4. Verify generated static documentation assets
    let out_dir = project_dir.join("target/doc");
    assert!(out_dir.join("index.html").exists());
    assert!(out_dir.join("rozedoc.css").exists());
    assert!(out_dir.join("search.js").exists());
    assert!(out_dir.join("search-index.js").exists());
    assert!(out_dir.join("playground.js").exists());
    assert!(out_dir.join("doc.json").exists());

    let index_html = fs::read_to_string(out_dir.join("index.html")).unwrap();
    assert!(index_html.contains("doc_test_app"));
    assert!(index_html.contains("ROZE CRATE"));

    let search_js = fs::read_to_string(out_dir.join("search-index.js")).unwrap();
    assert!(search_js.contains("square"));
    assert!(search_js.contains("Point"));
}




