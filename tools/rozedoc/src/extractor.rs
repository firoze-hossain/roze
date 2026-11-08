// tools/rozedoc/src/extractor.rs
//
// AST and Docstring Extractor for Roze source files and packages.
// Parses Roze source code using roze_compiler lexer & parser,
// and extracts modules, functions, classes, enums, traits, constants,
// type signatures, line numbers, and markdown doc comments.

use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::markdown::parse_docstring;
use crate::model::*;

/// Extract documentation for an entire package directory.
pub fn extract_package(dir: &Path) -> Result<DocPackage> {
    let mut pkg_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("roze_package").to_string();
    let mut pkg_version = "1.0.0".to_string();
    let mut description = None;
    let mut authors = Vec::new();
    let mut license = None;
    let mut repository = None;
    let mut readme = None;

    // Check for bsr.mfz or roze.toml
    let mfz_path = dir.join("bsr.mfz");
    let toml_path = dir.join("roze.toml");
    let manifest_path = if mfz_path.exists() {
        Some(mfz_path)
    } else if toml_path.exists() {
        Some(toml_path)
    } else {
        None
    };

    if let Some(ref mp) = manifest_path {
        if let Ok(content) = fs::read_to_string(mp) {
            parse_manifest_metadata(&content, &mut pkg_name, &mut pkg_version, &mut description, &mut authors, &mut license, &mut repository);
        }
    }

    // Check for README.md
    for readme_name in &["README.md", "readme.md", "README"] {
        let rp = dir.join(readme_name);
        if rp.exists() {
            if let Ok(content) = fs::read_to_string(&rp) {
                readme = Some(content);
                break;
            }
        }
    }

    let mut package = DocPackage::new(pkg_name, pkg_version);
    package.description = description;
    package.authors = authors;
    package.license = license;
    package.repository = repository;
    package.readme = readme;

    // Find all .roze files in src/ or dir
    let src_dir = dir.join("src");
    let scan_dir = if src_dir.exists() && src_dir.is_dir() {
        &src_dir
    } else {
        dir
    };

    let mut roze_files = Vec::new();
    for entry in WalkDir::new(scan_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "roze") {
            roze_files.push(path.to_path_buf());
        }
    }

    // Sort files for deterministic documentation output
    roze_files.sort();

    for file_path in roze_files {
        match extract_module(&file_path, dir) {
            Ok(module) => {
                package.modules.push(module);
            }
            Err(e) => {
                eprintln!("rozedoc: warning: failed to parse module {}: {}", file_path.display(), e);
            }
        }
    }

    Ok(package)
}

/// Extract documentation from a single `.roze` file.
pub fn extract_single_file(file_path: &Path) -> Result<DocPackage> {
    let file_stem = file_path.file_stem().and_then(|s| s.to_str()).unwrap_or("module");
    let mut package = DocPackage::new(file_stem, "1.0.0");
    let parent_dir = file_path.parent().unwrap_or(Path::new("."));
    let module = extract_module(file_path, parent_dir)?;
    package.modules.push(module);
    Ok(package)
}

/// Extract a single module from a `.roze` file.
pub fn extract_module(file_path: &Path, root_dir: &Path) -> Result<DocModule> {
    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read source file: {}", file_path.display()))?;
    let lines: Vec<&str> = content.lines().collect();

    let module_name = compute_module_name(file_path, root_dir);
    let mut module = DocModule::new(&module_name, file_path.display().to_string());

    // Extract module-level doc comment (//!)
    module.doc = get_module_docstring(&lines);

    // Tokenize
    let tokens = roze_compiler::lexer::tokenize(&content);

    // Try AST parse
    match roze_compiler::parser::parse(tokens) {
        Ok(program) => {
            extract_from_ast(&program, &lines, &mut module);
        }
        Err(_err) => {
            // Fallback scanner if syntax parser fails (e.g. on WIP code)
            fallback_extract_from_lines(&lines, &mut module);
        }
    }

    Ok(module)
}

fn compute_module_name(file_path: &Path, root_dir: &Path) -> String {
    let src_dir = root_dir.join("src");
    let rel_path = if let Ok(rel) = file_path.strip_prefix(&src_dir) {
        rel
    } else if let Ok(rel) = file_path.strip_prefix(root_dir) {
        rel
    } else {
        file_path
    };

    let without_ext = rel_path.with_extension("");
    let parts: Vec<String> = without_ext
        .components()
        .filter_map(|c| c.as_os_str().to_str().map(|s| s.to_string()))
        .collect();

    if parts.is_empty() {
        "main".to_string()
    } else {
        parts.join("::")
    }
}

fn extract_from_ast(program: &roze_compiler::parser::ast::Program, lines: &[&str], module: &mut DocModule) {
    for stmt in &program.statements {
        process_statement(stmt, lines, module);
    }
}

fn process_statement(stmt: &roze_compiler::parser::ast::Statement, lines: &[&str], module: &mut DocModule) {
    use roze_compiler::parser::ast::*;

    match stmt {
        Statement::Function {
            name,
            type_params,
            params,
            return_type,
            is_async,
            location,
            ..
        } => {
            let doc_raw = get_docstring_preceding(lines, location.line);
            let extracted = doc_raw.as_deref().map(parse_docstring);

            let doc_params = params
                .iter()
                .map(|p| {
                    let desc = extracted
                        .as_ref()
                        .and_then(|ex| ex.params.iter().find(|(pn, _)| pn == &p.name).map(|(_, d)| d.clone()));
                    DocParam {
                        name: p.name.clone(),
                        type_name: p.type_name.clone(),
                        description: desc,
                    }
                })
                .collect();

            let signature = format_function_signature(*is_async, false, name, type_params, params, return_type.as_deref());

            let doc_fn = DocFunction {
                name: name.clone(),
                signature,
                type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                params: doc_params,
                return_type: return_type.clone(),
                is_async: *is_async,
                is_extern: false,
                doc: extracted.as_ref().map(|ex| ex.summary.clone()).or(doc_raw),
                return_doc: extracted.as_ref().and_then(|ex| ex.return_doc.clone()),
                throws_doc: extracted.as_ref().map(|ex| ex.throws.clone()).unwrap_or_default(),
                examples: extracted.as_ref().map(|ex| ex.examples.clone()).unwrap_or_default(),
                line: location.line,
            };

            module.functions.push(doc_fn);
        }

        Statement::ExternFunction {
            abi: _,
            name,
            params,
            return_type,
            location,
        } => {
            let doc_raw = get_docstring_preceding(lines, location.line);
            let extracted = doc_raw.as_deref().map(parse_docstring);

            let doc_params = params
                .iter()
                .map(|p| DocParam {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                    description: None,
                })
                .collect();

            let signature = format_function_signature(false, true, name, &[], params, return_type.as_deref());

            let doc_fn = DocFunction {
                name: name.clone(),
                signature,
                type_params: Vec::new(),
                params: doc_params,
                return_type: return_type.clone(),
                is_async: false,
                is_extern: true,
                doc: extracted.as_ref().map(|ex| ex.summary.clone()).or(doc_raw),
                return_doc: extracted.as_ref().and_then(|ex| ex.return_doc.clone()),
                throws_doc: extracted.as_ref().map(|ex| ex.throws.clone()).unwrap_or_default(),
                examples: extracted.as_ref().map(|ex| ex.examples.clone()).unwrap_or_default(),
                line: location.line,
            };

            module.functions.push(doc_fn);
        }

        Statement::ClassDecl {
            name,
            type_params,
            fields,
            location,
        } => {
            let doc_raw = get_docstring_preceding(lines, location.line);
            let mut doc_fields = Vec::new();

            for f in fields {
                doc_fields.push(DocField {
                    name: f.name.clone(),
                    type_name: f.type_name.clone(),
                    doc: None,
                    line: location.line,
                });
            }

            let doc_class = DocClass {
                name: name.clone(),
                type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                doc: doc_raw,
                fields: doc_fields,
                methods: Vec::new(),
                line: location.line,
            };

            module.classes.push(doc_class);
        }

        Statement::ImplDecl {
            trait_name: _,
            class_name,
            associated_types: _,
            methods,
            location: _,
        } => {
            let mut extracted_methods = Vec::new();
            for m in methods {
                if let Statement::Function {
                    name,
                    type_params,
                    params,
                    return_type,
                    is_async,
                    location,
                    ..
                } = m
                {
                    let m_doc_raw = get_docstring_preceding(lines, location.line);
                    let m_extracted = m_doc_raw.as_deref().map(parse_docstring);
                    let signature = format_function_signature(*is_async, false, name, type_params, params, return_type.as_deref());
                    let m_params = params
                        .iter()
                        .map(|p| DocParam {
                            name: p.name.clone(),
                            type_name: p.type_name.clone(),
                            description: None,
                        })
                        .collect();

                    extracted_methods.push(DocFunction {
                        name: name.clone(),
                        signature,
                        type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                        params: m_params,
                        return_type: return_type.clone(),
                        is_async: *is_async,
                        is_extern: false,
                        doc: m_extracted.as_ref().map(|ex| ex.summary.clone()).or(m_doc_raw),
                        return_doc: m_extracted.as_ref().and_then(|ex| ex.return_doc.clone()),
                        throws_doc: m_extracted.as_ref().map(|ex| ex.throws.clone()).unwrap_or_default(),
                        examples: m_extracted.as_ref().map(|ex| ex.examples.clone()).unwrap_or_default(),
                        line: location.line,
                    });
                }
            }

            if let Some(cls) = module.classes.iter_mut().find(|c| &c.name == class_name) {
                cls.methods.extend(extracted_methods);
            } else {
                // If class was not declared in same module, create entry
                module.classes.push(DocClass {
                    name: class_name.clone(),
                    type_params: Vec::new(),
                    doc: None,
                    fields: Vec::new(),
                    methods: extracted_methods,
                    line: 0,
                });
            }
        }

        Statement::EnumDecl {
            name,
            type_params,
            variants,
            location,
        } => {
            let doc_raw = get_docstring_preceding(lines, location.line);
            let mut doc_variants = Vec::new();

            for v in variants {
                let v_doc = get_docstring_preceding(lines, v.location.line);
                doc_variants.push(DocVariant {
                    name: v.name.clone(),
                    payload_types: v.payload_types.clone(),
                    doc: v_doc,
                    line: v.location.line,
                });
            }

            module.enums.push(DocEnum {
                name: name.clone(),
                type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                doc: doc_raw,
                variants: doc_variants,
                line: location.line,
            });
        }

        Statement::TraitDecl {
            name,
            associated_types: _,
            methods,
            location,
        } => {
            let doc_raw = get_docstring_preceding(lines, location.line);
            let mut doc_methods = Vec::new();

            for m in methods {
                let m_doc_raw = get_docstring_preceding(lines, m.location.line);
                let m_extracted = m_doc_raw.as_deref().map(parse_docstring);
                let signature = format_function_signature(false, false, &m.name, &[], &m.params, m.return_type.as_deref());
                let m_params = m
                    .params
                    .iter()
                    .map(|p| DocParam {
                        name: p.name.clone(),
                        type_name: p.type_name.clone(),
                        description: None,
                    })
                    .collect();

                doc_methods.push(DocFunction {
                    name: m.name.clone(),
                    signature,
                    type_params: Vec::new(),
                    params: m_params,
                    return_type: m.return_type.clone(),
                    is_async: false,
                    is_extern: false,
                    doc: m_extracted.as_ref().map(|ex| ex.summary.clone()).or(m_doc_raw),
                    return_doc: m_extracted.as_ref().and_then(|ex| ex.return_doc.clone()),
                    throws_doc: m_extracted.as_ref().map(|ex| ex.throws.clone()).unwrap_or_default(),
                    examples: m_extracted.as_ref().map(|ex| ex.examples.clone()).unwrap_or_default(),
                    line: m.location.line,
                });
            }

            module.traits.push(DocTrait {
                name: name.clone(),
                doc: doc_raw,
                methods: doc_methods,
                line: location.line,
            });
        }

        Statement::Let {
            name,
            type_name,
            location,
            ..
        } => {
            let doc_raw = get_docstring_preceding(lines, location.line);
            // Only document top-level let declarations that have docstrings or uppercase naming
            if doc_raw.is_some() || name.chars().all(|c| c.is_uppercase() || c == '_') {
                module.constants.push(DocConstant {
                    name: name.clone(),
                    type_name: type_name.clone(),
                    value: None,
                    doc: doc_raw,
                    line: location.line,
                });
            }
        }

        Statement::Import { path, .. } => {
            if !module.imports.contains(path) {
                module.imports.push(path.clone());
            }
        }

        Statement::Annotated { statement, .. } => {
            process_statement(statement, lines, module);
        }

        _ => {}
    }
}

fn format_function_signature(
    is_async: bool,
    is_extern: bool,
    name: &str,
    type_params: &[roze_compiler::parser::ast::TypeParam],
    params: &[roze_compiler::parser::ast::FunctionParam],
    return_type: Option<&str>,
) -> String {
    let mut sig = String::new();
    if is_extern {
        sig.push_str("extern ");
    }
    if is_async {
        sig.push_str("async ");
    }
    sig.push_str("func ");
    sig.push_str(name);

    if !type_params.is_empty() {
        sig.push('<');
        let tp_strs: Vec<String> = type_params
            .iter()
            .map(|tp| {
                if tp.bounds.is_empty() {
                    tp.name.clone()
                } else {
                    format!("{}: {}", tp.name, tp.bounds.join(" + "))
                }
            })
            .collect();
        sig.push_str(&tp_strs.join(", "));
        sig.push('>');
    }

    sig.push('(');
    let p_strs: Vec<String> = params
        .iter()
        .map(|p| {
            if let Some(ref t) = p.type_name {
                format!("{}: {}", p.name, t)
            } else {
                p.name.clone()
            }
        })
        .collect();
    sig.push_str(&p_strs.join(", "));
    sig.push(')');

    if let Some(ret) = return_type {
        sig.push_str(" -> ");
        sig.push_str(ret);
    }

    sig
}

/// Fallback extraction when AST parsing fails or file has partial code.
fn fallback_extract_from_lines(lines: &[&str], module: &mut DocModule) {
    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_num = idx + 1;

        if trimmed.starts_with("func ") || trimmed.starts_with("async func ") || trimmed.starts_with("pub func ") {
            let doc_raw = get_docstring_preceding(lines, line_num);
            let name = extract_func_name(trimmed);
            if !name.is_empty() {
                let sig = trimmed.split('{').next().unwrap_or(trimmed).trim().to_string();
                module.functions.push(DocFunction {
                    name,
                    signature: sig,
                    type_params: Vec::new(),
                    params: Vec::new(),
                    return_type: None,
                    is_async: trimmed.contains("async"),
                    is_extern: false,
                    doc: doc_raw,
                    return_doc: None,
                    throws_doc: Vec::new(),
                    examples: Vec::new(),
                    line: line_num,
                });
            }
        } else if trimmed.starts_with("class ") || trimmed.starts_with("pub class ") {
            let doc_raw = get_docstring_preceding(lines, line_num);
            let name = extract_ident_after(trimmed, "class");
            if !name.is_empty() {
                module.classes.push(DocClass {
                    name,
                    type_params: Vec::new(),
                    doc: doc_raw,
                    fields: Vec::new(),
                    methods: Vec::new(),
                    line: line_num,
                });
            }
        } else if trimmed.starts_with("enum ") || trimmed.starts_with("pub enum ") {
            let doc_raw = get_docstring_preceding(lines, line_num);
            let name = extract_ident_after(trimmed, "enum");
            if !name.is_empty() {
                module.enums.push(DocEnum {
                    name,
                    type_params: Vec::new(),
                    doc: doc_raw,
                    variants: Vec::new(),
                    line: line_num,
                });
            }
        } else if trimmed.starts_with("trait ") || trimmed.starts_with("pub trait ") {
            let doc_raw = get_docstring_preceding(lines, line_num);
            let name = extract_ident_after(trimmed, "trait");
            if !name.is_empty() {
                module.traits.push(DocTrait {
                    name,
                    doc: doc_raw,
                    methods: Vec::new(),
                    line: line_num,
                });
            }
        } else if trimmed.starts_with("import ") {
            let path = trimmed["import ".len()..].trim_matches(';').trim().to_string();
            if !path.is_empty() && !module.imports.contains(&path) {
                module.imports.push(path);
            }
        }
    }
}

fn extract_func_name(line: &str) -> String {
    let after_func = if let Some(pos) = line.find("func ") {
        &line[pos + 5..]
    } else {
        return String::new();
    };
    let name_part = after_func.split(|c: char| c == '(' || c == '<' || c.is_whitespace()).next().unwrap_or("");
    name_part.to_string()
}

fn extract_ident_after(line: &str, keyword: &str) -> String {
    let pattern = format!("{} ", keyword);
    if let Some(pos) = line.find(&pattern) {
        let after = &line[pos + pattern.len()..];
        let name = after.split(|c: char| c == '{' || c == '<' || c.is_whitespace()).next().unwrap_or("");
        name.to_string()
    } else {
        String::new()
    }
}

fn get_module_docstring(lines: &[&str]) -> Option<String> {
    let mut doc_lines = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("//!") {
            let doc_text = trimmed.strip_prefix("//!").unwrap();
            let doc_text = doc_text.strip_prefix(' ').unwrap_or(doc_text);
            doc_lines.push(doc_text.to_string());
        } else {
            break;
        }
    }
    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

fn get_docstring_preceding(lines: &[&str], target_line: usize) -> Option<String> {
    if target_line == 0 || target_line > lines.len() {
        return None;
    }
    let mut idx = target_line - 1; // 0-based index of target statement

    // Skip empty lines or annotations directly preceding target
    while idx > 0 && (lines[idx - 1].trim().is_empty() || lines[idx - 1].trim().starts_with('@')) {
        idx -= 1;
    }

    let mut doc_lines = Vec::new();
    while idx > 0 {
        let trimmed = lines[idx - 1].trim();
        if trimmed.starts_with("///") {
            let doc_text = trimmed.strip_prefix("///").unwrap();
            let doc_text = doc_text.strip_prefix(' ').unwrap_or(doc_text);
            doc_lines.push(doc_text.to_string());
            idx -= 1;
        } else if trimmed.starts_with("/**") || trimmed.ends_with("*/") || trimmed.starts_with('*') {
            let cleaned = trimmed
                .trim_start_matches("/**")
                .trim_end_matches("*/")
                .trim_start_matches('*')
                .trim();
            doc_lines.push(cleaned.to_string());
            idx -= 1;
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        doc_lines.reverse();
        Some(doc_lines.join("\n"))
    }
}

fn parse_manifest_metadata(
    content: &str,
    name: &mut String,
    version: &mut String,
    description: &mut Option<String>,
    authors: &mut Vec<String>,
    license: &mut Option<String>,
    repository: &mut Option<String>,
) {
    let mut in_package_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if trimmed == "[package]" {
            in_package_section = true;
            continue;
        } else if trimmed.starts_with('[') {
            in_package_section = false;
            continue;
        }

        if in_package_section {
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');

                match key {
                    "name" => *name = val.to_string(),
                    "version" => *version = val.to_string(),
                    "description" => *description = Some(val.to_string()),
                    "license" => *license = Some(val.to_string()),
                    "repository" => *repository = Some(val.to_string()),
                    "authors" => {
                        // Authors might be an array: ["Author 1", "Author 2"]
                        let val_inner = v.trim().trim_matches('[').trim_matches(']');
                        for a in val_inner.split(',') {
                            let clean_a = a.trim().trim_matches('"').trim_matches('\'');
                            if !clean_a.is_empty() {
                                authors.push(clean_a.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_function_and_docs() {
        let code = r#"
//! Math utilities module.

/// Adds two numbers together.
///
/// @param a First operand
/// @param b Second operand
/// @return The sum
func add(a: int, b: int) -> int {
    return a + b;
}

/// Represents a 2D coordinate.
class Point {
    x: float,
    y: float,
}
"#;
        let tmp_file = std::env::temp_dir().join("test_math.roze");
        fs::write(&tmp_file, code).unwrap();

        let module = extract_module(&tmp_file, Path::new("/tmp")).unwrap();

        assert_eq!(module.doc.as_deref(), Some("Math utilities module."));
        assert_eq!(module.functions.len(), 1);
        let f = &module.functions[0];
        assert_eq!(f.name, "add");
        assert_eq!(f.signature, "func add(a: int, b: int) -> int");
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name, "a");
        assert_eq!(f.params[0].description.as_deref(), Some("First operand"));
        assert_eq!(f.return_doc.as_deref(), Some("The sum"));

        assert_eq!(module.classes.len(), 1);
        assert_eq!(module.classes[0].name, "Point");

        let _ = fs::remove_file(tmp_file);
    }
}
