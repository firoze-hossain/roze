// compiler/src/semantic/mod.rs
use crate::parser::ast::*;
use std::collections::HashMap;
use anyhow::{Result, anyhow};

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
            return Err(anyhow!("Symbol '{}' already defined in this scope at line {}", name, line));
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
                let outer_function = self.current_function.replace(name.clone());
                let outer_return_type = std::mem::replace(
                    &mut self.current_return_type,
                    return_type.as_deref().map(Type::from_name).unwrap_or(Type::Void),
                );

                self.symbol_table.push_scope();
                for param in params {
                    let param_type = param.type_name.as_deref().map(Type::from_name).unwrap_or(Type::Unknown);
                    self.symbol_table.define(&param.name, param_type, location.line, location.column)?;
                }

                self.check_statement(body)?;

                self.symbol_table.pop_scope();
                self.current_function = outer_function;
                self.current_return_type = outer_return_type;
            }
            Statement::Let { name, value, location } => {
                let value_type = self.check_expression(value)?;
                self.symbol_table.define(name, value_type, location.line, location.column)?;
            }
            Statement::Expression { expr, .. } => {
                self.check_expression(expr)?;
            }
            Statement::Return { value, .. } => {
                if let Some(expr) = value {
                    // We type-check the returned expression, but don't yet
                    // cross-check it against `current_return_type`. Full
                    // return-type enforcement (and unifying it with the
                    // function's declared type) is a reasonable next step,
                    // tracked separately -- see ROADMAP.
                    self.check_expression(expr)?;
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
            Statement::Import { .. } => {
                // No module system yet -- import statements parse but
                // don't pull in another file's declarations. See ROADMAP.
            }
            Statement::Assign { name, value, location } => {
                self.check_expression(value)?;
                if self.symbol_table.lookup(name).is_none() {
                    return Err(anyhow!("Cannot assign to undefined variable '{}' at line {}", name, location.line));
                }
                // Note: we don't yet enforce that the assigned value's type
                // matches the variable's original declared type -- Roze
                // doesn't have a `let mut` / immutability distinction yet
                // either, so any existing variable can be reassigned. Both
                // are reasonable next steps (see ROADMAP).
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
                    Err(anyhow!("Undefined variable '{}' at line {}", name, location.line))
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
                            Err(anyhow!("Cannot add types {:?} and {:?} at line {}", left_type, right_type, location.line))
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                        let numeric_ok = |t: &Type| matches!(t, Type::Int | Type::Unknown);
                        if numeric_ok(&left_type) && numeric_ok(&right_type) {
                            Ok(Type::Int)
                        } else {
                            Err(anyhow!("Cannot perform arithmetic on {:?} and {:?} at line {}", left_type, right_type, location.line))
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
                    return Err(anyhow!("Call to undefined function '{}' at line {}", name, location.line));
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
        return Err(anyhow!("Type checking failed"));
    }
    Ok(())
}
