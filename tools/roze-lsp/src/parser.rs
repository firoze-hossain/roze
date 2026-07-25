// tools/roze-lsp/src/parser.rs
//
// A thin adapter over the real Roze compiler's lexer + parser
// (roze_compiler::lexer / roze_compiler::parser), replacing what used to
// be a hand-rolled, line-by-line heuristic scanner here: naive
// `split_whitespace()` plus manual brace-counting, no real tokenization
// at all. That old scanner would misparse a `{`/`}` inside a string
// literal, couldn't handle a multi-line function signature, and
// "detected" classes via a bare `line.starts_with("class")` even though
// the real compiler doesn't parse classes at all yet -- so it would
// happily report symbols the compiler could never actually build.
//
// Reusing the real compiler here means every grammar fix made to
// `compiler` (like `for` loops, or fixing `-> ReturnType` parsing) is
// automatically reflected in the editor experience too, instead of
// needing to be hand-ported to a second, separate parser that drifts
// further from reality over time.
use roze_compiler::lexer::tokenize;
use roze_compiler::parser::ast::{Program, Statement};
use roze_compiler::parser::parse as compiler_parse;

#[derive(Debug, Clone)]
pub struct Ast {
    pub functions: Vec<Function>,
    pub variables: Vec<Variable>,
    /// Roze doesn't have classes/structs yet (the real parser tokenizes
    /// `class` but doesn't parse it into anything -- see ROADMAP.md), so
    /// this is always empty. Kept only so callers matching on this shape
    /// don't need to change.
    pub classes: Vec<Class>,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub return_type: Option<String>,
    /// 0-indexed, matching LSP `Position` conventions (the compiler's own
    /// `Location` is 1-indexed, for human-readable error messages).
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub type_: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub fields: Vec<Variable>,
    pub methods: Vec<Function>,
    pub line: usize,
    pub column: usize,
}

/// Parses `source` with the real compiler front-end. Returns `None` if it
/// doesn't even tokenize/parse into a `Program` (e.g. the user is
/// mid-edit and has an unclosed brace right now) -- callers that want the
/// actual syntax error with a position should use `DiagnosticEngine`
/// instead, which surfaces it properly rather than just giving up.
pub fn parse(source: &str) -> Option<Ast> {
    let tokens = tokenize(source);
    let program: Program = compiler_parse(tokens).ok()?;

    let mut ast = Ast {
        functions: Vec::new(),
        variables: Vec::new(),
        classes: Vec::new(),
        imports: Vec::new(),
    };

    collect_statements(&program.statements, &mut ast);
    Some(ast)
}

/// Recursively walks statements (both top-level and nested inside
/// function bodies / if / while / for) collecting functions, variables,
/// and imports as it goes. Functions are only meaningful when found here
/// at any level, since Roze doesn't support nested function declarations
/// today; if that changes, this already handles them correctly since
/// there's no special-casing of "top level" here at all.
fn collect_statements(statements: &[Statement], ast: &mut Ast) {
    for stmt in statements {
        collect_statement(stmt, ast);
    }
}

fn collect_statement(stmt: &Statement, ast: &mut Ast) {
    match stmt {
        Statement::Function { name, params, return_type, body, location } => {
            ast.functions.push(Function {
                name: name.clone(),
                params: params.iter()
                    .map(|p| match &p.type_name {
                        Some(t) => format!("{}: {}", p.name, t),
                        None => p.name.clone(),
                    })
                    .collect(),
                return_type: return_type.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
            });
            // Descend into the body too, so `let`s declared inside a
            // function still show up as variables -- matching the old
            // scanner's behavior, which didn't distinguish scope either
            // (a genuinely scope-aware symbol table is a reasonable next
            // step, not attempted here).
            collect_statement(body, ast);
        }
        Statement::Let { name, location, .. } => {
            ast.variables.push(Variable {
                name: name.clone(),
                type_: None,
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
            });
        }
        Statement::Import { path, .. } => {
            ast.imports.push(path.clone());
        }
        Statement::Block { statements, .. } => collect_statements(statements, ast),
        Statement::If { then_branch, else_branch, .. } => {
            collect_statement(then_branch, ast);
            if let Some(else_stmt) = else_branch {
                collect_statement(else_stmt, ast);
            }
        }
        Statement::While { body, .. } => collect_statement(body, ast),
        Statement::For { init, body, .. } => {
            // The loop's own `let i = 0;` init clause is a real variable
            // declaration too.
            collect_statement(init, ast);
            collect_statement(body, ast);
        }
        Statement::Expression { .. } | Statement::Return { .. } | Statement::Assign { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_top_level_function_with_correct_position() {
        let ast = parse("func add(a: int, b: int) -> int {\n    return a + b;\n}\n").unwrap();
        assert_eq!(ast.functions.len(), 1);
        let f = &ast.functions[0];
        assert_eq!(f.name, "add");
        assert_eq!(f.return_type.as_deref(), Some("int"));
        assert_eq!(f.params, vec!["a: int".to_string(), "b: int".to_string()]);
        // `func` starts at line 1, column 1 in the compiler's 1-indexed
        // Location -- 0-indexed for LSP, so (0, 0).
        assert_eq!(f.line, 0);
        assert_eq!(f.column, 0);
    }

    #[test]
    fn does_not_get_confused_by_braces_inside_strings() {
        // The old line-scanner's brace-counting would miscount here.
        let ast = parse("func f() {\n    println(\"{ not a real brace }\");\n}\n").unwrap();
        assert_eq!(ast.functions.len(), 1);
    }

    #[test]
    fn finds_variables_declared_inside_a_function_body() {
        let ast = parse("func main() {\n    let x = 5;\n    let y = 10;\n}\n").unwrap();
        let names: Vec<&str> = ast.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["x", "y"]);
    }

    #[test]
    fn finds_variables_nested_inside_if_and_while() {
        let ast = parse("func main() {\n    if true {\n        let a = 1;\n    }\n    while true {\n        let b = 2;\n    }\n}\n").unwrap();
        let names: Vec<&str> = ast.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn finds_for_loop_init_variable() {
        let ast = parse("func main() {\n    for let i = 0; i < 3; i = i + 1 {\n        let doubled = i * 2;\n    }\n}\n").unwrap();
        let names: Vec<&str> = ast.variables.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["i", "doubled"]);
    }

    #[test]
    fn collects_import_paths() {
        let ast = parse("import \"core\";\nfunc main() { }\n").unwrap();
        assert_eq!(ast.imports, vec!["core".to_string()]);
    }

    #[test]
    fn classes_are_always_empty_since_the_language_has_none_yet() {
        // Deliberately not testing "class Foo { ... }" text here: unlike
        // the old scanner, this parser doesn't pretend classes parse just
        // because the word "class" appears -- there is currently no
        // Statement::Class in the real AST at all.
        let ast = parse("func main() { }\n").unwrap();
        assert!(ast.classes.is_empty());
    }

    #[test]
    fn unparseable_source_returns_none_rather_than_a_wrong_guess() {
        assert!(parse("func main() {").is_none());
    }
}
