// tools/roze-lsp/src/parser.rs
//
// Enhanced parser & AST symbol extraction for Roze Language Server Protocol.
// Maps Roze compiler AST structures into high-fidelity semantic symbols
// with exact positions, docstrings, and cross-reference records.

use ropey::Rope;
use roze_compiler::lexer::tokenize;
use roze_compiler::parser::ast::{
    Expression, MatchArm, Pattern, Program, Statement,
};
use roze_compiler::parser::parse as compiler_parse;

#[derive(Debug, Clone, Default)]
pub struct Ast {
    pub functions: Vec<Function>,
    pub variables: Vec<Variable>,
    pub classes: Vec<Class>,
    pub enums: Vec<Enum>,
    pub traits: Vec<Trait>,
    pub imports: Vec<Import>,
    pub usages: Vec<SymbolUsage>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<String>,
    pub param_details: Vec<ParamDetail>,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub is_extern: bool,
    /// 0-indexed line & column for LSP
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParamDetail {
    pub name: String,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub type_: Option<String>,
    pub is_mut: bool,
    pub line: usize,
    pub column: usize,
    pub scope_start: usize,
    pub scope_end: usize,
}

#[derive(Debug, Clone)]
pub struct ClassField {
    pub name: String,
    pub type_name: Option<String>,
    pub line: usize,
    pub column: usize,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<ClassField>,
    pub methods: Vec<Function>,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub types: Vec<String>,
    pub line: usize,
    pub column: usize,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub type_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Trait {
    pub name: String,
    pub methods: Vec<Function>,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsageKind {
    Call,
    TypeRef,
    VariableRef,
    FieldAccess,
    VariantRef,
}

#[derive(Debug, Clone)]
pub struct SymbolUsage {
    pub name: String,
    pub line: usize,
    pub column: usize,
    pub length: usize,
    pub kind: UsageKind,
}

/// Extracts doc comment lines directly preceding `line` (0-indexed).
pub fn extract_doc_comment(text: &Rope, line: usize) -> Option<String> {
    if line == 0 || line > text.len_lines() {
        return None;
    }

    let mut current_line = line;
    let mut comment_lines = Vec::new();

    while current_line > 0 {
        current_line -= 1;
        let line_text = text.line(current_line).to_string();
        let trimmed = line_text.trim();

        if trimmed.starts_with("///") {
            let doc_text = trimmed.strip_prefix("///").unwrap().trim();
            comment_lines.push(doc_text.to_string());
        } else if trimmed.starts_with("//") && !trimmed.starts_with("// =") && !trimmed.starts_with("// -") {
            let doc_text = trimmed.strip_prefix("//").unwrap().trim();
            comment_lines.push(doc_text.to_string());
        } else {
            break;
        }
    }

    if comment_lines.is_empty() {
        None
    } else {
        comment_lines.reverse();
        Some(comment_lines.join("\n"))
    }
}

/// Parses source text with real Roze compiler lexer + parser.
pub fn parse(source: &str) -> Option<Ast> {
    let text = Rope::from_str(source);
    let tokens = tokenize(source);
    let program: Program = compiler_parse(tokens).ok()?;

    let mut ast = Ast::default();
    let total_lines = text.len_lines();

    for stmt in &program.statements {
        collect_statement(stmt, &mut ast, &text, 0, total_lines);
    }

    Some(ast)
}

fn collect_statement(
    stmt: &Statement,
    ast: &mut Ast,
    text: &Rope,
    scope_start: usize,
    scope_end: usize,
) {
    match stmt {
        Statement::Function {
            name,
            type_params,
            params,
            return_type,
            body,
            is_async,
            location,
        } => {
            let line = location.line.saturating_sub(1);
            let column = location.column.saturating_sub(1);
            let doc_comment = extract_doc_comment(text, line);

            let param_details = params
                .iter()
                .map(|p| ParamDetail {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                })
                .collect::<Vec<_>>();

            let param_strs = params
                .iter()
                .map(|p| match &p.type_name {
                    Some(t) => format!("{}: {}", p.name, t),
                    None => p.name.clone(),
                })
                .collect::<Vec<_>>();

            // Register parameter variables in scope
            for p in params {
                ast.variables.push(Variable {
                    name: p.name.clone(),
                    type_: p.type_name.clone(),
                    is_mut: false,
                    line,
                    column,
                    scope_start: line,
                    scope_end,
                });
            }

            ast.functions.push(Function {
                name: name.clone(),
                type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                params: param_strs,
                param_details,
                return_type: return_type.clone(),
                is_async: *is_async,
                is_extern: false,
                line,
                column,
                end_line: line + 1,
                end_column: column + name.len(),
                doc_comment,
            });

            collect_statement(body, ast, text, line, scope_end);
        }
        Statement::ExternFunction {
            abi: _,
            name,
            params,
            return_type,
            location,
        } => {
            let line = location.line.saturating_sub(1);
            let column = location.column.saturating_sub(1);
            let doc_comment = extract_doc_comment(text, line);

            let param_details = params
                .iter()
                .map(|p| ParamDetail {
                    name: p.name.clone(),
                    type_name: p.type_name.clone(),
                })
                .collect::<Vec<_>>();

            let param_strs = params
                .iter()
                .map(|p| match &p.type_name {
                    Some(t) => format!("{}: {}", p.name, t),
                    None => p.name.clone(),
                })
                .collect::<Vec<_>>();

            ast.functions.push(Function {
                name: name.clone(),
                type_params: Vec::new(),
                params: param_strs,
                param_details,
                return_type: return_type.clone(),
                is_async: false,
                is_extern: true,
                line,
                column,
                end_line: line,
                end_column: column + name.len(),
                doc_comment,
            });
        }
        Statement::ClassDecl {
            name,
            type_params,
            fields,
            location,
        } => {
            let line = location.line.saturating_sub(1);
            let column = location.column.saturating_sub(1);
            let doc_comment = extract_doc_comment(text, line);

            let class_fields = fields
                .iter()
                .map(|f| ClassField {
                    name: f.name.clone(),
                    type_name: f.type_name.clone(),
                    line,
                    column,
                    doc_comment: None,
                })
                .collect::<Vec<_>>();

            ast.classes.push(Class {
                name: name.clone(),
                type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                fields: class_fields,
                methods: Vec::new(),
                line,
                column,
                end_line: line + fields.len() + 1,
                end_column: column + name.len(),
                doc_comment,
            });
        }
        Statement::EnumDecl {
            name,
            type_params,
            variants,
            location,
        } => {
            let line = location.line.saturating_sub(1);
            let column = location.column.saturating_sub(1);
            let doc_comment = extract_doc_comment(text, line);

            let enum_variants = variants
                .iter()
                .map(|v| EnumVariant {
                    name: v.name.clone(),
                    types: v.payload_types.clone(),
                    line: v.location.line.saturating_sub(1),
                    column: v.location.column.saturating_sub(1),
                    doc_comment: None,
                })
                .collect::<Vec<_>>();

            ast.enums.push(Enum {
                name: name.clone(),
                type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                variants: enum_variants,
                line,
                column,
                end_line: line + variants.len() + 1,
                end_column: column + name.len(),
                doc_comment,
            });
        }
        Statement::TraitDecl {
            name,
            associated_types: _,
            methods,
            location,
        } => {
            let line = location.line.saturating_sub(1);
            let column = location.column.saturating_sub(1);
            let doc_comment = extract_doc_comment(text, line);

            let trait_methods = methods
                .iter()
                .map(|m| Function {
                    name: m.name.clone(),
                    type_params: Vec::new(),
                    params: m
                        .params
                        .iter()
                        .map(|p| match &p.type_name {
                            Some(t) => format!("{}: {}", p.name, t),
                            None => p.name.clone(),
                        })
                        .collect(),
                    param_details: m
                        .params
                        .iter()
                        .map(|p| ParamDetail {
                            name: p.name.clone(),
                            type_name: p.type_name.clone(),
                        })
                        .collect(),
                    return_type: m.return_type.clone(),
                    is_async: false,
                    is_extern: false,
                    line: m.location.line.saturating_sub(1),
                    column: m.location.column.saturating_sub(1),
                    end_line: m.location.line.saturating_sub(1),
                    end_column: m.location.column.saturating_sub(1) + m.name.len(),
                    doc_comment: None,
                })
                .collect::<Vec<_>>();

            ast.traits.push(Trait {
                name: name.clone(),
                methods: trait_methods,
                line,
                column,
                end_line: line + methods.len() + 1,
                end_column: column + name.len(),
                doc_comment,
            });
        }
        Statement::ImplDecl {
            trait_name: _,
            class_name,
            associated_types: _,
            methods,
            location: _,
        } => {
            // Find existing class if present to attach methods
            for m in methods {
                collect_statement(m, ast, text, scope_start, scope_end);
            }
            if let Some(c) = ast.classes.iter_mut().find(|c| c.name == *class_name) {
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
                        c.methods.push(Function {
                            name: name.clone(),
                            type_params: type_params.iter().map(|tp| tp.name.clone()).collect(),
                            params: params
                                .iter()
                                .map(|p| match &p.type_name {
                                    Some(t) => format!("{}: {}", p.name, t),
                                    None => p.name.clone(),
                                })
                                .collect(),
                            param_details: params
                                .iter()
                                .map(|p| ParamDetail {
                                    name: p.name.clone(),
                                    type_name: p.type_name.clone(),
                                })
                                .collect(),
                            return_type: return_type.clone(),
                            is_async: *is_async,
                            is_extern: false,
                            line: location.line.saturating_sub(1),
                            column: location.column.saturating_sub(1),
                            end_line: location.line.saturating_sub(1),
                            end_column: location.column.saturating_sub(1) + name.len(),
                            doc_comment: None,
                        });
                    }
                }
            }
        }
        Statement::Let {
            name,
            type_name,
            value,
            location,
        } => {
            let line = location.line.saturating_sub(1);
            let column = location.column.saturating_sub(1);

            ast.variables.push(Variable {
                name: name.clone(),
                type_: type_name.clone(),
                is_mut: false,
                line,
                column,
                scope_start: line,
                scope_end,
            });

            collect_expression(value, ast);
        }
        Statement::Import { path, location } => {
            ast.imports.push(Import {
                path: path.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
            });
        }
        Statement::Block {
            statements,
            location,
        } => {
            let start = location.line.saturating_sub(1);
            for s in statements {
                collect_statement(s, ast, text, start, scope_end);
            }
        }
        Statement::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expression(condition, ast);
            collect_statement(then_branch, ast, text, scope_start, scope_end);
            if let Some(else_s) = else_branch {
                collect_statement(else_s, ast, text, scope_start, scope_end);
            }
        }
        Statement::While {
            condition, body, ..
        } => {
            collect_expression(condition, ast);
            collect_statement(body, ast, text, scope_start, scope_end);
        }
        Statement::For {
            init,
            condition,
            update,
            body,
            ..
        } => {
            collect_statement(init, ast, text, scope_start, scope_end);
            collect_expression(condition, ast);
            collect_statement(update, ast, text, scope_start, scope_end);
            collect_statement(body, ast, text, scope_start, scope_end);
        }
        Statement::Expression { expr, .. } => {
            collect_expression(expr, ast);
        }
        Statement::Return { value, .. } => {
            if let Some(val) = value {
                collect_expression(val, ast);
            }
        }
        Statement::Assign { name, value, location } => {
            ast.usages.push(SymbolUsage {
                name: name.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: name.len(),
                kind: UsageKind::VariableRef,
            });
            collect_expression(value, ast);
        }
        Statement::FieldAssign {
            object,
            field,
            value,
            location,
        } => {
            collect_expression(object, ast);
            ast.usages.push(SymbolUsage {
                name: field.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: field.len(),
                kind: UsageKind::FieldAccess,
            });
            collect_expression(value, ast);
        }
        Statement::Match { expr, arms, .. } => {
            collect_expression(expr, ast);
            for arm in arms {
                collect_match_arm(arm, ast, text, scope_start, scope_end);
            }
        }
        Statement::UnsafeBlock { body, .. } => {
            collect_statement(body, ast, text, scope_start, scope_end);
        }
        Statement::Comptime { statement, .. } => {
            collect_statement(statement, ast, text, scope_start, scope_end);
        }
        Statement::Annotated {
            annotations: _,
            statement,
            ..
        } => {
            collect_statement(statement, ast, text, scope_start, scope_end);
        }
        Statement::Asm { .. } => {}
    }
}

fn collect_match_arm(
    arm: &MatchArm,
    ast: &mut Ast,
    text: &Rope,
    scope_start: usize,
    scope_end: usize,
) {
    if let Some(guard) = &arm.guard {
        collect_expression(guard, ast);
    }
    collect_pattern(&arm.pattern, ast, arm.location.line.saturating_sub(1), scope_end);
    collect_statement(&arm.body, ast, text, scope_start, scope_end);
}

fn collect_pattern(pat: &Pattern, ast: &mut Ast, line: usize, scope_end: usize) {
    match pat {
        Pattern::Variable { name, location } => {
            let l = location.line.saturating_sub(1);
            let c = location.column.saturating_sub(1);
            ast.variables.push(Variable {
                name: name.clone(),
                type_: None,
                is_mut: false,
                line: l,
                column: c,
                scope_start: line,
                scope_end,
            });
        }
        Pattern::Variant {
            enum_name,
            variant,
            bindings,
            location,
        } => {
            if let Some(e_name) = enum_name {
                ast.usages.push(SymbolUsage {
                    name: e_name.clone(),
                    line: location.line.saturating_sub(1),
                    column: location.column.saturating_sub(1),
                    length: e_name.len(),
                    kind: UsageKind::TypeRef,
                });
            }
            ast.usages.push(SymbolUsage {
                name: variant.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: variant.len(),
                kind: UsageKind::VariantRef,
            });
            for b in bindings {
                collect_pattern(b, ast, line, scope_end);
            }
        }
        Pattern::Struct {
            class_name,
            fields,
            location,
        } => {
            ast.usages.push(SymbolUsage {
                name: class_name.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: class_name.len(),
                kind: UsageKind::TypeRef,
            });
            for (_, p) in fields {
                collect_pattern(p, ast, line, scope_end);
            }
        }
        Pattern::Tuple { elements, .. } | Pattern::Slice { prefix: elements, .. } => {
            for e in elements {
                collect_pattern(e, ast, line, scope_end);
            }
        }
        _ => {}
    }
}

fn collect_expression(expr: &Expression, ast: &mut Ast) {
    match expr {
        Expression::Identifier { name, location } => {
            ast.usages.push(SymbolUsage {
                name: name.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: name.len(),
                kind: UsageKind::VariableRef,
            });
        }
        Expression::Call {
            function,
            arguments,
            location,
        } => {
            if let Expression::Identifier { name, .. } = function.as_ref() {
                ast.usages.push(SymbolUsage {
                    name: name.clone(),
                    line: location.line.saturating_sub(1),
                    column: location.column.saturating_sub(1),
                    length: name.len(),
                    kind: UsageKind::Call,
                });
            } else {
                collect_expression(function, ast);
            }
            for arg in arguments {
                collect_expression(arg, ast);
            }
        }
        Expression::GenericCall {
            function,
            type_args,
            arguments,
            location,
        } => {
            ast.usages.push(SymbolUsage {
                name: function.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: function.len(),
                kind: UsageKind::Call,
            });
            for ta in type_args {
                ast.usages.push(SymbolUsage {
                    name: ta.clone(),
                    line: location.line.saturating_sub(1),
                    column: location.column.saturating_sub(1),
                    length: ta.len(),
                    kind: UsageKind::TypeRef,
                });
            }
            for arg in arguments {
                collect_expression(arg, ast);
            }
        }
        Expression::New {
            class_name,
            type_args: _,
            arguments,
            location,
        } => {
            ast.usages.push(SymbolUsage {
                name: class_name.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: class_name.len(),
                kind: UsageKind::TypeRef,
            });
            for arg in arguments {
                collect_expression(arg, ast);
            }
        }
        Expression::FieldAccess {
            object,
            field,
            location,
        } => {
            collect_expression(object, ast);
            ast.usages.push(SymbolUsage {
                name: field.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: field.len(),
                kind: UsageKind::FieldAccess,
            });
        }
        Expression::MethodCall {
            object,
            method,
            arguments,
            location,
        } => {
            collect_expression(object, ast);
            ast.usages.push(SymbolUsage {
                name: method.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: method.len(),
                kind: UsageKind::Call,
            });
            for arg in arguments {
                collect_expression(arg, ast);
            }
        }
        Expression::EnumConstruct {
            enum_name,
            variant,
            arguments,
            location,
        } => {
            if let Some(e_name) = enum_name {
                ast.usages.push(SymbolUsage {
                    name: e_name.clone(),
                    line: location.line.saturating_sub(1),
                    column: location.column.saturating_sub(1),
                    length: e_name.len(),
                    kind: UsageKind::TypeRef,
                });
            }
            ast.usages.push(SymbolUsage {
                name: variant.clone(),
                line: location.line.saturating_sub(1),
                column: location.column.saturating_sub(1),
                length: variant.len(),
                kind: UsageKind::VariantRef,
            });
            for arg in arguments {
                collect_expression(arg, ast);
            }
        }
        Expression::Binary { left, right, .. } => {
            collect_expression(left, ast);
            collect_expression(right, ast);
        }
        Expression::Unary { operand, .. } | Expression::Try { expr: operand, .. } => {
            collect_expression(operand, ast);
        }
        Expression::Await { expr, .. } => {
            collect_expression(expr, ast);
        }
        Expression::Unsafe { expr, .. } => {
            collect_expression(expr, ast);
        }
        Expression::Comptime { expr, .. } => {
            collect_expression(expr, ast);
        }
        Expression::Closure {
            params: _,
            return_type: _,
            body,
            ..
        } => {
            let text = Rope::new();
            collect_statement(body, ast, &text, 0, usize::MAX);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_top_level_function_with_docstring() {
        let source = "/// Calculates the sum of two integers.\n/// Returns the result.\nfunc add(a: int, b: int) -> int {\n    return a + b;\n}\n";
        let ast = parse(source).unwrap();
        assert_eq!(ast.functions.len(), 1);
        let f = &ast.functions[0];
        assert_eq!(f.name, "add");
        assert_eq!(f.return_type.as_deref(), Some("int"));
        assert_eq!(f.params, vec!["a: int", "b: int"]);
        assert!(f.doc_comment.is_some());
        assert!(f.doc_comment.as_ref().unwrap().contains("Calculates the sum"));
    }

    #[test]
    fn parses_classes_and_enums() {
        let source = "class Point {\n    x: int,\n    y: int,\n}\n\nenum Option {\n    Some(int),\n    None,\n}\n";
        let ast = parse(source).unwrap();
        assert_eq!(ast.classes.len(), 1);
        assert_eq!(ast.classes[0].name, "Point");
        assert_eq!(ast.classes[0].fields.len(), 2);
        assert_eq!(ast.classes[0].fields[0].name, "x");

        assert_eq!(ast.enums.len(), 1);
        assert_eq!(ast.enums[0].name, "Option");
        assert_eq!(ast.enums[0].variants.len(), 2);
    }

    #[test]
    fn parses_valid_source() {
        let valid_source = "import \"core\";\n\nfunc classify(x: int) -> string {\n    if x > 0 {\n        return \"positive\";\n    } else {\n        return \"non-positive\";\n    }\n}\n\nfunc main() {\n    for let i = 0; i < 3; i = i + 1 {\n        println(classify(i));\n    }\n    println(square(5));\n}\n";
        let ast = parse(valid_source).expect("should parse valid source");
        println!("ast functions: {:?}", ast.functions);
        assert_eq!(ast.functions.len(), 2);
    }
}
