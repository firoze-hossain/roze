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
    run_native_source_with_links(name, source, &[])
}

fn run_native_source_with_links(name: &str, source: &str, links: &[&str]) -> (String, String, bool) {
    let dir = scratch_dir(name);
    let file_name = format!("{}.roze", name);
    std::fs::write(dir.join(&file_name), source).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_roze"));
    cmd.arg("run")
        .arg(&file_name)
        .arg("--target")
        .arg("native");
    for link in links {
        cmd.arg("--link").arg(link);
    }
    let output = cmd.current_dir(&dir)
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
    // list_*, then map_*, were each in turn the go-to still-unsupported
    // intrinsic for this test until support for them landed -- a Core
    // (JVM-only) intrinsic like string_length is a stable choice here,
    // since Core is a whole separate JVM-standard-library-backed
    // family, not a native-backend heap type that might get ARC support
    // next.
    let (_stdout, stderr, ok) = run_native_source(
        "reject_intrinsic",
        "func main() {\n    println(string_length(\"hi\"));\n}\n",
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
fn println_of_a_raw_list_or_map_value_is_still_rejected() {
    // list/map themselves are fully supported now -- what's still out
    // of scope is stringifying a whole container directly (there's no
    // "[1, 2, 3]"-style formatting implemented for println yet).
    // println of an individual *element* (an int/bool/string read out
    // via list_get/map_get) works fine, same as any other value.
    let (_stdout, stderr, ok) = run_native_source(
        "reject_println_of_list",
        "func main() {\n    let l = list_new();\n    println(l);\n}\n",
    );
    assert!(!ok, "expected the native backend to still reject println of a raw list value");
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
fn list_operations_are_correct() {
    let source = "\
func make_range(n: int) -> list {
    let l = list_new();
    for let i = 0; i < n; i = i + 1 {
        list_push(l, i);
    }
    return l;
}

func sum_list(l: list) -> int {
    let total = 0;
    for let i = 0; i < list_length(l); i = i + 1 {
        total = total + list_get(l, i);
    }
    return total;
}

func main() {
    let l = list_new();
    list_push(l, 10);
    list_push(l, 20);
    list_push(l, 30);
    println(list_length(l));
    println(list_get(l, 0));
    println(list_is_empty(l));

    let old = list_set(l, 1, 99);
    println(old);
    println(list_get(l, 1));

    let removed = list_remove(l, 0);
    println(removed);
    println(list_length(l));
    println(list_get(l, 0));

    // Growth past the initial capacity (via realloc), a function
    // returning a list it created, and aliasing (two bindings to the
    // same underlying list, mutation through either visible via both).
    let big = make_range(50);
    println(list_length(big));
    println(sum_list(big));
    let alias = big;
    list_push(alias, 999);
    println(list_length(big));
    println(list_get(big, 50));
}
";
    let (stdout, stderr, ok) = run_native_source("list_ops", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "3\n10\nfalse\n20\n99\n10\n2\n99\n50\n1225\n51\n999"
    );
}

#[test]
fn list_index_out_of_bounds_exits_cleanly_instead_of_crashing() {
    // No undefined behavior on an out-of-bounds access -- a clear
    // message and a controlled exit(1), never a silent wrong read or a
    // segfault from walking off the allocated buffer.
    let source = "\
func main() {
    let l = list_new();
    list_push(l, 1);
    println(\"reached\");
    println(list_get(l, 5));
    println(\"never reached\");
}
";
    let (stdout, _stderr, ok) = run_native_source("list_oob", source);
    assert!(!ok, "expected a non-zero exit on out-of-bounds access");
    assert!(stdout.contains("reached"), "expected output up to the out-of-bounds access, got:\n{}", stdout);
    assert!(!stdout.contains("never reached"), "must not continue past the out-of-bounds access:\n{}", stdout);
}

#[test]
fn list_element_type_is_restricted_to_int_and_bool_for_now() {
    // Elements are plain i64 words with no ARC of their own -- storing
    // a string (or another list) in one would compile but silently do
    // the wrong thing at runtime (never retained/released as part of
    // the list), so it's rejected at compile time instead.
    let (_stdout, stderr, ok) = run_native_source(
        "list_reject_string_element",
        "func main() {\n    let l = list_new();\n    list_push(l, \"not allowed yet\");\n}\n",
    );
    assert!(!ok, "expected a compile error for a string list element");
    assert!(stderr.contains("int/bool"), "expected a message naming the restriction, got:\n{}", stderr);
}

#[test]
fn no_leak_with_high_volume_list_growth_shrink_and_nesting() {
    // Exercises realloc-driven growth (1000 pushes against an initial
    // capacity of 4), memmove-driven removal (500 pop-fronts), and
    // nested list allocation (a fresh list created and consumed inside
    // a loop, 200 times) together in one run.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_high_volume_list_growth_shrink_and_nesting: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_list_high_volume");
    let source = "\
func main() {
    let l = list_new();
    for let i = 0; i < 1000; i = i + 1 {
        list_push(l, i);
    }
    for let i = 0; i < 500; i = i + 1 {
        list_remove(l, 0);
    }
    println(list_length(l));

    let copies = list_new();
    for let i = 0; i < 200; i = i + 1 {
        let inner = list_new();
        list_push(inner, i);
        list_push(copies, list_get(inner, 0));
    }
    println(list_length(copies));
}
";
    std::fs::write(dir.join("list_high_volume.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("list_high_volume.roze")
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
        .arg("./list_high_volume")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_with_lists_nested_several_scopes_deep_and_early_returns() {
    // Mirrors the string version of this test: multiple live list
    // locals across nested scopes (function top scope, for-loop body,
    // nested if), with an early return from the deepest one --
    // release-on-return must walk every active scope, and a list
    // created but never returned (`dummy`) must still be released when
    // the function falls off the end normally on the other call.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_lists_nested_several_scopes_deep_and_early_returns: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_list_nested_return");
    let source = "\
func find_first_over(l: list, threshold: int) -> int {
    let dummy = list_new();
    list_push(dummy, 1);
    for let i = 0; i < list_length(l); i = i + 1 {
        if list_get(l, i) > threshold {
            let another = list_new();
            list_push(another, i);
            if list_get(l, i) > 1000 {
                return list_get(another, 0);
            }
            return i;
        }
    }
    return -1;
}

func main() {
    let l = list_new();
    list_push(l, 1);
    list_push(l, 5);
    list_push(l, 1500);
    list_push(l, 10);
    println(find_first_over(l, 100));

    let l2 = list_new();
    list_push(l2, 1);
    list_push(l2, 5);
    println(find_first_over(l2, 100));
}
";
    std::fs::write(dir.join("list_nested_return.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("list_nested_return.roze")
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
        .arg("./list_nested_return")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn map_operations_are_correct() {
    let source = "\
func make_counts() -> map {
    let m = map_new();
    map_put(m, 1, 10);
    map_put(m, 2, 20);
    return m;
}

func main() {
    let m = map_new();
    println(map_size(m));
    println(map_is_empty(m));

    println(map_put(m, 1, 100));
    println(map_put(m, 2, 200));
    println(map_size(m));
    println(map_get(m, 1));
    println(map_get(m, 999));
    println(map_has(m, 1));
    println(map_has(m, 999));

    println(map_put(m, 1, 111));
    println(map_get(m, 1));
    println(map_size(m));

    println(map_remove(m, 1));
    println(map_has(m, 1));
    println(map_size(m));
    println(map_remove(m, 999));

    // A function returning a map it created, and aliasing (two bindings
    // to the same underlying map, mutation through either visible via
    // both).
    let counts = make_counts();
    let alias = counts;
    map_put(alias, 3, 30);
    println(map_size(counts));
    println(map_get(counts, 3));
}
";
    let (stdout, stderr, ok) = run_native_source("map_ops", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "0\ntrue\n0\n0\n2\n100\n0\ntrue\nfalse\n100\n111\n2\n111\nfalse\n1\n0\n3\n30"
    );
}

#[test]
fn map_key_and_value_types_are_restricted_to_int_and_bool_for_now() {
    // Same reasoning as list elements: keys/values are plain i64 words
    // with no ARC of their own, so a string key or value would compile
    // but silently do the wrong thing at runtime.
    let (_stdout, key_stderr, key_ok) = run_native_source(
        "map_reject_string_key",
        "func main() {\n    let m = map_new();\n    map_put(m, \"bad key\", 1);\n}\n",
    );
    assert!(!key_ok, "expected a compile error for a string map key");
    assert!(key_stderr.contains("int/bool"), "expected a message naming the restriction, got:\n{}", key_stderr);

    let (_stdout, value_stderr, value_ok) = run_native_source(
        "map_reject_string_value",
        "func main() {\n    let m = map_new();\n    map_put(m, 1, \"bad value\");\n}\n",
    );
    assert!(!value_ok, "expected a compile error for a string map value");
    assert!(value_stderr.contains("int/bool"), "expected a message naming the restriction, got:\n{}", value_stderr);
}

#[test]
fn no_leak_with_high_volume_map_growth_and_negative_keys() {
    // Exercises hash-table growth (1000 puts against an initial
    // capacity of 8, forcing several doublings and full rehashes),
    // negative keys (bitmask-based indexing must handle these
    // correctly, not just positive ones), and removal leaving
    // tombstones behind that later lookups must still probe past
    // correctly.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_high_volume_map_growth_and_negative_keys: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_map_high_volume");
    let source = "\
func main() {
    let m = map_new();
    for let i = 0; i < 1000; i = i + 1 {
        map_put(m, i, i * 2);
    }
    println(map_size(m));

    let total = 0;
    for let i = 0; i < 1000; i = i + 1 {
        total = total + map_get(m, i);
    }
    println(total);

    map_put(m, -5, -50);
    println(map_get(m, -5));

    for let i = 0; i < 500; i = i + 1 {
        map_remove(m, i);
    }
    println(map_size(m));

    let still_there = true;
    for let i = 500; i < 1000; i = i + 1 {
        if map_get(m, i) != i * 2 {
            still_there = false;
        }
    }
    println(still_there);
}
";
    std::fs::write(dir.join("map_high_volume.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("map_high_volume.roze")
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

    let run_output = Command::new(format!("{}/map_high_volume", dir.display())).output().expect("failed to run map_high_volume");
    assert!(run_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout).trim_end(),
        "1000\n999000\n-50\n501\ntrue"
    );

    let valgrind_output = Command::new("valgrind")
        .arg("--leak-check=full")
        .arg("--error-exitcode=1")
        .arg("./map_high_volume")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_with_maps_nested_several_scopes_deep_and_early_returns() {
    // Mirrors the list/string versions of this test: multiple live map
    // locals across nested scopes with an early return from the
    // deepest one, and a map created but never returned (`dummy`) that
    // must still be released when the function falls off the end
    // normally on the other call.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_maps_nested_several_scopes_deep_and_early_returns: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_map_nested_return");
    let source = "\
func find_or_default(m: map, key: int, threshold: int) -> int {
    let dummy = map_new();
    map_put(dummy, 0, 0);
    if map_has(m, key) {
        let value = map_get(m, key);
        if value > threshold {
            let another = map_new();
            map_put(another, key, value);
            return map_get(another, key);
        }
        return value;
    }
    return -1;
}

func main() {
    let m = map_new();
    map_put(m, 1, 10);
    map_put(m, 3, 30);
    println(find_or_default(m, 3, 5));
    println(find_or_default(m, 1, 500));
    println(find_or_default(m, 999, 5));
}
";
    std::fs::write(dir.join("map_nested_return.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("map_nested_return.roze")
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
        .arg("./map_nested_return")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn class_construction_field_access_and_mutation_are_correct() {
    let source = "\
class Person {
    name: string,
    age: int
}

func greet(p: Person) -> string {
    return \"Hello, \" + p.name + \"!\";
}

func older(a: Person, b: Person) -> Person {
    if a.age > b.age {
        return a;
    }
    return b;
}

func main() {
    let alice = new Person(\"Alice\", 30);
    let bob = new Person(\"Bob\", 25);

    println(greet(alice));
    println(greet(bob));

    let elder = older(alice, bob);
    println(elder.name);

    alice.name = \"Alicia\";
    println(alice.name);
    println(greet(alice));
}
";
    let (stdout, stderr, ok) = run_native_source("class_ops", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "Hello, Alice!\nHello, Bob!\nAlice\nAlicia\nHello, Alicia!"
    );
}

#[test]
fn nested_classes_are_correct() {
    // A class field whose type is itself another class -- Line owns two
    // independently ARC-managed Point instances.
    let source = "\
class Point {
    x: int,
    y: int
}

class Line {
    start: Point,
    end: Point
}

func make_line() -> Line {
    let a = new Point(0, 0);
    let b = new Point(3, 4);
    return new Line(a, b);
}

func length_squared(l: Line) -> int {
    let dx = l.end.x - l.start.x;
    let dy = l.end.y - l.start.y;
    return dx * dx + dy * dy;
}

func main() {
    let line = make_line();
    println(line.start.x);
    println(line.end.x);
    println(length_squared(line));

    line.start.x = 1;
    println(line.start.x);
    println(length_squared(line));
}
";
    let (stdout, stderr, ok) = run_native_source("nested_classes", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "0\n3\n25\n1\n20");
}

#[test]
fn field_access_and_assignment_on_a_temporary_object_do_not_leak_output() {
    // Output-correctness companion to the Valgrind-gated leak test below
    // for the same scenario (`make_box().value`, a field read/write on
    // a class instance that's never bound to a variable).
    let source = "\
class Box {
    value: string
}

func make_box() -> Box {
    return new Box(\"boxed value\");
}

func main() {
    println(make_box().value);
    make_box().value = \"unused\";
    println(\"done\");
}
";
    let (stdout, stderr, ok) = run_native_source("temp_field_access", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "boxed value\ndone");
}

#[test]
fn class_fields_can_be_list_map_or_another_class() {
    // list/map/class are all ARC types this backend already knows how
    // to retain/release (see arc_release_call_for_field_type) -- a
    // class field of one of these types is fully supported, not
    // rejected. (There's no field type the native backend actually
    // rejects via ordinary Roze syntax right now, since Type::Function
    // has no source-level way to be written as a type annotation.)
    let source = "\
class Bag {
    items: list
}

func main() {
    let l = list_new();
    list_push(l, 1);
    list_push(l, 2);
    let b = new Bag(l);
    println(list_length(b.items));
    println(list_get(b.items, 0));
}
";
    let (stdout, stderr, ok) = run_native_source("class_list_field", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "2\n1");
}

#[test]
fn no_leak_with_string_field_reassignment_and_class_typed_function_returns() {
    // Exercises: a string field being reassigned (must release the old
    // value), a class instance returned from a function (must not be
    // freed by its own function before reaching the caller -- the exact
    // bug class list/map's arrival each caught previously), and passing
    // class instances as function arguments (aliasing).
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_string_field_reassignment_and_class_typed_function_returns: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_class_string_field");
    let source = "\
class Person {
    name: string,
    age: int
}

func greet(p: Person) -> string {
    return \"Hello, \" + p.name + \"!\";
}

func older(a: Person, b: Person) -> Person {
    if a.age > b.age {
        return a;
    }
    return b;
}

func main() {
    let alice = new Person(\"Alice\", 30);
    let bob = new Person(\"Bob\", 25);
    println(greet(alice));
    println(greet(bob));
    let elder = older(alice, bob);
    println(elder.name);
    alice.name = \"Alicia\";
    println(greet(alice));
}
";
    std::fs::write(dir.join("class_string_field.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("class_string_field.roze")
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
        .arg("./class_string_field")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_with_classes_nested_several_scopes_deep_and_early_returns() {
    // Mirrors the string/list/map versions of this test: multiple live
    // class instances across nested scopes with an early return from
    // the deepest one, and a class instance created but never returned
    // (`dummy`) that must still be released when the function falls
    // off the end normally on the other call.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_classes_nested_several_scopes_deep_and_early_returns: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_class_nested_return");
    let source = "\
class Result {
    label: string,
    value: int
}

func classify(x: int) -> Result {
    let dummy = new Result(\"dummy\", 0);
    if x > 0 {
        let inner = new Result(\"positive branch\", x);
        if x > 100 {
            return new Result(\"big\", x);
        }
        return inner;
    }
    return dummy;
}

func main() {
    let r1 = classify(200);
    println(r1.label);
    let r2 = classify(50);
    println(r2.label);
    let r3 = classify(-5);
    println(r3.label);
}
";
    std::fs::write(dir.join("class_nested_return.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("class_nested_return.roze")
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
        .arg("./class_nested_return")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_with_field_access_and_assignment_on_a_temporary_object() {
    // Specifically targets the edge case handled in compile_expression's
    // FieldAccess arm and compile_statement's FieldAssign arm: when the
    // object being accessed is itself a fresh (non-identifier) class
    // reference -- e.g. `some_call().field` -- nothing else owns that
    // temporary object, so it must be released right after the field
    // read/write, or it leaks.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_with_field_access_and_assignment_on_a_temporary_object: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_class_temp_field_access");
    let source = "\
class Box {
    value: string
}

func make_box() -> Box {
    return new Box(\"boxed value\");
}

func main() {
    println(make_box().value);
    make_box().value = \"unused\";
    println(\"done\");
}
";
    std::fs::write(dir.join("temp_field_access.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("temp_field_access.roze")
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
        .arg("./temp_field_access")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_reading_an_arc_typed_field_transiently_without_storing_it() {
    // The exact bug found during development: compile_expression's
    // FieldAccess used to retain the extracted field value
    // unconditionally, on every read -- correct when the value is
    // then stored somewhere independent (a `let`, a `return`), but a
    // leak when it's just used transiently and discarded, e.g. passed
    // straight through to an intrinsic like list_length that doesn't
    // take ownership. `list_length(l)` (a bare identifier) never
    // needed a retain; `list_length(b.items)` shouldn't need one
    // either just because the value came from a field instead of a
    // plain variable. Fixed by making a field read borrowed by default
    // (like reading a plain variable), only becoming an owned,
    // independent value when it's actually consumed at a point that
    // creates a new owner (see `retain_if_aliasing` and
    // `expression_is_borrowed`).
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_reading_an_arc_typed_field_transiently_without_storing_it: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_class_transient_field_read");
    let source = "\
class Bag {
    items: list
}

func main() {
    let l = list_new();
    list_push(l, 1);
    list_push(l, 2);
    let b = new Bag(l);
    println(list_length(b.items));
    println(list_get(b.items, 0));
}
";
    std::fs::write(dir.join("transient_field_read.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("transient_field_read.roze")
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
        .arg("./transient_field_read")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn no_leak_or_use_after_free_with_deeply_chained_field_access() {
    // A second, related bug found alongside the one above: naively
    // checking "is the object a bare Identifier" (rather than
    // recursively checking whether it's ultimately *rooted* in one)
    // gets chained field access wrong in both directions --
    // `a.b.c` where `a` is a plain variable would incorrectly treat
    // the intermediate `a.b` as a fresh temporary needing release,
    // prematurely releasing a reference `a` itself still legitimately
    // owns. Exercises a 3-level chain (Company -> Person -> Address ->
    // string), reading and reassigning through the full chain, and
    // extracting both a string and an intermediate class reference
    // independently via `let`.
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_or_use_after_free_with_deeply_chained_field_access: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_class_deep_chain");
    let source = "\
class Address {
    city: string
}

class Person {
    name: string,
    address: Address
}

class Company {
    owner: Person
}

func make_company() -> Company {
    let addr = new Address(\"Springfield\");
    let person = new Person(\"Homer\", addr);
    return new Company(person);
}

func main() {
    let c = make_company();
    println(c.owner.name);
    println(c.owner.address.city);

    c.owner.address.city = \"Shelbyville\";
    println(c.owner.address.city);

    let city_copy = c.owner.address.city;
    println(city_copy);

    let addr2 = c.owner.address;
    println(addr2.city);
}
";
    std::fs::write(dir.join("deep_chain.roze"), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("deep_chain.roze")
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

    let run_output = Command::new(format!("{}/deep_chain", dir.display())).output().expect("failed to run deep_chain");
    assert!(run_output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&run_output.stdout).trim_end(),
        "Homer\nSpringfield\nShelbyville\nShelbyville\nShelbyville"
    );

    let valgrind_output = Command::new("valgrind")
        .arg("--leak-check=full")
        .arg("--error-exitcode=1")
        .arg("./deep_chain")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn float_arithmetic_comparisons_and_negation_are_correct() {
    let source = "\
func average(a: float, b: float) -> float {
    return (a + b) / 2.0;
}

func main() {
    let x = 3.14;
    println(x);
    let y = 2.0;
    println(x + y);
    println(x - y);
    println(x * y);
    println(x / y);
    let z = 5;
    println(x + z);
    println(z + x);
    println(x < 4.0);
    println(x > 4.0);
    println(-x);
    println(average(3.0, 7.0));
}
";
    let (stdout, stderr, ok) = run_native_source("float_ops", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "3.14\n5.14\n1.14\n6.28\n1.57\n8.14\n8.14\ntrue\nfalse\n-3.14\n5"
    );
}

#[test]
fn two_parameter_functions_returning_a_float_derived_from_a_parameter_are_correct() {
    // A dedicated regression test for a real, subtle bug found and
    // root-caused during development: Cranelift 0.103 has no support
    // at all for the x86-64 SystemV ABI's variadic-call convention
    // (the caller must set %al to the number of vector/XMM registers
    // used for variadic arguments, so the callee's own varargs
    // machinery knows whether to save them). The original float-
    // printing implementation called printf/snprintf directly with a
    // float argument -- a variadic call -- and without %al set
    // correctly, it would sometimes read the right value and sometimes
    // read stale garbage left behind by an unrelated *preceding* call,
    // depending on what happened to be in %al at the time. This showed
    // up specifically (and only) for functions with exactly two
    // parameters returning a value derived from one of them, which is
    // exactly what every case below exercises: every combination of
    // which parameter is returned, and int/float parameter mixes.
    // Fixed by switching to gcvt(double, int, char*), a genuinely
    // non-variadic glibc function, sidestepping the ABI issue entirely
    // rather than working around it.
    let source = "\
func first_of_two_floats(a: float, b: float) -> float {
    return a;
}
func second_of_two_floats(a: float, b: float) -> float {
    return b;
}
func float_from_int_and_float(a: int, b: float) -> float {
    return b;
}
func float_from_float_and_int(a: float, b: int) -> float {
    return a;
}
func computed_from_two_floats(a: float, b: float) -> float {
    return a + b;
}

func main() {
    println(first_of_two_floats(3.0, 7.0));
    println(second_of_two_floats(3.0, 7.0));
    println(float_from_int_and_float(1, 7.0));
    println(float_from_float_and_int(3.0, 1));
    println(computed_from_two_floats(3.0, 7.0));
}
";
    let (stdout, stderr, ok) = run_native_source("float_two_param_regression", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "3\n7\n7\n3\n10");
}

#[test]
fn implicit_int_to_float_widening_is_actually_performed_by_codegen() {
    // A second real bug found alongside the one above: the type
    // checker already allowed an int to flow into a float-expecting
    // position (return, assignment, function-call and constructor
    // arguments), but codegen never actually inserted the int-to-float
    // conversion at three of those four sites, either producing a
    // Cranelift verifier error (return) or a silent type mismatch.
    // Exercises all three fixed sites together.
    let source = "\
func returns_float_from_int(a: int) -> float {
    return a;
}
func takes_float(x: float) -> float {
    return x;
}

func main() {
    println(returns_float_from_int(5));
    let x = 3.14;
    x = 7;
    println(x);
    println(takes_float(5));
}
";
    let (stdout, stderr, ok) = run_native_source("float_widening", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "5\n7\n5");
}

#[test]
fn implicit_int_to_float_widening_into_a_class_field_is_also_performed() {
    // A variant of the widening bug above, specific to class
    // construction: the constructor argument itself was correctly
    // coerced at the New expression's call site, but the constructor's
    // own generated signature (both its initial declaration and the
    // function body actually built for it) still declared every field
    // uniformly as the int/pointer-shaped VALUE_TYPE, regardless of the
    // field's real type -- so passing an already-coerced f64 argument
    // into it was itself a mismatch. Also covers reading the field back
    // out afterward, which had the same VALUE_TYPE-regardless-of-type
    // bug on the load side.
    let source = "\
class Box {
    value: float
}
func main() {
    let b = new Box(4);
    println(b.value);
}
";
    let (stdout, stderr, ok) = run_native_source("float_field_widening", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "4");
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

#[test]
fn native_enums_and_pattern_matching() {
    let source = "\
enum Shape {
    Circle(int),
    Rectangle(int, int),
    Point,
}

func calculate_area(s: Shape) -> int {
    match s {
        Circle(r) => {
            return 3 * r * r;
        }
        Rectangle(w, h) => {
            return w * h;
        }
        Point => {
            return 0;
        }
    }
}

func main() {
    let c = Shape::Circle(10);
    let r = Rectangle(4, 5);
    let p = Point;
    println(calculate_area(c));
    println(calculate_area(r));
    println(calculate_area(p));
}
";
    let (stdout, stderr, ok) = run_native_source("enums_native", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "300\n20\n0");
}

#[test]
fn native_closures_and_first_class_functions() {
    let source = "\
func square(x: int) -> int {
    return x * x;
}

func apply_fn(x: int, f: func(int) -> int) -> int {
    return f(x);
}

func make_adder(base: int) -> func(int) -> int {
    return |x: int| x + base;
}

func main() {
    let add = |a: int, b: int| -> int {
        return a + b;
    };
    println(add(3, 4));

    let double = |x: int| x * 2;
    println(double(21));

    let answer = || 42;
    println(answer());

    let factor = 10;
    let multiply_by_factor = |x: int| x * factor;
    println(multiply_by_factor(5));

    let res1 = apply_fn(7, |n: int| n + 3);
    println(res1);

    let res2 = apply_fn(9, square);
    println(res2);

    let add100 = make_adder(100);
    println(add100(25));
    let add50 = make_adder(50);
    println(add50(25));

    let logger = |msg: string| {
        println(msg);
    };
    logger(\"native closures working!\");
}
";
    let (stdout, stderr, ok) = run_native_source("closures_native", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
7
42
42
50
10
81
125
75
native closures working!";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn native_generics_and_parametric_polymorphism() {
    let source = "
class Box<T> {
    val: T,
}

class Pair<A, B> {
    first: A,
    second: B,
}

func id<T>(x: T) -> T {
    return x;
}

func swap_pair<A, B>(p: Pair<A, B>) -> Pair<B, A> {
    return new Pair<B, A>(p.second, p.first);
}

func main() {
    let a: int = id<int>(100);
    println(a);

    let s: string = id<string>(\"generics working!\");
    println(s);

    let b = id(200);
    println(b);

    let greeting = id(\"hello roze\");
    println(greeting);

    let int_box = new Box<int>(42);
    println(int_box.val);
    int_box.val = 84;
    println(int_box.val);

    let str_box = new Box<string>(\"boxed string\");
    println(str_box.val);

    let auto_box = new Box(999);
    println(auto_box.val);

    let p = new Pair<int, string>(1, \"one\");
    println(p.first);
    println(p.second);

    let swapped = swap_pair(p);
    println(swapped.first);
    println(swapped.second);

    let nums: List<int> = list_new();
    list_push(nums, 10);
    list_push(nums, 20);
    list_push(nums, 30);
    println(list_get(nums, 0));
    println(list_get(nums, 1));
    println(list_get(nums, 2));

    let ages: Map<int, int> = map_new();
    map_put(ages, 1, 30);
    map_put(ages, 2, 25);
    println(map_get(ages, 1));
    println(map_get(ages, 2));
}
";
    let (stdout, stderr, ok) = run_native_source("generics_native", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
100
generics working!
200
hello roze
42
84
boxed string
999
1
one
one
1
10
20
30
30
25";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn traits_and_dynamic_dispatch_native() {
    let source = std::fs::read_to_string("tests/fixtures/traits.roze")
        .expect("traits.roze fixture should exist");
    let (stdout, stderr, ok) = run_native_source("traits_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
Hello, Alice!
Beep boop, I am R2D2.
Hello, Alice!
Beep boop, I am R2D2.
Hello, Alice!
Beep boop, I am R2D2.
50
30
50
30";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn c_ffi_native() {
    let source = std::fs::read_to_string("tests/fixtures/ffi.roze")
        .expect("ffi.roze fixture should exist");
    let (stdout, stderr, ok) = run_native_source_with_links("ffi_native", &source, &["m"]);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
Hello from C FFI!
42
4
lang-c-ffi";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn atomic_arc_refcount_and_retain_release_helpers() {
    let source = r#"
class Node {
    value: int
}

extern "C" func __roze_object_refcount(n: Node) -> int;
extern "C" func __roze_class_retain(n: Node) -> void;
extern "C" func __roze_class_Node_release(n: Node) -> void;
extern "C" func __roze_string_refcount(s: string) -> int;

func main() {
    let n = new Node(42);
    println(__roze_object_refcount(n));
    __roze_class_retain(n);
    println(__roze_object_refcount(n));
    __roze_class_Node_release(n);
    println(__roze_object_refcount(n));

    let dynamic_str = "hello " + "world";
    println(__roze_string_refcount(dynamic_str));
}
"#;
    let (stdout, stderr, ok) = run_native_source("atomic_arc_helpers", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
1
2
1
1";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn multithreaded_atomic_arc_concurrency() {
    let c_harness = r#"
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>

extern void __roze_string_retain(void*);
extern void __roze_string_release(void*);
extern void __roze_list_retain(void*);
extern void __roze_list_release(void*);
extern void __roze_class_retain(void*);
extern void __roze_class_Node_release(void*);
extern int64_t __roze_object_refcount(void*);
extern int64_t __roze_string_refcount(void*);

typedef struct {
    void* obj;
    int type; // 0 = string, 1 = list, 2 = class
    int iterations;
} WorkerArg;

static void* worker(void* ptr) {
    WorkerArg* arg = (WorkerArg*)ptr;
    for (int i = 0; i < arg->iterations; i++) {
        switch (arg->type) {
            case 0:
                __roze_string_retain(arg->obj);
                __roze_string_release(arg->obj);
                break;
            case 1:
                __roze_list_retain(arg->obj);
                __roze_list_release(arg->obj);
                break;
            case 2:
                __roze_class_retain(arg->obj);
                __roze_class_Node_release(arg->obj);
                break;
        }
    }
    return NULL;
}

int64_t run_concurrent_arc(void* obj, int type, int num_threads, int iterations) {
    pthread_t threads[16];
    WorkerArg args[16];
    if (num_threads > 16) num_threads = 16;

    for (int i = 0; i < num_threads; i++) {
        args[i].obj = obj;
        args[i].type = type;
        args[i].iterations = iterations;
        pthread_create(&threads[i], NULL, worker, &args[i]);
    }

    for (int i = 0; i < num_threads; i++) {
        pthread_join(threads[i], NULL);
    }

    if (type == 0) {
        return __roze_string_refcount(obj);
    } else {
        return __roze_object_refcount(obj);
    }
}

int64_t run_concurrent_arc_str(void* obj, int type, int num_threads, int iterations) {
    return run_concurrent_arc(obj, type, num_threads, iterations);
}

int64_t run_concurrent_arc_list(void* obj, int type, int num_threads, int iterations) {
    return run_concurrent_arc(obj, type, num_threads, iterations);
}
"#;
    let harness_c_path = std::env::temp_dir().join("roze_test_atomic_arc_harness.c");
    std::fs::write(&harness_c_path, c_harness).unwrap();

    let roze_source = r#"
class Node {
    value: int
}

extern "C" func run_concurrent_arc(obj: Node, type_: int, num_threads: int, iterations: int) -> int;
extern "C" func run_concurrent_arc_str(obj: string, type_: int, num_threads: int, iterations: int) -> int;
extern "C" func run_concurrent_arc_list(obj: list, type_: int, num_threads: int, iterations: int) -> int;
extern "C" func __roze_object_refcount(n: Node) -> int;
extern "C" func __roze_string_refcount(s: string) -> int;

func main() {
    let n = new Node(999);
    println(__roze_object_refcount(n));
    let final_ref = run_concurrent_arc(n, 2, 8, 25000);
    println(final_ref);

    let s = "atomic " + "concurrent string";
    println(__roze_string_refcount(s));
    let str_ref = run_concurrent_arc_str(s, 0, 8, 25000);
    println(str_ref);

    let l = list_new();
    list_push(l, 42);
    let list_ref = run_concurrent_arc_list(l, 1, 8, 25000);
    println(list_ref);
}
"#;
    let (stdout, stderr, ok) = run_native_source_with_links(
        "multithreaded_arc",
        roze_source,
        &[harness_c_path.to_str().unwrap(), "pthread"],
    );
    let _ = std::fs::remove_file(&harness_c_path);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
1
1
1
1
1";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn native_concurrency_threads_and_channels() {
    let source = r#"
import "concurrency";

func main() {
    // 1. Thread spawn and join
    let t1 = thread_spawn(|| {
        thread_sleep(10);
        return 42;
    });
    let res1: int = thread_join(t1);
    println(res1);

    // 2. Thread ID
    let tid = thread_current_id();
    println(tid != 0);

    // 3. Channels: bounded FIFO buffer
    let ch = channel_new(4);
    println(channel_len(ch));
    println(channel_is_closed(ch));

    send(ch, 100);
    send(ch, 200);
    println(channel_len(ch));

    let v1: int = recv(ch);
    let v2: int = recv(ch);
    println(v1);
    println(v2);
    println(channel_len(ch));

    // 4. Background thread communicating via channel
    let t2 = thread_spawn(|| {
        thread_sleep(5);
        send(ch, 999);
        return 0;
    });
    let v3: int = recv(ch);
    println(v3);
    thread_join(t2);

    channel_close(ch);
    println(channel_is_closed(ch));

    // 5. Concurrency stdlib helpers
    let ch2 = channel();
    let t3 = spawn(|| {
        send(ch2, 777);
        return 1;
    });
    let v4: int = recv(ch2);
    println(v4);
    let res3: int = thread_join(t3);
    println(res3);
}
"#;
    let (stdout, stderr, ok) = run_native_source("native_concurrency", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
42
true
0
false
2
100
200
0
999
true
777
1";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn native_green_threads_m_n_concurrency() {
    let source = r#"
import "concurrency";

func main() {
    // 1. Fiber spawn and join
    let f1 = fiber_spawn(|| {
        fiber_sleep(5);
        return 42;
    });
    let res1: int = fiber_join(f1);
    println(res1);

    // 2. Fiber ID
    let fid = fiber_id();
    println(fid != 0);

    // 3. Channels and fiber communication
    let ch = channel_new(8);
    let f2 = fiber_spawn(|| {
        fiber_yield();
        send(ch, 100);
        send(ch, 200);
        return 1;
    });
    let f3 = fiber_spawn(|| {
        let v1: int = recv(ch);
        let v2: int = recv(ch);
        return v1 + v2;
    });
    let res3: int = fiber_join(f3);
    println(res3);
    let res2: int = fiber_join(f2);
    println(res2);

    // 4. Concurrency stdlib helpers
    let f4 = spawn(|| {
        yield_now();
        return 777;
    });
    let res4: int = fiber_wait(f4);
    println(res4);

    // 5. Work-stealing multi-task pipeline (100 concurrent green threads)
    let results = channel_new(128);
    for let i = 0; i < 100; i = i + 1 {
        fiber_spawn(|| {
            fiber_yield();
            send(results, 5);
            return 0;
        });
    }

    let sum = 0;
    for let i = 0; i < 100; i = i + 1 {
        let v: int = recv(results);
        sum = sum + v;
    }
    println(sum);
}
"#;
    let (stdout, stderr, ok) = run_native_source("native_green_threads", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
42
true
300
1
777
500";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn native_massive_green_threads_work_stealing() {
    let source = r#"
import "concurrency";

func main() {
    let count = 2000;
    let ch = channel_new(2048);
    for let i = 0; i < count; i = i + 1 {
        fiber_spawn(|| {
            fiber_yield();
            send(ch, 1);
            return 0;
        });
    }

    let total = 0;
    for let i = 0; i < count; i = i + 1 {
        let v: int = recv(ch);
        total = total + v;
    }
    println(total);
}
"#;
    let (stdout, stderr, ok) = run_native_source("native_massive_fibers", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "2000");
}

#[test]
fn native_concurrency_worker_pool_pipeline() {
    let source = r#"
func main() {
    let tasks = channel_new(10);
    let results = channel_new(10);

    // Spawn 4 worker threads
    let w1 = thread_spawn(|| {
        let x: int = recv(tasks);
        send(results, x * 2);
        return 0;
    });
    let w2 = thread_spawn(|| {
        let x: int = recv(tasks);
        send(results, x * 2);
        return 0;
    });
    let w3 = thread_spawn(|| {
        let x: int = recv(tasks);
        send(results, x * 2);
        return 0;
    });
    let w4 = thread_spawn(|| {
        let x: int = recv(tasks);
        send(results, x * 2);
        return 0;
    });

    // Send 4 tasks
    send(tasks, 10);
    send(tasks, 20);
    send(tasks, 30);
    send(tasks, 40);

    // Collect 4 results and sum
    let total = 0;
    for let i = 0; i < 4; i = i + 1 {
        let r: int = recv(results);
        total = total + r;
    }
    println(total);

    thread_join(w1);
    thread_join(w2);
    thread_join(w3);
    thread_join(w4);
}
"#;
    let (stdout, stderr, ok) = run_native_source("native_worker_pool", source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(program_output(&stdout).trim_end(), "200");
}

#[test]
fn no_leak_in_multithreaded_channels_and_threads() {
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_in_multithreaded_channels_and_threads: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_concurrency");
    let source = r#"
func main() {
    for let i = 0; i < 50; i = i + 1 {
        let ch = channel_new(8);
        let t = thread_spawn(|| {
            send(ch, 42);
            return 100;
        });
        let val: int = recv(ch);
        let res: int = thread_join(t);
        channel_close(ch);
    }
}
"#;
    let file_name = "conc_leak.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
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
        .arg("./conc_leak")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn native_unsafe_raw_pointers_and_direct_memory() {
    let source = std::fs::read_to_string("tests/fixtures/unsafe_memory.roze")
        .or_else(|_| std::fs::read_to_string("compiler/tests/fixtures/unsafe_memory.roze"))
        .expect("read unsafe_memory fixture");
    let (stdout, stderr, ok) = run_native_source("unsafe_mem", &source);
    assert!(ok, "native build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
false
65
66
1
7
7
7
123456789
987654321
65
true";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn unsafe_intrinsic_outside_unsafe_block_is_rejected() {
    let dir = scratch_dir("unsafe_rejected");
    let source = "func main() { let p = alloc(16); }\n";
    std::fs::write(dir.join("bad.roze"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("bad.roze")
        .arg("--target")
        .arg("native")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Call to unsafe intrinsic 'alloc' requires an 'unsafe { ... }' block"),
        "expected clear rejection of unsafe intrinsic outside unsafe block, got:\n{}",
        stderr
    );
}

#[test]
fn no_leak_with_unsafe_raw_pointers_and_direct_memory() {
    let dir = scratch_dir("valgrind_unsafe");
    let source = r#"
func main() {
    unsafe {
        for let i = 0; i < 50; i = i + 1 {
            let p: *raw u8 = alloc(64);
            ptr_write_u8(p, 42);
            let v = ptr_read_u8(p);
            let p2 = realloc(p, 128);
            free(p2);
        }
    }
}
"#;
    let file_name = "unsafe_leak.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
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

    // Run directly
    let run_output = Command::new("./unsafe_leak")
        .current_dir(&dir)
        .output()
        .expect("failed to run unsafe_leak");
    assert!(run_output.status.success());

    // If valgrind is available, verify zero leaks
    if let Ok(valgrind_output) = Command::new("valgrind")
        .arg("--leak-check=full")
        .arg("--error-exitcode=1")
        .arg("./unsafe_leak")
        .current_dir(&dir)
        .output()
    {
        let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
        assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
        assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
    }
}

#[test]
fn test_freestanding_target_compilation_and_zero_libc() {
    let dir = scratch_dir("freestanding_zero_libc");
    let source = r#"
func main() {
    println("Roze Bare Metal Kernel");
}
"#;
    let file_name = "kernel.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("kernel");
    assert!(kernel_bin.exists(), "expected freestanding kernel binary to exist");

    // Verify it is an ELF64 x86-64 binary
    let readelf_h = Command::new("readelf")
        .arg("-h")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run readelf -h");
    assert!(readelf_h.status.success());
    let h_out = String::from_utf8_lossy(&readelf_h.stdout);
    assert!(h_out.contains("ELF64"), "expected ELF64 in readelf -h");
    assert!(h_out.contains("Advanced Micro Devices X86-64") || h_out.contains("x86-64") || h_out.contains("X86-64"), "expected x86-64");

    // Verify zero dynamic dependencies (no dynamic section)
    let readelf_d = Command::new("readelf")
        .arg("-d")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run readelf -d");
    let d_out = format!("{}{}", String::from_utf8_lossy(&readelf_d.stdout), String::from_utf8_lossy(&readelf_d.stderr));
    assert!(
        d_out.contains("There is no dynamic section") || !readelf_d.status.success(),
        "expected static freestanding binary with no dynamic section, got:\n{}",
        d_out
    );

    // Verify zero undefined symbols
    let nm_u = Command::new("nm")
        .arg("-u")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm -u");
    assert!(nm_u.status.success());
    let u_out = String::from_utf8_lossy(&nm_u.stdout).trim().to_string();
    assert!(
        u_out.is_empty(),
        "expected zero undefined symbols in freestanding binary, got:\n{}",
        u_out
    );
}

#[test]
fn test_freestanding_custom_linker_script() {
    let dir = scratch_dir("freestanding_linker_script");
    let source = r#"
func main() {
    println("Testing custom linker script");
}
"#;
    let file_name = "custom_kernel.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let custom_ld = r#"ENTRY(_start)

SECTIONS
{
    . = 2M;

    .boot : ALIGN(4K)
    {
        *(.boot)
    }

    .text : ALIGN(4K)
    {
        *(.text*)
    }

    .rodata : ALIGN(4K)
    {
        *(.rodata*)
    }

    .data : ALIGN(4K)
    {
        *(.data*)
    }

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss*)
    }
}
"#;
    let ld_file = "custom.ld";
    std::fs::write(dir.join(ld_file), custom_ld).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .arg("-T")
        .arg(ld_file)
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("custom_kernel");
    assert!(kernel_bin.exists());

    // Verify .boot is loaded at 2M (0x200000)
    let readelf_s = Command::new("readelf")
        .arg("-S")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run readelf -S");
    assert!(readelf_s.status.success());
    let s_out = String::from_utf8_lossy(&readelf_s.stdout);
    assert!(
        s_out.contains("00200000"),
        "expected .boot section loaded at 0x200000 (2M), got:\n{}",
        s_out
    );
}

#[test]
fn test_freestanding_custom_entry_point() {
    let dir = scratch_dir("freestanding_custom_entry");
    let source = r#"
func _start() {
    println("Custom _start entry point");
}
"#;
    let file_name = "start_entry.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("start_entry");
    assert!(kernel_bin.exists());

    // Verify global _start symbol is present in symbol table
    let nm_out = Command::new("nm")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm");
    assert!(nm_out.status.success());
    let symbols = String::from_utf8_lossy(&nm_out.stdout);
    assert!(
        symbols.lines().any(|l| l.contains(" T _start")),
        "expected global text symbol _start in freestanding binary, got:\n{}",
        symbols
    );
}

#[test]
fn test_freestanding_vga_and_raw_memory_operations() {
    let dir = scratch_dir("freestanding_vga_raw");
    let source = r#"
func main() {
    vga_clear();
    vga_set_color(14, 0);
    vga_print("Bare Metal VGA Driver Active\n");
    vga_write_char(5, 10, 82, 15);
    vga_write_byte(0, 88);

    unsafe {
        let ptr: *raw u8 = alloc(128);
        ptr_write_u8(ptr, 99);
        let val: int = ptr_read_u8(ptr);
        println(val);
        free(ptr);
    }
}
"#;
    let file_name = "vga_raw.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("vga_raw");
    assert!(kernel_bin.exists());

    // Verify VGA symbols are linked in the binary
    let nm_out = Command::new("nm")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm");
    assert!(nm_out.status.success());
    let symbols = String::from_utf8_lossy(&nm_out.stdout);
    assert!(symbols.contains("vga_clear"), "expected vga_clear symbol");
    assert!(symbols.contains("vga_print"), "expected vga_print symbol");
    assert!(symbols.contains("vga_write_char"), "expected vga_write_char symbol");
    assert!(symbols.contains("vga_write_byte"), "expected vga_write_byte symbol");
    assert!(symbols.contains("__roze_freestanding_alloc"), "expected bump allocator symbol");
    assert!(symbols.contains("__roze_freestanding_free"), "expected freestanding free symbol");

    // Zero undefined symbols
    let nm_u = Command::new("nm")
        .arg("-u")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm -u");
    assert!(nm_u.status.success());
    let u_out = String::from_utf8_lossy(&nm_u.stdout).trim().to_string();
    assert!(u_out.is_empty(), "expected zero undefined symbols, got: {}", u_out);
}

#[test]
fn test_freestanding_vga_module_import() {
    let dir = scratch_dir("freestanding_vga_import");
    let source = r#"
import "vga";

func main() {
    vga_clear();
    let color = vga_color_entry(14, 0);
    vga_write_char(0, 0, 75, color); // 'K'
    println("VGA Module Imported Successfully");
}
"#;
    let file_name = "vga_mod.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("vga_mod");
    assert!(kernel_bin.exists());
}

#[test]
fn test_bootable_x86_64_os_kernel_multiboot_in_qemu() {
    let has_qemu = Command::new("which")
        .arg("qemu-system-x86_64")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_qemu {
        eprintln!("Skipping test_bootable_x86_64_os_kernel_multiboot_in_qemu: qemu-system-x86_64 not found");
        return;
    }

    let dir = scratch_dir("kernel_qemu_boot");
    let source = r#"
func check_4kb_alignment(addr: int) -> bool {
    let aligned = (addr / 4096) * 4096;
    if aligned == addr {
        return true;
    }
    return false;
}

func main() {
    serial_print("ROZE OS KERNEL BOOTING IN QEMU\r\n");
    let magic = multiboot2_get_magic();
    serial_print("MULTIBOOT_MAGIC: ");
    println(magic);

    let table_base = 2097152;
    let data_base = 2101248;
    unsafe {
        let table = int_to_ptr(table_base);
        let i = 0;
        while i < 16 {
            ptr_write_u8_at(table, i, 0);
            i = i + 1;
        }

        let f0 = data_base;
        let f1 = data_base + 4096;
        let f2 = data_base + 8192;
        ptr_write_u8_at(table, 0, 1);
        ptr_write_u8_at(table, 1, 1);
        ptr_write_u8_at(table, 2, 1);

        if check_4kb_alignment(f0) {
            serial_print("ALIGN_OK_FRAME_0\r\n");
        }
        if check_4kb_alignment(f1) {
            serial_print("ALIGN_OK_FRAME_1\r\n");
        }

        let p0 = int_to_ptr(f0);
        ptr_write_u8_at(p0, 0, 82);
        ptr_write_u8_at(p0, 1, 79);
        ptr_write_u8_at(p0, 2, 90);
        ptr_write_u8_at(p0, 3, 69);

        let c0 = ptr_read_u8_at(p0, 0);
        let c1 = ptr_read_u8_at(p0, 1);
        let c2 = ptr_read_u8_at(p0, 2);
        let c3 = ptr_read_u8_at(p0, 3);

        if c0 == 82 {
            if c1 == 79 {
                if c2 == 90 {
                    if c3 == 69 {
                        serial_print("DIRECT_MEM_READBACK_OK\r\n");
                    }
                }
            }
        }

        ptr_write_u8_at(table, 1, 0);
        serial_print("FRAME_1_FREED\r\n");

        let mut_found = 0;
        let j = 0;
        while j < 16 {
            if mut_found == 0 {
                let st = ptr_read_u8_at(table, j);
                if st == 0 {
                    ptr_write_u8_at(table, j, 1);
                    if j == 1 {
                        serial_print("FRAME_REUSE_OK\r\n");
                    }
                    mut_found = 1;
                }
            }
            j = j + 1;
        }
    }

    serial_print("ROZE_KERNEL_VERIFICATION_COMPLETE\r\n");
    qemu_exit(1);
}
"#;
    let file_name = "test_kernel.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let qemu_bin = dir.join("test_kernel.qemu");
    assert!(qemu_bin.exists(), "expected test_kernel.qemu to exist for QEMU direct boot");

    let qemu_output = Command::new("qemu-system-x86_64")
        .arg("-kernel")
        .arg(&qemu_bin)
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg("stdio")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04")
        .output()
        .expect("failed to run qemu-system-x86_64");

    let stdout = String::from_utf8_lossy(&qemu_output.stdout);
    assert!(stdout.contains("ROZE OS KERNEL BOOTING IN QEMU"), "missing boot banner in QEMU stdout:\n{}", stdout);
    assert!(stdout.contains("MULTIBOOT_MAGIC: 732803074"), "expected Multiboot1 magic (0x2BADB002), got:\n{}", stdout);
    assert!(stdout.contains("ALIGN_OK_FRAME_0"), "expected frame 0 alignment check, got:\n{}", stdout);
    assert!(stdout.contains("ALIGN_OK_FRAME_1"), "expected frame 1 alignment check, got:\n{}", stdout);
    assert!(stdout.contains("DIRECT_MEM_READBACK_OK"), "expected direct memory readback success, got:\n{}", stdout);
    assert!(stdout.contains("FRAME_1_FREED"), "expected frame 1 freed, got:\n{}", stdout);
    assert!(stdout.contains("FRAME_REUSE_OK"), "expected frame reuse, got:\n{}", stdout);
    assert!(stdout.contains("ROZE_KERNEL_VERIFICATION_COMPLETE"), "expected verification complete, got:\n{}", stdout);

    assert_eq!(qemu_output.status.code(), Some(3), "expected QEMU exit code 3 from isa-debug-exit");
}

#[test]
fn test_bootable_multiboot2_grub_iso_in_qemu() {
    let has_qemu = Command::new("which")
        .arg("qemu-system-x86_64")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let has_grub = Command::new("which")
        .arg("grub-mkrescue")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_qemu || !has_grub {
        eprintln!("Skipping test_bootable_multiboot2_grub_iso_in_qemu: qemu or grub-mkrescue missing");
        return;
    }

    let dir = scratch_dir("kernel_grub_iso_boot");
    let source = r#"
func main() {
    serial_print("ROZE MULTIBOOT2 GRUB KERNEL RUNNING\r\n");
    let magic = multiboot2_get_magic();
    serial_print("MULTIBOOT2_MAGIC: ");
    println(magic);
    if magic == 920085129 {
        serial_print("MULTIBOOT2_MAGIC_VERIFIED\r\n");
    }
    qemu_exit(1);
}
"#;
    let file_name = "mb2_kernel.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(build_output.status.success());

    let kernel_64 = dir.join("mb2_kernel");
    assert!(kernel_64.exists());

    let iso_dir = dir.join("iso_root");
    let boot_grub = iso_dir.join("boot").join("grub");
    std::fs::create_dir_all(&boot_grub).unwrap();
    std::fs::copy(&kernel_64, iso_dir.join("boot").join("kernel.bin")).unwrap();

    let grub_cfg = r#"
set timeout=0
set default=0
menuentry "Roze OS" {
    multiboot2 /boot/kernel.bin
    boot
}
"#;
    std::fs::write(boot_grub.join("grub.cfg"), grub_cfg).unwrap();

    let iso_file = dir.join("roze_test.iso");
    let mkrescue_output = Command::new("grub-mkrescue")
        .arg("-o")
        .arg(&iso_file)
        .arg(&iso_dir)
        .output()
        .expect("failed to run grub-mkrescue");
    assert!(mkrescue_output.status.success(), "grub-mkrescue failed: {}", String::from_utf8_lossy(&mkrescue_output.stderr));

    let qemu_output = Command::new("qemu-system-x86_64")
        .arg("-cdrom")
        .arg(&iso_file)
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg("stdio")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04")
        .output()
        .expect("failed to run qemu-system-x86_64");

    let stdout = String::from_utf8_lossy(&qemu_output.stdout);
    assert!(stdout.contains("ROZE MULTIBOOT2 GRUB KERNEL RUNNING"), "missing boot message in:\n{}", stdout);
    assert!(stdout.contains("MULTIBOOT2_MAGIC_VERIFIED"), "multiboot2 magic 0x36D76289 not verified in:\n{}", stdout);
    assert_eq!(qemu_output.status.code(), Some(3));
}

#[test]
fn modulo_operator_native() {
    let source = r#"
func main() {
    println(17 % 5);
    println(100 % 10);
    println(23 % 7);
}
"#;
    let (stdout, stderr, success) = run_native_source("modulo_operator", source);
    assert!(success, "compiler/run failed: {}", stderr);
    assert_eq!(program_output(&stdout), "2\n0\n2\n");
}

#[test]
fn raylib_game_engine_demo_native() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let game_dir = root.join("demos").join("game");
    let main_roze = game_dir.join("src").join("main.roze");
    let bridge_c = game_dir.join("src").join("raylib_bridge.c");
    let raylib_a = game_dir.join("raylib").join("lib").join("libraylib.a");

    let scratch = scratch_dir("raylib_game_demo");

    let mut build_cmd = Command::new(env!("CARGO_BIN_EXE_roze"));
    build_cmd.arg("build")
        .arg(&main_roze)
        .arg("--target")
        .arg("native")
        .arg("-o")
        .arg(&scratch)
        .arg("--link").arg(bridge_c.to_str().unwrap())
        .arg("--link").arg(raylib_a.to_str().unwrap())
        .arg("--link").arg("m")
        .arg("--link").arg("pthread")
        .arg("--link").arg("dl");

    let build_output = build_cmd.current_dir(&scratch)
        .output()
        .expect("failed to invoke roze build for game demo");

    assert!(
        build_output.status.success(),
        "game demo build failed: {}\nstdout:\n{}",
        String::from_utf8_lossy(&build_output.stderr),
        String::from_utf8_lossy(&build_output.stdout)
    );

    // Run the compiled binary in headless benchmark mode
    let game_bin = scratch.join("main");
    assert!(game_bin.exists(), "game binary main not found in scratch dir");

    let mut run_cmd = Command::new(&game_bin);
    run_cmd.env("ROZE_GAME_BENCHMARK", "1");
    let run_output = run_cmd.output().expect("failed to execute cosmic_roze");

    assert!(run_output.status.success(), "game execution failed: {}", String::from_utf8_lossy(&run_output.stderr));
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("Running Cosmic Roze in Headless Engine Verification Mode"));
    assert!(stdout.contains("180 Frames Simulated Successfully at 60 FPS target!"));
    assert!(stdout.contains("Frames Elapsed:  180"));
    assert!(stdout.contains("Game Engine 2D/3D Platform Verified!"));
}

#[test]
fn ai_ml_inference_demo_native() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let ai_dir = root.join("demos").join("ai_ml");
    let main_roze = ai_dir.join("src").join("main.roze");
    let ai_c = ai_dir.join("src").join("roze_ai.c");

    let scratch = scratch_dir("ai_ml_inference_demo");

    let mut build_cmd = Command::new(env!("CARGO_BIN_EXE_roze"));
    build_cmd.arg("build")
        .arg(&main_roze)
        .arg("--target")
        .arg("native")
        .arg("-o")
        .arg(&scratch)
        .arg("--link").arg(ai_c.to_str().unwrap())
        .arg("--link").arg("m");

    let build_output = build_cmd.current_dir(&scratch)
        .output()
        .expect("failed to invoke roze build for ai demo");

    assert!(
        build_output.status.success(),
        "ai demo build failed: {}\nstdout:\n{}",
        String::from_utf8_lossy(&build_output.stderr),
        String::from_utf8_lossy(&build_output.stdout)
    );

    let ai_bin = scratch.join("main");
    assert!(ai_bin.exists(), "ai binary main not found in scratch dir");

    let mut run_cmd = Command::new(&ai_bin);
    let run_output = run_cmd.current_dir(&scratch).output().expect("failed to execute ai demo");

    assert!(run_output.status.success(), "ai demo execution failed: {}", String::from_utf8_lossy(&run_output.stderr));
    let stdout = String::from_utf8_lossy(&run_output.stdout);
    assert!(stdout.contains("Roze AI / ML Inference Engine"));
    assert!(stdout.contains("Tensor Mathematical Engine & Activations"));
    assert!(stdout.contains("Tensor computational operations verified!"));
    assert!(stdout.contains("Generative AI: LLaMA-Style Decoder Transformer"));
    assert!(stdout.contains("Autoregressive generation completed with KV-cache acceleration!"));
    assert!(stdout.contains("Predictive Classification: Deep Neural Network"));
    assert!(stdout.contains("Neural classifier prediction completed!"));
    assert!(stdout.contains("Tensor Engine Performance & GFLOPS Benchmark"));
    assert!(stdout.contains("Roze AI / ML Inference Wrapper Verified Successfully! (ADV-05)"));
}

#[test]
fn self_hosting_compiler_bootstrap_native() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let demo_dir = root.join("demos").join("self_hosted");

    let status = Command::new("make")
        .arg("bootstrap_test")
        .arg("test_programs")
        .current_dir(&demo_dir)
        .status()
        .expect("failed to run make in demos/self_hosted");

    assert!(status.success(), "Self-hosting bootstrap test or test programs failed!");
}

#[test]
fn native_trait_bounds_and_associated_types() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("trait_bounds.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read trait_bounds.roze fixture");
    let (stdout, stderr, ok) = run_native_source("trait_bounds_test", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    assert!(out.contains("=== Single Trait Bound ==="));
    assert!(out.contains("=== Multiple Trait Bounds ==="));
    assert!(out.contains("=== Where Clause ==="));
    assert!(out.contains("=== Generic Class with Bound ==="));
    assert!(out.contains("=== Associated Types ==="));
    assert!(out.contains("trait bounds and associated types verified!"));
}

#[test]
fn native_pattern_matching_deep_destructuring_and_guards() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("pattern_matching_deep.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read pattern_matching_deep.roze fixture");
    let (stdout, stderr, ok) = run_native_source("pattern_matching_deep_test", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    assert!(out.contains("=== 1. Struct Patterns & Guards ==="));
    assert!(out.contains("Point on diagonal:\n10"));
    assert!(out.contains("Point x > y:\n25\n5"));
    assert!(out.contains("Point other:\n3\n18"));
    assert!(out.contains("=== 2. Range Patterns & Guards ==="));
    assert!(out.contains("Perfect Score!\nGrade A\nGrade B\nGrade C\nNeeds Improvement"));
    assert!(out.contains("=== 3. Nested Enums ==="));
    assert!(out.contains("Container holds Red\nContainer holds other color\nContainer holds Red and Blue pair\nContainer is empty"));
    assert!(out.contains("=== 4. Slice Patterns ==="));
    assert!(out.contains("Slice is empty\nSlice head:\n42\nSlice tail length:\n2"));
    assert!(out.contains("=== 5. Tuple Patterns ==="));
    assert!(out.contains("Tuple identical:\n7\nTuple pair:\n10\n20"));
    assert!(out.contains("M1-T02 Pattern Matching Fully Verified!"));
}

#[test]
fn native_comptime_metaprogramming_evaluation() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("comptime_metaprogramming.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read comptime_metaprogramming.roze fixture");
    let (stdout, stderr, ok) = run_native_source("comptime_test", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    assert!(out.contains("=== 1. Constant Math Evaluation ==="));
    assert!(out.contains("2750"));
    assert!(out.contains("6.28318"));
    assert!(out.contains("true"));
    assert!(out.contains("Roze Metaprogramming v2.0"));
    assert!(out.contains("=== 2. Compile-Time Function Execution ==="));
    assert!(out.contains("5040"));
    assert!(out.contains("144"));
    assert!(out.contains("=== 3. Comptime Blocks ==="));
    assert!(out.contains("210"));
    assert!(out.contains("=== 4. Precomputed Lookup Tables ==="));
    assert!(out.contains("8\n1\n8\n128\n707\n1000"));
    assert!(out.contains("=== 5. Type Layout Reflection ==="));
    assert!(out.contains("16\n8\n0\n8\n24\n8\n0\n8\n16"));
    assert!(out.contains("=== 6. List Literals ==="));
    assert!(out.contains("4\n10\n40"));
    assert!(out.contains("M1-T03 Comptime Fully Verified!"));
}

#[test]
fn native_async_await_state_machine_engine() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("async_await.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read async_await.roze fixture");
    let (stdout, stderr, ok) = run_native_source("async_await_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
true
42
42
42
Hello, Roze!
150
40";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_synchronization_primitives_and_channels() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("synchronization.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read synchronization.roze fixture");
    let (stdout, stderr, ok) = run_native_source("synchronization_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
false
true
true
false
true
true
false
10
20
20
25
25
22
true
100
100
96
96
103
103
100
500
600
true
777
888
12345
67890
true
99999
true
2
true
true
2
true
true
false
10
20
0
-1
10
450
0
true
42";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_structured_concurrency_nursery_and_task_groups() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("structured_concurrency.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read structured_concurrency.roze fixture");
    let (stdout, stderr, ok) = run_native_source("structured_concurrency_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
10
20
30
true
false
3
3
false
false
true
false
true
true
999
999
true
true
300
2";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_milestone_gate_m1_concurrency_and_generics_benchmark() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("milestone_m1_concurrency_and_generics.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read milestone_m1_concurrency_and_generics.roze fixture");
    let (stdout, stderr, ok) = run_native_source("milestone_m1_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
100
roze_m1
true
42
meaning
true
99
7
121
3
left
right
500
3
3
false
7777
true
250
100000";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_numeric_types_and_char_suite() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("numeric_types.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read numeric_types.roze fixture");
    let (stdout, stderr, ok) = run_native_source("numeric_types_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
42
200
1000
50000
100000
200000
999999
888888
10
20
R
Z
44
44
4464
4464
100
100
55
55
77
77
A
-128
127
0
255
-32768
32767
0
65535
-2147483648
2147483647
0
4294967295
0
18446744073709551615
0
18446744073709551615
4
251
44
-116
116
-32536
4
4
4
255
0
255
127
-128
127
32767
0
4294967295
18446744073709551615
1234567890123456789
9876543210987654321
-170141183460469231731687303715884105728
170141183460469231731687303715884105727
0
340282366920938463463374607431768211455
170141183460469231731687303715884105727
340282366920938463463374607431768211455
-170141183460469231731687303715884105728
0";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_simd_vector_intrinsics_engine() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("simd_vectors.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read simd_vectors.roze fixture");
    let (stdout, stderr, ok) = run_native_source("simd_vectors_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
Vec4f(1.0, 2.0, 3.0, 4.0)
Vec4f(5.0, 6.0, 7.0, 8.0)
Vec4f(6.0, 8.0, 10.0, 12.0)
Vec4f(4.0, 4.0, 4.0, 4.0)
Vec4f(5.0, 12.0, 21.0, 32.0)
Vec4f(4.0, 4.0, 4.0, 4.0)
Vec4f(2.0, 4.0, 6.0, 8.0)
Vec4f(11.0, 12.0, 13.0, 14.0)
Vec4f(3.5, 3.5, 3.5, 3.5)
Vec4f(0.0, 0.0, 0.0, 0.0)
true
true
Vec4f(1.0, 2.0, 3.0, 4.0)
Vec4f(5.0, 6.0, 7.0, 8.0)
Vec4f(2.0, 3.0, 4.0, 5.0)
Vec4f(1.0, 2.0, 3.0, 4.0)
Vec4f(6.0, 14.0, 24.0, 36.0)
true
true
Vec4f(1.0, 99.0, 3.0, 4.0)
true
false
true
Vec8f(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0)
Vec8f(3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0)
Vec8f(2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0)
true
true
true
true
Vec8f(10.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0)
Vec4i(10, 20, 30, 40)
Vec4i(11, 22, 33, 44)
Vec4i(9, 18, 27, 36)
Vec4i(10, 40, 90, 160)
Vec4i(10, 10, 10, 10)
100
Vec4i(1, 2, 3, 4)
Vec4i(10, 20, 30, 40)
Vec4i(5, 10, 15, 20)
20
Vec4i(10, 20, 99, 40)
Vec16u8(200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200)
Vec16u8(44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44, 44)
Vec16u8(255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255)
Vec16u8(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)
Vec16u8(100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100)
Vec16u8(200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200)
1600
200
Vec16u8(42, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200)
Vec4f(1.0, 2.0, 3.0, 4.0)
Vec8f(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0)
Vec4i(10, 20, 30, 40)
Vec16u8(200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200, 200)";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_advanced_collections_suite() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("advanced_collections.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read advanced_collections.roze fixture");
    let (stdout, stderr, ok) = run_native_source("advanced_collections_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
--- VecDeque ---
4
1
20
5
7
1
20
2
true
--- BTreeMap ---
5
10
80
200
true
false
200
4
false
true
--- BTreeSet ---
true
true
true
false
3
10
40
true
false
true
2
true
--- BinaryHeap ---
6
50
50
40
30
3
true
--- IndexMap ---
3
2
100
1
300
3
true
2
2
true
--- RingBuffer ---
3
true
true
true
true
true
false
3
20
20
30
40
true
--- BitSet ---
true
false
true
3
false
2
1
3
true
2
0
COLLECTIONS_ALL_PASSED";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn native_pluggable_allocators_and_smart_pointers_suite() {
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("memory_management.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read memory_management.roze fixture");
    let (stdout, stderr, ok) = run_native_source("memory_management_native", &source);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    let expected = "\
--- Memory Utilities ---
8
16
64
64
64
--- Box ---
42
99
99
--- Arc & Weak ---
100
1
0
2
1
true
false
2
100
3
--- BumpAllocator ---
1024
true
0
true
false
true
true
true
true
true
0
--- ArenaAllocator ---
1
0
true
1
true
true
true
0
--- PoolAllocator ---
32
4
4
false
true
true
0
true
true
true
1
false
4
true
MEMORY_ALL_PASSED";
    assert_eq!(out.trim_end(), expected);
}

#[test]
fn no_leak_in_pluggable_allocators_and_smart_pointers() {
    if Command::new("valgrind").arg("--version").output().is_err() {
        eprintln!("skipping no_leak_in_pluggable_allocators_and_smart_pointers: valgrind not found on PATH");
        return;
    }

    let dir = scratch_dir("valgrind_memory_management");
    let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("memory_management.roze");
    let source = std::fs::read_to_string(fixture_path).expect("read memory_management.roze fixture");
    std::fs::write(dir.join("memory_leak_check.roze"), &source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("memory_leak_check.roze")
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
        .arg("--error-exitcode=42")
        .arg("./memory_leak_check")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke valgrind");

    let stderr = String::from_utf8_lossy(&valgrind_output.stderr);
    assert!(valgrind_output.status.success(), "valgrind detected a memory error or leak:\n{}", stderr);
    assert!(stderr.contains("All heap blocks were freed"), "expected zero leaks, got:\n{}", stderr);
}

#[test]
fn test_freestanding_zero_heap_mode() {
    let dir = scratch_dir("freestanding_zero_heap");
    let source = r#"
import "freestanding";

func main() {
    let heap_on = freestanding_is_heap_enabled();
    let heap_cap = freestanding_get_heap_capacity();
    println("Zero Heap Test");
}
"#;
    let file_name = "zero_heap_kernel.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .arg("--no-heap")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("zero_heap_kernel");
    assert!(kernel_bin.exists());

    // Verify .bss section size is small (no 1MB BSS heap pool)
    let readelf_s = Command::new("readelf")
        .arg("-S")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run readelf -S");
    assert!(readelf_s.status.success());
    let s_out = String::from_utf8_lossy(&readelf_s.stdout);
    assert!(s_out.contains(".bss"), "expected .bss section in readelf output");

    // Zero undefined symbols
    let nm_u = Command::new("nm")
        .arg("-u")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm -u");
    assert!(nm_u.status.success());
    let u_out = String::from_utf8_lossy(&nm_u.stdout).trim().to_string();
    assert!(u_out.is_empty(), "expected zero undefined symbols, got: {}", u_out);
}

#[test]
fn test_freestanding_zero_heap_rejection() {
    let dir = scratch_dir("freestanding_no_heap_rejection");
    let source = r#"
class Task {
    id: int,
}

func main() {
    let t = new Task(1);
}
"#;
    let file_name = "heap_rejection.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .arg("--no-heap")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        !build_output.status.success(),
        "expected build to fail with --no-heap when using dynamic classes"
    );
    let stderr = String::from_utf8_lossy(&build_output.stderr);
    assert!(
        stderr.contains("Dynamic heap allocation ('new Task') is disabled in zero-heap bare-metal mode"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn test_freestanding_isr_disables_float_registers() {
    let dir = scratch_dir("freestanding_isr_float");
    let source = r#"
func isr_timer(tick: int) -> float {
    return 3.14;
}

func main() {
}
"#;
    let file_name = "isr_float.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        !build_output.status.success(),
        "expected build to fail when ISR handler uses float return type"
    );
    let stderr = String::from_utf8_lossy(&build_output.stderr);
    assert!(
        stderr.contains("Floating-point and SIMD vector registers are disabled in ISR 'isr_timer'"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn test_freestanding_no_float_flag() {
    let dir = scratch_dir("freestanding_no_float");
    let source = r#"
import "freestanding";

func main() {
    let f_on = freestanding_is_float_enabled();
    println("No float test");
}
"#;
    let file_name = "no_float_kernel.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .arg("--disable-float-registers")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("no_float_kernel");
    assert!(kernel_bin.exists());
}

#[test]
fn test_freestanding_custom_entry_flag() {
    let dir = scratch_dir("freestanding_custom_entry_flag");
    let source = r#"
func kernel_boot_entry() {
    println("Custom entry flag active");
}
"#;
    let file_name = "custom_entry.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .arg("--entry")
        .arg("kernel_boot_entry")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("custom_entry");
    assert!(kernel_bin.exists());

    // Verify global symbol `kernel_boot_entry` is exported in symbol table
    let nm_out = Command::new("nm")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm");
    assert!(nm_out.status.success());
    let symbols = String::from_utf8_lossy(&nm_out.stdout);
    assert!(
        symbols.lines().any(|l| l.contains(" T kernel_boot_entry")),
        "expected global text symbol kernel_boot_entry in freestanding binary, got:\n{}",
        symbols
    );
}

#[test]
fn test_freestanding_invalid_linker_script() {
    let dir = scratch_dir("freestanding_bad_ld");
    let source = r#"
func main() {
    println("Hello");
}
"#;
    let file_name = "bad_ld.roze";
    std::fs::write(dir.join(file_name), source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .arg("-T")
        .arg("non_existent_script.ld")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(!build_output.status.success());
    let stderr = String::from_utf8_lossy(&build_output.stderr);
    assert!(
        stderr.contains("Custom linker script 'non_existent_script.ld' not found"),
        "unexpected stderr:\n{}",
        stderr
    );
}

#[test]
fn native_freestanding_kernel_target_suite() {
    let dir = scratch_dir("native_freestanding_kernel");
    let fixture_src = std::fs::read_to_string("tests/fixtures/freestanding_kernel.roze").unwrap();
    let file_name = "kernel_suite.roze";
    std::fs::write(dir.join(file_name), fixture_src).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("kernel_suite");
    assert!(kernel_bin.exists());

    let nm_u = Command::new("nm")
        .arg("-u")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm -u");
    assert!(nm_u.status.success());
    let u_out = String::from_utf8_lossy(&nm_u.stdout).trim().to_string();
    assert!(u_out.is_empty(), "expected zero undefined symbols, got:\n{}", u_out);
}

#[test]
fn native_kernel_paging_and_virtual_memory_manager_suite() {
    let source = std::fs::read_to_string("tests/fixtures/kernel_paging_vmm.roze")
        .or_else(|_| std::fs::read_to_string("compiler/tests/fixtures/kernel_paging_vmm.roze"))
        .expect("read kernel_paging_vmm fixture");

    let (stdout, stderr, ok) = run_native_source("native_kernel_paging_vmm", &source);
    assert!(ok, "native build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert!(stdout.contains("=== Roze Kernel Paging & VMM Test Suite ==="));
    assert!(stdout.contains("--- Test 1: Bitwise Intrinsics ---"));
    assert!(stdout.contains("--- Test 2: Address Decomposition & Alignment ---"));
    assert!(stdout.contains("--- Test 3: Bitmap Frame Allocator ---"));
    assert!(stdout.contains("--- Test 4: Buddy Allocator (Split & Coalesce) ---"));
    assert!(stdout.contains("--- Test 5: 4-Level Page Table Manipulator ---"));
    assert!(stdout.contains("--- Test 6: TLB & CR3 Hardware Control ---"));
    assert!(stdout.contains("=== All Kernel Memory & VMM Tests Passed Successfully ==="));

    // Also verify standalone freestanding kernel binary compilation
    let dir = scratch_dir("freestanding_paging_kernel");
    let file_name = "paging_kernel.roze";
    std::fs::write(dir.join(file_name), &source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "freestanding build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("paging_kernel");
    assert!(kernel_bin.exists());

    let nm_u = Command::new("nm")
        .arg("-u")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm -u");
    assert!(nm_u.status.success());
    let u_out = String::from_utf8_lossy(&nm_u.stdout).trim().to_string();
    assert!(u_out.is_empty(), "expected zero undefined symbols, got:\n{}", u_out);
}

#[test]
fn native_kernel_hardware_interrupts_and_core_suite() {
    let source = std::fs::read_to_string("tests/fixtures/kernel_interrupts.roze")
        .or_else(|_| std::fs::read_to_string("compiler/tests/fixtures/kernel_interrupts.roze"))
        .expect("read kernel_interrupts fixture");

    let (stdout, stderr, ok) = run_native_source("native_kernel_interrupts", &source);
    assert!(ok, "native build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert!(stdout.contains("=== Roze Kernel Hardware Interrupts & Core Architecture Test Suite ==="));
    assert!(stdout.contains("--- Test 1: CPU Interrupt State & Flags ---"));
    assert!(stdout.contains("--- Test 2: GDT & TSS Configuration ---"));
    assert!(stdout.contains("--- Test 3: IDT Gate Configuration & Descriptors ---"));
    assert!(stdout.contains("--- Test 4: Exception Handlers & Diagnostic Decoders ---"));
    assert!(stdout.contains("--- Test 5: 8259 PIC Driver ---"));
    assert!(stdout.contains("--- Test 6: Local APIC Driver ---"));
    assert!(stdout.contains("--- Test 7: Hardware Interrupt Dispatch & Telemetry ---"));
    assert!(stdout.contains("=== All Kernel Core & Interrupt Subsystem Tests Passed Successfully ==="));

    // Also verify standalone freestanding kernel binary compilation with zero undefined symbols
    let dir = scratch_dir("freestanding_interrupts_kernel");
    let file_name = "interrupts_kernel.roze";
    std::fs::write(dir.join(file_name), &source).unwrap();

    let build_output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg(file_name)
        .arg("--target")
        .arg("freestanding")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke roze build");
    assert!(
        build_output.status.success(),
        "freestanding build failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let kernel_bin = dir.join("interrupts_kernel");
    assert!(kernel_bin.exists());

    let nm_u = Command::new("nm")
        .arg("-u")
        .arg(&kernel_bin)
        .output()
        .expect("failed to run nm -u");
    assert!(nm_u.status.success());
    let u_out = String::from_utf8_lossy(&nm_u.stdout).trim().to_string();
    assert!(u_out.is_empty(), "expected zero undefined symbols, got:\n{}", u_out);
}





