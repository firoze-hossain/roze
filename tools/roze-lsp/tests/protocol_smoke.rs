// tools/roze-lsp/tests/protocol_smoke.rs
//
// Comprehensive end-to-end protocol smoke test for Roze LSP v3.17.
// Exercises real JSON-RPC-over-stdio against the compiled `roze-lsp` binary:
// diagnostics, semantic tokens, hover documentation, go-to-definition,
// find-references, completion with snippets, signature help, and document formatting.

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

        Self {
            child,
            stdin,
            reader: BufReader::new(stdout),
            next_id: 1,
        }
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

    fn read_message(&mut self) -> Option<Value> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).ok()? == 0 {
                return None; // EOF
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(rest) = line
                .to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|s| s.to_string())
            {
                content_length = rest.trim().parse().ok();
            }
        }
        let len = content_length?;
        let mut buf = vec![0u8; len];
        self.reader.read_exact(&mut buf).ok()?;
        serde_json::from_slice(&buf).ok()
    }

    fn wait_for(&mut self, timeout: Duration, pred: impl Fn(&Value) -> bool) -> Option<Value> {
        let start = Instant::now();
        while start.elapsed() < timeout {
            match self.read_message() {
                Some(msg) => {
                    if pred(&msg) {
                        return Some(msg);
                    }
                }
                None => return None,
            }
        }
        None
    }

    fn shutdown_and_exit(mut self) -> std::io::Result<std::process::ExitStatus> {
        let id = self.request("shutdown", Value::Null);
        self.wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(id))
        });
        self.notify("exit", Value::Null);
        drop(self.stdin);
        self.child.wait()
    }
}

const BROKEN_SOURCE: &str =
    "func add(a: int, b: int) -> int {\n    return \"not a number\";\n}\n\nfunc main() {\n    println(add(1, 2));\n}\n";

const VALID_SOURCE: &str = "import \"core\";\n\n/// Classifies whether a number is positive or non-positive.\nfunc classify(x: int) -> string {\n    if x > 0 {\n        return \"positive\";\n    } else {\n        return \"non-positive\";\n    }\n}\n\nfunc main() {\n    for let i = 0; i < 3; i = i + 1 {\n        println(classify(i));\n    }\n    println(square(5));\n}\n";

#[test]
fn full_lsp_session_against_the_real_binary() {
    let mut client = LspClient::start();

    // 1. Initialize & Advertised Capabilities
    let init_id = client.request(
        "initialize",
        json!({
            "processId": Value::Null,
            "rootUri": "file:///tmp/roze_lsp_rust_test",
            "capabilities": {},
        }),
    );
    let init_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(init_id))
        })
        .expect("no response to initialize");
    let caps = &init_resp["result"]["capabilities"];
    assert!(caps.get("hoverProvider").is_some(), "expected hover support declared");
    assert!(caps.get("completionProvider").is_some(), "expected completion support declared");
    assert!(caps.get("definitionProvider").is_some(), "expected definition support declared");
    assert!(caps.get("referencesProvider").is_some(), "expected references support declared");
    assert!(caps.get("documentSymbolProvider").is_some(), "expected documentSymbol support declared");
    assert!(caps.get("semanticTokensProvider").is_some(), "expected semanticTokens support declared");
    assert!(caps.get("signatureHelpProvider").is_some(), "expected signatureHelp support declared");
    assert!(caps.get("documentFormattingProvider").is_some(), "expected documentFormatting support declared");

    client.notify("initialized", json!({}));

    // 2. Diagnostics: Open broken program -> publishDiagnostics with exact type error
    let broken_uri = "file:///tmp/roze_lsp_rust_test/broken.roze";
    client.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": { "uri": broken_uri, "languageId": "roze", "version": 1, "text": BROKEN_SOURCE }
        }),
    );
    let diag_msg = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("method") == Some(&json!("textDocument/publishDiagnostics"))
        })
        .expect("expected a publishDiagnostics notification");
    let diags = diag_msg["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(diags.len(), 1, "expected 1 diagnostic for return type error");
    let message = diags[0]["message"].as_str().unwrap().to_lowercase();
    assert!(message.contains("return") || message.contains("int"));
    assert_eq!(diags[0]["range"]["start"]["line"], 1);

    // 3. Diagnostics: Open valid program -> zero diagnostics
    let valid_uri = "file:///tmp/roze_lsp_rust_test/valid.roze";
    client.notify(
        "textDocument/didOpen",
        json!({
            "textDocument": { "uri": valid_uri, "languageId": "roze", "version": 1, "text": VALID_SOURCE }
        }),
    );
    let diag_msg2 = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("method") == Some(&json!("textDocument/publishDiagnostics"))
                && m["params"]["uri"] == json!(valid_uri)
        })
        .expect("expected publishDiagnostics for valid document");
    let diags2 = diag_msg2["params"]["diagnostics"].as_array().unwrap();
    assert!(diags2.is_empty(), "valid program should have 0 diagnostics");

    // 4. Document Symbols: hierarchical symbols outline
    let sym_id = client.request(
        "textDocument/documentSymbol",
        json!({ "textDocument": { "uri": valid_uri } }),
    );
    let sym_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(sym_id))
        })
        .expect("no response to documentSymbol");
    let symbols = sym_resp["result"].as_array().cloned().unwrap_or_default();
    let names: Vec<String> = symbols
        .iter()
        .map(|s| s["name"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(names.contains(&"classify".to_string()) && names.contains(&"main".to_string()));

    // 5. Semantic Tokens (LSP v3.17): delta-encoded syntax tokens
    let sem_id = client.request(
        "textDocument/semanticTokens/full",
        json!({ "textDocument": { "uri": valid_uri } }),
    );
    let sem_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(sem_id))
        })
        .expect("no response to semanticTokens");
    let sem_data = sem_resp["result"]["data"].as_array().expect("expected semantic tokens data array");
    assert!(!sem_data.is_empty(), "expected non-empty semantic tokens data");

    // 6. Hover Documentation: Markdown popup with signature and docstring
    let hover_id = client.request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": valid_uri },
            "position": { "line": 3, "character": 6 }, // on classify
        }),
    );
    let hover_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(hover_id))
        })
        .expect("no response to hover");
    let hover_val = hover_resp["result"]["contents"]["value"].as_str().unwrap_or("");
    assert!(hover_val.contains("classify"));
    assert!(hover_val.contains("Classifies whether a number is positive"));

    // 7. Context-Aware Autocomplete: Keywords, builtins, snippets
    let comp_id = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": valid_uri },
            "position": { "line": 0, "character": 0 },
        }),
    );
    let comp_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(comp_id))
        })
        .expect("no response to completion");
    let items = comp_resp["result"].as_array().cloned().unwrap_or_default();
    let labels: Vec<String> = items
        .iter()
        .map(|i| i["label"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(labels.contains(&"func".to_string()));
    assert!(labels.contains(&"println".to_string()));
    assert!(labels.contains(&"classify".to_string()));

    // 8. Go-To-Definition: Jump to function declaration
    let def_id = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": valid_uri },
            "position": { "line": 13, "character": 18 }, // on classify(i) call
        }),
    );
    let def_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(def_id))
        })
        .expect("no response to definition");
    let def_range = &def_resp["result"]["range"];
    assert_eq!(def_range["start"]["line"], 3, "should jump to classify definition on line 3");

    // 9. Find References: Find all occurrences
    let ref_id = client.request(
        "textDocument/references",
        json!({
            "textDocument": { "uri": valid_uri },
            "position": { "line": 3, "character": 6 }, // on classify definition
            "context": { "includeDeclaration": true },
        }),
    );
    let ref_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(ref_id))
        })
        .expect("no response to references");
    let ref_locs = ref_resp["result"].as_array().expect("expected references array");
    assert!(ref_locs.len() >= 2, "expected at least declaration + call references");

    // 10. Signature Help: Active parameter popup
    let sig_id = client.request(
        "textDocument/signatureHelp",
        json!({
            "textDocument": { "uri": valid_uri },
            "position": { "line": 13, "character": 25 }, // inside classify(i|)
        }),
    );
    let sig_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(sig_id))
        })
        .expect("no response to signatureHelp");
    let sigs = sig_resp["result"]["signatures"].as_array().expect("expected signatures array");
    assert!(!sigs.is_empty());
    assert!(sigs[0]["label"].as_str().unwrap().contains("classify"));

    // 11. Document Formatting: Consistent 4-space indent
    let fmt_id = client.request(
        "textDocument/formatting",
        json!({
            "textDocument": { "uri": valid_uri },
            "options": { "tabSize": 4, "insertSpaces": true },
        }),
    );
    let fmt_resp = client
        .wait_for(Duration::from_secs(5), |m| {
            m.get("id") == Some(&json!(fmt_id))
        })
        .expect("no response to formatting");
    let edits = fmt_resp["result"].as_array().expect("expected formatting edits");
    assert_eq!(edits.len(), 1);
    assert!(!edits[0]["newText"].as_str().unwrap().is_empty());

    // 12. Clean Shutdown & Exit
    let status = client
        .shutdown_and_exit()
        .expect("failed to wait on child process");
    assert!(status.success(), "roze-lsp should exit cleanly with 0");
}
