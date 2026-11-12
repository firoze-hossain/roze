// compiler/tests/unified_toolchain.rs
//
// 🌹 Integration Test Suite for the Monolithic 'roze' CLI Toolchain Suite (M6-T09)

use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn roze_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test binary name
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("roze")
}

#[test]
fn test_roze_fmt_formatting_and_idempotence() {
    let unformatted = r#"
func calculate_values(  a:int,b:string   )->int{
let x=10+20*3;
if(x>50&&a!=0){
println("Hello "+b);
return x*2;
}else{
return 0;
}
}
"#;

    let formatted = roze_compiler::fmt::format_source(unformatted, 4);
    assert!(formatted.contains("func calculate_values (a: int, b: string) -> int {") || formatted.contains("func calculate_values(a: int, b: string) -> int {"));
    assert!(formatted.contains("let x = 10 + 20 * 3;"));
    assert!(formatted.contains("if (x > 50 && a != 0) {"));
    assert!(formatted.contains("return x * 2;"));

    // Verify idempotence
    let second_pass = roze_compiler::fmt::format_source(&formatted, 4);
    assert_eq!(formatted, second_pass, "Formatter must be strictly idempotent!");
}

#[test]
fn test_roze_repl_evaluation() {
    let mut session = roze_compiler::repl::ReplSession::new();

    // Variable assignment & arithmetic
    let res1 = session.eval("let a = 15;").unwrap();
    assert_eq!(res1, Some(roze_compiler::repl::ReplValue::Int(15)));

    let res2 = session.eval("let b = 27;").unwrap();
    assert_eq!(res2, Some(roze_compiler::repl::ReplValue::Int(27)));

    let res3 = session.eval("a + b").unwrap();
    assert_eq!(res3, Some(roze_compiler::repl::ReplValue::Int(42)));

    // String operations
    let res4 = session.eval("let name = \"Roze\";").unwrap();
    assert_eq!(res4, Some(roze_compiler::repl::ReplValue::String("Roze".to_string())));

    let res5 = session.eval("\"Hello, \" + name + \"!\"").unwrap();
    assert_eq!(res5, Some(roze_compiler::repl::ReplValue::String("Hello, Roze!".to_string())));

    // Comparisons
    let res6 = session.eval("a < b").unwrap();
    assert_eq!(res6, Some(roze_compiler::repl::ReplValue::Bool(true)));

    let res7 = session.eval("a == b").unwrap();
    assert_eq!(res7, Some(roze_compiler::repl::ReplValue::Bool(false)));

    // Builtin functions
    let res8 = session.eval("string_length(name)").unwrap();
    assert_eq!(res8, Some(roze_compiler::repl::ReplValue::Int(4)));

    let res9 = session.eval("math_sqrt(144)").unwrap();
    assert_eq!(res9, Some(roze_compiler::repl::ReplValue::Float(12.0)));

    // Variables inspection
    assert_eq!(session.variables.len(), 3);
    assert_eq!(session.variables.get("a").unwrap().type_name, "int");
    assert_eq!(session.variables.get("name").unwrap().type_name, "string");
}

#[test]
fn test_roze_fmt_file_cli() {
    let bin = roze_bin();
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("unformatted.roze");

    let messy_code = "func add(a:int,b:int)->int{return a+b;}\n";
    fs::write(&file_path, messy_code).unwrap();

    // 1. Run roze fmt --check (should return error code 1 because file needs formatting)
    let status_check = Command::new(&bin)
        .arg("fmt")
        .arg("--check")
        .arg(&file_path)
        .status()
        .expect("Failed to execute roze fmt --check");
    assert!(!status_check.success());

    // 2. Run roze fmt (in-place formatting)
    let status_fmt = Command::new(&bin)
        .arg("fmt")
        .arg(&file_path)
        .status()
        .expect("Failed to execute roze fmt");
    assert!(status_fmt.success());

    // 3. Verify content is formatted
    let formatted_content = fs::read_to_string(&file_path).unwrap();
    assert!(formatted_content.contains("a: int, b: int"));
    assert!(formatted_content.contains("return a + b;"));

    // 4. Run roze fmt --check again (should now succeed with 0)
    let status_check_after = Command::new(&bin)
        .arg("fmt")
        .arg("--check")
        .arg(&file_path)
        .status()
        .expect("Failed to execute roze fmt --check");
    assert!(status_check_after.success());
}

#[test]
fn test_roze_check_and_ssa_cli() {
    let bin = roze_bin();
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("sample.roze");

    let code = r#"
func add(a: int, b: int) -> int {
    return a + b;
}

func main() {
    let sum = add(10, 20);
    println("Sum: " + sum);
}
"#;
    fs::write(&file_path, code).unwrap();

    // Test roze check
    let output_check = Command::new(&bin)
        .arg("check")
        .arg(&file_path)
        .output()
        .expect("Failed to execute roze check");
    assert!(output_check.status.success());
    let stdout_check = String::from_utf8_lossy(&output_check.stdout);
    assert!(stdout_check.contains("Semantic check passed"));

    // Test roze ssa
    let output_ssa = Command::new(&bin)
        .arg("ssa")
        .arg(&file_path)
        .output()
        .expect("Failed to execute roze ssa");
    assert!(output_ssa.status.success());
    let stdout_ssa = String::from_utf8_lossy(&output_ssa.stdout);
    assert!(stdout_ssa.contains("Typed SSA Intermediate Representation"));
}

#[test]
fn test_roze_doc_cli() {
    let bin = roze_bin();
    let temp = tempdir().unwrap();
    let file_path = temp.path().join("documented.roze");
    let out_doc = temp.path().join("doc_out");

    let code = r#"
//! Module level documentation comment for testing

/// Computes the square of a number.
///
/// # Arguments
/// * `n` - The input integer
func square(n: int) -> int {
    return n * n;
}
"#;
    fs::write(&file_path, code).unwrap();

    let output_doc = Command::new(&bin)
        .arg("doc")
        .arg(&file_path)
        .arg("--output")
        .arg(&out_doc)
        .arg("--json")
        .output()
        .expect("Failed to execute roze doc");
    assert!(output_doc.status.success());

    assert!(out_doc.join("index.html").exists());
    assert!(out_doc.join("doc.json").exists());
    assert!(out_doc.join("search-index.js").exists());
    assert!(out_doc.join("rozedoc.css").exists());
}

#[test]
fn test_roze_new_and_test_project_lifecycle() {
    let bin = roze_bin();
    let temp = tempdir().unwrap();
    let project_dir = temp.path().join("unified_app");

    // 1. roze new <project_dir>
    let status_new = Command::new(&bin)
        .arg("new")
        .arg(&project_dir)
        .status()
        .expect("Failed to execute roze new");
    assert!(status_new.success());

    assert!(project_dir.join("bsr.mfz").exists());
    assert!(project_dir.join("src/main.roze").exists());

    // 2. roze test inside project_dir
    let output_test = Command::new(&bin)
        .current_dir(&project_dir)
        .arg("test")
        .output()
        .expect("Failed to execute roze test");
    assert!(output_test.status.success());
    let stdout_test = String::from_utf8_lossy(&output_test.stdout);
    assert!(stdout_test.contains("test result: ok") || stdout_test.contains("All tests passed"));

    // 3. roze clean
    let status_clean = Command::new(&bin)
        .current_dir(&project_dir)
        .arg("clean")
        .status()
        .expect("Failed to execute roze clean");
    assert!(status_clean.success());
}
