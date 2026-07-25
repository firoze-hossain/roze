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
