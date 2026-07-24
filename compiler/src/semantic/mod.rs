// compiler/src/semantic/mod.rs
use crate::error::RozeError;
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

    pub fn lookup_current(&self, name: &str) -> Option<&Symbol> {
        self.scopes.last().and_then(|frame| frame.get(name))
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

    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        // First pass: register every top-level function's signature before
        // checking any bodies, so forward references and mutual recursion
        // type-check correctly regardless of source order.
        for stmt in &program.statements {
            if let Statement::Function { name, params, return_type, .. } = stmt {
                let param_types = params.iter()
                    .map(|p| p.type_name.as_deref().map(Type::from_name).unwrap_or(Type::Unknown))
                    .collect();
                let ret = return_type.as_deref().map(Type::from_name).unwrap_or(Type::Void);
                self.functions.insert(name.clone(), FunctionSig { params: param_types, return_type: ret });
            }
        }

        for stmt in &program.statements {
            self.check_statement(stmt)?;
        }
        Ok(())
    }

    pub fn check_statement(&mut self, stmt: &Statement) -> Result<()> {
        match stmt {
            Statement::Function { name, params, return_type, body, location } => {
                if name == "main" && return_type.is_some() {
                    return Err(RozeError::type_error(
                        "'main' always returns void and cannot declare a return type",
                        location.line,
                        location.column,
                    ).with_hint("remove the '-> ...' after main()'s parameter list").into());
                }

                let outer_function = self.current_function.replace(name.clone());
                let outer_return_type = std::mem::replace(
                    &mut self.current_return_type,
                    return_type.as_deref().map(Type::from_name).unwrap_or(Type::Void),
                );
                let outer_in_main = std::mem::replace(&mut self.in_main, name == "main");

                self.symbol_table.push_scope();
                for param in params {
                    let param_type = param.type_name.as_deref().map(Type::from_name).unwrap_or(Type::Unknown);
                    self.symbol_table.define(&param.name, param_type, location.line, location.column)?;
                }

                self.check_statement(body)?;

                self.symbol_table.pop_scope();
                self.current_function = outer_function;
                self.current_return_type = outer_return_type;
                self.in_main = outer_in_main;
            }
            Statement::Let { name, value, location } => {
                let value_type = self.check_expression(value)?;
                self.symbol_table.define(name, value_type, location.line, location.column)?;
            }
            Statement::Expression { expr, .. } => {
                self.check_expression(expr)?;
            }
            Statement::Return { value, location } => {
                match value {
                    Some(expr) => {
                        let actual = self.check_expression(expr)?;
                        if !types_compatible(&self.current_return_type, &actual) {
                            let fn_name = self.current_function.as_deref().unwrap_or("<anonymous>");
                            return Err(RozeError::type_error(
                                format!(
                                    "'{}' is declared to return {}, but this returns {}",
                                    fn_name, self.current_return_type, actual
                                ),
                                location.line,
                                location.column,
                            ).with_hint(format!(
                                "either change the returned value's type, or change the function's declared return type ('-> {}')",
                                actual
                            )).into());
                        }
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
                    }
                }
            }
            Statement::Block { statements, .. } => {
                self.symbol_table.push_scope();
                for stmt in statements {
                    self.check_statement(stmt)?;
                }
                self.symbol_table.pop_scope();
            }
            Statement::If { condition, then_branch, else_branch, .. } => {
                self.check_expression(condition)?;
                self.check_statement(then_branch)?;
                if let Some(else_stmt) = else_branch {
                    self.check_statement(else_stmt)?;
                }
            }
            Statement::While { condition, body, .. } => {
                self.check_expression(condition)?;
                self.check_statement(body)?;
            }
            Statement::For { init, condition, update, body, .. } => {
                // A scope of its own so the init clause's variable (e.g.
                // `let i` in `for let i = 0; ...`) is visible to the
                // condition/update/body but doesn't leak past the loop.
                self.symbol_table.push_scope();
                self.check_statement(init)?;
                self.check_expression(condition)?;
                self.check_statement(update)?;
                self.check_statement(body)?;
                self.symbol_table.pop_scope();
            }
            Statement::Import { .. } => {
                // Imports are already resolved into real functions before
                // type-checking even runs (see imports::resolve_imports),
                // so in practice there's nothing left to do here -- this
                // arm only exists in case that ever changes.
            }
            Statement::Assign { name, value, location } => {
                let value_type = self.check_expression(value)?;
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
                        if !types_compatible(&symbol.type_, &value_type) {
                            return Err(RozeError::type_error(
                                format!(
                                    "Cannot assign a value of type {} to '{}', which was declared as {}",
                                    value_type, name, symbol.type_
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
            }
        }
        Ok(())
    }

    pub fn check_expression(&mut self, expr: &Expression) -> Result<Type> {
        match expr {
            Expression::Number { .. } => Ok(Type::Int),
            Expression::String { .. } => Ok(Type::String),
            Expression::Boolean { .. } => Ok(Type::Bool),
            Expression::Null { .. } => Ok(Type::Unknown),
            Expression::Identifier { name, location } => {
                if let Some(symbol) = self.symbol_table.lookup(name) {
                    Ok(symbol.type_.clone())
                } else {
                    Err(RozeError::type_error(
                        format!("Undefined variable '{}'", name),
                        location.line,
                        location.column,
                    ).with_length(name.chars().count()).into())
                }
            }
            Expression::Unary { operand, .. } => self.check_expression(operand),
            Expression::Binary { left, operator, right, location } => {
                let left_type = self.check_expression(left)?;
                let right_type = self.check_expression(right)?;

                match operator {
                    BinaryOperator::Add => {
                        if left_type == Type::Int && right_type == Type::Int {
                            Ok(Type::Int)
                        } else if left_type == Type::String || right_type == Type::String {
                            Ok(Type::String)
                        } else if left_type == Type::Unknown || right_type == Type::Unknown {
                            // Untyped (dynamic-ish) operand: allow it and
                            // defer to runtime/codegen, rather than
                            // rejecting code that may well be valid.
                            Ok(Type::Unknown)
                        } else {
                            Err(RozeError::type_error(
                                format!("Cannot add {} and {}", left_type, right_type),
                                location.line,
                                location.column,
                            ).into())
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                        let numeric_ok = |t: &Type| matches!(t, Type::Int | Type::Unknown);
                        if numeric_ok(&left_type) && numeric_ok(&right_type) {
                            Ok(Type::Int)
                        } else {
                            Err(RozeError::type_error(
                                format!("Cannot perform arithmetic on {} and {}", left_type, right_type),
                                location.line,
                                location.column,
                            ).into())
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual |
                    BinaryOperator::LessThan | BinaryOperator::GreaterThan |
                    BinaryOperator::LessEqual | BinaryOperator::GreaterEqual |
                    BinaryOperator::And | BinaryOperator::Or => Ok(Type::Bool),
                }
            }
            Expression::Call { function, arguments, location } => {
                for arg in arguments {
                    self.check_expression(arg)?;
                }
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    if name == "println" {
                        return Ok(Type::Void);
                    }
                    if let Some(sig) = self.functions.get(name) {
                        return Ok(sig.return_type.clone());
                    }
                    return Err(RozeError::type_error(
                        format!("Call to undefined function '{}'", name),
                        location.line,
                        location.column,
                    ).with_length(name.chars().count()).into());
                }
                Ok(Type::Unknown)
            }
        }
    }
}

pub fn check_types(program: &Program) -> Result<()> {
    let mut checker = TypeChecker::new();
    checker.check_program(program)?;

    if !checker.errors.is_empty() {
        for error in &checker.errors {
            eprintln!("{}", error);
        }
        return Err(anyhow::anyhow!("Type checking failed"));
    }
    Ok(())
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
    fn for_loop_variable_is_scoped_to_the_loop() {
        // `i` from the for-loop's init clause must be visible inside the
        // loop body/condition/update...
        assert!(check_source("func main() { for let i = 0; i < 3; i = i + 1 { println(i); } }").is_ok());
        // ...but must NOT leak out past the loop.
        assert!(check_source("func main() { for let i = 0; i < 3; i = i + 1 { } println(i); }").is_err());
    }
}
