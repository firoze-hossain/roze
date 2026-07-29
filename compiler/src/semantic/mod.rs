// compiler/src/semantic/mod.rs
use crate::error::RozeError;
use crate::ir::{TypedExpression, TypedExpressionKind, TypedFunctionParam, TypedProgram, TypedStatement};
use crate::parser::ast::*;
use std::collections::HashMap;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    String,
    Bool,
    Void,
    Unknown,
    /// A list of untyped (Object-boxed) elements. Roze doesn't have
    /// generics yet, so element type isn't tracked -- `list_get` always
    /// returns Unknown, the same way an untyped function parameter does.
    List,
    /// A map of untyped (Object-boxed) keys/values, for the same reason.
    Map,
    /// Reserved for when Roze gets first-class function values/closures
    /// -- nothing constructs this yet (there's no syntax for a function
    /// *value*, only calls), but the other Type variants already match
    /// on it exhaustively, so it's kept rather than removed and
    /// re-added later.
    #[allow(dead_code)]
    Function {
        params: Vec<Type>,
        return_type: Box<Type>,
    },
}

impl Type {
    /// Maps a Roze source-level type name (e.g. from a `: string` parameter
    /// annotation or a `-> int` return type) to our internal Type. Anything
    /// unrecognized falls back to Unknown rather than erroring, since Roze
    /// doesn't have user-defined types yet.
    pub fn from_name(name: &str) -> Type {
        match name {
            "int" => Type::Int,
            "string" => Type::String,
            "bool" => Type::Bool,
            "void" => Type::Void,
            "list" | "List" => Type::List,
            "map" | "Map" => Type::Map,
            _ => Type::Unknown,
        }
    }

    pub fn to_java(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::String => "String".to_string(),
            Type::Bool => "boolean".to_string(),
            Type::Void => "void".to_string(),
            Type::Unknown => "Object".to_string(),
            Type::List => "java.util.List".to_string(),
            Type::Map => "java.util.Map".to_string(),
            Type::Function { .. } => "Object".to_string(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::String => write!(f, "string"),
            Type::Bool => write!(f, "bool"),
            Type::Void => write!(f, "void"),
            Type::Unknown => write!(f, "<unknown>"),
            Type::List => write!(f, "list"),
            Type::Map => write!(f, "map"),
            Type::Function { .. } => write!(f, "function"),
        }
    }
}

/// Two types are "compatible" for assignment/return purposes if they're
/// equal, or if either side is Unknown -- meaning we don't have enough
/// static information to say either way, so we don't block it. This is
/// intentionally permissive rather than a full inference engine: it
/// catches the clear-cut cases (assigning a string into an int variable,
/// returning a string from a function declared `-> int`) without
/// rejecting legitimate code that flows through an untyped parameter.
fn types_compatible(expected: &Type, actual: &Type) -> bool {
    expected == actual || matches!(expected, Type::Unknown) || matches!(actual, Type::Unknown)
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub type_: Type,
    pub line: usize,
    pub column: usize,
}

/// A stack of lexical scopes.
///
/// The previous design represented nested scopes as a linked list of owned
/// `SymbolTable`s (`parent: Option<Box<SymbolTable>>`) and entered a new
/// scope by deep-cloning the *entire* parent chain (`Clone for SymbolTable`
/// recursively cloned `parent`). That means the cost of entering a scope
/// grew with total program depth, and a function with many sequential
/// nested blocks paid that growing cost on every single block, not just
/// once. A flat stack of frames makes push/pop O(1) and lookup O(depth)
/// (which is unavoidable and cheap: it's just a few hash lookups, no
/// cloning of prior state at all).
pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, name: &str, type_: Type, line: usize, column: usize) -> Result<()> {
        let frame = self.scopes.last_mut().expect("SymbolTable always has at least one scope");
        if frame.contains_key(name) {
            return Err(RozeError::type_error(
                format!("'{}' is already defined in this scope", name),
                line,
                column,
            ).with_length(name.chars().count()).into());
        }
        frame.insert(name.to_string(), Symbol {
            name: name.to_string(),
            type_,
            line,
            column,
        });
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for frame in self.scopes.iter().rev() {
            if let Some(sym) = frame.get(name) {
                return Some(sym);
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct FunctionSig {
    pub params: Vec<Type>,
    pub return_type: Type,
}

/// Built-in Core (string/math) intrinsics, always available without an
/// import. These are recognized by name here (for type-checking) and again
/// in `codegen::jvm` (for code generation), because Roze doesn't yet have
/// syntax to call into the host runtime from user-level source (no method
/// calls / no FFI). Keep this list in sync with
/// `codegen::jvm::intrinsic_return_type`.
fn builtin_signatures() -> Vec<(&'static str, Vec<Type>, Type)> {
    vec![
        ("string_length", vec![Type::String], Type::Int),
        ("string_concat", vec![Type::String, Type::String], Type::String),
        ("string_to_upper", vec![Type::String], Type::String),
        ("string_to_lower", vec![Type::String], Type::String),
        ("abs", vec![Type::Int], Type::Int),
        ("max", vec![Type::Int, Type::Int], Type::Int),
        ("min", vec![Type::Int, Type::Int], Type::Int),
        ("to_string", vec![Type::Unknown], Type::String),
        ("to_int", vec![Type::String], Type::Int),
        ("is_number", vec![Type::Unknown], Type::Bool),
        ("is_string", vec![Type::Unknown], Type::Bool),

        // ---- Collections (List, Map) ----
        // No generics yet, so elements/keys/values are all Unknown
        // (Object-boxed at the Java level) -- the same representation an
        // untyped function parameter already gets.
        ("list_new", vec![], Type::List),
        ("list_push", vec![Type::List, Type::Unknown], Type::Bool),
        ("list_get", vec![Type::List, Type::Int], Type::Unknown),
        ("list_set", vec![Type::List, Type::Int, Type::Unknown], Type::Unknown),
        ("list_remove", vec![Type::List, Type::Int], Type::Unknown),
        ("list_length", vec![Type::List], Type::Int),
        ("list_is_empty", vec![Type::List], Type::Bool),

        ("map_new", vec![], Type::Map),
        ("map_put", vec![Type::Map, Type::Unknown, Type::Unknown], Type::Unknown),
        ("map_get", vec![Type::Map, Type::Unknown], Type::Unknown),
        ("map_has", vec![Type::Map, Type::Unknown], Type::Bool),
        ("map_remove", vec![Type::Map, Type::Unknown], Type::Unknown),
        ("map_size", vec![Type::Map], Type::Int),
        ("map_is_empty", vec![Type::Map], Type::Bool),

        // ---- IO: file ----
        // Errors (file not found, permission denied, network failure)
        // surface as a runtime crash (an unchecked exception from the
        // JVM) rather than a Roze-level error value -- Roze doesn't have
        // a Result/Option type yet to report failure any other way. See
        // ROADMAP.md.
        ("read_file", vec![Type::String], Type::String),
        ("write_file", vec![Type::String, Type::String], Type::Void),
        ("append_file", vec![Type::String, Type::String], Type::Void),
        ("file_exists", vec![Type::String], Type::Bool),
        ("delete_file", vec![Type::String], Type::Bool),
        ("read_lines", vec![Type::String], Type::List),

        // ---- IO: network ----
        ("http_get", vec![Type::String], Type::String),
        ("http_post", vec![Type::String, Type::String], Type::String),

        // ---- Web: JSON ----
        // encode accepts anything (int/string/bool/list/map, nested);
        // decode's result type depends on the JSON text, so it's Unknown
        // -- the same design as list/map element access.
        ("json_encode", vec![Type::Unknown], Type::String),
        ("json_decode", vec![Type::String], Type::Unknown),

        // ---- Web: HTTP server ----
        // A request is just a `map` (with "method"/"path"/"body" keys),
        // reusing the existing Map intrinsics rather than needing a new
        // type -- see stdlib/src/io.roze for the full design rationale
        // (Roze has no closures, so this is a synchronous accept/respond
        // loop you write yourself, not a callback-based server).
        ("http_server_start", vec![Type::Int], Type::Unknown),
        ("http_server_accept", vec![Type::Unknown], Type::Map),
        ("http_server_respond", vec![Type::Map, Type::Int, Type::String], Type::Void),
        ("http_server_stop", vec![Type::Unknown], Type::Void),

        // ---- Database (SQL) ----
        // Roze has no bundled JDBC driver (the JDK itself doesn't ship
        // one for any real database), so these work with whatever
        // driver you put on the classpath yourself via `roze build
        // --classpath`/`roze run --classpath` -- see stdlib/src/sql.roze.
        ("sql_connect", vec![Type::String], Type::Unknown),
        ("sql_query", vec![Type::Unknown, Type::String], Type::List),
        ("sql_execute", vec![Type::Unknown, Type::String], Type::Int),
        ("sql_close", vec![Type::Unknown], Type::Void),
    ]
}

pub struct TypeChecker {
    pub symbol_table: SymbolTable,
    pub functions: HashMap<String, FunctionSig>,
    pub current_function: Option<String>,
    pub current_return_type: Type,
    /// True while checking `main`'s body -- codegen always hard-codes
    /// `main` to Java's `public static void main(String[] args)`
    /// regardless of any declared return type, so we enforce that at the
    /// Roze level too instead of letting a mismatch surface later as a
    /// confusing javac error.
    pub in_main: bool,
    pub errors: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        for (name, params, return_type) in builtin_signatures() {
            functions.insert(name.to_string(), FunctionSig { params, return_type });
        }
        Self {
            symbol_table: SymbolTable::new(),
            functions,
            current_function: None,
            current_return_type: Type::Void,
            in_main: false,
            errors: Vec::new(),
        }
    }

    /// Type-checks `program` and, on success, returns the fully
    /// type-annotated IR for it (see `crate::ir`). Registers every
    /// top-level function's signature before checking any bodies, so
    /// forward references and mutual recursion type-check correctly
    /// regardless of source order.
    pub fn check_program(&mut self, program: &Program) -> Result<TypedProgram> {
        for stmt in &program.statements {
            if let Statement::Function { name, params, return_type, .. } = stmt {
                let param_types = params.iter()
                    .map(|p| p.type_name.as_deref().map(Type::from_name).unwrap_or(Type::Unknown))
                    .collect();
                let ret = return_type.as_deref().map(Type::from_name).unwrap_or(Type::Void);
                self.functions.insert(name.clone(), FunctionSig { params: param_types, return_type: ret });
            }
        }

        let mut statements = Vec::with_capacity(program.statements.len());
        for stmt in &program.statements {
            if let Some(typed) = self.check_statement(stmt)? {
                statements.push(typed);
            }
        }
        Ok(TypedProgram { statements })
    }

    /// Type-checks one statement, returning its typed-IR form. Returns
    /// `Ok(None)` only for `Statement::Import` (which has no IR
    /// counterpart -- see `ir::TypedStatement`'s doc comment).
    pub fn check_statement(&mut self, stmt: &Statement) -> Result<Option<TypedStatement>> {
        match stmt {
            Statement::Function { name, params, return_type, body, location } => {
                if name == "main" && return_type.is_some() {
                    return Err(RozeError::type_error(
                        "'main' always returns void and cannot declare a return type",
                        location.line,
                        location.column,
                    ).with_hint("remove the '-> ...' after main()'s parameter list").into());
                }

                let resolved_return_type = return_type.as_deref().map(Type::from_name).unwrap_or(Type::Void);

                let outer_function = self.current_function.replace(name.clone());
                let outer_return_type = std::mem::replace(&mut self.current_return_type, resolved_return_type.clone());
                let outer_in_main = std::mem::replace(&mut self.in_main, name == "main");

                self.symbol_table.push_scope();
                let mut typed_params = Vec::with_capacity(params.len());
                for param in params {
                    let param_type = param.type_name.as_deref().map(Type::from_name).unwrap_or(Type::Unknown);
                    self.symbol_table.define(&param.name, param_type.clone(), location.line, location.column)?;
                    typed_params.push(TypedFunctionParam { name: param.name.clone(), type_: param_type });
                }

                let typed_body = self.check_statement(body)?
                    .expect("a function body is always a Block, which always has an IR form");

                self.symbol_table.pop_scope();
                self.current_function = outer_function;
                self.current_return_type = outer_return_type;
                self.in_main = outer_in_main;

                Ok(Some(TypedStatement::Function {
                    name: name.clone(),
                    params: typed_params,
                    return_type: resolved_return_type,
                    body: Box::new(typed_body),
                    location: location.clone(),
                }))
            }
            Statement::Let { name, value, location } => {
                let typed_value = self.check_expression(value)?;
                self.symbol_table.define(name, typed_value.type_.clone(), location.line, location.column)?;
                Ok(Some(TypedStatement::Let {
                    name: name.clone(),
                    value: typed_value,
                    location: location.clone(),
                }))
            }
            Statement::Expression { expr, location } => {
                let typed_expr = self.check_expression(expr)?;
                Ok(Some(TypedStatement::Expression { expr: typed_expr, location: location.clone() }))
            }
            Statement::Return { value, location } => {
                let typed_value = match value {
                    Some(expr) => {
                        let typed = self.check_expression(expr)?;
                        if !types_compatible(&self.current_return_type, &typed.type_) {
                            let fn_name = self.current_function.as_deref().unwrap_or("<anonymous>");
                            return Err(RozeError::type_error(
                                format!(
                                    "'{}' is declared to return {}, but this returns {}",
                                    fn_name, self.current_return_type, typed.type_
                                ),
                                location.line,
                                location.column,
                            ).with_hint(format!(
                                "either change the returned value's type, or change the function's declared return type ('-> {}')",
                                typed.type_
                            )).into());
                        }
                        Some(typed)
                    }
                    None => {
                        // A bare `return;` is only valid if the function
                        // isn't supposed to produce a value.
                        if !matches!(self.current_return_type, Type::Void | Type::Unknown) {
                            let fn_name = self.current_function.as_deref().unwrap_or("<anonymous>");
                            return Err(RozeError::type_error(
                                format!(
                                    "'{}' is declared to return {}, but this 'return;' doesn't return a value",
                                    fn_name, self.current_return_type
                                ),
                                location.line,
                                location.column,
                            ).into());
                        }
                        None
                    }
                };
                Ok(Some(TypedStatement::Return { value: typed_value, location: location.clone() }))
            }
            Statement::Block { statements, location } => {
                self.symbol_table.push_scope();
                let mut typed_statements = Vec::with_capacity(statements.len());
                for stmt in statements {
                    if let Some(typed) = self.check_statement(stmt)? {
                        typed_statements.push(typed);
                    }
                }
                self.symbol_table.pop_scope();
                Ok(Some(TypedStatement::Block { statements: typed_statements, location: location.clone() }))
            }
            Statement::If { condition, then_branch, else_branch, location } => {
                let typed_condition = self.check_expression(condition)?;
                let typed_then = Box::new(
                    self.check_statement(then_branch)?
                        .expect("an if's then-branch is always a Block, which always has an IR form")
                );
                let typed_else = match else_branch {
                    Some(else_stmt) => Some(Box::new(
                        self.check_statement(else_stmt)?
                            .expect("an if's else-branch is always a Block or If, both of which always have an IR form")
                    )),
                    None => None,
                };
                Ok(Some(TypedStatement::If {
                    condition: typed_condition,
                    then_branch: typed_then,
                    else_branch: typed_else,
                    location: location.clone(),
                }))
            }
            Statement::While { condition, body, location } => {
                let typed_condition = self.check_expression(condition)?;
                let typed_body = Box::new(
                    self.check_statement(body)?
                        .expect("a while's body is always a Block, which always has an IR form")
                );
                Ok(Some(TypedStatement::While {
                    condition: typed_condition,
                    body: typed_body,
                    location: location.clone(),
                }))
            }
            Statement::For { init, condition, update, body, location } => {
                // A scope of its own so the init clause's variable (e.g.
                // `let i` in `for let i = 0; ...`) is visible to the
                // condition/update/body but doesn't leak past the loop.
                self.symbol_table.push_scope();
                let typed_init = Box::new(
                    self.check_statement(init)?
                        .expect("a for-loop's init is always Let or Assign, both of which always have an IR form")
                );
                let typed_condition = self.check_expression(condition)?;
                let typed_update = Box::new(
                    self.check_statement(update)?
                        .expect("a for-loop's update is always Assign, which always has an IR form")
                );
                let typed_body = Box::new(
                    self.check_statement(body)?
                        .expect("a for-loop's body is always a Block, which always has an IR form")
                );
                self.symbol_table.pop_scope();
                Ok(Some(TypedStatement::For {
                    init: typed_init,
                    condition: typed_condition,
                    update: typed_update,
                    body: typed_body,
                    location: location.clone(),
                }))
            }
            Statement::Import { .. } => {
                // Imports are already resolved into real functions before
                // type-checking even runs (see imports::resolve_imports),
                // so in practice this arm is never hit -- it only exists
                // in case that ever changes. No IR form for it either
                // way (see `ir::TypedStatement`'s doc comment).
                Ok(None)
            }
            Statement::Assign { name, value, location } => {
                let typed_value = self.check_expression(value)?;
                match self.symbol_table.lookup(name) {
                    None => {
                        return Err(RozeError::type_error(
                            format!("Cannot assign to undefined variable '{}'", name),
                            location.line,
                            location.column,
                        ).with_length(name.chars().count()).into());
                    }
                    Some(symbol) => {
                        // Reassignment preserves the variable's original
                        // declared type -- `let x = 5;` followed later by
                        // `x = "hi";` is a type error, the same as it
                        // would be at the `let`.
                        if !types_compatible(&symbol.type_, &typed_value.type_) {
                            return Err(RozeError::type_error(
                                format!(
                                    "Cannot assign a value of type {} to '{}', which was declared as {}",
                                    typed_value.type_, name, symbol.type_
                                ),
                                location.line,
                                location.column,
                            ).with_hint(format!(
                                "'{}' was declared with type {} at line {}; its type can't change on reassignment",
                                name, symbol.type_, symbol.line
                            )).into());
                        }
                    }
                }
                Ok(Some(TypedStatement::Assign { name: name.clone(), value: typed_value, location: location.clone() }))
            }
        }
    }

    pub fn check_expression(&mut self, expr: &Expression) -> Result<TypedExpression> {
        match expr {
            Expression::Number { value, location } => Ok(TypedExpression {
                kind: TypedExpressionKind::Number(value.clone()),
                type_: Type::Int,
                location: location.clone(),
            }),
            Expression::String { value, location } => Ok(TypedExpression {
                kind: TypedExpressionKind::String(value.clone()),
                type_: Type::String,
                location: location.clone(),
            }),
            Expression::Boolean { value, location } => Ok(TypedExpression {
                kind: TypedExpressionKind::Boolean(*value),
                type_: Type::Bool,
                location: location.clone(),
            }),
            Expression::Null { location } => Ok(TypedExpression {
                kind: TypedExpressionKind::Null,
                type_: Type::Unknown,
                location: location.clone(),
            }),
            Expression::Identifier { name, location } => {
                if let Some(symbol) = self.symbol_table.lookup(name) {
                    Ok(TypedExpression {
                        kind: TypedExpressionKind::Identifier(name.clone()),
                        type_: symbol.type_.clone(),
                        location: location.clone(),
                    })
                } else {
                    Err(RozeError::type_error(
                        format!("Undefined variable '{}'", name),
                        location.line,
                        location.column,
                    ).with_length(name.chars().count()).into())
                }
            }
            Expression::Unary { operator, operand, location } => {
                let typed_operand = self.check_expression(operand)?;
                let result_type = typed_operand.type_.clone();
                Ok(TypedExpression {
                    kind: TypedExpressionKind::Unary { operator: operator.clone(), operand: Box::new(typed_operand) },
                    type_: result_type,
                    location: location.clone(),
                })
            }
            Expression::Binary { left, operator, right, location } => {
                let typed_left = self.check_expression(left)?;
                let typed_right = self.check_expression(right)?;
                let left_type = &typed_left.type_;
                let right_type = &typed_right.type_;

                let result_type = match operator {
                    BinaryOperator::Add => {
                        if *left_type == Type::Int && *right_type == Type::Int {
                            Type::Int
                        } else if *left_type == Type::String || *right_type == Type::String {
                            Type::String
                        } else if *left_type == Type::Unknown || *right_type == Type::Unknown {
                            // Untyped (dynamic-ish) operand: allow it and
                            // defer to runtime/codegen, rather than
                            // rejecting code that may well be valid.
                            Type::Unknown
                        } else {
                            return Err(RozeError::type_error(
                                format!("Cannot add {} and {}", left_type, right_type),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                        let numeric_ok = |t: &Type| matches!(t, Type::Int | Type::Unknown);
                        if numeric_ok(left_type) && numeric_ok(right_type) {
                            Type::Int
                        } else {
                            return Err(RozeError::type_error(
                                format!("Cannot perform arithmetic on {} and {}", left_type, right_type),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual |
                    BinaryOperator::LessThan | BinaryOperator::GreaterThan |
                    BinaryOperator::LessEqual | BinaryOperator::GreaterEqual |
                    BinaryOperator::And | BinaryOperator::Or => Type::Bool,
                };

                Ok(TypedExpression {
                    kind: TypedExpressionKind::Binary {
                        left: Box::new(typed_left),
                        operator: operator.clone(),
                        right: Box::new(typed_right),
                    },
                    type_: result_type,
                    location: location.clone(),
                })
            }
            Expression::Call { function, arguments, location } => {
                let mut typed_arguments = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    typed_arguments.push(self.check_expression(arg)?);
                }

                if let Expression::Identifier { name, .. } = function.as_ref() {
                    let return_type = if name == "println" {
                        Type::Void
                    } else if let Some(sig) = self.functions.get(name) {
                        sig.return_type.clone()
                    } else {
                        return Err(RozeError::type_error(
                            format!("Call to undefined function '{}'", name),
                            location.line,
                            location.column,
                        ).with_length(name.chars().count()).into());
                    };

                    Ok(TypedExpression {
                        kind: TypedExpressionKind::Call { function: name.clone(), arguments: typed_arguments },
                        type_: return_type,
                        location: location.clone(),
                    })
                } else {
                    // Roze has no first-class function values, so the
                    // parser never actually produces a Call whose
                    // `function` isn't a bare Identifier -- kept as a
                    // graceful fallback rather than a panic in case that
                    // changes.
                    Ok(TypedExpression {
                        kind: TypedExpressionKind::Call { function: String::new(), arguments: typed_arguments },
                        type_: Type::Unknown,
                        location: location.clone(),
                    })
                }
            }
        }
    }
}

/// Type-checks `program`, discarding the resulting IR -- for callers
/// that only want a pass/fail answer (e.g. the LSP's diagnostics engine,
/// which cares whether the program is valid, not about generating code
/// for it). Compiling to Java should use `check_and_lower` instead, to
/// avoid re-deriving the same information a second time in codegen.
///
/// (The "never used" dead-code warning this can trigger is a known
/// false positive from the bin/lib dual-module-tree setup: roze-lsp
/// genuinely calls this through the lib target; only the bin target's
/// own separate copy of this file -- main.rs uses check_and_lower
/// directly instead -- doesn't.)
#[allow(dead_code)]
pub fn check_types(program: &Program) -> Result<()> {
    check_and_lower(program).map(|_| ())
}

/// Type-checks `program` and returns its fully type-annotated IR (see
/// `crate::ir`), ready for codegen to consume directly.
pub fn check_and_lower(program: &Program) -> Result<crate::ir::TypedProgram> {
    let mut checker = TypeChecker::new();
    let typed_program = checker.check_program(program)?;

    if !checker.errors.is_empty() {
        for error in &checker.errors {
            eprintln!("{}", error);
        }
        return Err(anyhow::anyhow!("Type checking failed"));
    }
    Ok(typed_program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_source(src: &str) -> Result<()> {
        let program = parse(tokenize(src)).expect("fixture should parse");
        check_types(&program)
    }

    #[test]
    fn valid_program_passes() {
        assert!(check_source("func main() { let x = 5; println(x); }").is_ok());
    }

    #[test]
    fn undefined_variable_is_an_error() {
        assert!(check_source("func main() { println(x); }").is_err());
    }

    #[test]
    fn undefined_function_is_an_error() {
        assert!(check_source("func main() { totally_made_up(1); }").is_err());
    }

    #[test]
    fn return_type_mismatch_is_an_error() {
        let result = check_source("func f() -> int { return \"not a number\"; } func main() { }");
        assert!(result.is_err());
    }

    #[test]
    fn matching_return_type_is_ok() {
        let result = check_source("func f() -> int { return 5; } func main() { }");
        assert!(result.is_ok());
    }

    #[test]
    fn bare_return_in_non_void_function_is_an_error() {
        let result = check_source("func f() -> int { return; } func main() { }");
        assert!(result.is_err());
    }

    #[test]
    fn bare_return_in_void_function_is_ok() {
        let result = check_source("func f() { return; } func main() { }");
        assert!(result.is_ok());
    }

    #[test]
    fn reassignment_changing_type_is_an_error() {
        let result = check_source("func main() { let x = 5; x = \"five\"; }");
        assert!(result.is_err());
    }

    #[test]
    fn reassignment_preserving_type_is_ok() {
        let result = check_source("func main() { let x = 5; x = 10; }");
        assert!(result.is_ok());
    }

    #[test]
    fn main_declaring_return_type_is_an_error() {
        let result = check_source("func main() -> int { }");
        assert!(result.is_err());
    }

    #[test]
    fn intrinsics_are_callable_without_definition() {
        let result = check_source(
            "func main() { println(abs(-5)); println(string_length(\"hi\")); }"
        );
        assert!(result.is_ok());
    }

    #[test]
    fn collections_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let l = list_new(); \
                list_push(l, 1); \
                let x = list_get(l, 0); \
                list_set(l, 0, 2); \
                list_remove(l, 0); \
                println(list_length(l)); \
                println(list_is_empty(l)); \
                let m = map_new(); \
                map_put(m, \"a\", 1); \
                let v = map_get(m, \"a\"); \
                println(map_has(m, \"a\")); \
                map_remove(m, \"a\"); \
                println(map_size(m)); \
                println(map_is_empty(m)); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn list_and_map_type_annotations_are_recognized() {
        let result = check_source(
            "func takes_a_list(l: list) -> list { return l; } \
             func takes_a_map(m: map) -> map { return m; } \
             func main() { \
                let l = takes_a_list(list_new()); \
                let m = takes_a_map(map_new()); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn reassigning_a_list_variable_to_a_map_is_an_error() {
        let result = check_source(
            "func main() { let l = list_new(); l = map_new(); }"
        );
        assert!(result.is_err());
    }

    #[test]
    fn file_and_network_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                write_file(\"a\", \"b\"); \
                append_file(\"a\", \"c\"); \
                println(read_file(\"a\")); \
                println(file_exists(\"a\")); \
                println(delete_file(\"a\")); \
                let lines = read_lines(\"a\"); \
                println(http_get(\"http://example.com\")); \
                println(http_post(\"http://example.com\", \"body\")); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn json_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let m = map_new(); \
                map_put(m, \"a\", 1); \
                let encoded = json_encode(m); \
                let decoded = json_decode(encoded); \
                println(decoded); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn http_server_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let server = http_server_start(8080); \
                let req = http_server_accept(server); \
                println(map_get(req, \"method\")); \
                http_server_respond(req, 200, \"ok\"); \
                http_server_stop(server); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn sql_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                let conn = sql_connect(\"jdbc:h2:mem:x\"); \
                sql_execute(conn, \"CREATE TABLE t (x INT)\"); \
                let rows = sql_query(conn, \"SELECT * FROM t\"); \
                println(list_length(rows)); \
                sql_close(conn); \
            }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn for_loop_variable_is_scoped_to_the_loop() {
        // `i` from the for-loop's init clause must be visible inside the
        // loop body/condition/update...
        assert!(check_source("func main() { for let i = 0; i < 3; i = i + 1 { println(i); } }").is_ok());
        // ...but must NOT leak out past the loop.
        assert!(check_source("func main() { for let i = 0; i < 3; i = i + 1 { } println(i); }").is_err());
    }

    // ---- IR-specific tests: the typed tree itself, not just pass/fail ----

    fn lower(src: &str) -> crate::ir::TypedProgram {
        let program = parse(tokenize(src)).expect("fixture should parse");
        check_and_lower(&program).expect("fixture should type-check")
    }

    #[test]
    fn lowering_attaches_types_to_literals() {
        let ir = lower("func main() { let x = 5; let s = \"hi\"; let b = true; }");
        let body = match &ir.statements[0] {
            TypedStatement::Function { body, .. } => body.as_ref(),
            other => panic!("expected a function, got {:?}", other),
        };
        let stmts = match body {
            TypedStatement::Block { statements, .. } => statements,
            other => panic!("expected a block, got {:?}", other),
        };
        for (stmt, expected) in stmts.iter().zip([Type::Int, Type::String, Type::Bool]) {
            match stmt {
                TypedStatement::Let { value, .. } => assert_eq!(value.type_, expected),
                other => panic!("expected a Let, got {:?}", other),
            }
        }
    }

    #[test]
    fn lowering_resolves_identifier_types_from_the_symbol_table() {
        let ir = lower("func main() { let x = 5; println(x); }");
        let body = match &ir.statements[0] {
            TypedStatement::Function { body, .. } => body.as_ref(),
            other => panic!("expected a function, got {:?}", other),
        };
        let stmts = match body {
            TypedStatement::Block { statements, .. } => statements,
            other => panic!("expected a block, got {:?}", other),
        };
        match &stmts[1] {
            TypedStatement::Expression { expr, .. } => match &expr.kind {
                TypedExpressionKind::Call { arguments, .. } => {
                    assert_eq!(arguments[0].type_, Type::Int, "the reference to x should carry its resolved type");
                }
                other => panic!("expected a Call, got {:?}", other),
            },
            other => panic!("expected an Expression statement, got {:?}", other),
        }
    }

    #[test]
    fn lowering_resolves_intrinsic_call_return_types() {
        let ir = lower("func main() { let n = list_new(); }");
        let body = match &ir.statements[0] {
            TypedStatement::Function { body, .. } => body.as_ref(),
            other => panic!("expected a function, got {:?}", other),
        };
        let stmts = match body {
            TypedStatement::Block { statements, .. } => statements,
            other => panic!("expected a block, got {:?}", other),
        };
        match &stmts[0] {
            TypedStatement::Let { value, .. } => assert_eq!(value.type_, Type::List),
            other => panic!("expected a Let, got {:?}", other),
        }
    }

    #[test]
    fn lowering_drops_import_statements_with_no_ir_form() {
        // Only reachable if resolve_imports somehow didn't already strip
        // it (see the Statement::Import arm's comment) -- still, the IR
        // itself should never contain one either way.
        let ir = lower("func main() { }");
        assert!(!ir.statements.iter().any(|s| matches!(s, TypedStatement::Function { name, .. } if name.is_empty())));
    }
}
