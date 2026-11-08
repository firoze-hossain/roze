// tools/roze-lsp/src/completion.rs
//
// Context-Aware Autocomplete Engine for Roze LSP v3.17.
// Delivers intelligent code completions with rich snippet expansions,
// scope-aware variable/parameter suggestions, dot-member access,
// enum variant completion, and enterprise annotations.

use tower_lsp::lsp_types::*;
use crate::Document;
use crate::Workspace;

pub struct CompletionEngine;

impl CompletionEngine {
    pub fn get_completions(
        doc: &Document,
        position: Position,
        workspace: &Workspace,
    ) -> Vec<CompletionItem> {
        let mut items = Vec::new();

        let line = position.line as usize;
        let char_idx = position.character as usize;

        if line >= doc.text.len_lines() {
            return get_global_completions();
        }

        let line_text = doc.text.line(line).to_string();
        let prefix = &line_text[..char_idx.min(line_text.len())];
        let trimmed_prefix = prefix.trim_end();

        // 1. Dot-completion: `object.` -> class fields & methods
        if trimmed_prefix.ends_with('.') {
            let target_var = extract_ident_before_dot(trimmed_prefix);
            if let Some(var_name) = target_var {
                add_dot_member_completions(var_name, doc, workspace, &mut items);
                if !items.is_empty() {
                    return items;
                }
            }
        }

        // 2. Colon-Colon completion: `Enum::` -> enum variants
        if trimmed_prefix.ends_with("::") {
            let enum_name = extract_ident_before_colon(trimmed_prefix);
            if let Some(e_name) = enum_name {
                add_enum_variant_completions(e_name, doc, workspace, &mut items);
                if !items.is_empty() {
                    return items;
                }
            }
        }

        // 3. Annotation completion: `@` -> framework annotations
        if trimmed_prefix.ends_with('@') || trimmed_prefix.contains('@') {
            add_annotation_completions(&mut items);
        }

        // 4. In-scope local variables and parameters
        if let Some(ast) = &doc.ast {
            for f in &ast.functions {
                if line >= f.line && line <= f.end_line {
                    for p in &f.param_details {
                        items.push(CompletionItem {
                            label: p.name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(format!("parameter: {}", p.type_name.as_deref().unwrap_or("any"))),
                            ..Default::default()
                        });
                    }
                }
            }

            for v in &ast.variables {
                if line >= v.scope_start && line <= v.scope_end {
                    items.push(CompletionItem {
                        label: v.name.clone(),
                        kind: Some(CompletionItemKind::VARIABLE),
                        detail: Some(format!("variable: {}", v.type_.as_deref().unwrap_or("inferred"))),
                        ..Default::default()
                    });
                }
            }

            // Functions in current file
            for f in &ast.functions {
                let params = f.params.join(", ");
                let ret = f.return_type.as_deref().unwrap_or("void");
                items.push(CompletionItem {
                    label: f.name.clone(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(format!("func({}) -> {}", params, ret)),
                    insert_text: Some(format!("{}($0)", f.name)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    documentation: f.doc_comment.as_ref().map(|d| Documentation::String(d.clone())),
                    ..Default::default()
                });
            }

            // Classes in current file
            for c in &ast.classes {
                items.push(CompletionItem {
                    label: c.name.clone(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(format!("class {}", c.name)),
                    documentation: c.doc_comment.as_ref().map(|d| Documentation::String(d.clone())),
                    ..Default::default()
                });
            }

            // Enums in current file
            for e in &ast.enums {
                items.push(CompletionItem {
                    label: e.name.clone(),
                    kind: Some(CompletionItemKind::ENUM),
                    detail: Some(format!("enum {}", e.name)),
                    documentation: e.doc_comment.as_ref().map(|d| Documentation::String(d.clone())),
                    ..Default::default()
                });
            }
        }

        // 5. Workspace symbols from other documents
        for entry in workspace.documents.iter() {
            let other_doc = entry.value();
            if other_doc.uri == doc.uri {
                continue;
            }
            if let Some(ast) = &other_doc.ast {
                for f in &ast.functions {
                    let params = f.params.join(", ");
                    let ret = f.return_type.as_deref().unwrap_or("void");
                    items.push(CompletionItem {
                        label: f.name.clone(),
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(format!("func({}) -> {} (imported)", params, ret)),
                        insert_text: Some(format!("{}($0)", f.name)),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        ..Default::default()
                    });
                }

                for c in &ast.classes {
                    items.push(CompletionItem {
                        label: c.name.clone(),
                        kind: Some(CompletionItemKind::CLASS),
                        detail: Some(format!("class {} (imported)", c.name)),
                        ..Default::default()
                    });
                }
            }
        }

        // 6. Builtin functions
        add_builtin_completions(&mut items);

        // 7. Keywords with smart snippets
        add_keyword_snippets(&mut items);

        // 8. Primitive types
        add_primitive_types(&mut items);

        items
    }
}

fn add_dot_member_completions(
    var_name: &str,
    doc: &Document,
    workspace: &Workspace,
    items: &mut Vec<CompletionItem>,
) {
    // Check if var_name is a known class instance
    let mut class_type: Option<String> = None;
    if let Some(ast) = &doc.ast {
        if let Some(v) = ast.variables.iter().find(|v| v.name == var_name) {
            class_type = v.type_.clone();
        }
    }

    if let Some(target_class) = class_type {
        // Find class definition in current doc or workspace
        let all_classes: Vec<_> = doc
            .ast
            .as_ref()
            .map(|a| a.classes.clone())
            .unwrap_or_default()
            .into_iter()
            .chain(workspace.documents.iter().flat_map(|d| {
                d.value().ast.as_ref().map(|a| a.classes.clone()).unwrap_or_default()
            }))
            .collect();

        if let Some(c) = all_classes.iter().find(|c| c.name == target_class) {
            for f in &c.fields {
                items.push(CompletionItem {
                    label: f.name.clone(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(format!("field: {}", f.type_name.as_deref().unwrap_or("any"))),
                    ..Default::default()
                });
            }
            for m in &c.methods {
                items.push(CompletionItem {
                    label: m.name.clone(),
                    kind: Some(CompletionItemKind::METHOD),
                    detail: Some(format!("method: func({}) -> {}", m.params.join(", "), m.return_type.as_deref().unwrap_or("void"))),
                    insert_text: Some(format!("{}($0)", m.name)),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
            }
            return;
        }
    }

    // Default container methods
    let container_methods = vec![
        ("length", "length() -> int", "length()"),
        ("push", "push(item: any) -> void", "push(${1:item})"),
        ("pop", "pop() -> any", "pop()"),
        ("get", "get(key_or_idx: any) -> any", "get(${1:index})"),
        ("set", "set(key_or_idx: any, val: any) -> void", "set(${1:key}, ${2:val})"),
        ("contains", "contains(needle: any) -> bool", "contains(${1:needle})"),
    ];

    for (lbl, det, snip) in container_methods {
        items.push(CompletionItem {
            label: lbl.to_string(),
            kind: Some(CompletionItemKind::METHOD),
            detail: Some(det.to_string()),
            insert_text: Some(snip.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_enum_variant_completions(
    enum_name: &str,
    doc: &Document,
    workspace: &Workspace,
    items: &mut Vec<CompletionItem>,
) {
    let all_enums: Vec<_> = doc
        .ast
        .as_ref()
        .map(|a| a.enums.clone())
        .unwrap_or_default()
        .into_iter()
        .chain(workspace.documents.iter().flat_map(|d| {
            d.value().ast.as_ref().map(|a| a.enums.clone()).unwrap_or_default()
        }))
        .collect();

    if let Some(e) = all_enums.iter().find(|e| e.name == enum_name) {
        for v in &e.variants {
            let snip = if v.types.is_empty() {
                v.name.clone()
            } else {
                format!("{}(${{1:value}})", v.name)
            };
            items.push(CompletionItem {
                label: v.name.clone(),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some(format!("variant of enum {}", e.name)),
                insert_text: Some(snip),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });
        }
    }
}

fn add_annotation_completions(items: &mut Vec<CompletionItem>) {
    let annotations = vec![
        ("Service", "@Service\nMarks a class as an IoC injectable service bean.", "Service"),
        ("Autowired", "@Autowired\nInjects a dependency bean automatically.", "Autowired"),
        ("RestController", "@RestController(base_path)\nRegisters a REST controller with a route prefix.", "RestController(\"${1:/api/v1}\")"),
        ("GetMapping", "@GetMapping(path)\nBinds an HTTP GET endpoint.", "GetMapping(\"${1:/path}\")"),
        ("PostMapping", "@PostMapping(path)\nBinds an HTTP POST endpoint.", "PostMapping(\"${1:/path}\")"),
        ("PutMapping", "@PutMapping(path)\nBinds an HTTP PUT endpoint.", "PutMapping(\"${1:/path}\")"),
        ("DeleteMapping", "@DeleteMapping(path)\nBinds an HTTP DELETE endpoint.", "DeleteMapping(\"${1:/path}\")"),
        ("Valid", "@Valid\nEnforces request payload validation.", "Valid"),
        ("repr(C)", "@repr(C)\nEnforces C-compatible struct alignment.", "repr(C)"),
    ];

    for (lbl, doc, snip) in annotations {
        items.push(CompletionItem {
            label: format!("@{}", lbl),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Roze Framework Annotation".to_string()),
            documentation: Some(Documentation::String(doc.to_string())),
            insert_text: Some(snip.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_keyword_snippets(items: &mut Vec<CompletionItem>) {
    let snippets = vec![
        ("func", "func ${1:name}(${2:params}) -> ${3:void} {\n\t${0}\n}", "Function definition"),
        ("async func", "async func ${1:name}(${2:params}) -> ${3:Future} {\n\t${0}\n}", "Asynchronous function"),
        ("let", "let ${1:name} = ${2:value};", "Immutable variable binding"),
        ("mut", "mut ${1:name} = ${2:value};", "Mutable variable binding"),
        ("if", "if ${1:condition} {\n\t${0}\n}", "If conditional"),
        ("ifelse", "if ${1:condition} {\n\t${2}\n} else {\n\t${0}\n}", "If-Else conditional"),
        ("for", "for let ${1:i} = 0; ${1:i} < ${2:len}; ${1:i} = ${1:i} + 1 {\n\t${0}\n}", "Counted for loop"),
        ("while", "while ${1:condition} {\n\t${0}\n}", "While loop"),
        ("class", "class ${1:Name} {\n\t${2:field}: ${3:type},\n}", "Class definition"),
        ("enum", "enum ${1:Name} {\n\t${2:Variant},\n}", "Enum declaration"),
        ("match", "match ${1:expr} {\n\t${2:pattern} => ${0},\n}", "Pattern matching"),
        ("import", "import \"${1:module}\";", "Module import"),
        ("comptime", "comptime {\n\t${0}\n}", "Compile-time evaluation"),
        ("unsafe", "unsafe {\n\t${0}\n}", "Unsafe execution block"),
        ("return", "return ${1:value};", "Return statement"),
    ];

    for (lbl, snip, desc) in snippets {
        items.push(CompletionItem {
            label: lbl.to_string(),
            kind: Some(CompletionItemKind::SNIPPET),
            detail: Some(desc.to_string()),
            insert_text: Some(snip.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_builtin_completions(items: &mut Vec<CompletionItem>) {
    let builtins = vec![
        ("println", "println(${1:\"text\"})", "println(value) -> void"),
        ("print", "print(${1:\"text\"})", "print(value) -> void"),
        ("read_line", "read_line()", "read_line() -> string"),
        ("assert", "assert(${1:cond}, \"${2:msg}\")", "assert(condition, message) -> void"),
        ("panic", "panic(\"${1:error message}\")", "panic(message) -> void"),
        ("to_string", "to_string(${1:val})", "to_string(value) -> string"),
        ("list_new", "list_new()", "list_new() -> List"),
        ("list_push", "list_push(${1:list}, ${2:item})", "list_push(list, item) -> void"),
        ("list_pop", "list_pop(${1:list})", "list_pop(list) -> any"),
        ("list_get", "list_get(${1:list}, ${2:idx})", "list_get(list, index) -> any"),
        ("list_set", "list_set(${1:list}, ${2:idx}, ${3:val})", "list_set(list, index, value) -> void"),
        ("list_length", "list_length(${1:list})", "list_length(list) -> int"),
        ("map_new", "map_new()", "map_new() -> Map"),
        ("map_get", "map_get(${1:map}, \"${2:key}\")", "map_get(map, key) -> any"),
        ("map_set", "map_set(${1:map}, \"${2:key}\", ${3:val})", "map_set(map, key, value) -> void"),
        ("map_contains_key", "map_contains_key(${1:map}, \"${2:key}\")", "map_contains_key(map, key) -> bool"),
        ("map_keys", "map_keys(${1:map})", "map_keys(map) -> List"),
        ("string_length", "string_length(${1:s})", "string_length(str) -> int"),
        ("string_slice", "string_slice(${1:s}, ${2:start}, ${3:end})", "string_slice(str, start, end) -> string"),
        ("string_contains", "string_contains(${1:s}, \"${2:sub}\")", "string_contains(haystack, needle) -> bool"),
        ("sha256_hex", "sha256_hex(\"${1:data}\")", "sha256_hex(data) -> string"),
        ("hex_encode", "hex_encode(${1:bytes})", "hex_encode(bytes) -> string"),
        ("hex_decode", "hex_decode(\"${1:hex_str}\")", "hex_decode(hex_str) -> List"),
        ("file_read", "file_read(\"${1:path}\")", "file_read(path) -> string"),
        ("file_write", "file_write(\"${1:path}\", ${2:content})", "file_write(path, content) -> bool"),
    ];

    for (name, snip, det) in builtins {
        items.push(CompletionItem {
            label: name.to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: Some(det.to_string()),
            insert_text: Some(snip.to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        });
    }
}

fn add_primitive_types(items: &mut Vec<CompletionItem>) {
    let types = vec![
        "int", "float", "string", "bool", "void", "List", "Map", "Option", "Result", "Future", "Box", "Rc", "Arc",
    ];

    for t in types {
        items.push(CompletionItem {
            label: t.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some("Roze Primitive / Standard Type".to_string()),
            ..Default::default()
        });
    }
}

fn get_global_completions() -> Vec<CompletionItem> {
    let mut items = Vec::new();
    add_keyword_snippets(&mut items);
    add_builtin_completions(&mut items);
    add_primitive_types(&mut items);
    items
}

fn extract_ident_before_dot(s: &str) -> Option<&str> {
    let s = s.strip_suffix('.')?;
    let trimmed = s.trim_end();
    let start = trimmed.rfind(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + 1).unwrap_or(0);
    if start < trimmed.len() {
        Some(&trimmed[start..])
    } else {
        None
    }
}

fn extract_ident_before_colon(s: &str) -> Option<&str> {
    let s = s.strip_suffix("::")?;
    let trimmed = s.trim_end();
    let start = trimmed.rfind(|c: char| !c.is_alphanumeric() && c != '_').map(|i| i + 1).unwrap_or(0);
    if start < trimmed.len() {
        Some(&trimmed[start..])
    } else {
        None
    }
}
