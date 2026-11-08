// tools/roze-lsp/src/hover.rs
//
// Rich Hover Documentation Engine for Roze LSP v3.17.
// Formats Markdown documentation cards with full type signatures,
// parameter lists, docstrings, and standard library descriptions.

use tower_lsp::lsp_types::*;
use crate::Document;

pub struct HoverEngine;

impl HoverEngine {
    pub fn get_hover(doc: &Document, position: Position) -> Option<Hover> {
        let line = position.line as usize;
        let char_idx = position.character as usize;

        if line >= doc.text.len_lines() {
            return None;
        }

        let line_text = doc.text.line(line).to_string();
        let word = find_word_at(&line_text, char_idx)?;

        // 1. Check local variables and function parameters in scope
        if let Some(ast) = &doc.ast {
            // Function parameters active at this line
            for f in &ast.functions {
                if line >= f.line && line <= f.end_line {
                    if let Some(param) = f.param_details.iter().find(|p| p.name == word) {
                        let type_str = param.type_name.as_deref().unwrap_or("any");
                        let markdown = format!(
                            "```roze\n(parameter) {}: {}\n```\n\nParameter of function `{}`",
                            param.name, type_str, f.name
                        );
                        return Some(make_hover(markdown));
                    }
                }
            }

            // In-scope local variables
            for v in &ast.variables {
                if v.name == word && line >= v.scope_start && line <= v.scope_end {
                    let mut_prefix = if v.is_mut { "mut " } else { "" };
                    let type_str = v.type_.as_deref().unwrap_or("inferred");
                    let markdown = format!(
                        "```roze\n(variable) let {}{}: {}\n```\n\nDeclared at line {}",
                        mut_prefix, v.name, type_str, v.line + 1
                    );
                    return Some(make_hover(markdown));
                }
            }

            // 2. Check functions in AST
            if let Some(func) = ast.functions.iter().find(|f| f.name == word) {
                let async_prefix = if func.is_async { "async " } else { "" };
                let extern_prefix = if func.is_extern { "extern \"C\" " } else { "" };
                let ret = func.return_type.as_deref().unwrap_or("void");
                let type_params = if func.type_params.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", func.type_params.join(", "))
                };
                let params = func.params.join(", ");

                let mut md = format!(
                    "```roze\n{}{}{}func {}{}({}) -> {}\n```",
                    extern_prefix, async_prefix, "", func.name, type_params, params, ret
                );

                if let Some(doc_str) = &func.doc_comment {
                    md.push_str("\n\n---\n\n");
                    md.push_str(doc_str);
                }

                return Some(make_hover(md));
            }

            // 3. Check classes in AST
            if let Some(c) = ast.classes.iter().find(|c| c.name == word) {
                let type_params = if c.type_params.is_empty() {
                    String::new()
                } else {
                    format!("<{}>", c.type_params.join(", "))
                };

                let mut field_lines = String::new();
                for f in &c.fields {
                    let t = f.type_name.as_deref().unwrap_or("any");
                    field_lines.push_str(&format!("    {}: {},\n", f.name, t));
                }

                let mut md = format!("```roze\nclass {}{} {{\n{}}}\n```", c.name, type_params, field_lines);
                if let Some(doc_str) = &c.doc_comment {
                    md.push_str("\n\n---\n\n");
                    md.push_str(doc_str);
                }

                return Some(make_hover(md));
            }

            // 4. Check enums in AST
            if let Some(e) = ast.enums.iter().find(|e| e.name == word) {
                let mut variant_lines = String::new();
                for v in &e.variants {
                    if v.types.is_empty() {
                        variant_lines.push_str(&format!("    {},\n", v.name));
                    } else {
                        variant_lines.push_str(&format!("    {}({}),\n", v.name, v.types.join(", ")));
                    }
                }

                let mut md = format!("```roze\nenum {} {{\n{}}}\n```", e.name, variant_lines);
                if let Some(doc_str) = &e.doc_comment {
                    md.push_str("\n\n---\n\n");
                    md.push_str(doc_str);
                }

                return Some(make_hover(md));
            }

            // 5. Check traits in AST
            if let Some(tr) = ast.traits.iter().find(|tr| tr.name == word) {
                let mut method_lines = String::new();
                for m in &tr.methods {
                    let ret = m.return_type.as_deref().unwrap_or("void");
                    method_lines.push_str(&format!("    func {}({}) -> {};\n", m.name, m.params.join(", "), ret));
                }

                let mut md = format!("```roze\ntrait {} {{\n{}}}\n```", tr.name, method_lines);
                if let Some(doc_str) = &tr.doc_comment {
                    md.push_str("\n\n---\n\n");
                    md.push_str(doc_str);
                }

                return Some(make_hover(md));
            }
        }

        // 6. Check standard library builtins
        if let Some(builtin_doc) = get_builtin_doc(word) {
            return Some(make_hover(builtin_doc));
        }

        // 7. Check keywords
        if let Some(keyword_doc) = get_keyword_doc(word) {
            return Some(make_hover(keyword_doc));
        }

        // Fallback: Symbol found in program
        Some(make_hover(format!(
            "```roze\n{}\n```\n\n**{}**\n\nIdentifier in Roze module",
            word, word
        )))
    }
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

fn make_hover(markdown: String) -> Hover {
    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: markdown,
        }),
        range: None,
    }
}

fn get_builtin_doc(name: &str) -> Option<String> {
    match name {
        "println" => Some(
            "```roze\nfunc println(value: any) -> void\n```\n\nPrints value followed by a newline to standard output."
                .to_string(),
        ),
        "print" => Some(
            "```roze\nfunc print(value: any) -> void\n```\n\nPrints value to standard output without trailing newline."
                .to_string(),
        ),
        "read_line" => Some(
            "```roze\nfunc read_line() -> string\n```\n\nReads a line of UTF-8 text from standard input."
                .to_string(),
        ),
        "assert" => Some(
            "```roze\nfunc assert(condition: bool, message: string) -> void\n```\n\nAsserts condition is true; aborts with panic if false."
                .to_string(),
        ),
        "panic" => Some(
            "```roze\nfunc panic(message: string) -> void\n```\n\nTerminates program execution immediately with error message and stack trace."
                .to_string(),
        ),
        "to_string" => Some(
            "```roze\nfunc to_string(value: any) -> string\n```\n\nConverts any primitive or object to its canonical string representation."
                .to_string(),
        ),
        "list_new" => Some(
            "```roze\nfunc list_new() -> List\n```\n\nCreates a new dynamic, resizable list."
                .to_string(),
        ),
        "list_push" => Some(
            "```roze\nfunc list_push(list: List, element: any) -> void\n```\n\nAppends an element to the end of the list."
                .to_string(),
        ),
        "list_pop" => Some(
            "```roze\nfunc list_pop(list: List) -> any\n```\n\nRemoves and returns the last element of the list."
                .to_string(),
        ),
        "list_get" => Some(
            "```roze\nfunc list_get(list: List, index: int) -> any\n```\n\nRetrieves element at 0-based index."
                .to_string(),
        ),
        "list_set" => Some(
            "```roze\nfunc list_set(list: List, index: int, value: any) -> void\n```\n\nUpdates element at 0-based index."
                .to_string(),
        ),
        "list_length" => Some(
            "```roze\nfunc list_length(list: List) -> int\n```\n\nReturns total number of elements in list."
                .to_string(),
        ),
        "map_new" => Some(
            "```roze\nfunc map_new() -> Map\n```\n\nCreates a new associative hash map."
                .to_string(),
        ),
        "map_get" => Some(
            "```roze\nfunc map_get(map: Map, key: string) -> any\n```\n\nRetrieves value associated with key, or null if absent."
                .to_string(),
        ),
        "map_set" => Some(
            "```roze\nfunc map_set(map: Map, key: string, value: any) -> void\n```\n\nInserts or updates key-value pair in map."
                .to_string(),
        ),
        "map_contains_key" => Some(
            "```roze\nfunc map_contains_key(map: Map, key: string) -> bool\n```\n\nReturns true if map contains the given key."
                .to_string(),
        ),
        "map_keys" => Some(
            "```roze\nfunc map_keys(map: Map) -> List\n```\n\nReturns list of all keys present in the map."
                .to_string(),
        ),
        "string_length" => Some(
            "```roze\nfunc string_length(s: string) -> int\n```\n\nReturns UTF-8 character length of string."
                .to_string(),
        ),
        "string_slice" => Some(
            "```roze\nfunc string_slice(s: string, start: int, end: int) -> string\n```\n\nReturns substring between start and end indices."
                .to_string(),
        ),
        "string_contains" => Some(
            "```roze\nfunc string_contains(haystack: string, needle: string) -> bool\n```\n\nReturns true if needle is contained in haystack."
                .to_string(),
        ),
        "string_split" => Some(
            "```roze\nfunc string_split(s: string, delimiter: string) -> List\n```\n\nSplits string by delimiter into a list of strings."
                .to_string(),
        ),
        "string_replace" => Some(
            "```roze\nfunc string_replace(s: string, from: string, to: string) -> string\n```\n\nReplaces all occurrences of pattern in string."
                .to_string(),
        ),
        "sha256" => Some(
            "```roze\nfunc sha256(data: List) -> List\n```\n\nComputes FIPS 180-4 SHA-256 digest over byte list."
                .to_string(),
        ),
        "sha256_hex" => Some(
            "```roze\nfunc sha256_hex(data: string) -> string\n```\n\nComputes SHA-256 digest and returns 64-character lowercase hexadecimal string."
                .to_string(),
        ),
        "hex_encode" => Some(
            "```roze\nfunc hex_encode(bytes: List) -> string\n```\n\nEncodes byte list to lowercase hexadecimal string."
                .to_string(),
        ),
        "hex_decode" => Some(
            "```roze\nfunc hex_decode(hex_str: string) -> List\n```\n\nDecodes hexadecimal string into raw byte list."
                .to_string(),
        ),
        "file_read" => Some(
            "```roze\nfunc file_read(path: string) -> string\n```\n\nReads entire file contents into a UTF-8 string."
                .to_string(),
        ),
        "file_write" => Some(
            "```roze\nfunc file_write(path: string, content: string) -> bool\n```\n\nWrites content to file, overwriting existing contents."
                .to_string(),
        ),
        "thread_spawn" => Some(
            "```roze\nfunc thread_spawn(entry: func() -> void) -> ThreadHandle\n```\n\nSpawns an OS worker thread executing entry function."
                .to_string(),
        ),
        "thread_join" => Some(
            "```roze\nfunc thread_join(handle: ThreadHandle) -> void\n```\n\nBlocks current thread until target worker thread finishes."
                .to_string(),
        ),
        _ => None,
    }
}

fn get_keyword_doc(kw: &str) -> Option<String> {
    match kw {
        "func" => Some("**`func`** keyword: Declares a function, closure, or method in Roze.".to_string()),
        "let" => Some("**`let`** keyword: Declares an immutable local variable binding.".to_string()),
        "mut" => Some("**`mut`** keyword: Declares a mutable variable binding allowing reassignment.".to_string()),
        "return" => Some("**`return`** keyword: Returns execution and optional value from function.".to_string()),
        "if" => Some("**`if`** keyword: Evaluates boolean condition and executes then-branch if true.".to_string()),
        "else" => Some("**`else`** keyword: Alternative execution branch for an `if` condition.".to_string()),
        "for" => Some("**`for`** keyword: Counted loop construct (`for let i = 0; i < N; i = i + 1 { ... }`).".to_string()),
        "while" => Some("**`while`** keyword: Executes loop body as long as condition evaluates to true.".to_string()),
        "class" => Some("**`class`** keyword: Defines a compound data structure with typed fields.".to_string()),
        "new" => Some("**`new`** keyword: Instantiates a class on the heap (`new ClassName(args)`).".to_string()),
        "enum" => Some("**`enum`** keyword: Declares an algebraic data type / tagged union with variants.".to_string()),
        "match" => Some("**`match`** keyword: Performs pattern matching on expressions with exhaustiveness.".to_string()),
        "trait" => Some("**`trait`** keyword: Defines an interface contract of callable method signatures.".to_string()),
        "impl" => Some("**`impl`** keyword: Implements methods or trait conformance for a class type.".to_string()),
        "import" => Some("**`import`** keyword: Imports modules or standalone bsr packages into source.".to_string()),
        "async" => Some("**`async`** keyword: Marks function or block as asynchronous, returning a Future.".to_string()),
        "await" => Some("**`await`** keyword: Suspends current coroutine until Future resolves.".to_string()),
        "comptime" => Some("**`comptime`** keyword: Evaluates expression or statement at compile time.".to_string()),
        "unsafe" => Some("**`unsafe`** keyword: Encloses block bypassing compiler safety checks.".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropey::Rope;

    #[test]
    fn hover_over_function_shows_signature_and_docstring() {
        let src = "/// Adds two numbers.\nfunc add(a: int, b: int) -> int {\n    return a + b;\n}\n";
        let doc = Document {
            uri: Url::parse("file:///test.roze").unwrap(),
            text: Rope::from_str(src),
            version: 1,
            ast: crate::parser::parse(src),
        };

        let hover = HoverEngine::get_hover(&doc, Position { line: 1, character: 6 }).unwrap();
        if let HoverContents::Markup(m) = hover.contents {
            assert!(m.value.contains("func add(a: int, b: int) -> int"));
            assert!(m.value.contains("Adds two numbers"));
        } else {
            panic!("Expected Markup hover contents");
        }
    }

    #[test]
    fn hover_over_builtin_shows_documentation() {
        let src = "func main() { println(\"hi\"); }";
        let doc = Document {
            uri: Url::parse("file:///test.roze").unwrap(),
            text: Rope::from_str(src),
            version: 1,
            ast: crate::parser::parse(src),
        };

        let hover = HoverEngine::get_hover(&doc, Position { line: 0, character: 16 }).unwrap();
        if let HoverContents::Markup(m) = hover.contents {
            assert!(m.value.contains("println"));
            assert!(m.value.contains("Prints value followed by a newline"));
        } else {
            panic!("Expected Markup hover contents");
        }
    }
}
