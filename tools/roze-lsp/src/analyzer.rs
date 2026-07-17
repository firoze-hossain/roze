// src/analyzer.rs - FINAL FIXED VERSION
use tower_lsp::lsp_types::*;
use crate::Document;

#[derive(Debug, Clone)]
pub struct Analyzer;

impl Analyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn get_hover_info(&self, doc: &Document, position: Position) -> Option<String> {
        let line = position.line as usize;
        let char = position.character as usize;

        // Rope's line() returns a RopeSlice, not Option
        // We need to check if the line exists by comparing with line count
        if line >= doc.text.len_lines() {
            return None;
        }

        let line_text = doc.text.line(line).to_string();
        let chars: Vec<char> = line_text.chars().collect();

        if char >= chars.len() {
            return None;
        }

        // Find word at position
        let mut start = char;
        let mut end = char;

        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        if start < end {
            let word = &line_text[start..end];
            return Some(format!(
                "```roze\n{}\n```\n\n**{}**\n\nSymbol found in Roze program",
                word, word
            ));
        }

        None
    }

    pub fn get_completions(&self, _doc: &Document, _position: Position) -> Vec<CompletionItem> {
        let mut completions = Vec::new();

        let keywords = vec![
            "func", "let", "mut", "return", "if", "else", "for", "while",
            "class", "interface", "import", "true", "false", "null",
            "int", "float", "string", "bool", "void", "match", "try", "catch",
        ];

        for keyword in keywords {
            completions.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Roze keyword".to_string()),
                ..Default::default()
            });
        }

        let builtins = vec!["println", "print", "read_line", "assert", "panic"];

        for builtin in builtins {
            completions.push(CompletionItem {
                label: builtin.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("Built-in function".to_string()),
                ..Default::default()
            });
        }

        completions
    }

    pub fn get_definition(&self, doc: &Document, _position: Position) -> Option<Location> {
        Some(Location {
            uri: doc.uri.clone(),
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position { line: 0, character: 10 },
            },
        })
    }

    pub fn get_references(&self, _doc: &Document, _position: Position) -> Vec<Location> {
        Vec::new()
    }

    pub fn get_document_symbols(&self, doc: &Document) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        if let Some(ast) = &doc.ast {
            for func in &ast.functions {
                symbols.push(DocumentSymbol {
                    name: func.name.clone(),
                    kind: SymbolKind::FUNCTION,
                    range: Range {
                        start: Position { line: func.line as u32, character: 0 },
                        end: Position { line: func.line as u32 + 5, character: 0 },
                    },
                    selection_range: Range {
                        start: Position { line: func.line as u32, character: 0 },
                        end: Position { line: func.line as u32, character: 100 },
                    },
                    children: None,
                    tags: None,
                    detail: None,
                    deprecated: None,
                });
            }

            for class in &ast.classes {
                symbols.push(DocumentSymbol {
                    name: class.name.clone(),
                    kind: SymbolKind::CLASS,
                    range: Range {
                        start: Position { line: class.line as u32, character: 0 },
                        end: Position { line: class.line as u32 + 10, character: 0 },
                    },
                    selection_range: Range {
                        start: Position { line: class.line as u32, character: 0 },
                        end: Position { line: class.line as u32, character: 100 },
                    },
                    children: None,
                    tags: None,
                    detail: None,
                    deprecated: None,
                });
            }
        }

        symbols
    }
}