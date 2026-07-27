// compiler/tests/native_golden.rs
//
// End-to-end tests for the native (Cranelift) backend: build and run
// real .roze fixtures with `--target native`, then check the actual
// executable's output -- the same "build it for real, run it for real"
// standard the JVM golden tests hold to (see golden.rs), applied to a
// completely different backend consuming the same typed IR.
//
// Needs a C compiler ('cc') on PATH for linking -- present on
// essentially every dev machine and CI image already.
use std::path::PathBuf;
use std::process::Command;

fn scratch_dir(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("roze-native-golden-{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch dir for native golden test");
    dir
}

/// Writes `source` as `<name>.roze` in a fresh scratch dir, builds and
/// runs it with `--target native`, and returns (stdout, stderr, success).
fn run_native_source(name: &str, source: &str) -> (String, String, bool) {
    let dir = scratch_dir(name);
    let file_name = format!("{}.roze", name);
    std::fs::write(dir.join(&file_name), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg(&file_name)
        .arg("--target")
        .arg("native")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}

/// Same line-ending normalization as the JVM golden tests' `program_output`
/// (not actually needed for the native backend's own printf-based output,
/// which always emits '\n', but kept for consistency and in case that
/// changes).
fn program_output(stdout: &str) -> String {
    match stdout.split_once("🚀 Running:") {
        Some((_, after)) => after.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default(),
        None => stdout.to_string(),
    }.replace("\r\n", "\n")
}

#[test]
fn hello_world_string_int_and_bool() {
    let (stdout, stderr, ok) = run_native_source(
        "hello",
        "func main() {\n    println(\"Hello from native Roze!\");\n    println(42);\n    println(true);\n    println(false);\n}\n",
    );
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "Hello from native Roze!\n42\ntrue\nfalse"
    );
}

#[test]
fn produces_a_real_standalone_executable_not_wrapped_in_a_jvm() {
    let dir = scratch_dir("standalone_check");
    std::fs::write(dir.join("prog.roze"), "func main() { println(1); }\n").unwrap();

    let build = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("prog.roze")
        .arg("--target")
        .arg("native")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");
    assert!(build.status.success());

    // The build should produce a real, directly-executable binary --
    // not a .class file needing `java` to interpret it.
    let exe_path = dir.join("prog");
    assert!(exe_path.exists(), "expected a native executable at {:?}", exe_path);

    let direct_run = Command::new(&exe_path).output().expect("failed to run the executable directly");
    assert!(direct_run.status.success());
    assert_eq!(String::from_utf8_lossy(&direct_run.stdout).trim_end(), "1");
}

#[test]
fn recursion_arithmetic_and_for_loop() {
    let source = "\
func fib(n: int) -> int {
    if n < 2 {
        return n;
    }
    return fib(n - 1) + fib(n - 2);
}

func sum_to(n: int) -> int {
    let total = 0;
    for let i = 1; i <= n; i = i + 1 {
        total = total + i;
    }
    return total;
}

func main() {
    println(fib(10));
    println(sum_to(100));
}
";
    let (stdout, stderr, ok) = run_native_source("fib_and_sum", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "55\n5050");
}

#[test]
fn if_else_if_else_chain() {
    let source = "\
func sign(x: int) -> int {
    if x > 0 {
        return 1;
    } else if x < 0 {
        return -1;
    } else {
        return 0;
    }
}

func main() {
    println(sign(5));
    println(sign(-5));
    println(sign(0));
}
";
    let (stdout, stderr, ok) = run_native_source("sign_fn", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "1\n-1\n0");
}

#[test]
fn while_loop_and_short_circuit_boolean_logic() {
    let source = "\
func main() {
    let i = 0;
    while i < 3 {
        println(i);
        i = i + 1;
    }

    let x = 5;
    let y = 10;
    println(x < y && y > 0);
    println(x > y || y > 0);
    println(!(x == y));
}
";
    let (stdout, stderr, ok) = run_native_source("while_and_bools", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "0\n1\n2\ntrue\ntrue\ntrue");
}

#[test]
fn short_circuit_and_genuinely_skips_the_right_hand_side() {
    // If && didn't actually short-circuit, calling `boom()` (which
    // divides by zero) would crash the program even though `false &&`
    // should never evaluate it.
    let source = "\
func boom() -> bool {
    let x = 1 / 0;
    return true;
}

func main() {
    println(false && boom());
    println(true || boom());
}
";
    let (stdout, stderr, ok) = run_native_source("short_circuit_real", source);
    assert!(ok, "build/run failed (short-circuit may not be skipping evaluation):\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "false\ntrue");
}

#[test]
fn calling_other_functions_including_void_ones() {
    let source = "\
func unused_void_function() {
    println(999);
}

func add(a: int, b: int) -> int {
    return a + b;
}

func print_twice(x: int) {
    println(x);
    println(x);
}

func main() {
    println(add(3, 4));
    print_twice(add(1, 1));
}
";
    let (stdout, stderr, ok) = run_native_source("calls", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "7\n2\n2");
}

// ---- Error paths: unsupported constructs must fail clearly, never silently miscompile ----

#[test]
fn intrinsic_call_is_rejected_with_a_clear_message() {
    let (_stdout, stderr, ok) = run_native_source(
        "reject_intrinsic",
        "func main() {\n    let l = list_new();\n    println(list_length(l));\n}\n",
    );
    assert!(!ok, "expected the native backend to reject an intrinsic call");
    assert!(stderr.contains("native backend"), "expected a clear native-backend-specific message, got:\n{}", stderr);
}

#[test]
fn general_string_variable_is_rejected_with_a_clear_message() {
    let (_stdout, stderr, ok) = run_native_source(
        "reject_string_var",
        "func main() {\n    let s = \"hi\";\n    println(s);\n}\n",
    );
    assert!(!ok, "expected the native backend to reject a general string value");
    assert!(stderr.contains("native backend"), "expected a clear native-backend-specific message, got:\n{}", stderr);
}

#[test]
fn jvm_backend_is_still_the_default_and_unaffected() {
    // No --target flag at all -- must still produce a working JVM build,
    // completely unaffected by the native backend's existence.
    let dir = scratch_dir("jvm_still_default");
    std::fs::write(dir.join("prog.roze"), "func main() { println(\"still jvm\"); }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg("prog.roze")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    assert!(output.status.success(), "stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Generated Java source"), "expected the JVM backend's own log line, got:\n{}", stdout);
    assert_eq!(program_output(&stdout).trim_end(), "still jvm");
}
