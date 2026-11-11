// compiler/tests/wasm.rs
//
// WebAssembly Target Integration Tests (M6-T07)
// Verifies WASM32 and WASM32-WASI compilation, linear memory export,
// execution via Node.js WASI preview1 runtime, and JS/TS binding generation.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn scratch_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roze-wasm-{}", test_name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir for wasm test");
    dir
}

#[test]
fn test_wasm32_wasi_build_and_run() {
    let dir = scratch_dir("wasi-exec");
    let src_file = dir.join("hello_wasi.roze");
    fs::write(
        &src_file,
        r#"
func add(a: int, b: int) -> int {
    return a + b;
}

func main() {
    println("Hello WebAssembly WASI from Roze!");
    let sum = add(17, 25);
    println(sum);
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    // 1. Build --target wasm32-wasi
    let build_output = Command::new(roze_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--target")
        .arg("wasm32-wasi")
        .current_dir(&dir)
        .output()
        .expect("invoke roze build --target wasm32-wasi");

    assert!(
        build_output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let wasm_file = dir.join("hello_wasi.wasm");
    assert!(wasm_file.exists(), "wasm file was not generated");

    // Check WASM magic bytes (\0asm)
    let bytes = fs::read(&wasm_file).unwrap();
    assert_eq!(&bytes[0..4], &[0x00, 0x61, 0x73, 0x6d]);

    // 2. Run --target wasm32-wasi via roze run
    let run_output = Command::new(roze_bin)
        .arg("run")
        .arg(&src_file)
        .arg("--target")
        .arg("wasm32-wasi")
        .current_dir(&dir)
        .output()
        .expect("invoke roze run --target wasm32-wasi");

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(
        run_output.status.success(),
        "roze run failed: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert!(
        stdout.contains("Hello WebAssembly WASI from Roze!"),
        "stdout missing greeting: {}",
        stdout
    );
    assert!(stdout.contains("42"), "stdout missing sum 42: {}", stdout);
}

#[test]
fn test_wasm32_core_module_exports() {
    let dir = scratch_dir("core-wasm");
    let src_file = dir.join("math.roze");
    fs::write(
        &src_file,
        r#"
func add(a: int, b: int) -> int {
    return a + b;
}

func sub(a: int, b: int) -> int {
    return a - b;
}

func square(x: int) -> int {
    return x * x;
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    let build_output = Command::new(roze_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--target")
        .arg("wasm32")
        .current_dir(&dir)
        .output()
        .expect("invoke roze build --target wasm32");

    assert!(
        build_output.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );

    let wasm_file = dir.join("math.wasm");
    assert!(wasm_file.exists());

    // Execute core functions via node
    let script = format!(
        r#"
const fs = require('node:fs');
const wasmBuffer = fs.readFileSync('{}');
const wasmModule = new WebAssembly.Module(wasmBuffer);
const wasmInstance = new WebAssembly.Instance(wasmModule, {{}});
const {{ add, sub, square, memory, roze_alloc, roze_free }} = wasmInstance.exports;

if (add(10, 32) !== 42) process.exit(1);
if (sub(100, 58) !== 42) process.exit(2);
if (square(9) !== 81) process.exit(3);
if (!memory) process.exit(4);
if (typeof roze_alloc !== 'function' || typeof roze_free !== 'function') process.exit(5);

console.log("ALL_EXPORTS_VALID");
"#,
        wasm_file.display()
    );

    let node_output = Command::new("node")
        .arg("-e")
        .arg(script)
        .current_dir(&dir)
        .output()
        .expect("invoke node");

    let node_stdout = String::from_utf8_lossy(&node_output.stdout);
    assert!(
        node_output.status.success(),
        "node script failed: {}",
        String::from_utf8_lossy(&node_output.stderr)
    );
    assert!(node_stdout.contains("ALL_EXPORTS_VALID"));
}

#[test]
fn test_roze_bindgen_cli() {
    let dir = scratch_dir("bindgen-cli");
    let src_file = dir.join("app.roze");
    fs::write(
        &src_file,
        r#"
func add(a: int, b: int) -> int {
    return a + b;
}

func main() {
    println("Bindgen Integration Verification!");
}
"#,
    )
    .unwrap();

    let roze_bin = env!("CARGO_BIN_EXE_roze");

    // Build wasm
    let build_output = Command::new(roze_bin)
        .arg("build")
        .arg(&src_file)
        .arg("--target")
        .arg("wasm32-wasi")
        .current_dir(&dir)
        .output()
        .expect("invoke roze build");
    assert!(build_output.status.success());

    let wasm_file = dir.join("app.wasm");
    let out_dir = dir.join("pkg");

    // Run roze bindgen
    let bindgen_output = Command::new(roze_bin)
        .arg("bindgen")
        .arg(&wasm_file)
        .arg("--out-dir")
        .arg(&out_dir)
        .current_dir(&dir)
        .output()
        .expect("invoke roze bindgen");

    assert!(
        bindgen_output.status.success(),
        "bindgen failed: {}",
        String::from_utf8_lossy(&bindgen_output.stderr)
    );

    // Verify generated artifacts
    assert!(out_dir.join("app.wasm").exists());
    assert!(out_dir.join("app.js").exists());
    assert!(out_dir.join("app.d.ts").exists());
    assert!(out_dir.join("package.json").exists());
    assert!(out_dir.join("run_node.mjs").exists());
    assert!(out_dir.join("index.html").exists());

    // Execute run_node.mjs
    let node_run = Command::new("node")
        .arg(out_dir.join("run_node.mjs"))
        .current_dir(&out_dir)
        .output()
        .expect("run node on generated binding");

    let stdout = String::from_utf8_lossy(&node_run.stdout);
    assert!(
        node_run.status.success(),
        "run_node.mjs failed: {}",
        String::from_utf8_lossy(&node_run.stderr)
    );
    assert!(stdout.contains("Bindgen Integration Verification!"));
}
