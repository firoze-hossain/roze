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

pub struct SymbolTable {
    pub symbols: HashMap<String, Symbol>,
    pub parent: Option<Box<SymbolTable>>,
    pub scope_level: usize,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            parent: None,
            scope_level: 0,
        }
    }

    pub fn new_child(&self) -> Self {
        Self {
            symbols: HashMap::new(),
            parent: Some(Box::new(self.clone())),
            scope_level: self.scope_level + 1,
        }
    }

    pub fn define(&mut self, name: &str, type_: Type, line: usize, column: usize) -> Result<()> {
        if self.symbols.contains_key(name) {
            return Err(anyhow!("Symbol '{}' already defined in this scope at line {}", name, line));
        }
        self.symbols.insert(name.to_string(), Symbol {
            name: name.to_string(),
            type_,
            line,
            column,
        });
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        if let Some(sym) = self.symbols.get(name) {
            return Some(sym);
        }
        if let Some(parent) = &self.parent {
            return parent.lookup(name);
        }
        None
    }

    pub fn lookup_current(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }
}

impl Clone for SymbolTable {
    fn clone(&self) -> Self {
        Self {
            symbols: self.symbols.clone(),
            parent: self.parent.clone(),
            scope_level: self.scope_level,
        }
    }
}

pub struct TypeChecker {
    pub symbol_table: SymbolTable,
    pub current_function: Option<String>,
    pub errors: Vec<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            symbol_table: SymbolTable::new(),
            current_function: None,
            errors: Vec::new(),
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        for stmt in &program.statements {
            self.check_statement(stmt)?;
        }
        Ok(())
    }

    pub fn check_statement(&mut self, stmt: &Statement) -> Result<()> {
        match stmt {
            Statement::Function { name, params, body, location } => {
                self.current_function = Some(name.clone());
                let mut func_table = self.symbol_table.new_child();

                // Add function name to outer scope
                self.symbol_table.define(name, Type::Unknown, location.line, location.column)?;

                // Add parameters to function scope
                let mut param_types = Vec::new();
                for param in params {
                    let param_type = Type::Unknown; // Will refine later
                    param_types.push(param_type.clone());
                    func_table.define(&param.name, param_type, location.line, location.column)?;
                }

                // Push function scope
                let old_table = std::mem::replace(&mut self.symbol_table, func_table);

                // Check body
                self.check_statement(body)?;

                // Pop scope
                self.symbol_table = old_table;
                self.current_function = None;
            }
            Statement::Let { name, value, location } => {
                let value_type = self.check_expression(value)?;
                self.symbol_table.define(name, value_type, location.line, location.column)?;
            }
            Statement::Expression { expr, .. } => {
                self.check_expression(expr)?;
            }
            Statement::Return { value, location } => {
                if let Some(expr) = value {
                    let return_type = self.check_expression(expr)?;
                    // Check return type matches function
                }
            }
            Statement::Block { statements, .. } => {
                let mut block_table = self.symbol_table.new_child();
                let old_table = std::mem::replace(&mut self.symbol_table, block_table);

                for stmt in statements {
                    self.check_statement(stmt)?;
                }

                self.symbol_table = old_table;
            }
            _ => {}
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
            Expression::Binary { left, operator, right, location } => {
                let left_type = self.check_expression(left)?;
                let right_type = self.check_expression(right)?;

                match operator {
                    BinaryOperator::Add => {
                        if left_type == Type::Int && right_type == Type::Int {
                            Ok(Type::Int)
                        } else if left_type == Type::String || right_type == Type::String {
                            Ok(Type::String)
                        } else {
                            Err(anyhow!("Cannot add types {:?} and {:?} at line {}", left_type, right_type, location.line))
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => {
                        if left_type == Type::Int && right_type == Type::Int {
                            Ok(Type::Int)
                        } else {
                            Err(anyhow!("Cannot perform arithmetic on {:?} and {:?} at line {}", left_type, right_type, location.line))
                        }
                    }
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        if left_type == right_type {
                            Ok(Type::Bool)
                        } else {
                            Ok(Type::Bool) // Allow comparison of different types
                        }
                    }
                    _ => Ok(Type::Unknown),
                }
            }
            Expression::Call { function, arguments, location } => {
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    // Check if println
                    if name == "println" {
                        for arg in arguments {
                            self.check_expression(arg)?;
                        }
                        return Ok(Type::Void);
                    }

                    // Check if it's a function call
                    if let Some(symbol) = self.symbol_table.lookup(name) {
                        // For now, return unknown
                        return Ok(Type::Unknown);
                    }
                }
                Ok(Type::Unknown)
            }
            _ => Ok(Type::Unknown),
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