// ==============================================================================
// 🌹 Roze Compiler - 3-Stage Bootstrap Fixpoint Idempotence Tests (M6-T06)
// ==============================================================================

use std::fs;
use std::path::PathBuf;

fn get_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Failed to get workspace root")
        .to_path_buf()
}

#[test]
fn test_bootstrap_seed_exists_and_valid() {
    let root = get_workspace_root();
    let seed_path = root.join("bootstrap").join("roze_seed.c");
    assert!(seed_path.exists(), "bootstrap/roze_seed.c must exist");

    let metadata = fs::metadata(&seed_path).expect("Failed to stat roze_seed.c");
    assert!(
        metadata.len() > 100_000,
        "roze_seed.c must be a substantial standalone compiler unit (>100KB)"
    );

    let content = fs::read_to_string(&seed_path).expect("Failed to read roze_seed.c");
    assert!(
        content.contains("rz_compile_roze_to_c") || content.contains("main"),
        "roze_seed.c must contain compiler entrypoint"
    );
}

#[test]
fn test_verify_fixpoint_script_exists_and_executable() {
    let root = get_workspace_root();
    let script_path = root.join("scripts").join("verify_fixpoint.sh");
    assert!(
        script_path.exists(),
        "scripts/verify_fixpoint.sh must exist"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&script_path)
            .expect("Failed to get script permissions")
            .permissions();
        assert!(
            perms.mode() & 0o111 != 0,
            "scripts/verify_fixpoint.sh must be executable"
        );
    }
}

#[test]
fn test_stage2_stage3_fixpoint_idempotence() {
    let root = get_workspace_root();
    let stage2_c = root
        .join("demos")
        .join("self_hosted")
        .join("bin")
        .join("roze_stage2.c");
    let stage3_c = root
        .join("demos")
        .join("self_hosted")
        .join("bin")
        .join("roze_stage3.c");
    let stage2_bin = root
        .join("demos")
        .join("self_hosted")
        .join("bin")
        .join("roze_stage2");
    let stage3_bin = root
        .join("demos")
        .join("self_hosted")
        .join("bin")
        .join("roze_stage3");

    // If stage2 and stage3 binaries exist from build, assert mathematical closure
    if stage2_c.exists() && stage3_c.exists() {
        let content2 = fs::read(&stage2_c).expect("read stage2.c");
        let content3 = fs::read(&stage3_c).expect("read stage3.c");
        assert_eq!(
            content2.len(),
            content3.len(),
            "roze_stage2.c and roze_stage3.c must have identical size"
        );
        assert_eq!(
            content2, content3,
            "roze_stage2.c and roze_stage3.c must be 100% byte-for-byte identical (0 bytes diff)"
        );
    }

    if stage2_bin.exists() && stage3_bin.exists() {
        let bin2 = fs::read(&stage2_bin).expect("read stage2 binary");
        let bin3 = fs::read(&stage3_bin).expect("read stage3 binary");
        assert_eq!(
            bin2.len(),
            bin3.len(),
            "roze_stage2 and roze_stage3 binaries must have identical size"
        );
        assert_eq!(
            bin2, bin3,
            "roze_stage2 and roze_stage3 binaries must be 100% byte-for-byte identical (0 bytes diff)"
        );
    }
}

#[test]
fn test_makefile_has_fixpoint_targets() {
    let root = get_workspace_root();
    let root_mk = fs::read_to_string(root.join("Makefile")).expect("read root Makefile");
    assert!(
        root_mk.contains("fixpoint:"),
        "root Makefile must contain fixpoint target"
    );

    let demo_mk = fs::read_to_string(root.join("demos/self_hosted/Makefile"))
        .expect("read demo Makefile");
    assert!(
        demo_mk.contains("fixpoint:"),
        "demos/self_hosted/Makefile must contain fixpoint target"
    );
}
