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
fn general_string_variables_now_work() {
    // This used to be explicitly rejected -- see the ARC implementation
    // in native.rs (compile_string_literal, string retain/release/
    // concat/eq) for when that changed, once the memory model decision
    // (docs/MEMORY_MODEL_DECISION.md) landed on ARC.
    let (stdout, stderr, ok) = run_native_source(
        "general_string_var",
        "func main() {\n    let s = \"hi\";\n    println(s);\n}\n",
    );
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "hi");
}

#[test]
fn list_and_map_are_still_rejected_with_a_clear_message() {
    // Unlike strings, list/map haven't had ARC ported to their
    // (multiple, variable-count) elements yet -- still out of scope.
    let (_stdout, stderr, ok) = run_native_source(
        "reject_list",
        "func main() {\n    let l = list_new();\n    println(l);\n}\n",
    );
    assert!(!ok, "expected the native backend to still reject list/map");
    assert!(stderr.contains("native backend"), "expected a clear native-backend-specific message, got:\n{}", stderr);
}

#[test]
fn no_memory_leaks_in_a_string_heavy_program() {
    // Gated on valgrind being available (not installed by default
    // almost anywhere, and Linux-only) -- skips with a clear message
    // rather than failing if it's missing. This directly guards
    // against the exact leak class found (via valgrind) during ARC's
    // initial implementation: a fresh/temporary string value consumed
    // as an operand to concat/equality/println and then discarded
    // needs an explicit release, since no named binding ever owns it.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_memory_leaks_in_a_string_heavy_program: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_leak_check");
    let source = "\
func digit_str(d: int) -> string {
    if d == 0 { return \"0\"; }
    if d == 1 { return \"1\"; }
    if d == 2 { return \"2\"; }
    return \"3+\";
}

func build(n: int) -> string {
    let result = \"\";
    let x = n;
    let i = 0;
    while i < 4 {
        result = digit_str(x - (x / 3) * 3) + result;
        x = x / 3;
        i = i + 1;
    }
    return \"n=\" + result;
}

func main() {
    let i = 0;
    while i < 100 {
        let s = build(i);
        i = i + 1;
    }
    println(\"done\");
}
";
    std::fs::write(dir.join("leak_check.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("leak_check.roze")
        .arg("--target")
        .arg("native")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let valgrind_output = Command::new("valgrind")
        .arg("--leak-check=full")
        .arg("--error-exitcode=1")
        .arg("./leak_check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn string_concat_equality_and_functions_are_correct() {
    // Output-correctness (as opposed to no_memory_leaks_in_a_string_heavy_program,
    // which checks for leaks specifically) for the core string
    // operations together: concatenation, content equality (not
    // pointer identity), passing a string into a function, and
    // returning one back out.
    let source = "\
func greet(name: string) -> string {
    return \"Hello, \" + name + \"!\";
}

func main() {
    let a = \"foo\";
    let b = \"foo\";
    println(a == b);
    println(a == \"bar\");
    println(a != \"bar\");
    println(greet(\"Roze\"));

    let combined = a + b + \"baz\";
    println(combined);
}
";
    let (stdout, stderr, ok) = run_native_source("string_ops", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "true\nfalse\ntrue\nHello, Roze!\nfoofoobaz"
    );
}

#[test]
fn no_leak_with_heap_allocated_for_loop_init_variable_and_early_return() {
    // Specifically targets a subtle case: a for-loop's *own* init
    // variable (`for let label = ...; ...`) can itself be a string --
    // nothing in the grammar restricts a for-loop's init to `int`, even
    // though that's the overwhelmingly common case. If that string is
    // heap-allocated (not an immortal literal) and the loop is exited
    // early via `return` from inside its body, the for-loop's own
    // scope-exit cleanup (which only runs on the *normal* loop-
    // completion path) must not be the only thing responsible for
    // releasing it -- `return`'s release-every-active-scope must cover
    // it too, or it leaks; and it must not *also* run on top of that
    // and double-release it.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_heap_allocated_for_loop_init_variable_and_early_return: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_for_loop_init_leak_check");
    let source = "\
func early_exit(n: int) -> string {
    for let label = \"loop\" + \" start\"; n < 100; n = n + 1 {
        if n > 5 {
            return \"escaped early\";
        }
    }
    return \"never found\";
}

func main() {
    println(early_exit(0));
}
";
    std::fs::write(dir.join("loop_init_leak.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("loop_init_leak.roze")
        .arg("--target")
        .arg("native")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let valgrind_output = Command::new("valgrind")
        .arg("--leak-check=full")
        .arg("--error-exitcode=1")
        .arg("./loop_init_leak")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_with_early_returns_nested_several_scopes_deep() {
    // Each level (function top scope, outer if, inner if) holds its own
    // live string local at the moment of return -- release_all_active_
    // scopes must walk *all* of them, not just the innermost.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_early_returns_nested_several_scopes_deep: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_nested_return_leak_check");
    let source = "\
func classify(x: int) -> string {
    let outer = \"outer value\";
    if x > 0 {
        let inner = \"inner value\";
        if x > 10 {
            let deepest = \"deepest\" + \" value\";
            return deepest;
        }
        return inner;
    }
    return outer;
}

func main() {
    println(classify(20));
    println(classify(5));
    println(classify(-1));
}
";
    std::fs::write(dir.join("nested_return_leak.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("nested_return_leak.roze")
        .arg("--target")
        .arg("native")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let valgrind_output = Command::new("valgrind")
        .arg("--leak-check=full")
        .arg("--error-exitcode=1")
        .arg("./nested_return_leak")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
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
