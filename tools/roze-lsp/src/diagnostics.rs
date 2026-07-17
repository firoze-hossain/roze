// src/diagnostics.rs
use tower_lsp::lsp_types::*;

#[derive(Debug, Clone)]
pub struct DiagnosticEngine;

impl DiagnosticEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn check(&self, source: &str) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let lines: Vec<&str> = source.lines().collect();

        for (line_num, line) in lines.iter().enumerate() {
            // Check for missing semicolons
            let trimmed = line.trim();
            if trimmed.ends_with('=') || trimmed.ends_with('+') || trimmed.ends_with('-') {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line: line_num as u32, character: 0 },
                        end: Position { line: line_num as u32, character: line.len() as u32 },
                    },
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("missing-semicolon".to_string())),
                    source: Some("roze".to_string()),
                    message: "Missing semicolon at end of line".to_string(),
                    ..Default::default()
                });
            }

            // Check for unbalanced parentheses
            let open_parens = line.chars().filter(|&c| c == '(').count();
            let close_parens = line.chars().filter(|&c| c == ')').count();
            if open_parens != close_parens {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line: line_num as u32, character: 0 },
                        end: Position { line: line_num as u32, character: line.len() as u32 },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("unbalanced-parens".to_string())),
                    source: Some("roze".to_string()),
                    message: "Unbalanced parentheses".to_string(),
                    ..Default::default()
                });
            }

            // Check for trailing whitespace
            if line.ends_with(' ') {
                diagnostics.push(Diagnostic {
                    range: Range {
                        start: Position { line: line_num as u32, character: line.len() as u32 - 1 },
                        end: Position { line: line_num as u32, character: line.len() as u32 },
                    },
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("trailing-whitespace".to_string())),
                    source: Some("roze".to_string()),
                    message: "Trailing whitespace".to_string(),
                    ..Default::default()
                });
            }
        }

        diagnostics
    }
}