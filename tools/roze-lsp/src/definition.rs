// tools/roze-lsp/src/definition.rs
//
// Go-To-Definition and Find-References Engine for Roze LSP v3.17.
// Navigates precisely to symbol definitions, cross-module imports,
// class/enum declarations, and gathers cross-file references.

use std::path::PathBuf;
use tower_lsp::lsp_types::*;
use crate::Document;
use crate::Workspace;
use roze_compiler::imports::resolve_modular_package;

pub struct DefinitionEngine;

impl DefinitionEngine {
    pub fn get_definition(
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Option<Location> {
        let line = position.line as usize;
        let char_idx = position.character as usize;

        if line >= doc.text.len_lines() {
            return None;
        }

        let line_text = doc.text.line(line).to_string();

        // 1. Check if on an import statement: e.g. import "std::json"; or import "rozeboot";
        if line_text.trim_start().starts_with("import ") {
            if let Some(import_target) = extract_import_string(&line_text) {
                if let Some(target_path) = resolve_import_path(&import_target, &doc.uri, &workspace.get_root()) {
                    if let Ok(target_url) = Url::from_file_path(target_path) {
                        return Some(Location {
                            uri: target_url,
                            range: Range {
                                start: Position { line: 0, character: 0 },
                                end: Position { line: 0, character: 0 },
                            },
                        });
                    }
                }
            }
        }

        let word = find_word_at(&line_text, char_idx)?;

        // 2. Search local variables and function parameters in current document
        if let Some(ast) = &doc.ast {
            // In-scope variables
            if let Some(v) = ast
                .variables
                .iter()
                .find(|v| v.name == word && line >= v.scope_start && line <= v.scope_end)
            {
                return Some(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: v.line as u32,
                            character: v.column as u32,
                        },
                        end: Position {
                            line: v.line as u32,
                            character: (v.column + v.name.len()) as u32,
                        },
                    },
                });
            }

            // Functions in current file
            if let Some(f) = ast.functions.iter().find(|f| f.name == word) {
                return Some(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: f.line as u32,
                            character: f.column as u32,
                        },
                        end: Position {
                            line: f.line as u32,
                            character: (f.column + f.name.len()) as u32,
                        },
                    },
                });
            }

            // Classes in current file
            if let Some(c) = ast.classes.iter().find(|c| c.name == word) {
                return Some(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: c.line as u32,
                            character: c.column as u32,
                        },
                        end: Position {
                            line: c.line as u32,
                            character: (c.column + c.name.len()) as u32,
                        },
                    },
                });
            }

            // Enums in current file
            if let Some(e) = ast.enums.iter().find(|e| e.name == word) {
                return Some(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: e.line as u32,
                            character: e.column as u32,
                        },
                        end: Position {
                            line: e.line as u32,
                            character: (e.column + e.name.len()) as u32,
                        },
                    },
                });
            }

            // Enum variants in current file
            for e in &ast.enums {
                if let Some(v) = e.variants.iter().find(|v| v.name == word) {
                    return Some(Location {
                        uri: doc.uri.clone(),
                        range: Range {
                            start: Position {
                                line: v.line as u32,
                                character: v.column as u32,
                            },
                            end: Position {
                                line: v.line as u32,
                                character: (v.column + v.name.len()) as u32,
                            },
                        },
                    });
                }
            }

            // Traits in current file
            if let Some(tr) = ast.traits.iter().find(|tr| tr.name == word) {
                return Some(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: tr.line as u32,
                            character: tr.column as u32,
                        },
                        end: Position {
                            line: tr.line as u32,
                            character: (tr.column + tr.name.len()) as u32,
                        },
                    },
                });
            }
        }

        // 3. Search other workspace documents
        for entry in workspace.documents.iter() {
            let other_doc = entry.value();
            if other_doc.uri == doc.uri {
                continue;
            }

            if let Some(ast) = &other_doc.ast {
                if let Some(f) = ast.functions.iter().find(|f| f.name == word) {
                    return Some(Location {
                        uri: other_doc.uri.clone(),
                        range: Range {
                            start: Position {
                                line: f.line as u32,
                                character: f.column as u32,
                            },
                            end: Position {
                                line: f.line as u32,
                                character: (f.column + f.name.len()) as u32,
                            },
                        },
                    });
                }

                if let Some(c) = ast.classes.iter().find(|c| c.name == word) {
                    return Some(Location {
                        uri: other_doc.uri.clone(),
                        range: Range {
                            start: Position {
                                line: c.line as u32,
                                character: c.column as u32,
                            },
                            end: Position {
                                line: c.line as u32,
                                character: (c.column + c.name.len()) as u32,
                            },
                        },
                    });
                }

                if let Some(e) = ast.enums.iter().find(|e| e.name == word) {
                    return Some(Location {
                        uri: other_doc.uri.clone(),
                        range: Range {
                            start: Position {
                                line: e.line as u32,
                                character: e.column as u32,
                            },
                            end: Position {
                                line: e.line as u32,
                                character: (e.column + e.name.len()) as u32,
                            },
                        },
                    });
                }
            }
        }

        None
    }

    pub fn get_references(
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Vec<Location> {
        let mut refs = Vec::new();

        let line = position.line as usize;
        let char_idx = position.character as usize;

        if line >= doc.text.len_lines() {
            return refs;
        }

        let line_text = doc.text.line(line).to_string();
        let word = match find_word_at(&line_text, char_idx) {
            Some(w) => w,
            None => return refs,
        };

        // Collect references from current document
        collect_doc_references(doc, word, &mut refs);

        // Collect references across all open workspace documents
        for entry in workspace.documents.iter() {
            let other_doc = entry.value();
            if other_doc.uri != doc.uri {
                collect_doc_references(other_doc, word, &mut refs);
            }
        }

        refs
    }
}

fn collect_doc_references(doc: &Document, word: &str, refs: &mut Vec<Location>) {
    if let Some(ast) = &doc.ast {
        // Definition references
        for f in &ast.functions {
            if f.name == word {
                refs.push(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: f.line as u32,
                            character: f.column as u32,
                        },
                        end: Position {
                            line: f.line as u32,
                            character: (f.column + f.name.len()) as u32,
                        },
                    },
                });
            }
        }

        for c in &ast.classes {
            if c.name == word {
                refs.push(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: c.line as u32,
                            character: c.column as u32,
                        },
                        end: Position {
                            line: c.line as u32,
                            character: (c.column + c.name.len()) as u32,
                        },
                    },
                });
            }
            for field in &c.fields {
                if field.name == word {
                    refs.push(Location {
                        uri: doc.uri.clone(),
                        range: Range {
                            start: Position {
                                line: field.line as u32,
                                character: field.column as u32,
                            },
                            end: Position {
                                line: field.line as u32,
                                character: (field.column + field.name.len()) as u32,
                            },
                        },
                    });
                }
            }
        }

        for e in &ast.enums {
            if e.name == word {
                refs.push(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: e.line as u32,
                            character: e.column as u32,
                        },
                        end: Position {
                            line: e.line as u32,
                            character: (e.column + e.name.len()) as u32,
                        },
                    },
                });
            }
        }

        for v in &ast.variables {
            if v.name == word {
                refs.push(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: v.line as u32,
                            character: v.column as u32,
                        },
                        end: Position {
                            line: v.line as u32,
                            character: (v.column + v.name.len()) as u32,
                        },
                    },
                });
            }
        }

        // Usage references
        for u in &ast.usages {
            if u.name == word {
                refs.push(Location {
                    uri: doc.uri.clone(),
                    range: Range {
                        start: Position {
                            line: u.line as u32,
                            character: u.column as u32,
                        },
                        end: Position {
                            line: u.line as u32,
                            character: (u.column + u.length) as u32,
                        },
                    },
                });
            }
        }
    }
}

fn extract_import_string(line: &str) -> Option<String> {
    let start_idx = line.find('"')?;
    let end_idx = line[start_idx + 1..].find('"')?;
    Some(line[start_idx + 1..start_idx + 1 + end_idx].to_string())
}

fn resolve_import_path(target: &str, doc_uri: &Url, _workspace_root: &PathBuf) -> Option<PathBuf> {
    let base_dir = doc_uri
        .to_file_path()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Direct file relative import: e.g. "ast.roze" or "./parser.roze"
    let direct = base_dir.join(target);
    if direct.is_file() {
        return Some(direct);
    }
    let direct_roze = base_dir.join(format!("{}.roze", target));
    if direct_roze.is_file() {
        return Some(direct_roze);
    }

    // 2. Modular package resolution (packages/<target>/src/lib.roze)
    if let Some((_, resolved_path)) = resolve_modular_package(target, &base_dir) {
        let p = PathBuf::from(resolved_path);
        if p.is_file() {
            return Some(p);
        }
    }

    None
}

fn find_word_at(line: &str, char_idx: usize) -> Option<&str> {
    let chars: Vec<char> = line.chars().collect();
    if char_idx >= chars.len() {
        return None;
    }

    let mut start = char_idx;
    let mut end = char_idx;

    while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
        start -= 1;
    }
    while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
        end += 1;
    }

    if start < end {
        let byte_start = chars[..start].iter().map(|c| c.len_utf8()).sum();
        let byte_end = chars[..end].iter().map(|c| c.len_utf8()).sum();
        Some(&line[byte_start..byte_end])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn definition_finds_local_function() {
        let src = "func compute(x: int) -> int { return x; }\nfunc main() { compute(10); }";
        let doc = Document {
            uri: Url::parse("file:///main.roze").unwrap(),
            text: Rope::from_str(src),
            version: 1,
            ast: crate::parser::parse(src),
        };
        let ws = Workspace::new(PathBuf::from("."));
        ws.open_document(doc.uri.clone(), src.to_string(), 1);

        let loc = DefinitionEngine::get_definition(&doc, Position { line: 1, character: 16 }, &ws).unwrap();
        assert_eq!(loc.range.start.line, 0);
        assert_eq!(loc.range.start.character, 0);
    }

    #[test]
    fn references_finds_all_occurrences() {
        let src = "func foo() {}\nfunc main() { foo(); foo(); }";
        let doc = Document {
            uri: Url::parse("file:///main.roze").unwrap(),
            text: Rope::from_str(src),
            version: 1,
            ast: crate::parser::parse(src),
        };
        let ws = Workspace::new(PathBuf::from("."));

        let refs = DefinitionEngine::get_references(&doc, Position { line: 0, character: 6 }, &ws);
        assert!(refs.len() >= 3); // 1 declaration + 2 calls
    }
}
