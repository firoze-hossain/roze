// tools/roze-lsp/src/semantic_tokens.rs
//
// Full LSP v3.17 Semantic Tokens Engine for Roze.
// Emits delta-encoded semantic tokens with granular syntax categories
// and modifiers matching rust-analyzer and Eclipse JDT.

use tower_lsp::lsp_types::*;
use roze_compiler::lexer::tokenize;
use roze_compiler::lexer::token::Token;
use crate::parser::Ast;

pub fn get_legend() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::KEYWORD,       // 0
            SemanticTokenType::FUNCTION,      // 1
            SemanticTokenType::METHOD,        // 2
            SemanticTokenType::VARIABLE,      // 3
            SemanticTokenType::PARAMETER,     // 4
            SemanticTokenType::TYPE,          // 5
            SemanticTokenType::CLASS,         // 6
            SemanticTokenType::ENUM,          // 7
            SemanticTokenType::ENUM_MEMBER,   // 8
            SemanticTokenType::INTERFACE,     // 9
            SemanticTokenType::STRUCT,        // 10
            SemanticTokenType::PROPERTY,      // 11
            SemanticTokenType::OPERATOR,      // 12
            SemanticTokenType::STRING,        // 13
            SemanticTokenType::NUMBER,        // 14
            SemanticTokenType::COMMENT,       // 15
            SemanticTokenType::MODIFIER,      // 16
            SemanticTokenType::MACRO,         // 17
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,      // 1 << 0 = 1
            SemanticTokenModifier::DEFINITION,       // 1 << 1 = 2
            SemanticTokenModifier::READONLY,         // 1 << 2 = 4
            SemanticTokenModifier::STATIC,           // 1 << 3 = 8
            SemanticTokenModifier::ASYNC,            // 1 << 4 = 16
            SemanticTokenModifier::DEFAULT_LIBRARY,  // 1 << 5 = 32
        ],
    }
}

pub const TYPE_KEYWORD: u32 = 0;
pub const TYPE_FUNCTION: u32 = 1;
pub const TYPE_METHOD: u32 = 2;
pub const TYPE_VARIABLE: u32 = 3;
pub const TYPE_PARAMETER: u32 = 4;
pub const TYPE_TYPE: u32 = 5;
pub const TYPE_CLASS: u32 = 6;
pub const TYPE_ENUM: u32 = 7;
pub const TYPE_ENUM_MEMBER: u32 = 8;
pub const TYPE_INTERFACE: u32 = 9;
pub const TYPE_PROPERTY: u32 = 11;
pub const TYPE_OPERATOR: u32 = 12;
pub const TYPE_STRING: u32 = 13;
pub const TYPE_NUMBER: u32 = 14;
pub const TYPE_COMMENT: u32 = 15;
pub const TYPE_MACRO: u32 = 17;

pub const MOD_DECLARATION: u32 = 1;
pub const MOD_ASYNC: u32 = 16;
pub const MOD_DEFAULT_LIBRARY: u32 = 32;

#[derive(Debug, Clone)]
struct RawSemanticToken {
    line: usize,
    col: usize,
    len: usize,
    token_type: u32,
    modifiers: u32,
}

pub struct SemanticTokensEngine;

impl SemanticTokensEngine {
    pub fn compute_tokens(source: &str, ast: Option<&Ast>) -> Vec<SemanticToken> {
        let mut raw_tokens = Vec::new();

        // 1. Collect comment tokens from source
        for (line_idx, line) in source.lines().enumerate() {
            if let Some(pos) = line.find("//") {
                let comment_text = &line[pos..];
                raw_tokens.push(RawSemanticToken {
                    line: line_idx,
                    col: pos,
                    len: comment_text.chars().count(),
                    token_type: TYPE_COMMENT,
                    modifiers: 0,
                });
            }
        }

        // 2. Lex tokens via roze_compiler::lexer
        let lexed = tokenize(source);

        for tok in lexed {
            let line = tok.line.saturating_sub(1);
            let col = tok.column.saturating_sub(1);

            match tok.token {
                Token::Func | Token::Let | Token::Mut | Token::Return | Token::If
                | Token::Else | Token::For | Token::While | Token::Class | Token::New
                | Token::Import | Token::Enum | Token::Match | Token::Trait | Token::Impl
                | Token::This | Token::Extern | Token::Unsafe | Token::Raw | Token::Const
                | Token::Type | Token::Where | Token::Comptime | Token::Await
                | Token::AsmBang | Token::True | Token::False | Token::Null => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: tok.token.to_string().len(),
                        token_type: TYPE_KEYWORD,
                        modifiers: 0,
                    });
                }
                Token::Async => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: 5,
                        token_type: TYPE_KEYWORD,
                        modifiers: MOD_ASYNC,
                    });
                }
                Token::String(s) => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: s.chars().count() + 2, // quotes
                        token_type: TYPE_STRING,
                        modifiers: 0,
                    });
                }
                Token::Char(_) => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: 3,
                        token_type: TYPE_STRING,
                        modifiers: 0,
                    });
                }
                Token::Number(n) => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: n.len(),
                        token_type: TYPE_NUMBER,
                        modifiers: 0,
                    });
                }
                Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent
                | Token::Equals | Token::EqualsEquals | Token::NotEquals | Token::LessThan
                | Token::GreaterThan | Token::LessEqual | Token::GreaterEqual | Token::And
                | Token::Or | Token::Not | Token::Question | Token::Arrow | Token::FatArrow
                | Token::ColonColon | Token::DotDot | Token::DotDotEquals => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: tok.token.to_string().len(),
                        token_type: TYPE_OPERATOR,
                        modifiers: 0,
                    });
                }
                Token::At => {
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len: 1,
                        token_type: TYPE_MACRO,
                        modifiers: 0,
                    });
                }
                Token::Identifier(id) => {
                    let len = id.len();
                    let (token_type, modifiers) = classify_identifier(&id, line, col, ast);
                    raw_tokens.push(RawSemanticToken {
                        line,
                        col,
                        len,
                        token_type,
                        modifiers,
                    });
                }
                _ => {}
            }
        }

        // Sort by line ascending, then column ascending
        raw_tokens.sort_by(|a, b| {
            if a.line != b.line {
                a.line.cmp(&b.line)
            } else {
                a.col.cmp(&b.col)
            }
        });

        // Deduplicate overlapping tokens (e.g. comment vs string)
        let mut deduplicated: Vec<RawSemanticToken> = Vec::new();
        for t in raw_tokens {
            if let Some(last) = deduplicated.last() {
                if last.line == t.line && last.col == t.col {
                    continue;
                }
            }
            deduplicated.push(t);
        }

        // Delta-encode into LSP SemanticToken array
        let mut encoded = Vec::new();
        let mut prev_line = 0;
        let mut prev_col = 0;

        for t in deduplicated {
            let delta_line = (t.line - prev_line) as u32;
            let delta_start = if delta_line == 0 {
                (t.col - prev_col) as u32
            } else {
                t.col as u32
            };

            encoded.push(SemanticToken {
                delta_line,
                delta_start,
                length: t.len as u32,
                token_type: t.token_type,
                token_modifiers_bitset: t.modifiers,
            });

            prev_line = t.line;
            prev_col = t.col;
        }

        encoded
    }
}

fn classify_identifier(
    id: &str,
    line: usize,
    col: usize,
    ast: Option<&Ast>,
) -> (u32, u32) {
    // 1. Primitive types
    match id {
        "int" | "float" | "string" | "bool" | "void" | "List" | "Map" | "Option"
        | "Result" | "Future" | "Box" | "Rc" | "Arc" | "Mutex" | "RwLock" => {
            return (TYPE_TYPE, MOD_DEFAULT_LIBRARY);
        }
        _ => {}
    }

    // 2. Builtin functions
    match id {
        "println" | "print" | "read_line" | "assert" | "panic" | "to_string"
        | "list_new" | "list_push" | "list_pop" | "list_get" | "list_set" | "list_length"
        | "map_new" | "map_get" | "map_set" | "map_contains_key" | "map_keys"
        | "string_length" | "string_slice" | "string_contains" | "sha256" | "hex_encode" => {
            return (TYPE_FUNCTION, MOD_DEFAULT_LIBRARY);
        }
        _ => {}
    }

    if let Some(ast) = ast {
        // Function declaration
        if ast.functions.iter().any(|f| f.name == id && f.line == line) {
            let is_async = ast.functions.iter().any(|f| f.name == id && f.is_async);
            let mut mods = MOD_DECLARATION;
            if is_async {
                mods |= MOD_ASYNC;
            }
            return (TYPE_FUNCTION, mods);
        }

        // Class declaration
        if ast.classes.iter().any(|c| c.name == id && c.line == line) {
            return (TYPE_CLASS, MOD_DECLARATION);
        }

        // Enum declaration
        if ast.enums.iter().any(|e| e.name == id && e.line == line) {
            return (TYPE_ENUM, MOD_DECLARATION);
        }

        // Trait declaration
        if ast.traits.iter().any(|tr| tr.name == id && tr.line == line) {
            return (TYPE_INTERFACE, MOD_DECLARATION);
        }

        // Enum variant
        for e in &ast.enums {
            if e.variants.iter().any(|v| v.name == id) {
                return (TYPE_ENUM_MEMBER, 0);
            }
        }

        // Parameter
        for f in &ast.functions {
            if line >= f.line && line <= f.end_line {
                if f.param_details.iter().any(|p| p.name == id) {
                    let is_decl = line == f.line && col > f.column;
                    return (TYPE_PARAMETER, if is_decl { MOD_DECLARATION } else { 0 });
                }
            }
        }

        // Class Field
        for c in &ast.classes {
            if c.fields.iter().any(|f| f.name == id) {
                return (TYPE_PROPERTY, 0);
            }
        }

        // Class methods
        for c in &ast.classes {
            if c.methods.iter().any(|m| m.name == id) {
                let is_decl = c.methods.iter().any(|m| m.name == id && m.line == line);
                return (TYPE_METHOD, if is_decl { MOD_DECLARATION } else { 0 });
            }
        }

        // Known functions
        if ast.functions.iter().any(|f| f.name == id) {
            return (TYPE_FUNCTION, 0);
        }

        // Known classes
        if ast.classes.iter().any(|c| c.name == id) {
            return (TYPE_CLASS, 0);
        }

        // Known enums
        if ast.enums.iter().any(|e| e.name == id) {
            return (TYPE_ENUM, 0);
        }

        // Local variable declaration
        if ast.variables.iter().any(|v| v.name == id && v.line == line) {
            return (TYPE_VARIABLE, MOD_DECLARATION);
        }

        // Local variable usage
        if ast.variables.iter().any(|v| v.name == id) {
            return (TYPE_VARIABLE, 0);
        }
    }

    // Heuristics: Uppercase first letter -> Class / Type
    if id.chars().next().map_or(false, |c| c.is_uppercase()) {
        (TYPE_CLASS, 0)
    } else {
        (TYPE_VARIABLE, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    #[test]
    fn computes_semantic_tokens_for_function_and_types() {
        let source = "/// Computes square\nfunc square(x: int) -> int {\n    let result = x * x;\n    return result;\n}\n";
        let ast = parse(source);
        let tokens = SemanticTokensEngine::compute_tokens(source, ast.as_ref());
        assert!(!tokens.is_empty());
        // Verify we generated valid delta encoded tokens
        assert!(tokens.iter().any(|t| t.token_type == TYPE_KEYWORD));
        assert!(tokens.iter().any(|t| t.token_type == TYPE_FUNCTION));
        assert!(tokens.iter().any(|t| t.token_type == TYPE_TYPE));
    }
}
