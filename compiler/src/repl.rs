// compiler/src/repl.rs
//
// 🌹 Roze Interactive REPL (Read-Eval-Print Loop)
// Interactive session for real-time expression evaluation, prototyping,
// and function experimentation.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;
use colored::*;
use anyhow::Result;

/// Value representation in the interactive REPL.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Void,
}

impl std::fmt::Display for ReplValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplValue::Int(i) => write!(f, "{}", i),
            ReplValue::Float(fl) => write!(f, "{}", fl),
            ReplValue::Bool(b) => write!(f, "{}", b),
            ReplValue::String(s) => write!(f, "\"{}\"", s),
            ReplValue::Void => write!(f, "()"),
        }
    }
}

/// Variable metadata stored in the REPL session.
#[derive(Debug, Clone)]
pub struct ReplVar {
    pub name: String,
    pub type_name: String,
    pub value: ReplValue,
}

/// Interactive REPL Session State.
pub struct ReplSession {
    pub variables: HashMap<String, ReplVar>,
    pub functions: Vec<String>,
    pub history: Vec<String>,
    pub line_number: usize,
}

impl Default for ReplSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplSession {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: Vec::new(),
            history: Vec::new(),
            line_number: 1,
        }
    }

    /// Evaluates a user input line or multi-line block.
    pub fn eval(&mut self, input: &str) -> Result<Option<ReplValue>> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        self.history.push(input.to_string());

        // 1. Check for REPL meta-commands (:help, :vars, :clear, :load, :type, :quit)
        if trimmed.starts_with(':') {
            return self.handle_meta_command(trimmed);
        }

        // 2. Handle variable declaration: `let x = <expr>;` or `let x: type = <expr>;`
        if trimmed.starts_with("let ") {
            return self.handle_let_decl(trimmed);
        }

        // 3. Handle function declaration: `func name(...) ...`
        if trimmed.starts_with("func ") {
            self.functions.push(trimmed.to_string());
            println!("{} defined function", "✓".green());
            return Ok(Some(ReplValue::Void));
        }

        // 4. Handle assignment: `x = <expr>;`
        if let Some((var_name, expr)) = parse_assignment(trimmed) {
            if self.variables.contains_key(&var_name) {
                let val = self.eval_expr(&expr)?;
                let type_name = infer_value_type(&val);
                self.variables.insert(var_name.clone(), ReplVar {
                    name: var_name,
                    type_name,
                    value: val.clone(),
                });
                return Ok(Some(val));
            }
        }

        // 5. Evaluate bare expression or statement
        let expr = trimmed.strip_suffix(';').unwrap_or(trimmed);
        let val = self.eval_expr(expr)?;
        Ok(Some(val))
    }

    fn handle_meta_command(&mut self, cmd: &str) -> Result<Option<ReplValue>> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let name = parts[0];

        match name {
            ":help" | ":h" => {
                println!("{}", "🌹 Roze Interactive REPL Commands:".bold().magenta());
                println!("  {}             Show this help menu", ":help, :h".bold().yellow());
                println!("  {}             List all defined session variables", ":vars, :v".bold().yellow());
                println!("  {}             Clear all session variables and functions", ":clear, :c".bold().yellow());
                println!("  {} <expr>      Inspect the inferred type of an expression", ":type, :t".bold().yellow());
                println!("  {} <file.roze> Load and execute a Roze source file", ":load, :l".bold().yellow());
                println!("  {}          Show evaluation history", ":history".bold().yellow());
                println!("  {}      Exit the REPL", ":quit, :q, :exit".bold().yellow());
            }
            ":vars" | ":v" => {
                if self.variables.is_empty() {
                    println!("  No variables currently defined in session.");
                } else {
                    println!("{}", "Defined Variables:".bold().cyan());
                    let mut vars: Vec<_> = self.variables.values().collect();
                    vars.sort_by_key(|v| &v.name);
                    for v in vars {
                        println!("  {} {}: {} = {}", "let".blue(), v.name.bold(), v.type_name.yellow(), v.value);
                    }
                }
            }
            ":clear" | ":c" => {
                self.variables.clear();
                self.functions.clear();
                println!("{} Session cleared.", "🧹".yellow());
            }
            ":type" | ":t" => {
                if parts.len() < 2 {
                    println!("Usage: :type <expression>");
                } else {
                    let expr = parts[1..].join(" ");
                    match self.eval_expr(&expr) {
                        Ok(val) => {
                            let t = infer_value_type(&val);
                            println!("{} : {}", expr.bold(), t.cyan().bold());
                        }
                        Err(e) => {
                            eprintln!("{} {}", "❌ Error:".bright_red(), e);
                        }
                    }
                }
            }
            ":load" | ":l" => {
                if parts.len() < 2 {
                    println!("Usage: :load <path/to/file.roze>");
                } else {
                    let file_path = Path::new(parts[1]);
                    if !file_path.exists() {
                        eprintln!("{} File not found: {}", "❌ Error:".bright_red(), file_path.display());
                    } else {
                        let content = fs_err_read(file_path)?;
                        println!("📂 Loading {}...", file_path.display().to_string().cyan());
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() && !trimmed.starts_with("//") {
                                let _ = self.eval(trimmed);
                            }
                        }
                        println!("{} Successfully loaded {}", "✅".green(), file_path.display());
                    }
                }
            }
            ":history" => {
                for (idx, line) in self.history.iter().enumerate() {
                    println!("  {:3}: {}", idx + 1, line);
                }
            }
            ":quit" | ":q" | ":exit" => {
                std::process::exit(0);
            }
            _ => {
                println!("Unknown command: '{}'. Type ':help' for available commands.", name);
            }
        }

        Ok(Some(ReplValue::Void))
    }

    fn handle_let_decl(&mut self, line: &str) -> Result<Option<ReplValue>> {
        // e.g., `let x = 10;` or `let msg: string = "hello";`
        let without_let = line.strip_prefix("let ").unwrap().trim();
        let clean = without_let.strip_suffix(';').unwrap_or(without_let);

        let parts: Vec<&str> = clean.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("Invalid variable declaration syntax. Expected 'let <name> = <expr>;'"));
        }

        let left = parts[0].trim();
        let right_expr = parts[1].trim();

        let (var_name, explicit_type) = if left.contains(':') {
            let type_parts: Vec<&str> = left.splitn(2, ':').collect();
            (type_parts[0].trim().to_string(), Some(type_parts[1].trim().to_string()))
        } else {
            (left.to_string(), None)
        };

        let val = self.eval_expr(right_expr)?;
        let inferred_type = infer_value_type(&val);
        let final_type = explicit_type.unwrap_or(inferred_type);

        self.variables.insert(var_name.clone(), ReplVar {
            name: var_name,
            type_name: final_type,
            value: val.clone(),
        });

        Ok(Some(val))
    }

    /// Evaluates expressions (arithmetic, string concat, variables, logic, math intrinsics).
    pub fn eval_expr(&self, expr: &str) -> Result<ReplValue> {
        let trimmed = expr.trim();

        // 1. Check if it matches an existing variable
        if let Some(v) = self.variables.get(trimmed) {
            return Ok(v.value.clone());
        }

        // 2. Boolean literals
        if trimmed == "true" {
            return Ok(ReplValue::Bool(true));
        }
        if trimmed == "false" {
            return Ok(ReplValue::Bool(false));
        }

        // 3. String literal: `"..."`
        if is_single_string_literal(trimmed) {
            let inner = &trimmed[1..trimmed.len() - 1];
            return Ok(ReplValue::String(inner.to_string()));
        }

        // 4. Integer literal
        if let Ok(i) = trimmed.parse::<i64>() {
            return Ok(ReplValue::Int(i));
        }

        // 5. Float literal
        if let Ok(fl) = trimmed.parse::<f64>() {
            return Ok(ReplValue::Float(fl));
        }

        // 6. Builtin function calls
        if trimmed.starts_with("println(") && trimmed.ends_with(')') {
            let arg = &trimmed[8..trimmed.len() - 1];
            let val = self.eval_expr(arg)?;
            match val {
                ReplValue::String(s) => println!("{}", s),
                other => println!("{}", other),
            }
            return Ok(ReplValue::Void);
        }

        if trimmed.starts_with("math_sqrt(") && trimmed.ends_with(')') {
            let arg = &trimmed[10..trimmed.len() - 1];
            let val = self.eval_expr(arg)?;
            let num = match val {
                ReplValue::Int(i) => i as f64,
                ReplValue::Float(f) => f,
                _ => return Err(anyhow::anyhow!("math_sqrt requires a numeric argument")),
            };
            return Ok(ReplValue::Float(num.sqrt()));
        }

        if trimmed.starts_with("string_length(") && trimmed.ends_with(')') {
            let arg = &trimmed[14..trimmed.len() - 1];
            let val = self.eval_expr(arg)?;
            if let ReplValue::String(s) = val {
                return Ok(ReplValue::Int(s.len() as i64));
            }
            return Err(anyhow::anyhow!("string_length requires a string argument"));
        }

        // 7. Binary operations: `+`, `-`, `*`, `/`, `%`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`
        // Logical OR `||`
        if let Some((left, right)) = split_binary_op(trimmed, "||") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            if let (ReplValue::Bool(a), ReplValue::Bool(b)) = (l, r) {
                return Ok(ReplValue::Bool(a || b));
            }
        }

        // Logical AND `&&`
        if let Some((left, right)) = split_binary_op(trimmed, "&&") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            if let (ReplValue::Bool(a), ReplValue::Bool(b)) = (l, r) {
                return Ok(ReplValue::Bool(a && b));
            }
        }

        // Equality `==`
        if let Some((left, right)) = split_binary_op(trimmed, "==") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            return Ok(ReplValue::Bool(l == r));
        }

        // Not Equal `!=`
        if let Some((left, right)) = split_binary_op(trimmed, "!=") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            return Ok(ReplValue::Bool(l != r));
        }

        // Comparisons `<=`, `>=`, `<`, `>`
        if let Some((left, right)) = split_binary_op(trimmed, "<=") {
            return eval_num_cmp(self.eval_expr(left)?, self.eval_expr(right)?, |a, b| a <= b);
        }
        if let Some((left, right)) = split_binary_op(trimmed, ">=") {
            return eval_num_cmp(self.eval_expr(left)?, self.eval_expr(right)?, |a, b| a >= b);
        }
        if let Some((left, right)) = split_binary_op(trimmed, "<") {
            return eval_num_cmp(self.eval_expr(left)?, self.eval_expr(right)?, |a, b| a < b);
        }
        if let Some((left, right)) = split_binary_op(trimmed, ">") {
            return eval_num_cmp(self.eval_expr(left)?, self.eval_expr(right)?, |a, b| a > b);
        }

        // Addition and string concatenation `+`
        if let Some((left, right)) = split_binary_op(trimmed, "+") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            match (l, r) {
                (ReplValue::Int(a), ReplValue::Int(b)) => return Ok(ReplValue::Int(a + b)),
                (ReplValue::Float(a), ReplValue::Float(b)) => return Ok(ReplValue::Float(a + b)),
                (ReplValue::String(a), ReplValue::String(b)) => return Ok(ReplValue::String(format!("{}{}", a, b))),
                (ReplValue::String(a), other) => return Ok(ReplValue::String(format!("{}{}", a, other))),
                (other, ReplValue::String(b)) => return Ok(ReplValue::String(format!("{}{}", other, b))),
                _ => return Err(anyhow::anyhow!("Type mismatch in addition")),
            }
        }

        // Subtraction `-`
        if let Some((left, right)) = split_binary_op(trimmed, "-") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            if let (ReplValue::Int(a), ReplValue::Int(b)) = (l, r) {
                return Ok(ReplValue::Int(a - b));
            }
        }

        // Multiplication `*`
        if let Some((left, right)) = split_binary_op(trimmed, "*") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            if let (ReplValue::Int(a), ReplValue::Int(b)) = (l, r) {
                return Ok(ReplValue::Int(a * b));
            }
        }

        // Division `/`
        if let Some((left, right)) = split_binary_op(trimmed, "/") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            if let (ReplValue::Int(a), ReplValue::Int(b)) = (l, r) {
                if b == 0 {
                    return Err(anyhow::anyhow!("Division by zero"));
                }
                return Ok(ReplValue::Int(a / b));
            }
        }

        // Modulo `%`
        if let Some((left, right)) = split_binary_op(trimmed, "%") {
            let l = self.eval_expr(left)?;
            let r = self.eval_expr(right)?;
            if let (ReplValue::Int(a), ReplValue::Int(b)) = (l, r) {
                if b == 0 {
                    return Err(anyhow::anyhow!("Modulo by zero"));
                }
                return Ok(ReplValue::Int(a % b));
            }
        }

        Err(anyhow::anyhow!("Undefined expression or variable: '{}'", trimmed))
    }
}

fn fs_err_read(p: &Path) -> Result<String> {
    std::fs::read_to_string(p).map_err(|e| anyhow::anyhow!("Failed to read {}: {}", p.display(), e))
}

fn infer_value_type(val: &ReplValue) -> String {
    match val {
        ReplValue::Int(_) => "int".to_string(),
        ReplValue::Float(_) => "float".to_string(),
        ReplValue::Bool(_) => "bool".to_string(),
        ReplValue::String(_) => "string".to_string(),
        ReplValue::Void => "void".to_string(),
    }
}

fn parse_assignment(line: &str) -> Option<(String, String)> {
    let clean = line.strip_suffix(';').unwrap_or(line).trim();
    if clean.contains('=') && !clean.contains("==") && !clean.contains("<=") && !clean.contains(">=") && !clean.contains("!=") {
        let parts: Vec<&str> = clean.splitn(2, '=').collect();
        let name = parts[0].trim();
        let expr = parts[1].trim();
        if is_valid_ident(name) {
            return Some((name.to_string(), expr.to_string()));
        }
    }
    None
}

fn is_valid_ident(name: &str) -> bool {
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        if !first.is_alphabetic() && first != '_' {
            return false;
        }
        for ch in chars {
            if !ch.is_alphanumeric() && ch != '_' {
                return false;
            }
        }
        true
    } else {
        false
    }
}

fn is_single_string_literal(s: &str) -> bool {
    if !s.starts_with('"') || !s.ends_with('"') || s.len() < 2 {
        return false;
    }
    let chars: Vec<char> = s.chars().collect();
    let mut escaped = false;
    for &c in &chars[1..chars.len() - 1] {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return false;
        }
    }
    true
}

fn split_binary_op<'a>(s: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    // Avoid splitting inside quotes or parentheses
    let mut depth_paren: usize = 0;
    let mut in_quote = false;
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let op_len = op.len();

    let mut found_idx = None;

    let mut i = 0;
    while i < chars.len() {
        let (byte_idx, ch) = chars[i];
        if ch == '"' {
            in_quote = !in_quote;
        } else if !in_quote {
            if ch == '(' {
                depth_paren += 1;
            } else if ch == ')' {
                depth_paren = depth_paren.saturating_sub(1);
            } else if depth_paren == 0 {
                if s[byte_idx..].starts_with(op) {
                    found_idx = Some(byte_idx);
                }
            }
        }
        i += 1;
    }

    if let Some(idx) = found_idx {
        let left = s[..idx].trim();
        let right = s[idx + op_len..].trim();
        if !left.is_empty() && !right.is_empty() {
            return Some((left, right));
        }
    }

    None
}

fn eval_num_cmp<F>(l: ReplValue, r: ReplValue, cmp: F) -> Result<ReplValue>
where
    F: Fn(f64, f64) -> bool,
{
    match (l, r) {
        (ReplValue::Int(a), ReplValue::Int(b)) => Ok(ReplValue::Bool(cmp(a as f64, b as f64))),
        (ReplValue::Float(a), ReplValue::Float(b)) => Ok(ReplValue::Bool(cmp(a, b))),
        _ => Err(anyhow::anyhow!("Comparison requires numeric operands")),
    }
}

/// Runs the interactive REPL session with terminal I/O.
pub fn run_repl_cli() -> Result<()> {
    println!("{}", "==================================================================".magenta());
    println!("{}", "🌹 Roze Interactive REPL v1.0.0".bold().magenta());
    println!("{}", "⚡ High-Performance Developer Console & Real-Time Prototyping".cyan());
    println!("{}", "Type ':help' for commands, ':vars' for scope, ':quit' to exit.".yellow());
    println!("{}", "==================================================================".magenta());

    let mut session = ReplSession::new();
    let stdin = io::stdin();
    let mut reader = stdin.lock();

    let mut buffer = String::new();

    loop {
        if buffer.is_empty() {
            print!("{} ", "🌹 roze>".bold().green());
        } else {
            print!("{} ", "   ... >".cyan());
        }
        io::stdout().flush()?;

        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            // EOF (Ctrl+D)
            println!("\n{}", "Goodbye! 🌹".magenta());
            break;
        }

        buffer.push_str(&line);

        // Check if multi-line is open (e.g. unmatched braces)
        let open_braces = buffer.chars().filter(|&c| c == '{').count();
        let close_braces = buffer.chars().filter(|&c| c == '}').count();

        if open_braces > close_braces {
            continue; // Continue reading next line
        }

        let input_to_eval = buffer.clone();
        buffer.clear();

        match session.eval(&input_to_eval) {
            Ok(Some(val)) => {
                if val != ReplValue::Void {
                    println!("{} {}", "=>".bold().bright_green(), val);
                }
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
            }
        }

        session.line_number += 1;
    }

    Ok(())
}
