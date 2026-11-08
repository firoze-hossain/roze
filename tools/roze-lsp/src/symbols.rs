// tools/roze-lsp/src/symbols.rs
//
// Hierarchical Document Symbols Engine for Roze LSP v3.17.
// Constructs rich symbol outline trees with nested classes, fields,
// methods, enums, variants, and functions.

use tower_lsp::lsp_types::*;
use crate::Document;

pub struct SymbolsEngine;

impl SymbolsEngine {
    #[allow(deprecated)]
    pub fn get_document_symbols(doc: &Document) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();

        if let Some(ast) = &doc.ast {
            // 1. Classes with nested fields and methods
            for c in &ast.classes {
                let mut children = Vec::new();

                for f in &c.fields {
                    children.push(DocumentSymbol {
                        name: f.name.clone(),
                        detail: f.type_name.clone(),
                        kind: SymbolKind::FIELD,
                        tags: None,
                        deprecated: None,
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
                        selection_range: Range {
                            start: Position {
                                line: f.line as u32,
                                character: f.column as u32,
                            },
                            end: Position {
                                line: f.line as u32,
                                character: (f.column + f.name.len()) as u32,
                            },
                        },
                        children: None,
                    });
                }

                for m in &c.methods {
                    let ret = m.return_type.as_deref().unwrap_or("void");
                    children.push(DocumentSymbol {
                        name: m.name.clone(),
                        detail: Some(format!("func({}) -> {}", m.params.join(", "), ret)),
                        kind: SymbolKind::METHOD,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: m.line as u32,
                                character: m.column as u32,
                            },
                            end: Position {
                                line: m.end_line as u32,
                                character: m.end_column as u32,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: m.line as u32,
                                character: m.column as u32,
                            },
                            end: Position {
                                line: m.line as u32,
                                character: (m.column + m.name.len()) as u32,
                            },
                        },
                        children: None,
                    });
                }

                symbols.push(DocumentSymbol {
                    name: c.name.clone(),
                    detail: Some("class".to_string()),
                    kind: SymbolKind::CLASS,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: c.line as u32,
                            character: c.column as u32,
                        },
                        end: Position {
                            line: c.end_line as u32,
                            character: c.end_column as u32,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: c.line as u32,
                            character: c.column as u32,
                        },
                        end: Position {
                            line: c.line as u32,
                            character: (c.column + c.name.len()) as u32,
                        },
                    },
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }

            // 2. Enums with nested variants
            for e in &ast.enums {
                let mut children = Vec::new();

                for v in &e.variants {
                    let detail = if v.types.is_empty() {
                        None
                    } else {
                        Some(format!("({})", v.types.join(", ")))
                    };
                    children.push(DocumentSymbol {
                        name: v.name.clone(),
                        detail,
                        kind: SymbolKind::ENUM_MEMBER,
                        tags: None,
                        deprecated: None,
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
                        selection_range: Range {
                            start: Position {
                                line: v.line as u32,
                                character: v.column as u32,
                            },
                            end: Position {
                                line: v.line as u32,
                                character: (v.column + v.name.len()) as u32,
                            },
                        },
                        children: None,
                    });
                }

                symbols.push(DocumentSymbol {
                    name: e.name.clone(),
                    detail: Some("enum".to_string()),
                    kind: SymbolKind::ENUM,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: e.line as u32,
                            character: e.column as u32,
                        },
                        end: Position {
                            line: e.end_line as u32,
                            character: e.end_column as u32,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: e.line as u32,
                            character: e.column as u32,
                        },
                        end: Position {
                            line: e.line as u32,
                            character: (e.column + e.name.len()) as u32,
                        },
                    },
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }

            // 3. Traits with methods
            for tr in &ast.traits {
                let mut children = Vec::new();
                for m in &tr.methods {
                    let ret = m.return_type.as_deref().unwrap_or("void");
                    children.push(DocumentSymbol {
                        name: m.name.clone(),
                        detail: Some(format!("func({}) -> {}", m.params.join(", "), ret)),
                        kind: SymbolKind::METHOD,
                        tags: None,
                        deprecated: None,
                        range: Range {
                            start: Position {
                                line: m.line as u32,
                                character: m.column as u32,
                            },
                            end: Position {
                                line: m.end_line as u32,
                                character: m.end_column as u32,
                            },
                        },
                        selection_range: Range {
                            start: Position {
                                line: m.line as u32,
                                character: m.column as u32,
                            },
                            end: Position {
                                line: m.line as u32,
                                character: (m.column + m.name.len()) as u32,
                            },
                        },
                        children: None,
                    });
                }

                symbols.push(DocumentSymbol {
                    name: tr.name.clone(),
                    detail: Some("trait".to_string()),
                    kind: SymbolKind::INTERFACE,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: tr.line as u32,
                            character: tr.column as u32,
                        },
                        end: Position {
                            line: tr.end_line as u32,
                            character: tr.end_column as u32,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: tr.line as u32,
                            character: tr.column as u32,
                        },
                        end: Position {
                            line: tr.line as u32,
                            character: (tr.column + tr.name.len()) as u32,
                        },
                    },
                    children: if children.is_empty() { None } else { Some(children) },
                });
            }

            // 4. Top-level functions
            for f in &ast.functions {
                let ret = f.return_type.as_deref().unwrap_or("void");
                let async_tag = if f.is_async { "async " } else { "" };
                symbols.push(DocumentSymbol {
                    name: f.name.clone(),
                    detail: Some(format!("{}func({}) -> {}", async_tag, f.params.join(", "), ret)),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range: Range {
                        start: Position {
                            line: f.line as u32,
                            character: f.column as u32,
                        },
                        end: Position {
                            line: f.end_line as u32,
                            character: f.end_column as u32,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: f.line as u32,
                            character: f.column as u32,
                        },
                        end: Position {
                            line: f.line as u32,
                            character: (f.column + f.name.len()) as u32,
                        },
                    },
                    children: None,
                });
            }
        }

        symbols
    }
}
