// compiler/src/semantic/comptime.rs
//
// Compile-Time Function Execution (comptime) evaluator for Roze.
// Evaluates constant math, pure function execution, loops, lookup table
// generation, and struct type layout reflection at compile-time.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use anyhow::Result;
use crate::error::RozeError;
use crate::parser::ast::*;
use crate::semantic::{parse_numeric_literal, TypeChecker};

const MAX_STEPS: usize = 100_000;

#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Null,
    List(Rc<RefCell<Vec<ComptimeValue>>>),
    Map(Rc<RefCell<HashMap<String, ComptimeValue>>>),
    ClassInstance {
        class_name: String,
        fields: Rc<RefCell<HashMap<String, ComptimeValue>>>,
    },
    EnumVariant {
        enum_name: Option<String>,
        variant: String,
        args: Vec<ComptimeValue>,
    },
    Void,
    Return(Box<ComptimeValue>),
}

impl ComptimeValue {
    pub fn is_truthy(&self) -> bool {
        match self {
            ComptimeValue::Bool(b) => *b,
            ComptimeValue::Int(n) => *n != 0,
            ComptimeValue::Float(f) => *f != 0.0,
            ComptimeValue::String(s) => !s.is_empty(),
            ComptimeValue::Null | ComptimeValue::Void => false,
            ComptimeValue::List(l) => !l.borrow().is_empty(),
            ComptimeValue::Map(m) => !m.borrow().is_empty(),
            ComptimeValue::ClassInstance { .. } | ComptimeValue::EnumVariant { .. } => true,
            ComptimeValue::Return(inner) => inner.is_truthy(),
        }
    }

    pub fn display(&self) -> String {
        match self {
            ComptimeValue::Int(n) => n.to_string(),
            ComptimeValue::Float(f) => {
                let s = f.to_string();
                if !s.contains('.') {
                    format!("{}.0", s)
                } else {
                    s
                }
            }
            ComptimeValue::Bool(b) => b.to_string(),
            ComptimeValue::String(s) => s.clone(),
            ComptimeValue::Null => "null".to_string(),
            ComptimeValue::Void => "void".to_string(),
            ComptimeValue::List(l) => {
                let items: Vec<String> = l.borrow().iter().map(|i| i.display()).collect();
                format!("[{}]", items.join(", "))
            }
            ComptimeValue::Map(m) => {
                let items: Vec<String> = m.borrow().iter().map(|(k, v)| format!("{}: {}", k, v.display())).collect();
                format!("{{{}}}", items.join(", "))
            }
            ComptimeValue::ClassInstance { class_name, fields } => {
                let items: Vec<String> = fields.borrow().iter().map(|(k, v)| format!("{}: {}", k, v.display())).collect();
                format!("{} {{{}}}", class_name, items.join(", "))
            }
            ComptimeValue::EnumVariant { enum_name, variant, args } => {
                let prefix = enum_name.as_deref().map(|e| format!("{}::", e)).unwrap_or_default();
                if args.is_empty() {
                    format!("{}{}", prefix, variant)
                } else {
                    let items: Vec<String> = args.iter().map(|a| a.display()).collect();
                    format!("{}{}({})", prefix, variant, items.join(", "))
                }
            }
            ComptimeValue::Return(r) => r.display(),
        }
    }
}

pub struct ComptimeEnv {
    scopes: Vec<HashMap<String, ComptimeValue>>,
}

impl ComptimeEnv {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn declare(&mut self, name: &str, val: ComptimeValue) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), val);
        }
    }

    pub fn lookup(&self, name: &str) -> Option<ComptimeValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn assign(&mut self, name: &str, val: ComptimeValue) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), val);
                return true;
            }
        }
        false
    }
}

pub struct ComptimeEvaluator<'a> {
    pub type_checker: &'a TypeChecker,
    pub env: ComptimeEnv,
    pub steps: usize,
}

impl<'a> ComptimeEvaluator<'a> {
    pub fn new(type_checker: &'a TypeChecker) -> Self {
        Self {
            type_checker,
            env: ComptimeEnv::new(),
            steps: 0,
        }
    }

    pub fn step(&mut self, location: &Location) -> Result<()> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(RozeError::type_error(
                "Compile-time evaluation error: maximum evaluation steps exceeded (possible infinite loop or recursion)",
                location.line,
                location.column,
            ).into());
        }
        Ok(())
    }

    pub fn eval_expr(&mut self, expr: &Expression) -> Result<ComptimeValue> {
        match expr {
            Expression::Number { value, .. } => {
                let (raw, _) = parse_numeric_literal(value);
                if raw.contains('.') {
                    let f: f64 = raw.parse().map_err(|_| {
                        RozeError::type_error(format!("Invalid float literal '{}'", value), 0, 0)
                    })?;
                    Ok(ComptimeValue::Float(f))
                } else {
                    let i: i64 = raw.parse().map_err(|_| {
                        RozeError::type_error(format!("Invalid integer literal '{}'", value), 0, 0)
                    })?;
                    Ok(ComptimeValue::Int(i))
                }
            }
            Expression::Char { value, .. } => Ok(ComptimeValue::Int(*value as u32 as i64)),
            Expression::String { value, .. } => Ok(ComptimeValue::String(value.clone())),
            Expression::Boolean { value, .. } => Ok(ComptimeValue::Bool(*value)),
            Expression::Null { .. } => Ok(ComptimeValue::Null),
            Expression::Identifier { name, location } => {
                if let Some(val) = self.env.lookup(name) {
                    Ok(val)
                } else {
                    Err(RozeError::type_error(
                        format!("Compile-time evaluation error: undefined variable '{}'", name),
                        location.line,
                        location.column,
                    ).into())
                }
            }
            Expression::Unary { operator, operand, location } => {
                self.step(location)?;
                let val = self.eval_expr(operand)?;
                match operator {
                    UnaryOperator::Negate => match val {
                        ComptimeValue::Int(n) => Ok(ComptimeValue::Int(-n)),
                        ComptimeValue::Float(f) => Ok(ComptimeValue::Float(-f)),
                        _ => Err(RozeError::type_error("Cannot negate non-numeric comptime value", location.line, location.column).into()),
                    },
                    UnaryOperator::Not => match val {
                        ComptimeValue::Bool(b) => Ok(ComptimeValue::Bool(!b)),
                        other => Ok(ComptimeValue::Bool(!other.is_truthy())),
                    },
                }
            }
            Expression::Binary { left, operator, right, location } => {
                self.step(location)?;
                // Handle short-circuiting for logical operators
                match operator {
                    BinaryOperator::And => {
                        let l_val = self.eval_expr(left)?;
                        if !l_val.is_truthy() {
                            return Ok(ComptimeValue::Bool(false));
                        }
                        let r_val = self.eval_expr(right)?;
                        return Ok(ComptimeValue::Bool(r_val.is_truthy()));
                    }
                    BinaryOperator::Or => {
                        let l_val = self.eval_expr(left)?;
                        if l_val.is_truthy() {
                            return Ok(ComptimeValue::Bool(true));
                        }
                        let r_val = self.eval_expr(right)?;
                        return Ok(ComptimeValue::Bool(r_val.is_truthy()));
                    }
                    _ => {}
                }

                let l_val = self.eval_expr(left)?;
                let r_val = self.eval_expr(right)?;

                match operator {
                    BinaryOperator::Add => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(a + b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a + b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a as f64 + b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Float(a + b as f64)),
                        (ComptimeValue::String(a), ComptimeValue::String(b)) => Ok(ComptimeValue::String(format!("{}{}", a, b))),
                        (ComptimeValue::String(a), other) => Ok(ComptimeValue::String(format!("{}{}", a, other.display()))),
                        (other, ComptimeValue::String(b)) => Ok(ComptimeValue::String(format!("{}{}", other.display(), b))),
                        _ => Err(RozeError::type_error("Unsupported types for '+' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::Subtract => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(a - b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a - b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a as f64 - b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Float(a - b as f64)),
                        _ => Err(RozeError::type_error("Unsupported types for '-' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::Multiply => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(a * b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a * b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a as f64 * b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Float(a * b as f64)),
                        _ => Err(RozeError::type_error("Unsupported types for '*' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::Divide => match (l_val, r_val) {
                        (ComptimeValue::Int(_), ComptimeValue::Int(0)) => {
                            Err(RozeError::type_error("Compile-time evaluation error: division by zero", location.line, location.column).into())
                        }
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(a / b)),
                        (ComptimeValue::Float(_), ComptimeValue::Float(b)) if b == 0.0 => {
                            Err(RozeError::type_error("Compile-time evaluation error: division by zero", location.line, location.column).into())
                        }
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Float(a / b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => {
                            if b == 0.0 {
                                return Err(RozeError::type_error("Compile-time evaluation error: division by zero", location.line, location.column).into());
                            }
                            Ok(ComptimeValue::Float(a as f64 / b))
                        }
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => {
                            if b == 0 {
                                return Err(RozeError::type_error("Compile-time evaluation error: division by zero", location.line, location.column).into());
                            }
                            Ok(ComptimeValue::Float(a / b as f64))
                        }
                        _ => Err(RozeError::type_error("Unsupported types for '/' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::Modulo => match (l_val, r_val) {
                        (ComptimeValue::Int(_), ComptimeValue::Int(0)) => {
                            Err(RozeError::type_error("Compile-time evaluation error: division by zero", location.line, location.column).into())
                        }
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Int(a % b)),
                        _ => Err(RozeError::type_error("Unsupported types for '%' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::Equal => Ok(ComptimeValue::Bool(l_val == r_val)),
                    BinaryOperator::NotEqual => Ok(ComptimeValue::Bool(l_val != r_val)),
                    BinaryOperator::LessThan => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a < b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(a < b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool((a as f64) < b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a < (b as f64))),
                        _ => Err(RozeError::type_error("Unsupported types for '<' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::LessEqual => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a <= b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(a <= b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool((a as f64) <= b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a <= (b as f64))),
                        _ => Err(RozeError::type_error("Unsupported types for '<=' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::GreaterThan => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a > b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(a > b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool((a as f64) > b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a > (b as f64))),
                        _ => Err(RozeError::type_error("Unsupported types for '>' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::GreaterEqual => match (l_val, r_val) {
                        (ComptimeValue::Int(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a >= b)),
                        (ComptimeValue::Float(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool(a >= b)),
                        (ComptimeValue::Int(a), ComptimeValue::Float(b)) => Ok(ComptimeValue::Bool((a as f64) >= b)),
                        (ComptimeValue::Float(a), ComptimeValue::Int(b)) => Ok(ComptimeValue::Bool(a >= (b as f64))),
                        _ => Err(RozeError::type_error("Unsupported types for '>=' at comptime", location.line, location.column).into()),
                    },
                    BinaryOperator::And | BinaryOperator::Or => unreachable!(),
                }
            }
            Expression::Call { function, arguments, location } => {
                if let Expression::Identifier { name, .. } = &**function {
                    self.eval_call(name, arguments, location)
                } else {
                    Err(RozeError::type_error("Indirect calls not yet supported inside comptime evaluator", location.line, location.column).into())
                }
            }
            Expression::GenericCall { function, type_args, arguments, location } => {
                self.step(location)?;
                match function.as_str() {
                    "size_of" => {
                        let (sz, _) = self.type_checker.compute_type_layout(&type_args[0])?;
                        Ok(ComptimeValue::Int(sz as i64))
                    }
                    "align_of" => {
                        let (_, al) = self.type_checker.compute_type_layout(&type_args[0])?;
                        Ok(ComptimeValue::Int(al as i64))
                    }
                    "offset_of" => {
                        if arguments.is_empty() {
                            return Err(RozeError::type_error("offset_of requires a field name argument", location.line, location.column).into());
                        }
                        let field_val = self.eval_expr(&arguments[0])?;
                        if let ComptimeValue::String(field_name) = field_val {
                            let off = self.type_checker.compute_field_offset(&type_args[0], &field_name)?;
                            Ok(ComptimeValue::Int(off as i64))
                        } else {
                            Err(RozeError::type_error("offset_of field argument must be a string", location.line, location.column).into())
                        }
                    }
                    _ => {
                        self.eval_call(function, arguments, location)
                    }
                }
            }
            Expression::New { class_name, arguments, location, .. } => {
                self.step(location)?;
                let fields_info = self.type_checker.classes.get(class_name).cloned().ok_or_else(|| {
                    RozeError::type_error(format!("Undefined class '{}' at comptime", class_name), location.line, location.column)
                })?;

                let mut field_map = HashMap::new();
                for (i, (fname, _fty)) in fields_info.iter().enumerate() {
                    let val = if i < arguments.len() {
                        self.eval_expr(&arguments[i])?
                    } else {
                        ComptimeValue::Null
                    };
                    field_map.insert(fname.clone(), val);
                }

                Ok(ComptimeValue::ClassInstance {
                    class_name: class_name.clone(),
                    fields: Rc::new(RefCell::new(field_map)),
                })
            }
            Expression::FieldAccess { object, field, location } => {
                self.step(location)?;
                let obj = self.eval_expr(object)?;
                if let ComptimeValue::ClassInstance { fields, .. } = obj {
                    if let Some(val) = fields.borrow().get(field).cloned() {
                        Ok(val)
                    } else {
                        Err(RozeError::type_error(format!("Field '{}' not found on class instance", field), location.line, location.column).into())
                    }
                } else {
                    Err(RozeError::type_error("Cannot access field on non-class comptime value", location.line, location.column).into())
                }
            }
            Expression::EnumConstruct { enum_name, variant, arguments, location } => {
                self.step(location)?;
                let mut args = Vec::with_capacity(arguments.len());
                for a in arguments {
                    args.push(self.eval_expr(a)?);
                }
                Ok(ComptimeValue::EnumVariant {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    args,
                })
            }
            Expression::Comptime { expr, .. } => {
                self.eval_expr(expr)
            }
            Expression::ComptimeBlock { block, .. } => {
                self.eval_stmt(block)
            }
            Expression::Unsafe { expr, .. } => {
                self.eval_expr(expr)
            }
            Expression::Try { expr, location } => {
                let val = self.eval_expr(expr)?;
                match &val {
                    ComptimeValue::EnumVariant { variant, args, .. } => {
                        if variant == "Ok" || variant == "Some" {
                            Ok(args.first().cloned().unwrap_or(ComptimeValue::Null))
                        } else if variant == "Err" || variant == "None" {
                            Ok(ComptimeValue::Return(Box::new(val)))
                        } else {
                            Err(RozeError::type_error("Cannot use '?' on variant other than Ok/Some/Err/None", location.line, location.column).into())
                        }
                    }
                    _ => Err(RozeError::type_error("Cannot use '?' on non-Result/Option type at comptime", location.line, location.column).into()),
                }
            }
            Expression::Closure { .. } | Expression::MethodCall { .. } => {
                Err(RozeError::type_error("Comptime closure definition/method call not directly supported in this context", 0, 0).into())
            }
            Expression::Await { location, .. } => {
                Err(RozeError::type_error("Cannot 'await' an asynchronous expression at compile time (comptime)", location.line, location.column).into())
            }
        }
    }

    pub fn eval_call(&mut self, name: &str, arguments: &[Expression], location: &Location) -> Result<ComptimeValue> {
        self.step(location)?;

        // Check for built-in compile-time intrinsics:
        match name {
            "list_new" => {
                return Ok(ComptimeValue::List(Rc::new(RefCell::new(Vec::new()))));
            }
            "list_push" => {
                if arguments.len() < 2 {
                    return Err(RozeError::type_error("list_push expects (list, item)", location.line, location.column).into());
                }
                let list_val = self.eval_expr(&arguments[0])?;
                let item_val = self.eval_expr(&arguments[1])?;
                if let ComptimeValue::List(l) = list_val {
                    l.borrow_mut().push(item_val);
                    return Ok(ComptimeValue::Void);
                } else {
                    return Err(RozeError::type_error("list_push expects a list as first argument", location.line, location.column).into());
                }
            }
            "list_get" => {
                if arguments.len() < 2 {
                    return Err(RozeError::type_error("list_get expects (list, index)", location.line, location.column).into());
                }
                let list_val = self.eval_expr(&arguments[0])?;
                let idx_val = self.eval_expr(&arguments[1])?;
                if let (ComptimeValue::List(l), ComptimeValue::Int(idx)) = (list_val, idx_val) {
                    let idx = idx as usize;
                    if idx < l.borrow().len() {
                        return Ok(l.borrow()[idx].clone());
                    } else {
                        return Err(RozeError::type_error(
                            format!("Compile-time evaluation error: list index {} out of bounds (len {})", idx, l.borrow().len()),
                            location.line,
                            location.column,
                        ).into());
                    }
                } else {
                    return Err(RozeError::type_error("list_get expects (list, int)", location.line, location.column).into());
                }
            }
            "list_set" => {
                if arguments.len() < 3 {
                    return Err(RozeError::type_error("list_set expects (list, index, value)", location.line, location.column).into());
                }
                let list_val = self.eval_expr(&arguments[0])?;
                let idx_val = self.eval_expr(&arguments[1])?;
                let item_val = self.eval_expr(&arguments[2])?;
                if let (ComptimeValue::List(l), ComptimeValue::Int(idx)) = (list_val, idx_val) {
                    let idx = idx as usize;
                    if idx < l.borrow().len() {
                        l.borrow_mut()[idx] = item_val;
                        return Ok(ComptimeValue::Void);
                    } else {
                        return Err(RozeError::type_error(
                            format!("Compile-time evaluation error: list index {} out of bounds", idx),
                            location.line,
                            location.column,
                        ).into());
                    }
                } else {
                    return Err(RozeError::type_error("list_set expects (list, int, value)", location.line, location.column).into());
                }
            }
            "list_length" => {
                if arguments.is_empty() {
                    return Err(RozeError::type_error("list_length expects (list)", location.line, location.column).into());
                }
                let list_val = self.eval_expr(&arguments[0])?;
                if let ComptimeValue::List(l) = list_val {
                    return Ok(ComptimeValue::Int(l.borrow().len() as i64));
                } else {
                    return Err(RozeError::type_error("list_length expects a list", location.line, location.column).into());
                }
            }
            "list_remove" => {
                if arguments.len() < 2 {
                    return Err(RozeError::type_error("list_remove expects (list, index)", location.line, location.column).into());
                }
                let list_val = self.eval_expr(&arguments[0])?;
                let idx_val = self.eval_expr(&arguments[1])?;
                if let (ComptimeValue::List(l), ComptimeValue::Int(idx)) = (list_val, idx_val) {
                    let idx = idx as usize;
                    if idx < l.borrow().len() {
                        let item = l.borrow_mut().remove(idx);
                        return Ok(item);
                    } else {
                        return Err(RozeError::type_error("list_remove index out of bounds", location.line, location.column).into());
                    }
                } else {
                    return Err(RozeError::type_error("list_remove expects (list, int)", location.line, location.column).into());
                }
            }
            "map_new" => {
                return Ok(ComptimeValue::Map(Rc::new(RefCell::new(HashMap::new()))));
            }
            "map_insert" => {
                if arguments.len() < 3 {
                    return Err(RozeError::type_error("map_insert expects (map, key, val)", location.line, location.column).into());
                }
                let map_val = self.eval_expr(&arguments[0])?;
                let key_val = self.eval_expr(&arguments[1])?;
                let item_val = self.eval_expr(&arguments[2])?;
                if let ComptimeValue::Map(m) = map_val {
                    m.borrow_mut().insert(key_val.display(), item_val);
                    return Ok(ComptimeValue::Void);
                } else {
                    return Err(RozeError::type_error("map_insert expects a map as first argument", location.line, location.column).into());
                }
            }
            "map_get" => {
                if arguments.len() < 2 {
                    return Err(RozeError::type_error("map_get expects (map, key)", location.line, location.column).into());
                }
                let map_val = self.eval_expr(&arguments[0])?;
                let key_val = self.eval_expr(&arguments[1])?;
                if let ComptimeValue::Map(m) = map_val {
                    let key = key_val.display();
                    return Ok(m.borrow().get(&key).cloned().unwrap_or(ComptimeValue::Null));
                } else {
                    return Err(RozeError::type_error("map_get expects a map as first argument", location.line, location.column).into());
                }
            }
            "map_contains" => {
                if arguments.len() < 2 {
                    return Err(RozeError::type_error("map_contains expects (map, key)", location.line, location.column).into());
                }
                let map_val = self.eval_expr(&arguments[0])?;
                let key_val = self.eval_expr(&arguments[1])?;
                if let ComptimeValue::Map(m) = map_val {
                    let key = key_val.display();
                    return Ok(ComptimeValue::Bool(m.borrow().contains_key(&key)));
                } else {
                    return Err(RozeError::type_error("map_contains expects a map as first argument", location.line, location.column).into());
                }
            }
            "map_length" => {
                if arguments.is_empty() {
                    return Err(RozeError::type_error("map_length expects (map)", location.line, location.column).into());
                }
                let map_val = self.eval_expr(&arguments[0])?;
                if let ComptimeValue::Map(m) = map_val {
                    return Ok(ComptimeValue::Int(m.borrow().len() as i64));
                } else {
                    return Err(RozeError::type_error("map_length expects a map", location.line, location.column).into());
                }
            }
            "abs" => {
                if arguments.is_empty() { return Ok(ComptimeValue::Int(0)); }
                let v = self.eval_expr(&arguments[0])?;
                return match v {
                    ComptimeValue::Int(n) => Ok(ComptimeValue::Int(n.abs())),
                    ComptimeValue::Float(f) => Ok(ComptimeValue::Float(f.abs())),
                    _ => Err(RozeError::type_error("abs expects a number", location.line, location.column).into()),
                };
            }
            "min" => {
                if arguments.len() < 2 { return Err(RozeError::type_error("min expects 2 arguments", location.line, location.column).into()); }
                let a = self.eval_expr(&arguments[0])?;
                let b = self.eval_expr(&arguments[1])?;
                return match (a, b) {
                    (ComptimeValue::Int(x), ComptimeValue::Int(y)) => Ok(ComptimeValue::Int(x.min(y))),
                    (ComptimeValue::Float(x), ComptimeValue::Float(y)) => Ok(ComptimeValue::Float(x.min(y))),
                    _ => Err(RozeError::type_error("min expects numbers", location.line, location.column).into()),
                };
            }
            "max" => {
                if arguments.len() < 2 { return Err(RozeError::type_error("max expects 2 arguments", location.line, location.column).into()); }
                let a = self.eval_expr(&arguments[0])?;
                let b = self.eval_expr(&arguments[1])?;
                return match (a, b) {
                    (ComptimeValue::Int(x), ComptimeValue::Int(y)) => Ok(ComptimeValue::Int(x.max(y))),
                    (ComptimeValue::Float(x), ComptimeValue::Float(y)) => Ok(ComptimeValue::Float(x.max(y))),
                    _ => Err(RozeError::type_error("max expects numbers", location.line, location.column).into()),
                };
            }
            "clamp" => {
                if arguments.len() < 3 { return Err(RozeError::type_error("clamp expects 3 arguments", location.line, location.column).into()); }
                let val = self.eval_expr(&arguments[0])?;
                let min_v = self.eval_expr(&arguments[1])?;
                let max_v = self.eval_expr(&arguments[2])?;
                return match (val, min_v, max_v) {
                    (ComptimeValue::Int(v), ComptimeValue::Int(mn), ComptimeValue::Int(mx)) => Ok(ComptimeValue::Int(v.clamp(mn, mx))),
                    (ComptimeValue::Float(v), ComptimeValue::Float(mn), ComptimeValue::Float(mx)) => Ok(ComptimeValue::Float(v.clamp(mn, mx))),
                    _ => Err(RozeError::type_error("clamp expects numbers", location.line, location.column).into()),
                };
            }
            "sqrt" => {
                if arguments.is_empty() { return Ok(ComptimeValue::Float(0.0)); }
                let v = self.eval_expr(&arguments[0])?;
                return match v {
                    ComptimeValue::Int(n) => Ok(ComptimeValue::Float((n as f64).sqrt())),
                    ComptimeValue::Float(f) => Ok(ComptimeValue::Float(f.sqrt())),
                    _ => Err(RozeError::type_error("sqrt expects a number", location.line, location.column).into()),
                };
            }
            "pow" => {
                if arguments.len() < 2 { return Err(RozeError::type_error("pow expects 2 arguments", location.line, location.column).into()); }
                let base = self.eval_expr(&arguments[0])?;
                let exp = self.eval_expr(&arguments[1])?;
                return match (base, exp) {
                    (ComptimeValue::Int(b), ComptimeValue::Int(e)) => Ok(ComptimeValue::Int(b.pow(e as u32))),
                    (ComptimeValue::Float(b), ComptimeValue::Float(e)) => Ok(ComptimeValue::Float(b.powf(e))),
                    _ => Err(RozeError::type_error("pow expects numbers", location.line, location.column).into()),
                };
            }
            "size_of" => {
                if arguments.is_empty() { return Err(RozeError::type_error("size_of expects a type name string", location.line, location.column).into()); }
                let arg = self.eval_expr(&arguments[0])?;
                if let ComptimeValue::String(ty_name) = arg {
                    let (sz, _) = self.type_checker.compute_type_layout(&ty_name)?;
                    return Ok(ComptimeValue::Int(sz as i64));
                } else {
                    return Err(RozeError::type_error("size_of expects a string type name", location.line, location.column).into());
                }
            }
            "align_of" => {
                if arguments.is_empty() { return Err(RozeError::type_error("align_of expects a type name string", location.line, location.column).into()); }
                let arg = self.eval_expr(&arguments[0])?;
                if let ComptimeValue::String(ty_name) = arg {
                    let (_, al) = self.type_checker.compute_type_layout(&ty_name)?;
                    return Ok(ComptimeValue::Int(al as i64));
                } else {
                    return Err(RozeError::type_error("align_of expects a string type name", location.line, location.column).into());
                }
            }
            "offset_of" => {
                if arguments.len() < 2 { return Err(RozeError::type_error("offset_of expects (type_name, field_name)", location.line, location.column).into()); }
                let arg0 = self.eval_expr(&arguments[0])?;
                let arg1 = self.eval_expr(&arguments[1])?;
                if let (ComptimeValue::String(ty_name), ComptimeValue::String(fname)) = (arg0, arg1) {
                    let off = self.type_checker.compute_field_offset(&ty_name, &fname)?;
                    return Ok(ComptimeValue::Int(off as i64));
                } else {
                    return Err(RozeError::type_error("offset_of expects (string, string)", location.line, location.column).into());
                }
            }
            _ => {}
        }

        // Check user-defined function in AST definitions:
        if let Some(Statement::Function { params, body, .. }) = self.type_checker.ast_functions.get(name).cloned() {
            let mut evaled_args = Vec::with_capacity(arguments.len());
            for a in arguments {
                evaled_args.push(self.eval_expr(a)?);
            }

            self.env.push_scope();
            for (param, val) in params.iter().zip(evaled_args) {
                self.env.declare(&param.name, val);
            }

            let result = self.eval_stmt(&body)?;
            self.env.pop_scope();

            return match result {
                ComptimeValue::Return(val) => Ok(*val),
                other => Ok(other),
            };
        }

        Err(RozeError::type_error(
            format!("Compile-time evaluation error: function '{}' cannot be evaluated at compile time", name),
            location.line,
            location.column,
        ).into())
    }

    pub fn eval_stmt(&mut self, stmt: &Statement) -> Result<ComptimeValue> {
        match stmt {
            Statement::Let { name, value, location, .. } => {
                self.step(location)?;
                let val = self.eval_expr(value)?;
                self.env.declare(name, val);
                Ok(ComptimeValue::Void)
            }
            Statement::Assign { name, value, location } => {
                self.step(location)?;
                let val = self.eval_expr(value)?;
                if !self.env.assign(name, val) {
                    return Err(RozeError::type_error(
                        format!("Compile-time evaluation error: undefined variable '{}'", name),
                        location.line,
                        location.column,
                    ).into());
                }
                Ok(ComptimeValue::Void)
            }
            Statement::FieldAssign { object, field, value, location } => {
                self.step(location)?;
                let obj = self.eval_expr(object)?;
                let val = self.eval_expr(value)?;
                if let ComptimeValue::ClassInstance { fields, .. } = obj {
                    fields.borrow_mut().insert(field.clone(), val);
                    Ok(ComptimeValue::Void)
                } else {
                    Err(RozeError::type_error("Cannot assign field on non-class comptime value", location.line, location.column).into())
                }
            }
            Statement::If { condition, then_branch, else_branch, location } => {
                self.step(location)?;
                let cond = self.eval_expr(condition)?;
                if cond.is_truthy() {
                    self.eval_stmt(then_branch)
                } else if let Some(else_b) = else_branch {
                    self.eval_stmt(else_b)
                } else {
                    Ok(ComptimeValue::Void)
                }
            }
            Statement::While { condition, body, location } => {
                while self.eval_expr(condition)?.is_truthy() {
                    self.step(location)?;
                    let res = self.eval_stmt(body)?;
                    if matches!(res, ComptimeValue::Return(_)) {
                        return Ok(res);
                    }
                }
                Ok(ComptimeValue::Void)
            }
            Statement::For { init, condition, update, body, location } => {
                self.env.push_scope();
                self.eval_stmt(init)?;
                while self.eval_expr(condition)?.is_truthy() {
                    self.step(location)?;
                    let res = self.eval_stmt(body)?;
                    if matches!(res, ComptimeValue::Return(_)) {
                        self.env.pop_scope();
                        return Ok(res);
                    }
                    self.eval_stmt(update)?;
                }
                self.env.pop_scope();
                Ok(ComptimeValue::Void)
            }
            Statement::Block { statements, .. } => {
                self.env.push_scope();
                let res = self.eval_block(statements);
                self.env.pop_scope();
                res
            }
            Statement::Return { value, location } => {
                self.step(location)?;
                let res = if let Some(v) = value {
                    self.eval_expr(v)?
                } else {
                    ComptimeValue::Void
                };
                Ok(ComptimeValue::Return(Box::new(res)))
            }
            Statement::Expression { expr, .. } => {
                self.eval_expr(expr)
            }
            Statement::Comptime { statement, .. } => {
                self.eval_stmt(statement)
            }
            Statement::UnsafeBlock { body, .. } => {
                self.eval_stmt(body)
            }
            _ => Ok(ComptimeValue::Void),
        }
    }

    pub fn eval_block(&mut self, statements: &[Statement]) -> Result<ComptimeValue> {
        let mut last_val = ComptimeValue::Void;
        for (i, stmt) in statements.iter().enumerate() {
            let res = self.eval_stmt(stmt)?;
            if let ComptimeValue::Return(v) = res {
                return Ok(ComptimeValue::Return(v));
            }
            if i == statements.len() - 1 {
                last_val = res;
            }
        }
        Ok(last_val)
    }
}

pub fn comptime_value_to_ast(val: &ComptimeValue, location: &Location) -> Result<Expression> {
    match val {
        ComptimeValue::Int(n) => Ok(Expression::Number { value: n.to_string(), location: location.clone() }),
        ComptimeValue::Float(f) => {
            let mut s = f.to_string();
            if !s.contains('.') {
                s.push_str(".0");
            }
            Ok(Expression::Number { value: s, location: location.clone() })
        }
        ComptimeValue::Bool(b) => Ok(Expression::Boolean { value: *b, location: location.clone() }),
        ComptimeValue::String(s) => Ok(Expression::String { value: s.clone(), location: location.clone() }),
        ComptimeValue::Null => Ok(Expression::Null { location: location.clone() }),
        other => Err(RozeError::type_error(
            format!("Cannot convert comptime value {:?} to AST", other),
            location.line,
            location.column,
        ).into()),
    }
}
