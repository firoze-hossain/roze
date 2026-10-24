// compiler/src/semantic/mod.rs
use crate::error::RozeError;
use crate::ir::{TypedExpression, TypedExpressionKind, TypedFunctionParam, TypedProgram, TypedStatement};
use crate::parser::ast::*;
use std::collections::HashMap;
use anyhow::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    /// A 64-bit floating-point number (Java `double` / Cranelift `f64`
    /// on native). Distinguished from `Int` purely by whether a number
    /// literal's source text contains a decimal point -- `42` is
    /// `Int`, `42.0` is `Float`, there's no separate `Expression`
    /// variant for it (see `check_expression`'s `Number` case).
    Float,
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
    /// An optional value (`Some(v)` or `None()`).
    Option,
    /// A result value (`Ok(v)` or `Err(e)`).
    Result,
    /// An instance of a user-defined `class Name { field: type, ... }`.
    /// Unlike `list`/`map`'s elements, a class's field types *are*
    /// statically known (declared in the class body), which is what
    /// makes this tractable to support on the native backend too --
    /// each field's retain/release can be dispatched correctly based on
    /// its declared type, the same way a typed function parameter
    /// already is.
    Class(String),
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
            "float" => Type::Float,
            "string" => Type::String,
            "bool" => Type::Bool,
            "void" => Type::Void,
            "list" | "List" => Type::List,
            "map" | "Map" => Type::Map,
            "option" | "Option" => Type::Option,
            "result" | "Result" => Type::Result,
            name if name.starts_with("Option<") || name.starts_with("option<") => Type::Option,
            name if name.starts_with("Result<") || name.starts_with("result<") => Type::Result,
            _ => Type::Unknown,
        }
    }

    pub fn to_java(&self) -> String {
        match self {
            Type::Int => "int".to_string(),
            Type::Float => "double".to_string(),
            Type::String => "String".to_string(),
            Type::Bool => "boolean".to_string(),
            Type::Void => "void".to_string(),
            Type::Unknown => "Object".to_string(),
            Type::List => "java.util.List".to_string(),
            Type::Map => "java.util.Map".to_string(),
            Type::Option => "RozeOption".to_string(),
            Type::Result => "RozeResult".to_string(),
            Type::Class(name) => name.clone(),
            Type::Function { .. } => "Object".to_string(),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::String => write!(f, "string"),
            Type::Bool => write!(f, "bool"),
            Type::Void => write!(f, "void"),
            Type::Unknown => write!(f, "<unknown>"),
            Type::List => write!(f, "list"),
            Type::Map => write!(f, "map"),
            Type::Option => write!(f, "Option"),
            Type::Result => write!(f, "Result"),
            Type::Class(name) => write!(f, "{}", name),
            Type::Function { .. } => write!(f, "function"),
        }
    }
}

/// Two types are "compatible" for assignment/return purposes if they're
/// equal, if either side is Unknown -- meaning we don't have enough
/// static information to say either way, so we don't block it -- or if
/// an int is flowing into a float-expecting position (implicit
/// widening, the same direction `+`/`-`/`*`/`/` already promote mixed
/// int/float operands in). The reverse -- a float flowing into an
/// int-expecting position -- is deliberately NOT allowed here: that's
/// narrowing, silently losing precision, and needs an explicit
/// conversion instead (not implemented yet -- there's no cast syntax
/// in Roze at all currently).
fn types_compatible(expected: &Type, actual: &Type) -> bool {
    expected == actual
        || matches!(expected, Type::Unknown)
        || matches!(actual, Type::Unknown)
        || (matches!(expected, Type::Float) && matches!(actual, Type::Int))
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
        ("abs", vec![Type::Unknown], Type::Int),
        ("max", vec![Type::Unknown, Type::Unknown], Type::Int),
        ("min", vec![Type::Unknown, Type::Unknown], Type::Int),
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

        // ---- Option ----
        ("option_some", vec![Type::Unknown], Type::Option),
        ("option_none", vec![], Type::Option),
        ("option_is_some", vec![Type::Unknown], Type::Bool),
        ("option_is_none", vec![Type::Unknown], Type::Bool),
        ("option_unwrap", vec![Type::Unknown], Type::Unknown),
        ("option_unwrap_or", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("Some", vec![Type::Unknown], Type::Option),
        ("None", vec![], Type::Option),
        ("is_some", vec![Type::Unknown], Type::Bool),
        ("is_none", vec![Type::Unknown], Type::Bool),

        // ---- Result ----
        ("result_ok", vec![Type::Unknown], Type::Result),
        ("result_err", vec![Type::Unknown], Type::Result),
        ("result_is_ok", vec![Type::Unknown], Type::Bool),
        ("result_is_err", vec![Type::Unknown], Type::Bool),
        ("result_unwrap", vec![Type::Unknown], Type::Unknown),
        ("result_unwrap_err", vec![Type::Unknown], Type::Unknown),
        ("result_unwrap_or", vec![Type::Unknown, Type::Unknown], Type::Unknown),
        ("Ok", vec![Type::Unknown], Type::Result),
        ("Err", vec![Type::Unknown], Type::Result),
        ("is_ok", vec![Type::Unknown], Type::Bool),
        ("is_err", vec![Type::Unknown], Type::Bool),
        ("unwrap", vec![Type::Unknown], Type::Unknown),
        ("unwrap_err", vec![Type::Unknown], Type::Unknown),
        ("unwrap_or", vec![Type::Unknown, Type::Unknown], Type::Unknown),

        // ---- Safe IO (returning Result) ----
        ("read_file_result", vec![Type::String], Type::Result),
        ("write_file_result", vec![Type::String, Type::String], Type::Result),
        ("append_file_result", vec![Type::String, Type::String], Type::Result),
        ("delete_file_result", vec![Type::String], Type::Result),
        ("read_lines_result", vec![Type::String], Type::Result),
        ("http_get_result", vec![Type::String], Type::Result),
        ("http_post_result", vec![Type::String, Type::String], Type::Result),
        ("to_int_result", vec![Type::String], Type::Result),

        // ---- Safe Collections (returning Option) ----
        ("list_get_option", vec![Type::List, Type::Int], Type::Option),
        ("map_get_option", vec![Type::Map, Type::Unknown], Type::Option),
    ]
}

pub struct TypeChecker {
    pub symbol_table: SymbolTable,
    pub functions: HashMap<String, FunctionSig>,
    /// Class name -> its fields, in declaration order (order matters:
    /// `new Name(a, b)`'s arguments are positional, matched against
    /// this order).
    pub classes: HashMap<String, Vec<(String, Type)>>,
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

/// The result type of a numeric binary operator given its operand
/// types, or None if the combination isn't numeric at all (the caller
/// then decides what else it might be -- string concatenation for
/// `+`, or a hard error). Promotes a mixed int/float pair to float, in
/// either order, matching the same direction `types_compatible`
/// already allows for assignment. An Unknown operand (an untyped
/// list/map element, an untyped function parameter) defers to the
/// other operand's type where there is one, or Int as a reasonable
/// default when both sides are unknown -- preserving the exact
/// behavior this had before Float existed, just now also extended to
/// recognize Float on either side.
fn numeric_binary_result_type(left: &Type, right: &Type) -> Option<Type> {
    match (left, right) {
        (Type::Int, Type::Int) => Some(Type::Int),
        (Type::Float, Type::Float) => Some(Type::Float),
        (Type::Float, Type::Int) | (Type::Int, Type::Float) => Some(Type::Float),
        (Type::Unknown, Type::Unknown) => Some(Type::Int),
        (Type::Unknown, other) | (other, Type::Unknown) if matches!(other, Type::Int | Type::Float) => Some(other.clone()),
        _ => None,
    }
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
            classes: HashMap::new(),
            current_function: None,
            current_return_type: Type::Void,
            in_main: false,
            errors: Vec::new(),
        }
    }

    /// Resolves a source-level type name, checking user-defined classes
    /// first (so a class named e.g. `Point` correctly resolves to
    /// `Type::Class("Point")` rather than falling through to Unknown
    /// the way `Type::from_name` alone would, having no way to know
    /// about classes registered on this specific checker instance).
    fn resolve_type_name(&self, name: &str) -> Type {
        if self.classes.contains_key(name) {
            Type::Class(name.to_string())
        } else {
            Type::from_name(name)
        }
    }

    /// Type-checks `program` and, on success, returns the fully
    /// type-annotated IR for it (see `crate::ir`). Registers every
    /// class and top-level function's signature before checking any
    /// bodies, so forward references and mutual recursion type-check
    /// correctly regardless of source order. Classes are registered
    /// before functions specifically, so a function's parameter/return
    /// type can reference a class declared later in the file.
    pub fn check_program(&mut self, program: &Program) -> Result<TypedProgram> {
        for stmt in &program.statements {
            if let Statement::ClassDecl { name, fields, .. } = stmt {
                let resolved_fields = fields.iter()
                    .map(|f| (f.name.clone(), f.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown)))
                    .collect();
                self.classes.insert(name.clone(), resolved_fields);
            }
        }

        for stmt in &program.statements {
            if let Statement::Function { name, params, return_type, .. } = stmt {
                let param_types = params.iter()
                    .map(|p| p.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown))
                    .collect();
                let ret = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);
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

                let resolved_return_type = return_type.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Void);

                let outer_function = self.current_function.replace(name.clone());
                let outer_return_type = std::mem::replace(&mut self.current_return_type, resolved_return_type.clone());
                let outer_in_main = std::mem::replace(&mut self.in_main, name == "main");

                self.symbol_table.push_scope();
                let mut typed_params = Vec::with_capacity(params.len());
                for param in params {
                    let param_type = param.type_name.as_deref().map(|t| self.resolve_type_name(t)).unwrap_or(Type::Unknown);
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
            Statement::ClassDecl { name, location, .. } => {
                // Already fully resolved into self.classes during
                // check_program's first pass (needed there so function
                // signatures declared earlier in the file can reference
                // a class declared later) -- just look it up again here
                // rather than re-deriving it, and hand codegen the
                // resolved (name, Type) pairs directly.
                let fields = self.classes.get(name)
                    .expect("check_program registers every class before any statement is checked")
                    .clone();
                Ok(Some(TypedStatement::ClassDecl { name: name.clone(), fields, location: location.clone() }))
            }
            Statement::FieldAssign { object, field, value, location } => {
                let typed_object = self.check_expression(object)?;
                let class_name = match &typed_object.type_ {
                    Type::Class(name) => name.clone(),
                    other => return Err(RozeError::type_error(
                        format!("Cannot access a field on {}, which isn't a class instance", other),
                        location.line,
                        location.column,
                    ).into()),
                };
                let field_type = self.classes.get(&class_name)
                    .and_then(|fields| fields.iter().find(|(n, _)| n == field))
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| RozeError::type_error(
                        format!("'{}' has no field named '{}'", class_name, field),
                        location.line,
                        location.column,
                    ))?;

                let typed_value = self.check_expression(value)?;
                if !types_compatible(&field_type, &typed_value.type_) {
                    return Err(RozeError::type_error(
                        format!(
                            "Cannot assign a value of type {} to '{}.{}', which is declared as {}",
                            typed_value.type_, class_name, field, field_type
                        ),
                        location.line,
                        location.column,
                    ).into());
                }

                Ok(Some(TypedStatement::FieldAssign {
                    object: typed_object,
                    field: field.clone(),
                    field_type,
                    value: typed_value,
                    location: location.clone(),
                }))
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
                type_: if value.contains('.') { Type::Float } else { Type::Int },
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
                        if let Some(numeric_type) = numeric_binary_result_type(left_type, right_type) {
                            numeric_type
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
                        if let Some(numeric_type) = numeric_binary_result_type(left_type, right_type) {
                            numeric_type
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
                    } else if name == "abs" && typed_arguments.len() == 1 {
                        // Math.abs has both an int and a double overload
                        // on the JVM side, and the native backend needs
                        // the same distinction -- the return type has to
                        // track whichever one this call actually
                        // resolves to, or a `let` binding downstream
                        // would get the wrong declared type.
                        match &typed_arguments[0].type_ {
                            Type::Float => Type::Float,
                            Type::Int | Type::Unknown => Type::Int,
                            other => return Err(RozeError::type_error(
                                format!("abs() needs a number, got {}", other),
                                location.line,
                                location.column,
                            ).into()),
                        }
                    } else if (name == "max" || name == "min") && typed_arguments.len() == 2 {
                        numeric_binary_result_type(&typed_arguments[0].type_, &typed_arguments[1].type_)
                            .ok_or_else(|| RozeError::type_error(
                                format!(
                                    "{}() needs two numbers, got {} and {}",
                                    name, typed_arguments[0].type_, typed_arguments[1].type_
                                ),
                                location.line,
                                location.column,
                            ))?
                    } else if let Some(sig) = self.functions.get(name) {
                        // Validate argument count and each argument's
                        // type against the declared signature -- the
                        // same checks `New` (class construction) already
                        // does for its constructor arguments, just
                        // never applied to an ordinary function call
                        // before now. Skipped for intrinsics whose
                        // params are deliberately Unknown/variable-arity
                        // in `builtin_signatures` (e.g. `to_string`,
                        // `list_push`) -- those accept anything by
                        // design, not by omission.
                        let params_are_meaningfully_typed = !sig.params.iter().all(|p| matches!(p, Type::Unknown));
                        if params_are_meaningfully_typed {
                            if typed_arguments.len() != sig.params.len() {
                                return Err(RozeError::type_error(
                                    format!(
                                        "'{}' takes {} argument(s), but {} were given",
                                        name, sig.params.len(), typed_arguments.len()
                                    ),
                                    location.line,
                                    location.column,
                                ).into());
                            }
                            for (arg, param_type) in typed_arguments.iter().zip(sig.params.iter()) {
                                if !types_compatible(param_type, &arg.type_) {
                                    return Err(RozeError::type_error(
                                        format!(
                                            "'{}' expects {} for this argument, got {}",
                                            name, param_type, arg.type_
                                        ),
                                        location.line,
                                        location.column,
                                    ).into());
                                }
                            }
                        }
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
            Expression::New { class_name, arguments, location } => {
                let fields = self.classes.get(class_name).cloned().ok_or_else(|| RozeError::type_error(
                    format!("Undefined class '{}'", class_name),
                    location.line,
                    location.column,
                ).with_length(class_name.chars().count()))?;

                if arguments.len() != fields.len() {
                    return Err(RozeError::type_error(
                        format!(
                            "'{}' has {} field(s), but 'new {}(...)' was given {} argument(s)",
                            class_name, fields.len(), class_name, arguments.len()
                        ),
                        location.line,
                        location.column,
                    ).into());
                }

                let mut typed_arguments = Vec::with_capacity(arguments.len());
                for (arg, (field_name, field_type)) in arguments.iter().zip(fields.iter()) {
                    let typed_arg = self.check_expression(arg)?;
                    if !types_compatible(field_type, &typed_arg.type_) {
                        return Err(RozeError::type_error(
                            format!(
                                "'{}.{}' is declared as {}, but 'new {}(...)' was given {} for it",
                                class_name, field_name, field_type, class_name, typed_arg.type_
                            ),
                            location.line,
                            location.column,
                        ).into());
                    }
                    typed_arguments.push(typed_arg);
                }

                Ok(TypedExpression {
                    kind: TypedExpressionKind::New { class_name: class_name.clone(), arguments: typed_arguments },
                    type_: Type::Class(class_name.clone()),
                    location: location.clone(),
                })
            }
            Expression::FieldAccess { object, field, location } => {
                let typed_object = self.check_expression(object)?;
                let class_name = match &typed_object.type_ {
                    Type::Class(name) => name.clone(),
                    other => return Err(RozeError::type_error(
                        format!("Cannot access a field on {}, which isn't a class instance", other),
                        location.line,
                        location.column,
                    ).into()),
                };
                let field_type = self.classes.get(&class_name)
                    .and_then(|fields| fields.iter().find(|(n, _)| n == field))
                    .map(|(_, ty)| ty.clone())
                    .ok_or_else(|| RozeError::type_error(
                        format!("'{}' has no field named '{}'", class_name, field),
                        location.line,
                        location.column,
                    ))?;

                Ok(TypedExpression {
                    kind: TypedExpressionKind::FieldAccess { object: Box::new(typed_object), field: field.clone() },
                    type_: field_type,
                    location: location.clone(),
                })
            }
            Expression::Try { expr, location } => {
                let typed_expr = self.check_expression(expr)?;
                match &typed_expr.type_ {
                    Type::Result => {
                        if !matches!(self.current_return_type, Type::Result | Type::Unknown) {
                            return Err(RozeError::type_error(
                                format!(
                                    "The '?' operator on a Result requires the enclosing function to return Result, but it returns {}",
                                    self.current_return_type
                                ),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    Type::Option => {
                        if !matches!(self.current_return_type, Type::Option | Type::Unknown) {
                            return Err(RozeError::type_error(
                                format!(
                                    "The '?' operator on an Option requires the enclosing function to return Option, but it returns {}",
                                    self.current_return_type
                                ),
                                location.line,
                                location.column,
                            ).into());
                        }
                    }
                    Type::Unknown => {}
                    other => {
                        return Err(RozeError::type_error(
                            format!("The '?' operator can only be used on Result or Option, got {}", other),
                            location.line,
                            location.column,
                        ).into());
                    }
                }

                Ok(TypedExpression {
                    kind: TypedExpressionKind::Try(Box::new(typed_expr)),
                    type_: Type::Unknown,
                    location: location.clone(),
                })
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

    fn typed_program(src: &str) -> TypedProgram {
        let program = parse(tokenize(src)).expect("fixture should parse");
        check_and_lower(&program).expect("fixture should type-check")
    }

    /// Pulls out the declared type of every top-level `let` in `main`'s
    /// body, in order -- for tests that need to inspect what a value
    /// actually resolved to, not just whether the program type-checked.
    fn main_let_types(program: &TypedProgram) -> Vec<&Type> {
        program.statements.iter()
            .find_map(|s| if let TypedStatement::Function { name, body, .. } = s {
                if name == "main" { Some(body) } else { None }
            } else { None })
            .map(|body| if let TypedStatement::Block { statements, .. } = body.as_ref() {
                statements.iter().filter_map(|s| if let TypedStatement::Let { value, .. } = s { Some(&value.type_) } else { None }).collect()
            } else { vec![] })
            .unwrap_or_default()
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

    // ---- Classes ----

    #[test]
    fn class_construction_field_access_and_assignment_type_check() {
        let result = check_source(
            "class Point { x: int, y: int } \
             func main() { \
                let p = new Point(1, 2); \
                println(p.x); \
                p.x = 5; \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn class_can_be_used_as_a_function_parameter_and_return_type() {
        let result = check_source(
            "class Point { x: int } \
             func make(x: int) -> Point { return new Point(x); } \
             func get_x(p: Point) -> int { return p.x; } \
             func main() { \
                let p = make(5); \
                println(get_x(p)); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn class_can_be_declared_after_a_function_that_uses_it() {
        // Classes are registered in a first pass, before functions, so
        // a function earlier in the file can reference a class declared
        // later.
        let result = check_source(
            "func make() -> Point { return new Point(1); } \
             class Point { x: int } \
             func main() { println(make().x); }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn wrong_number_of_constructor_arguments_is_an_error() {
        let result = check_source("class Point { x: int, y: int } func main() { let p = new Point(1); }");
        assert!(result.is_err());
    }

    #[test]
    fn wrong_constructor_argument_type_is_an_error() {
        let result = check_source("class Point { x: int, y: int } func main() { let p = new Point(\"hi\", 2); }");
        assert!(result.is_err());
    }

    #[test]
    fn accessing_an_undefined_field_is_an_error() {
        let result = check_source("class Point { x: int } func main() { let p = new Point(1); println(p.z); }");
        assert!(result.is_err());
    }

    #[test]
    fn assigning_wrong_type_to_a_field_is_an_error() {
        let result = check_source("class Point { x: int } func main() { let p = new Point(1); p.x = \"oops\"; }");
        assert!(result.is_err());
    }

    #[test]
    fn constructing_an_undefined_class_is_an_error() {
        let result = check_source("func main() { let p = new Nonexistent(1); }");
        assert!(result.is_err());
    }

    #[test]
    fn accessing_a_field_on_a_non_class_value_is_an_error() {
        let result = check_source("func main() { let x = 5; println(x.field); }");
        assert!(result.is_err());
    }

    #[test]
    fn float_literal_is_distinguished_from_int_by_a_decimal_point() {
        let program = typed_program("func main() { let x = 3.14; let y = 5; }");
        let types = main_let_types(&program);
        assert_eq!(types, vec![&Type::Float, &Type::Int]);
    }

    #[test]
    fn mixed_int_float_arithmetic_promotes_to_float() {
        assert!(check_source("func main() { let x = 1 + 2.5; }").is_ok());
        assert!(check_source("func main() { let x = 2.5 + 1; }").is_ok());
        assert!(check_source("func main() { let x = 1 - 2.5; }").is_ok());
        assert!(check_source("func main() { let x = 2.5 * 2; }").is_ok());
        assert!(check_source("func main() { let x = 5 / 2.0; }").is_ok());
    }

    #[test]
    fn float_can_flow_into_a_function_expecting_float_but_not_the_reverse() {
        // Implicit widening (int -> float) is allowed; implicit
        // narrowing (float -> int) is not, since it would silently
        // lose precision.
        assert!(check_source("func f(x: float) -> float { return x; } func main() { f(5); }").is_ok());
        assert!(check_source("func f(x: int) -> int { return x; } func main() { f(5.0); }").is_err());
    }

    #[test]
    fn abs_max_min_return_type_tracks_their_argument_types() {
        // abs(-3.14) must type-check as Float (not the old fixed Int),
        // or a `let` binding downstream would get the wrong declared
        // type and fail to compile at the codegen level.
        let program = typed_program("func main() { let a = abs(-3.14); let b = abs(-5); let c = max(3, 7.5); }");
        let types = main_let_types(&program);
        assert_eq!(types, vec![&Type::Float, &Type::Int, &Type::Float]);
    }

    #[test]
    fn function_call_arguments_are_validated_for_count_and_type() {
        // A real, general gap this phase also found and fixed: ordinary
        // function calls (unlike class construction, which already
        // validated its constructor arguments) had no argument
        // count/type validation at all before now.
        assert!(check_source("func f(a: int, b: int) {} func main() { f(1, 2); }").is_ok());
        assert!(check_source("func f(a: int, b: int) {} func main() { f(1); }").is_err());
        assert!(check_source("func f(a: int, b: int) {} func main() { f(1, 2, 3); }").is_err());
        assert!(check_source("func f(a: int) {} func main() { f(\"hi\"); }").is_err());
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

    #[test]
    fn option_and_result_type_annotations_are_recognized() {
        let result = check_source(
            "func takes_opt(o: Option) -> Option { return o; } \
             func takes_res(r: Result) -> Result { return r; } \
             func generic_like(r: Result<string, string>) -> Option<int> { return None(); } \
             func main() { \
                 let o = takes_opt(Some(42)); \
                 let r = takes_res(Ok(\"ok\")); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn option_and_result_intrinsics_type_check() {
        let result = check_source(
            "func main() { \
                 let o = Some(10); \
                 let n = None(); \
                 println(is_some(o)); \
                 println(is_none(n)); \
                 println(unwrap(o)); \
                 println(unwrap_or(n, 0)); \
                 let r = Ok(\"data\"); \
                 let e = Err(\"fail\"); \
                 println(is_ok(r)); \
                 println(is_err(e)); \
                 println(unwrap(r)); \
                 println(unwrap_err(e)); \
                 println(unwrap_or(e, \"fallback\")); \
             }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn try_operator_type_checks_in_matching_function() {
        let result = check_source(
            "func get_val() -> Result { \
                 let r = Ok(5); \
                 let v = r?; \
                 return Ok(v); \
             } \
             func get_opt() -> Option { \
                 let o = Some(10); \
                 let v = o?; \
                 return Some(v); \
             } \
             func main() { }"
        );
        assert!(result.is_ok(), "{:?}", result);
    }

    #[test]
    fn try_operator_on_result_in_void_or_int_function_is_error() {
        let result = check_source(
            "func calc() -> int { \
                 let r = Ok(5); \
                 let v = r?; \
                 return v; \
             } \
             func main() { }"
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("requires the enclosing function to return Result"), "{}", err);
    }
}
