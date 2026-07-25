// tools/roze-lsp/src/diagnostics.rs
//
// Real diagnostics, backed by the actual compiler pipeline (tokenize ->
// parse -> resolve imports -> type-check), instead of the previous
// per-line heuristics: "does this line end with '='?" (false-positives
// on any multi-line expression), "are parens balanced on this line?"
// (false-positives on any multi-line call), and a trailing-whitespace
// style nit that isn't a Roze error at all. Those heuristics couldn't
// catch a single *real* Roze error (an undefined variable, a return-type
// mismatch, an actual syntax error) and would flag plenty of valid code.
//
// Known limitation: the compiler pipeline stops at the first error it
// hits, so this surfaces at most one diagnostic per check rather than
// every problem in the file at once. That's still a strict improvement
// over the old heuristics (which produced several diagnostics, but ones
// that were frequently either wrong or not real Roze errors at all) --
// collecting multiple errors per pass would mean changing the compiler's
// own fail-fast design, which is a larger change than this LSP crate
// should make on its own.
use roze_compiler::error::RozeError;
use roze_compiler::imports::resolve_imports;
use roze_compiler::lexer::tokenize;
use roze_compiler::parser::parse;
use roze_compiler::semantic::check_types;
use std::path::Path;
use tower_lsp::lsp_types::*;

#[derive(Debug, Clone)]
pub struct DiagnosticEngine;

impl DiagnosticEngine {
    pub fn new() -> Self {
        Self
    }

    /// Type-checks `source` (a file whose directory is `base_dir`, used
    /// to resolve any `import "...";` statements) and returns at most one
    /// diagnostic: the first error the real compiler pipeline hits, if
    /// any.
    pub fn check(&self, source: &str, base_dir: &Path) -> Vec<Diagnostic> {
        let tokens = tokenize(source);

        let program = match parse(tokens) {
            Ok(p) => p,
            Err(e) => return vec![to_diagnostic(&e)],
        };

        let program = match resolve_imports(program, base_dir) {
            Ok(p) => p,
            Err(e) => return vec![to_diagnostic(&e)],
        };

        if let Err(e) = check_types(&program) {
            return vec![to_diagnostic(&e)];
        }

        Vec::new()
    }
}

/// Converts a compiler error into an LSP `Diagnostic`. `RozeError`s carry
/// real line/column/length, which map directly onto an LSP `Range`
/// (adjusting for the compiler's 1-indexed positions vs. LSP's 0-indexed
/// ones). Anything else (e.g. an `AlreadyReported` sentinel from a
/// failed import, which prints its own detail to the server's log rather
/// than through this path) gets a generic single-point diagnostic so a
/// failure is never silently swallowed.
fn to_diagnostic(err: &anyhow::Error) -> Diagnostic {
    if let Some(roze_err) = err.downcast_ref::<RozeError>() {
        let line = roze_err.line.saturating_sub(1) as u32;
        let start_col = roze_err.column.saturating_sub(1) as u32;
        let end_col = start_col + roze_err.length.max(1) as u32;

        Diagnostic {
            range: Range {
                start: Position { line, character: start_col },
                end: Position { line, character: end_col },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            source: Some("roze".to_string()),
            message: match &roze_err.hint {
                Some(hint) => format!("{}\nhelp: {}", roze_err.message, hint),
                None => roze_err.message.clone(),
            },
            ..Default::default()
        }
    } else {
        Diagnostic {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 1 },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("roze".to_string()),
            message: err.to_string(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn valid_program_has_no_diagnostics() {
        let engine = DiagnosticEngine::new();
        let diags = engine.check("func main() { println(\"hi\"); }", Path::new("."));
        assert!(diags.is_empty());
    }

    #[test]
    fn syntax_error_is_reported_with_a_real_position() {
        let engine = DiagnosticEngine::new();
        let diags = engine.check("func main() {\n    println(\"hi\")\n", Path::new("."));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::ERROR));
    }

    #[test]
    fn type_error_is_reported() {
        let engine = DiagnosticEngine::new();
        let diags = engine.check(
            "func add(a: int, b: int) -> int {\n    return \"nope\";\n}\nfunc main() { }\n",
            Path::new("."),
        );
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("return"), "expected a return-type message, got: {}", diags[0].message);
    }

    #[test]
    fn does_not_false_positive_on_valid_multiline_call() {
        // The old per-line paren-balance heuristic would have flagged
        // this as "unbalanced parentheses" on more than one line.
        let engine = DiagnosticEngine::new();
        let source = "func main() {\n    println(\n        \"hi\"\n    );\n}\n";
        let diags = engine.check(source, Path::new("."));
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", diags);
    }

    #[test]
    fn does_not_false_positive_on_trailing_operator_across_lines() {
        // The old heuristic flagged any line ending in '+'/'-'/'=' as a
        // "missing semicolon," even when it's a perfectly normal
        // multi-line expression continuation... which Roze doesn't even
        // support, but the point is the new engine judges by real syntax,
        // not by guessing from the last character of a line.
        let engine = DiagnosticEngine::new();
        let diags = engine.check("func main() {\n    let x = 1 + 2;\n}\n", Path::new("."));
        assert!(diags.is_empty());
    }
}
