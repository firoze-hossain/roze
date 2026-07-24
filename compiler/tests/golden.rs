// compiler/tests/golden.rs
//
// End-to-end "golden output" tests: build and run a real .roze fixture
// through the actual `roze` binary (not just its internal library
// functions), and assert its stdout matches exactly. These are what
// would have caught essentially every bug fixed across this project's
// recent history automatically, instead of by manual inspection.
//
// Needs a JDK (`javac`/`java`) on PATH, same as using the compiler
// normally -- see the CI workflows for how that's provisioned.
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures")
}

/// A scratch directory for one test to build/run its fixture in, so
/// parallel tests never fight over the same generated .java/.class
/// files.
fn scratch_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roze-golden-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir); // start clean if a previous run left one behind
    std::fs::create_dir_all(&dir).expect("create scratch dir for golden test");
    dir
}

/// Copies `fixture` into a fresh scratch dir, runs `roze run <fixture>`
/// there, and returns (stdout, stderr, success).
fn run_fixture(fixture: &str) -> (String, String, bool) {
    let dir = scratch_dir(fixture.trim_end_matches(".roze"));
    std::fs::copy(fixtures_dir().join(fixture), dir.join(fixture))
        .unwrap_or_else(|e| panic!("failed to copy fixture {}: {}", fixture, e));

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg(fixture)
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

/// Golden tests care about the *program's* output, not the compiler's own
/// progress messages ("🔤 Lexer: N tokens", etc). Everything the program
/// itself printed comes after the "🚀 Running: <name>" line.
fn program_output(stdout: &str) -> String {
    match stdout.split_once("🚀 Running:") {
        Some((_, after)) => after.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default(),
        None => stdout.to_string(),
    }
}

#[test]
fn hello_world() {
    let (stdout, stderr, ok) = run_fixture("hello.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "Hello, golden world!");
}

#[test]
fn if_else_and_while() {
    let (stdout, stderr, ok) = run_fixture("control_flow.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "positive\nnegative\nzero\n0\n1\n2");
}

#[test]
fn for_loop() {
    let (stdout, stderr, ok) = run_fixture("for_loop.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "0\n1\n2\n3\n4\nsum=55");
}

#[test]
fn core_intrinsics() {
    let (stdout, stderr, ok) = run_fixture("core_intrinsics.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "ROZE\nroze\n4\nfoobar\n42\n9\n3\n123\n457\ntrue\ntrue";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn import_core_module() {
    let (stdout, stderr, ok) = run_fixture("import_core.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "10\n0\n-1\n1\n0\n36\nababab");
}

#[test]
fn string_equality_uses_content_not_reference() {
    let (stdout, stderr, ok) = run_fixture("string_equality.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "true\nfalse\ntrue");
}

#[test]
fn syntax_error_reports_cleanly_and_never_leaks_a_backtrace() {
    let dir = scratch_dir("syntax_error");
    std::fs::write(dir.join("broken.roze"), "func main() {\n    println(\"hi\")\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("broken.roze")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    assert!(!output.status.success(), "a program with a missing '}}' should fail to build");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Parse error"), "expected a parse error, got:\n{}", stderr);
    assert!(stderr.contains("-->"), "expected a file:line:column pointer, got:\n{}", stderr);
    assert!(!stderr.contains("std::rt::lang_start"), "leaked a Rust panic backtrace:\n{}", stderr);
    assert!(!stderr.contains("Stack backtrace"), "leaked a Rust panic backtrace:\n{}", stderr);
}

#[test]
fn type_error_reports_cleanly() {
    let dir = scratch_dir("type_error");
    std::fs::write(
        dir.join("bad_return.roze"),
        "func add(a: int, b: int) -> int {\n    return \"nope\";\n}\nfunc main() {\n    println(add(1, 2));\n}\n",
    ).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("bad_return.roze")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Type error"), "expected a type error, got:\n{}", stderr);
    assert!(stderr.contains("bad_return.roze:2:"), "expected it to point at line 2, got:\n{}", stderr);
}

#[test]
fn missing_import_reports_cleanly() {
    let dir = scratch_dir("missing_import");
    std::fs::write(
        dir.join("main.roze"),
        "import \"does_not_exist\";\nfunc main() {\n    println(\"hi\");\n}\n",
    ).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("main.roze")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Import error"), "expected an import error, got:\n{}", stderr);
}
