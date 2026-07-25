// tools/roze-lsp/tests/protocol_smoke.rs
//
// An end-to-end protocol smoke test: speaks real LSP JSON-RPC-over-stdio
// to the actual compiled `roze-lsp` binary, the same way VS Code's
// language client does. The VS Code extension itself (see ide/vscode) is
// thin boilerplate that just spawns this binary and forwards messages
// between the editor UI and it -- the real behavior worth testing is the
// server's, which this exercises directly and completely without needing
// an actual VS Code GUI.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct LspClient {
    child: Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: i64,
}

impl LspClient {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_roze-lsp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start roze-lsp");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        Self { child, stdin, reader: BufReader::new(stdout), next_id: 1 }
    }

    fn write_message(&mut self, msg: &Value) {
        let body = serde_json::to_vec(msg).unwrap();
        write!(self.stdin, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        self.stdin.write_all(&body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> i64 {
        let id = self.next_id;
        self.next_id += 1;
        self.write_message(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params,
        }));
        id
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.write_message(&json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    /// Reads one framed message from the server.
    fn read_message(&mut self) -> Option<Value> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).ok()? == 0 {
                return None; // EOF
            }
            let line = line.trim_end();
            if line.is_empty() {
                break; // end of headers
            }
            if let Some(rest) = line.to_ascii_lowercase().strip_prefix("content-length:").map(|s| s.to_string()) {
                content_length = rest.trim().parse().ok();
            }
        }
        let len = content_length?;
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).ok()?;
        serde_json::from_slice(&buf).ok()
    }

    /// Reads messages (skipping ones that don't match, e.g. log
    /// notifications) until `pred` matches one or `timeout` elapses.
    fn wait_for(&mut self, timeout: Duration, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            match self.read_message() {
                Some(msg) => {
                    if pred(&msg) {
                        return Some(msg);
                    }
                    // else: an unrelated message (e.g. window/logMessage) -- keep reading
                }
                None => return None,
            }
        }
        None
    }

    fn shutdown_and_exit(mut self) -> std::io::Result<std::process::ExitStatus> {
        let id = self.request("shutdown", Value::Null);
        self.wait_for(Duration::from_secs(5), |m| m.get("id") == Some(&json!(id)));
        self.notify("exit", Value::Null);
        drop(self.stdin); // close stdin -- the server's read loop ends on EOF, same as a real client detaching
        self.child.wait()
    }
}

const BROKEN_SOURCE: &str = "func add(a: int, b: int) -> int {\n    return \"not a number\";\n}\n\nfunc main() {\n    println(add(1, 2));\n}\n";

const VALID_SOURCE: &str = "import \"core\";\n\nfunc classify(x: int) -> string {\n    if x > 0 {\n        return \"positive\";\n    } else {\n        return \"non-positive\";\n    }\n}\n\nfunc main() {\n    for let i = 0; i < 3; i = i + 1 {\n        println(classify(i));\n    }\n    println(square(5));\n}\n";

#[test]
fn full_lsp_session_against_the_real_binary() {
    let mut client = LspClient::start();

    // ---- initialize / initialized ----
    let init_id = client.request("initialize", json!({
        "processId": Value::Null,
        "rootUri": "file:///tmp/roze_lsp_rust_test",
        "capabilities": {},
    }));
    let init_resp = client.wait_for(Duration::from_secs(5), |m| m.get("id") == Some(&json!(init_id)))
        .expect("no response to initialize");
    let caps = &init_resp["result"]["capabilities"];
    assert!(caps.get("hoverProvider").is_some(), "expected hover support declared");
    assert!(caps.get("completionProvider").is_some(), "expected completion support declared");
    assert!(caps.get("documentSymbolProvider").is_some(), "expected documentSymbol support declared");

    client.notify("initialized", json!({}));

    // ---- open a program with a real type error ----
    let broken_uri = "file:///tmp/roze_lsp_rust_test/broken.roze";
    client.notify("textDocument/didOpen", json!({
        "textDocument": { "uri": broken_uri, "languageId": "roze", "version": 1, "text": BROKEN_SOURCE }
    }));
    let diag_msg = client.wait_for(Duration::from_secs(5), |m| m.get("method") == Some(&json!("textDocument/publishDiagnostics")))
        .expect("expected a publishDiagnostics notification");
    let diags = diag_msg["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1, "expected exactly one diagnostic for the return-type error, got {:?}", diags);
    let message = diags[0]["message"].as_str().unwrap().to_lowercase();
    assert!(message.contains("return") || message.contains("int"), "diagnostic should describe the real type error, got: {}", message);
    assert_eq!(diags[0]["range"]["start"]["line"], 1, "should point at the return statement (0-indexed line 1)");

    // ---- open a genuinely valid program using for-loops + imports, expect zero diagnostics ----
    let valid_uri = "file:///tmp/roze_lsp_rust_test/valid.roze";
    client.notify("textDocument/didOpen", json!({
        "textDocument": { "uri": valid_uri, "languageId": "roze", "version": 1, "text": VALID_SOURCE }
    }));
    let diag_msg2 = client.wait_for(Duration::from_secs(5), |m| {
        m.get("method") == Some(&json!("textDocument/publishDiagnostics"))
            && m["params"]["uri"] == json!(valid_uri)
    }).expect("expected a publishDiagnostics notification for the valid document");
    let diags2 = diag_msg2["params"]["diagnostics"].as_array().unwrap();
    assert!(diags2.is_empty(), "a genuinely valid program (using for-loops and imports) should have zero diagnostics, got {:?}", diags2);

    // ---- documentSymbol on the valid document ----
    let sym_id = client.request("textDocument/documentSymbol", json!({ "textDocument": { "uri": valid_uri } }));
    let sym_resp = client.wait_for(Duration::from_secs(5), |m| m.get("id") == Some(&json!(sym_id)))
        .expect("no response to documentSymbol");
    let symbols = sym_resp["result"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = symbols.iter().map(|s| s["name"].as_str().unwrap_or("").to_string()).collect();
    assert!(names.contains(&"classify".to_string()) && names.contains(&"main".to_string()),
        "expected both functions in document symbols, got {:?}", names);

    // ---- hover ----
    let hover_id = client.request("textDocument/hover", json!({
        "textDocument": { "uri": valid_uri },
        "position": { "line": 2, "character": 6 },
    }));
    let hover_resp = client.wait_for(Duration::from_secs(5), |m| m.get("id") == Some(&json!(hover_id)))
        .expect("no response to hover");
    assert!(hover_resp.get("result").is_some());

    // ---- completion ----
    let comp_id = client.request("textDocument/completion", json!({
        "textDocument": { "uri": valid_uri },
        "position": { "line": 0, "character": 0 },
    }));
    let comp_resp = client.wait_for(Duration::from_secs(5), |m| m.get("id") == Some(&json!(comp_id)))
        .expect("no response to completion");
    let items = comp_resp["result"].as_array().cloned().unwrap_or_default();
    let labels: Vec<String> = items.iter().map(|i| i["label"].as_str().unwrap_or("").to_string()).collect();
    assert!(labels.contains(&"func".to_string()), "expected 'func' keyword in completions, got {:?}", labels);

    // ---- shutdown / exit ----
    let status = client.shutdown_and_exit().expect("failed to wait on child process");
    assert!(status.success(), "roze-lsp should exit cleanly after shutdown+exit, got {:?}", status);
}
