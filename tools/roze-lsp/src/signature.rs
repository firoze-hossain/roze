// tools/roze-lsp/src/signature.rs
//
// Signature Help Engine for Roze LSP v3.17.
// Shows real-time function signature popups with active parameter
// highlighting and parameter documentation while typing.

use tower_lsp::lsp_types::*;
use crate::Document;
use crate::Workspace;

pub struct SignatureHelpEngine;

impl SignatureHelpEngine {
    pub fn get_signature_help(
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Option<SignatureHelp> {
        let line = position.line as usize;
        let char_idx = position.character as usize;

        if line >= doc.text.len_lines() {
            return None;
        }

        let line_text = doc.text.line(line).to_string();
        let prefix = &line_text[..char_idx.min(line_text.len())];

        // Find enclosing open parenthesis
        let open_paren_idx = find_enclosing_paren(prefix)?;
        let before_paren = &prefix[..open_paren_idx];
        let func_name = extract_last_ident(before_paren)?;

        // Count commas between open_paren and cursor to find active parameter
        let inside_parens = &prefix[open_paren_idx + 1..];
        let active_param_idx = inside_parens.chars().filter(|c| *c == ',').count() as u32;

        // 1. Look up function in current document AST
        if let Some(ast) = &doc.ast {
            if let Some(f) = ast.functions.iter().find(|f| f.name == func_name) {
                return Some(build_signature_help(
                    &f.name,
                    &f.params,
                    f.return_type.as_deref().unwrap_or("void"),
                    f.doc_comment.as_deref(),
                    active_param_idx,
                ));
            }
        }

        // 2. Look up function in workspace documents
        for entry in workspace.documents.iter() {
            let other = entry.value();
            if let Some(ast) = &other.ast {
                if let Some(f) = ast.functions.iter().find(|f| f.name == func_name) {
                    return Some(build_signature_help(
                        &f.name,
                        &f.params,
                        f.return_type.as_deref().unwrap_or("void"),
                        f.doc_comment.as_deref(),
                        active_param_idx,
                    ));
                }
            }
        }

        // 3. Builtin functions
        if let Some(sig) = get_builtin_signature(func_name, active_param_idx) {
            return Some(sig);
        }

        None
    }
}

fn build_signature_help(
    name: &str,
    params: &[String],
    ret: &str,
    doc: Option<&str>,
    active_param: u32,
) -> SignatureHelp {
    let param_infos = params
        .iter()
        .map(|p| ParameterInformation {
            label: ParameterLabel::Simple(p.clone()),
            documentation: None,
        })
        .collect();

    let full_label = format!("func {}({}) -> {}", name, params.join(", "), ret);

    SignatureHelp {
        signatures: vec![SignatureInformation {
            label: full_label,
            documentation: doc.map(|d| Documentation::String(d.to_string())),
            parameters: Some(param_infos),
            active_parameter: Some(active_param),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_param),
    }
}

fn find_enclosing_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    for (idx, c) in s.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    return Some(idx);
                } else {
                    depth -= 1;
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_last_ident(s: &str) -> Option<&str> {
    let trimmed = s.trim_end();
    let start = trimmed.rfind(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + 1).unwrap_or(0);
    if start < trimmed.len() {
        Some(&trimmed[start..])
    } else {
        None
    }
}

fn get_builtin_signature(name: &str, active_param: u32) -> Option<SignatureHelp> {
    let (params, ret, doc) = match name {
        "println" => (vec!["value: any"], "void", "Prints value to stdout followed by newline."),
        "print" => (vec!["value: any"], "void", "Prints value to stdout."),
        "assert" => (vec!["condition: bool", "message: string"], "void", "Asserts boolean condition."),
        "panic" => (vec!["message: string"], "void", "Aborts execution immediately."),
        "to_string" => (vec!["value: any"], "string", "Converts value to canonical string."),
        "list_push" => (vec!["list: List", "element: any"], "void", "Appends element to list."),
        "list_get" => (vec!["list: List", "index: int"], "any", "Gets element by index."),
        "list_set" => (vec!["list: List", "index: int", "value: any"], "void", "Sets element at index."),
        "map_get" => (vec!["map: Map", "key: string"], "any", "Gets value associated with key."),
        "map_set" => (vec!["map: Map", "key: string", "value: any"], "void", "Inserts key-value pair into map."),
        "string_slice" => (vec!["s: string", "start: int", "end: int"], "string", "Returns substring slice."),
        "string_contains" => (vec!["haystack: string", "needle: string"], "bool", "Checks if string contains substring."),
        _ => return None,
    };

    let p_strings: Vec<String> = params.into_iter().map(|s| s.to_string()).collect();
    Some(build_signature_help(name, &p_strings, ret, Some(doc), active_param))
}
