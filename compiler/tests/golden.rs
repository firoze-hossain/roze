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

/// Like `run_fixture`, but passes `--classpath` through to `roze run`
/// (needed for the SQL intrinsics, which require a JDBC driver on the
/// classpath -- see sql_connect_query_execute_against_a_real_database).
fn run_fixture_with_classpath(fixture: &str, classpath: &str) -> (String, String, bool) {
    let dir = scratch_dir(fixture.trim_end_matches(".roze"));
    std::fs::copy(fixtures_dir().join(fixture), dir.join(fixture))
        .unwrap_or_else(|e| panic!("failed to copy fixture {}: {}", fixture, e));

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg(fixture)
        .arg("--classpath")
        .arg(classpath)
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.success(),
    )
}
/// progress messages ("🔤 Lexer: N tokens", etc). Everything the program
/// itself printed comes after the "🚀 Running: <name>" line.
///
/// Also normalizes line endings: Java's `println` emits the platform
/// line separator (`\r\n` on Windows, `\n` elsewhere), but the expected
/// strings in these tests are written once, in Unix style. Without this,
/// every test here fails on Windows despite the program's output being
/// perfectly correct -- a difference in line-ending convention, not a
/// compiler bug.
fn program_output(stdout: &str) -> String {
    let after_running = match stdout.split_once("🚀 Running:") {
        Some((_, after)) => after.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default(),
        None => stdout.to_string(),
    };
    after_running.replace("\r\n", "\n")
}

#[cfg(test)]
mod program_output_tests {
    use super::program_output;

    #[test]
    fn normalizes_windows_line_endings() {
        let windows_style = "🌹 Roze Compiler v0.1\r\n📁 Compiling: x.roze\r\n🚀 Running: x\r\nhello\r\nworld\r\n";
        assert_eq!(program_output(windows_style).trim_end(), "hello\nworld");
    }

    #[test]
    fn leaves_unix_line_endings_unchanged() {
        let unix_style = "🌹 Roze Compiler v0.1\n📁 Compiling: x.roze\n🚀 Running: x\nhello\nworld\n";
        assert_eq!(program_output(unix_style).trim_end(), "hello\nworld");
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
fn source_file_in_a_subdirectory_compiles_and_runs() {
    // The exact bug this guards against: passing a path with a
    // directory component (e.g. "tests\test.roze" on Windows,
    // "subdir/test.roze" here) used to embed the *entire path* --
    // separator and all -- as the literal Java class name, producing
    // invalid Java ("illegal character: '\'" / similarly for other
    // separators the naive split('/') didn't handle).
    let dir = scratch_dir("subdirectory_source");
    let subdir = dir.join("subdir");
    std::fs::create_dir_all(&subdir).unwrap();
    std::fs::write(subdir.join("nested.roze"), "func main() { println(\"hi from a subdirectory\"); }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg("subdir/nested.roze")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(program_output(&stdout).trim_end(), "hi from a subdirectory");
}

#[test]
fn float_arithmetic_functions_and_intrinsics_are_correct() {
    let (stdout, stderr, ok) = run_fixture("floats.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "3.14\n5.140000000000001\n1.1400000000000001\n6.28\n1.57\n8.14\n8.14\ntrue\nfalse\n-3.14\n5.0\n3.14\n5\n3.14\n7\n7.5\n2.0\ntrue\nfalse\n3.14\n4.0"
    );
}

#[test]
fn java_reserved_words_as_identifiers_compile_and_run_correctly() {
    // The exact bug from ROADMAP.md's "Known issue found but not yet
    // fixed" section: a Roze function/variable/parameter/loop-variable
    // name that happens to be a Java keyword used to fail to compile,
    // since codegen emitted Roze identifiers verbatim.
    let (stdout, stderr, ok) = run_fixture("reserved_words.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "ok\n5\n42\nhello\nworld\n0\n1\n2"
    );
}

#[test]
fn classes_construction_field_access_and_mutation() {
    let (stdout, stderr, ok) = run_fixture("classes.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "3\n4\n25\n116\nHello, Alice! You are 30.\nHello, Bob! You are 25.\nAlice\nHello, Alice! You are 31."
    );
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

#[test]
fn collections_list_and_map() {
    let (stdout, stderr, ok) = run_fixture("collections.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "3\napple\nblueberry\n2\nblueberry\nfalse\n2\n30\ntrue\nfalse\n1\nfalse";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn file_io_read_write_append_delete() {
    let (stdout, stderr, ok) = run_fixture("file_io.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "true\nHello from Roze!\nHello from Roze!\nSecond line\n2\nHello from Roze!\nSecond line\ntrue\nfalse";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

/// A minimal, hand-rolled HTTP/1.1 server for testing `http_get`/
/// `http_post` against real network I/O without depending on any
/// external network access -- loopback-only, so this works the same
/// regardless of the sandbox/CI environment's network policy. Handles
/// exactly the two requests the network golden test below makes, then
/// its thread finishes.
mod test_http_server {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    pub fn start() -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind test HTTP server");
        let port = listener.local_addr().unwrap().port();

        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                if let Ok(mut stream) = stream {
                    handle_one_request(&mut stream);
                }
            }
        });

        (port, handle)
    }

    fn handle_one_request(stream: &mut TcpStream) {
        let mut buf = [0u8; 8192];
        let mut request = Vec::new();

        loop {
            let n = stream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buf[..n]);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }

        let header_end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(request.len());
        let header_text = String::from_utf8_lossy(&request[..header_end]).to_string();
        let request_line = header_text.lines().next().unwrap_or("");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("GET").to_string();
        let path = parts.next().unwrap_or("/").to_string();

        let content_length: usize = header_text
            .lines()
            .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_string()))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let body_start = (header_end + 4).min(request.len());
        let mut body = request[body_start..].to_vec();
        while body.len() < content_length {
            let n = stream.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            body.extend_from_slice(&buf[..n]);
        }
        let body_text = String::from_utf8_lossy(&body).to_string();

        let response_body = if method == "GET" {
            format!("hello from GET {}", path)
        } else {
            format!("echo: {}", body_text)
        };

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    }
}

#[test]
fn network_http_get_and_post_against_a_local_server() {
    let (port, _server) = test_http_server::start();

    let dir = scratch_dir("network");
    let source = format!(
        "func main() {{\n    println(http_get(\"http://127.0.0.1:{port}/hello\"));\n    println(http_post(\"http://127.0.0.1:{port}/echo\", \"some data\"));\n}}\n",
        port = port,
    );
    std::fs::write(dir.join("network_test.roze"), source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg("network_test.roze")
        .current_dir(&dir)
        .output()
        .expect("failed to invoke the roze binary");

    assert!(
        output.status.success(),
        "build/run failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        program_output(&stdout).trim_end(),
        "hello from GET /hello\necho: some data"
    );
}

#[test]
fn json_encode_and_decode() {
    let (stdout, stderr, ok) = run_fixture("json.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "Alice\n30\nreading\ncycling\n4\n1\nfour";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn http_server_accepts_and_responds_to_real_client_requests() {
    use std::io::{BufRead, BufReader, Read, Write};

    // Reserve a free port by briefly binding it in this test process,
    // then release it immediately for the Roze server subprocess to
    // bind -- a standard (if not airtight) pattern for picking a free
    // port to hand to a child process.
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();

    let dir = scratch_dir("http_server");
    let source = format!(
        "func main() {{\n    \
            let server = http_server_start({port});\n    \
            println(\"SERVER_READY\");\n    \
            let req1 = http_server_accept(server);\n    \
            http_server_respond(req1, 200, \"hello from roze server\");\n    \
            let req2 = http_server_accept(server);\n    \
            http_server_respond(req2, 201, \"got: \" + map_get(req2, \"body\"));\n    \
            http_server_stop(server);\n\
        }}\n",
        port = port,
    );
    std::fs::write(dir.join("server_test.roze"), source).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("run")
        .arg("server_test.roze")
        .current_dir(&dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn the roze binary");

    // Wait for the "SERVER_READY" marker (printed right after the
    // ServerSocket is bound, so the port is genuinely already accepting
    // connections into its backlog by the time we see it) rather than
    // an arbitrary sleep.
    let stdout_pipe = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout_pipe);
    let mut saw_ready = false;
    for _ in 0..200 {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line.trim() == "SERVER_READY" {
            saw_ready = true;
            break;
        }
    }
    assert!(saw_ready, "server never printed its ready marker");

    // First request: plain GET.
    let mut client1 = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect (GET)");
    client1.write_all(b"GET /greet HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut response1 = String::new();
    client1.read_to_string(&mut response1).unwrap();
    assert!(response1.contains("200"), "expected a 200 status, got:\n{}", response1);
    assert!(response1.ends_with("hello from roze server"), "unexpected body:\n{}", response1);

    // Second request: POST with a body, echoed back by the server.
    let body = "some payload";
    let mut client2 = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect (POST)");
    let request2 = format!(
        "POST /submit HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    client2.write_all(request2.as_bytes()).unwrap();
    let mut response2 = String::new();
    client2.read_to_string(&mut response2).unwrap();
    assert!(response2.contains("201"), "expected a 201 status, got:\n{}", response2);
    assert!(response2.ends_with("got: some payload"), "unexpected body:\n{}", response2);

    let status = child.wait().expect("failed to wait on the server process");
    assert!(status.success(), "the roze server process should exit cleanly after handling both requests");
}

/// SQL is the one intrinsic family that fundamentally needs something
/// Roze can't provide itself: a real JDBC driver on the classpath (the
/// JDK ships no database driver at all, for any database -- see
/// stdlib/src/sql.roze). Rather than either committing a driver jar to
/// this repo or skipping SQL testing entirely, this test is gated on an
/// environment variable pointing at one: set `ROZE_TEST_JDBC_JAR` to a
/// driver jar's path (e.g. an H2 jar) to actually exercise it. The CI
/// workflow downloads one and sets this for exactly that reason -- see
/// .github/workflows/test.yml. Without it, the test explicitly skips
/// (with a clear message) rather than silently reporting a pass for
/// something that was never run.
#[test]
fn sql_connect_query_execute_against_a_real_database() {
    let jar = match std::env::var("ROZE_TEST_JDBC_JAR") {
        Ok(path) if !path.is_empty() => path,
        _ => {
            eprintln!("skipping sql_connect_query_execute_against_a_real_database: set ROZE_TEST_JDBC_JAR to a JDBC driver jar (e.g. an H2 jar) to run it");
            return;
        }
    };

    let (stdout, stderr, ok) = run_fixture_with_classpath("sql.roze", &jar);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "2\nAlice\n30\nBob\n1\n31\ndone";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn result_and_option_types_and_safe_error_propagation() {
    let (stdout, stderr, ok) = run_fixture("result_option.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
true\n\
false\n\
false\n\
true\n\
42\n\
100\n\
true\n\
apple\n\
true\n\
not found\n\
localhost\n\
default_db\n\
true\n\
false\n\
false\n\
true\n\
operation succeeded\n\
connection refused\n\
fallback value\n\
true\n\
500\n\
true\n\
true\n\
true\n\
true\n\
Safe I/O in Roze\n\
true\n\
21\n\
true\n\
1020\n\
true\n\
roze_lang\n\
true";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn try_operator_type_errors_report_cleanly() {
    let dir = scratch_dir("try_type_errors");

    // 1. Result ? inside int function
    std::fs::write(
        dir.join("bad_ret_type.roze"),
        "func f() -> int { let res = Ok(1); let val = res?; return val; } func main() {}\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("bad_ret_type.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("The '?' operator on a Result requires the enclosing function to return Result"), "got:\n{}", stderr);

    // 2. Option ? inside Result function
    std::fs::write(
        dir.join("option_in_result.roze"),
        "func f() -> Result { let opt = Some(1); let val = opt?; return Ok(val); } func main() {}\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("option_in_result.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("The '?' operator on an Option requires the enclosing function to return Option"), "got:\n{}", stderr);

    // 3. ? on integer
    std::fs::write(
        dir.join("try_on_int.roze"),
        "func f() -> int { let x = 42; return x?; } func main() {}\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("try_on_int.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("The '?' operator can only be used on Result or Option"), "got:\n{}", stderr);
}

#[test]
fn enums_and_pattern_matching() {
    let (stdout, stderr, ok) = run_fixture("enums_pattern_matching.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
color is red
color is green
color is blue
300
20
0
action: quit
action: move
100
250
action: write
Hello Roze enums!
action: change_color
255
128
0
got circle
10
got something else
got something else";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn closures_and_first_class_functions() {
    let (stdout, stderr, ok) = run_fixture("closures.roze");
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
closures working smoothly!";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn generics_and_parametric_polymorphism() {
    let (stdout, stderr, ok) = run_fixture("generics.roze");
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
fn traits_and_dynamic_dispatch() {
    let (stdout, stderr, ok) = run_fixture("traits.roze");
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
fn c_ffi_throws_unsupported_on_jvm() {
    let (_stdout, stderr, ok) = run_fixture("ffi.roze");
    assert!(!ok, "JVM run should fail with UnsupportedOperationException");
    assert!(
        stderr.contains("UnsupportedOperationException") && stderr.contains("native-only"),
        "stderr should mention UnsupportedOperationException and native-only, got:\n{}",
        stderr
    );
}

#[test]
fn concurrency_threads_and_channels() {
    let (stdout, stderr, ok) = run_fixture("concurrency.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
42
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
fn green_threads_m_n_concurrency() {
    let (stdout, stderr, ok) = run_fixture("green_threads_m_n.roze");
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
fn concurrent_async_http_server_and_router() {
    let (stdout, stderr, ok) = run_fixture("web_server.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
123
admin
Roze Server
42
assets/styles/main.css
100
200
5";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn unsafe_raw_pointers_and_direct_memory() {
    let (stdout, stderr, ok) = run_fixture("unsafe_memory.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
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
fn rozeboot_enterprise_framework_core() {
    let (stdout, stderr, ok) = run_fixture("rozeboot_core.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
time_sec_ok
time_ms_ok
true
false
admin_user_42
SUPER_ADMIN
false
SELECT id, username, email FROM users WHERE role = 'admin' AND status = 1 ORDER BY created_at DESC LIMIT 25 OFFSET 50
DELETE FROM sessions WHERE user_id = '42'
INSERT INTO users (name) VALUES ('Charlie')
UPDATE users SET status = 'suspended' WHERE id = '10'
UserController
/api/v1
3
RozeBootShowcaseApp
8080
1
3";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn rozeboot_enterprise_web_server_with_jwt_auth() {
    let (stdout, stderr, ok) = run_fixture("rozeboot_server.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let out = program_output(&stdout);
    assert!(out.contains("RozeBoot Enterprise Framework"), "stdout missing banner:\n{}", out);
    assert!(out.contains("UP"), "missing /info UP:\n{}", out);
    assert!(out.contains("Unauthorized"), "missing 401 unauthorized:\n{}", out);
    assert!(out.contains("1001"), "missing order id 1001:\n{}", out);
    assert!(out.contains("ADMIN"), "missing authorized ADMIN role:\n{}", out);
}

#[test]
fn rozeboot_connection_pool_and_orm_database() {
    let jar = match std::env::var("ROZE_TEST_JDBC_JAR") {
        Ok(j) => j,
        Err(_) => {
            let default_h2 = "/home/firoze/.m2/repository/com/h2database/h2/2.2.224/h2-2.2.224.jar";
            if std::path::Path::new(default_h2).exists() {
                default_h2.to_string()
            } else {
                eprintln!("skipping rozeboot_connection_pool_and_orm_database: set ROZE_TEST_JDBC_JAR to run");
                return;
            }
        }
    };

    let (stdout, stderr, ok) = run_fixture_with_classpath("rozeboot_db.roze", &jar);
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
0
2
Roze Book
49
Roze Keyboard
129
true";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn trait_bounds_and_associated_types() {
    let (stdout, stderr, ok) = run_fixture("trait_bounds.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
=== Single Trait Bound ===
User id:
42
Product price:
199
=== Multiple Trait Bounds ===
User id:
42
42
=== Where Clause ===
User id:
42
Product price:
199
=== Generic Class with Bound ===
User id:
42
=== Associated Types ===
777
trait bounds and associated types verified!";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn trait_bound_violation_reports_cleanly() {
    let dir = scratch_dir("trait_bound_violation");
    std::fs::write(
        dir.join("bound_error.roze"),
        "trait Describable { func desc() -> string; }\nclass Item { id: int }\nfunc inspect<T: Describable>(x: T) { x.desc(); }\nfunc main() { let i = new Item(1); inspect<Item>(i); }\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("bound_error.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Type 'Item' does not satisfy trait bound 'Describable'"), "got:\n{}", stderr);
}

#[test]
fn pattern_matching_deep_destructuring_and_guards() {
    let (stdout, stderr, ok) = run_fixture("pattern_matching_deep.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
=== 1. Struct Patterns & Guards ===
Point on diagonal:
10
Point x > y:
25
5
Point other:
3
18
=== 2. Range Patterns & Guards ===
Perfect Score!
Grade A
Grade B
Grade C
Needs Improvement
=== 3. Nested Enums ===
Container holds Red
Container holds other color
Container holds Red and Blue pair
Container is empty
=== 4. Slice Patterns ===
Slice is empty
Slice head:
42
Slice tail length:
2
=== 5. Tuple Patterns ===
Tuple identical:
7
Tuple pair:
10
20
M1-T02 Pattern Matching Fully Verified!";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn pattern_matching_non_exhaustive_error() {
    let dir = scratch_dir("pattern_match_non_exhaustive");
    std::fs::write(
        dir.join("non_exhaustive.roze"),
        "enum Color { Red, Green, Blue }\nfunc test(c: Color) -> int {\n    match c {\n        Color::Red => { return 1; },\n        Color::Green => { return 2; }\n    }\n}\nfunc main() {}\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("non_exhaustive.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Non-exhaustive pattern match") && stderr.contains("missing variant(s) Blue"), "got:\n{}", stderr);
}

#[test]
fn comptime_metaprogramming_evaluation() {
    let (stdout, stderr, ok) = run_fixture("comptime_metaprogramming.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
=== 1. Constant Math Evaluation ===
2750
6.28318
true
Roze Metaprogramming v2.0
=== 2. Compile-Time Function Execution ===
5040
144
=== 3. Comptime Blocks ===
210
=== 4. Precomputed Lookup Tables ===
8
1
8
128
707
1000
=== 5. Type Layout Reflection ===
16
8
0
8
24
8
0
8
16
=== 6. List Literals ===
4
10
40
M1-T03 Comptime Fully Verified!";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn comptime_division_by_zero_fails_at_compile_time() {
    let dir = scratch_dir("comptime_div_zero");
    std::fs::write(
        dir.join("div_zero.roze"),
        "func main() {\n    let x = comptime (100 / 0);\n}\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("div_zero.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Compile-time evaluation error: division by zero"), "got:\n{}", stderr);
}

#[test]
fn comptime_recursion_limit_fails_at_compile_time() {
    let dir = scratch_dir("comptime_infinite_loop");
    std::fs::write(
        dir.join("inf_loop.roze"),
        "func loop_forever() -> int {\n    while true {}\n    return 0;\n}\nfunc main() {\n    let x = comptime loop_forever();\n}\n",
    ).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_roze"))
        .arg("build")
        .arg("inf_loop.roze")
        .current_dir(&dir)
        .output()
        .expect("invoke roze");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("maximum evaluation steps exceeded"), "got:\n{}", stderr);
}

#[test]
fn async_await_state_machine_engine() {
    let (stdout, stderr, ok) = run_fixture("async_await.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
    let expected = "\
true
42
42
42
Hello, Roze!
150
40";
    assert_eq!(program_output(&stdout).trim_end(), expected);
}

#[test]
fn synchronization_primitives_and_channels() {
    let (stdout, stderr, ok) = run_fixture("synchronization.roze");
    assert!(ok, "build/run failed:\nstdout:\n{}\nstderr:\n{}", stdout, stderr);
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
    assert_eq!(program_output(&stdout).trim_end(), expected);
}


