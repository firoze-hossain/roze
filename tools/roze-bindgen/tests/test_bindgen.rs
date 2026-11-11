// tools/roze-bindgen/tests/test_bindgen.rs
//
// Tests for roze-bindgen tool generating TypeScript & JavaScript interface bindings

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn scratch_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roze-bindgen-test-{}", test_name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir for bindgen test");
    dir
}

#[test]
fn test_standalone_roze_bindgen_cli() {
    let dir = scratch_dir("standalone");
    let out_dir = dir.join("pkg");
    let wasm_file = dir.join("calc.wasm");

    // Standard valid WebAssembly binary exporting `add(i32, i32) -> i32` and `memory`
    let wasm_bytes: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // Magic & Version
        0x01, 0x07, 0x01, 0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f, // Type section: (i32, i32) -> i32
        0x03, 0x02, 0x01, 0x00, // Function section: 1 func with type 0
        0x05, 0x03, 0x01, 0x00, 0x01, // Memory section: 1 page
        0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64, 0x00, 0x00, // Export: "add" func 0
        0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b, // Code: local.get 0, local.get 1, i32.add, end
    ];
    fs::write(&wasm_file, &wasm_bytes).unwrap();

    // Run roze-bindgen binary
    let bindgen_bin = env!("CARGO_BIN_EXE_roze-bindgen");
    let bindgen_res = Command::new(bindgen_bin)
        .arg(&wasm_file)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("invoke roze-bindgen binary");

    assert!(
        bindgen_res.status.success(),
        "roze-bindgen failed: {}",
        String::from_utf8_lossy(&bindgen_res.stderr)
    );

    // Verify all generated files
    assert!(out_dir.join("calc.js").exists());
    assert!(out_dir.join("calc.d.ts").exists());
    assert!(out_dir.join("package.json").exists());
    assert!(out_dir.join("index.html").exists());
    assert!(out_dir.join("run_node.mjs").exists());

    // Verify TypeScript declarations contain functions and class
    let dts_content = fs::read_to_string(out_dir.join("calc.d.ts")).unwrap();
    assert!(dts_content.contains("export class RozeModule"));
    assert!(dts_content.contains("add(arg0: number, arg1: number): number;"));

    // Verify package.json contains exports
    let pkg_content = fs::read_to_string(out_dir.join("package.json")).unwrap();
    assert!(pkg_content.contains("\"types\": \"calc.d.ts\""));
    assert!(pkg_content.contains("\"main\": \"calc.js\""));

    // Run run_node.mjs via node
    let node_res = Command::new("node")
        .arg(out_dir.join("run_node.mjs"))
        .current_dir(&out_dir)
        .output()
        .expect("execute node on generated binding");

    let stdout = String::from_utf8_lossy(&node_res.stdout);
    assert!(
        node_res.status.success(),
        "node run failed: {}",
        String::from_utf8_lossy(&node_res.stderr)
    );
    assert!(stdout.contains("WebAssembly Module Loaded Successfully!"));
    assert!(stdout.contains("add("));
}
