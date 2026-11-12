// compiler/tests/seed_packages.rs
//
// 🌹 Reference Seed Ecosystem & Community Package Registry Test Suite (M6-T10)
// Validates official registry seeding, Go proxy protocol, Ed25519 envelope signatures,
// dependency installation, and real compilation for:
// 1. rozeboot (enterprise web microservices)
// 2. roze-orm (high-throughput SQL ORM)
// 3. roze-torch (tensors, autograd, neural layers & CUDA FFI)
// 4. roze-game (2D/3D Raylib game engine)
// 5. roze-ui (declarative desktop GUI framework)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

use roze_compiler::pkg::crypto::ed25519_generate_keypair;
use roze_compiler::pkg::dependency::DependencyManager;
use roze_compiler::pkg::lockfile::LockFile;
use roze_compiler::pkg::registry::{RegistryClient, SignedPackageEnvelope};

fn roze_bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("roze")
}

#[test]
fn test_seed_packages_registry_catalog() {
    let client = RegistryClient::new();
    let seeds = ["rozeboot", "roze-orm", "roze-torch", "roze-game", "roze-ui"];

    for seed in &seeds {
        let meta = client.query_package(seed);
        assert!(meta.is_some(), "Seed package '{}' must be registered in official registry", seed);
        let meta = meta.unwrap();
        assert_eq!(meta.name, *seed);
        assert!(!meta.versions.is_empty(), "Seed package '{}' must have versions available", seed);
        assert_eq!(meta.versions.last().unwrap().version.to_string(), "1.0.0");
    }
}

#[test]
fn test_seed_packages_search() {
    let client = RegistryClient::new();

    let search_torch = client.search_packages("torch");
    assert!(search_torch.iter().any(|p| p.name == "roze-torch"));

    let search_game = client.search_packages("game");
    assert!(search_game.iter().any(|p| p.name == "roze-game"));

    let search_ui = client.search_packages("ui");
    assert!(search_ui.iter().any(|p| p.name == "roze-ui"));

    let search_boot = client.search_packages("boot");
    assert!(search_boot.iter().any(|p| p.name == "rozeboot"));

    let search_orm = client.search_packages("orm");
    assert!(search_orm.iter().any(|p| p.name == "roze-orm"));
}

#[test]
fn test_seed_packages_go_proxy_protocol() {
    let client = RegistryClient::new();
    let seeds = ["rozeboot", "roze-orm", "roze-torch", "roze-game", "roze-ui"];

    for seed in &seeds {
        // 1. /@v/list
        let versions = client.go_proxy_list_versions(seed);
        assert!(versions.contains(&"v1.0.0".to_string()), "Version v1.0.0 must be listed for {}", seed);

        // 2. /@v/<version>.info
        let info = client.go_proxy_version_info(seed, "v1.0.0").expect("version info");
        assert_eq!(info.version, "v1.0.0");

        // 3. /@v/<version>.mfz
        let mfz = client.go_proxy_manifest(seed, "v1.0.0").expect("manifest");
        assert!(mfz.contains(&format!("name = \"{}\"", seed)));
        assert!(mfz.contains("version = \"1.0.0\""));
    }
}

#[test]
fn test_seed_packages_ed25519_digital_envelopes() {
    let kp = ed25519_generate_keypair();
    let seeds = ["rozeboot", "roze-orm", "roze-torch", "roze-game", "roze-ui"];

    for seed in &seeds {
        let dummy_checksum = format!("sha256_{}_tree_hash_verified", seed);
        let envelope = SignedPackageEnvelope::new(seed, "1.0.0", &dummy_checksum, &kp.seed);

        // 1. Valid envelope passes verification
        assert!(envelope.verify().unwrap(), "Signature verification must succeed for {}", seed);

        // 2. Tampered envelope is rejected
        let mut tampered = envelope.clone();
        tampered.checksum = format!("{}_tampered", dummy_checksum);
        assert!(!tampered.verify().unwrap(), "Tampered checksum must fail verification for {}", seed);
    }
}

#[test]
fn test_seed_packages_installation_and_compilation() {
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("ml_game_app");
    fs::create_dir_all(&project_dir).unwrap();

    // 1. Create bsr.mfz requesting all 5 reference seed packages
    let manifest = r#"[package]
name = "ml_game_app"
version = "0.1.0"
edition = "2026"
authors = ["Test User"]
main = "src/main.roze"

[dependencies]
roze-torch = "^1.0.0"
roze-game = "^1.0.0"
roze-ui = "^1.0.0"
roze-orm = "^1.0.0"
rozeboot = "^1.0.0"
"#;
    fs::write(project_dir.join("bsr.mfz"), manifest).unwrap();

    // 2. Run dependency installer
    let mut mgr = DependencyManager::new(project_dir.clone());
    mgr.install_all().expect("Install all seed packages");

    let lockfile = LockFile::load(&project_dir.join("bsr.lock")).expect("Load bsr.lock");
    assert!(lockfile.contains_package("roze-torch", "1.0.0"));
    assert!(lockfile.contains_package("roze-game", "1.0.0"));
    assert!(lockfile.contains_package("roze-ui", "1.0.0"));
    assert!(lockfile.contains_package("roze-orm", "1.0.0"));
    assert!(lockfile.contains_package("rozeboot", "1.0.0"));

    assert!(project_dir.join("libs/roze-torch/lib.roze").exists() || project_dir.join("libs/roze-torch/src/lib.roze").exists());
    assert!(project_dir.join("libs/roze-game/lib.roze").exists() || project_dir.join("libs/roze-game/src/lib.roze").exists());
    assert!(project_dir.join("libs/roze-ui/lib.roze").exists() || project_dir.join("libs/roze-ui/src/lib.roze").exists());

    // 3. Create consumer application source using the seed packages
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let app_code = r#"
import "roze-torch";
import "roze-game";
import "roze-ui";

func main() {
    // 1. Use roze-torch
    let t = tensor_ones_2d(2, 2);
    let v = tensor_get_2d(t, 0, 0);

    // 2. Use roze-game
    let pos = vec2_new(100.0, 200.0);
    let box = rect_new(0.0, 0.0, 32.0, 32.0);

    // 3. Use roze-ui
    let btn = button_new("Play Game");
    let lbl = label_new("AI Game Dashboard");

    println("Seed packages successfully verified!");
}
"#;
    fs::write(src_dir.join("main.roze"), app_code).unwrap();

    // 4. Test compilation via monolithic roze check CLI
    let bin = roze_bin();
    let output = Command::new(&bin)
        .current_dir(&project_dir)
        .arg("check")
        .arg("src/main.roze")
        .output()
        .expect("invoke roze check");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "roze check failed on seed packages:\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
}

#[test]
fn test_seed_package_torch_standalone_execution() {
    let bin = roze_bin();
    let torch_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("packages/roze-torch/tests/test_torch.roze");

    let output = Command::new(&bin)
        .arg("check")
        .arg(&torch_test)
        .output()
        .expect("check test_torch.roze");

    assert!(output.status.success(), "test_torch.roze semantic check failed: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_seed_package_game_standalone_execution() {
    let bin = roze_bin();
    let game_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("packages/roze-game/tests/test_game.roze");

    let output = Command::new(&bin)
        .arg("check")
        .arg(&game_test)
        .output()
        .expect("check test_game.roze");

    assert!(output.status.success(), "test_game.roze semantic check failed: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_seed_package_ui_standalone_execution() {
    let bin = roze_bin();
    let ui_test = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("packages/roze-ui/tests/test_ui.roze");

    let output = Command::new(&bin)
        .arg("check")
        .arg(&ui_test)
        .output()
        .expect("check test_ui.roze");

    assert!(output.status.success(), "test_ui.roze semantic check failed: {}", String::from_utf8_lossy(&output.stderr));
}
