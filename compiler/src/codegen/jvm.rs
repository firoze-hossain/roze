use crate::parser::ast::*;
use anyhow::Result;
use std::collections::HashMap;

/// Java type used for Roze's "no annotation given" / dynamic-ish values.
const OBJECT_TYPE: &str = "Object";

/// Fixed helper methods emitted into every generated class, backing the
/// file/network intrinsics. Java requires checked exceptions
/// (`IOException`, `InterruptedException`) to be caught or declared;
/// Roze has no `throws` syntax and generated methods never declare any,
/// so these wrap each checked-exception-throwing call once here and
/// rethrow as an unchecked `RuntimeException` -- callers just see a
/// plain method call with no exception-handling of their own to write,
/// consistent with Roze not having a Result/Option type yet (see
/// ROADMAP.md).
const IO_HELPER_METHODS: &str = r#"
    private static String __roze_read_file(String path) {
        try {
            return new String(java.nio.file.Files.readAllBytes(java.nio.file.Paths.get(path)));
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_write_file(String path, String content) {
        try {
            java.nio.file.Files.write(java.nio.file.Paths.get(path), content.getBytes());
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static void __roze_append_file(String path, String content) {
        try {
            java.nio.file.Files.write(
                java.nio.file.Paths.get(path),
                content.getBytes(),
                java.nio.file.StandardOpenOption.CREATE,
                java.nio.file.StandardOpenOption.APPEND
            );
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static boolean __roze_delete_file(String path) {
        try {
            return java.nio.file.Files.deleteIfExists(java.nio.file.Paths.get(path));
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static java.util.List __roze_read_lines(String path) {
        try {
            return new java.util.ArrayList(java.nio.file.Files.readAllLines(java.nio.file.Paths.get(path)));
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        }
    }

    private static String __roze_http_get(String url) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url))
                .GET()
                .build();
            java.net.http.HttpResponse<String> response = client.send(request, java.net.http.HttpResponse.BodyHandlers.ofString());
            return response.body();
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
    }

    private static String __roze_http_post(String url, String body) {
        try {
            java.net.http.HttpClient client = java.net.http.HttpClient.newHttpClient();
            java.net.http.HttpRequest request = java.net.http.HttpRequest.newBuilder()
                .uri(java.net.URI.create(url))
                .header("Content-Type", "application/json")
                .POST(java.net.http.HttpRequest.BodyPublishers.ofString(body))
                .build();
            java.net.http.HttpResponse<String> response = client.send(request, java.net.http.HttpResponse.BodyHandlers.ofString());
            return response.body();
        } catch (java.io.IOException e) {
            throw new RuntimeException(e);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new RuntimeException(e);
        }
    }
"#;

/// Returns the Java return type for a built-in Core (string/math)
/// intrinsic, or None if `name` isn't one. Keep this in sync with
/// `semantic::builtin_signatures`.
fn intrinsic_return_type(name: &str) -> Option<&'static str> {
    match name {
        "string_length" => Some("int"),
        "string_concat" => Some("String"),
        "string_to_upper" => Some("String"),
        "string_to_lower" => Some("String"),
        "abs" => Some("int"),
        "max" => Some("int"),
        "min" => Some("int"),
        "to_string" => Some("String"),
        "to_int" => Some("int"),
        "is_number" => Some("boolean"),
        "is_string" => Some("boolean"),

        "list_new" => Some("java.util.List"),
        "list_push" => Some("boolean"),
        "list_get" => Some(OBJECT_TYPE),
        "list_set" => Some(OBJECT_TYPE),
        "list_remove" => Some(OBJECT_TYPE),
        "list_length" => Some("int"),
        "list_is_empty" => Some("boolean"),

        "map_new" => Some("java.util.Map"),
        "map_put" => Some(OBJECT_TYPE),
        "map_get" => Some(OBJECT_TYPE),
        "map_has" => Some("boolean"),
        "map_remove" => Some(OBJECT_TYPE),
        "map_size" => Some("int"),
        "map_is_empty" => Some("boolean"),

        "read_file" => Some("String"),
        "write_file" => Some("void"),
        "append_file" => Some("void"),
        "file_exists" => Some("boolean"),
        "delete_file" => Some("boolean"),
        "read_lines" => Some("java.util.List"),

        "http_get" => Some("String"),
        "http_post" => Some("String"),

        _ => None,
    }
}

fn is_intrinsic(name: &str) -> bool {
    intrinsic_return_type(name).is_some()
}

/// Escapes a raw string value (already fully resolved by the Roze
/// lexer -- e.g. a literal `\n` in the source is, by this point, a real
/// newline byte, not the two characters backslash-n) back into the
/// contents of a Java string literal. Must handle every control
/// character the lexer can produce, not just backslash/quote: emitting a
/// raw newline (or tab, or carriage return) directly into generated
/// Java source text produces invalid Java ("unclosed string literal"),
/// since Java string literals can't contain a literal newline.
fn escape_for_java_string_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),  // backspace
            '\u{c}' => out.push_str("\\f"),  // form feed
            _ => out.push(c),
        }
    }
    out
}

/// Maps a Roze source-level type annotation to a Java type name.
/// `default_java` is used when no annotation was given at all (distinct
/// from an annotation the compiler doesn't recognize, which still maps to
/// Object).
fn roze_type_to_java(type_name: Option<&str>, default_java: &str) -> String {
    match type_name {
        Some("int") => "int".to_string(),
        Some("string") => "String".to_string(),
        Some("bool") => "boolean".to_string(),
        Some("void") => "void".to_string(),
        Some("list") | Some("List") => "java.util.List".to_string(),
        Some("map") | Some("Map") => "java.util.Map".to_string(),
        Some(_) => OBJECT_TYPE.to_string(),
        None => default_java.to_string(),
    }
}

/// name -> java type, for locals/params visible at the current point in
/// generation. A `Vec` of frames acts as a scope stack (innermost last).
type Scope = Vec<HashMap<String, String>>;

pub struct JavaSourceGenerator {
    program: Program,
    class_name: String,
    /// name -> (java param types, java return type) for every user-defined
    /// top-level function, so call sites and `let` type inference never
    /// have to guess.
    functions: HashMap<String, (Vec<String>, String)>,
}

impl JavaSourceGenerator {
    pub fn new(program: Program, class_name: String) -> Self {
        let mut functions = HashMap::new();
        for stmt in &program.statements {
            if let Statement::Function { name, params, return_type, .. } = stmt {
                let param_types: Vec<String> = params.iter()
                    .map(|p| roze_type_to_java(p.type_name.as_deref(), OBJECT_TYPE))
                    .collect();
                let ret = roze_type_to_java(return_type.as_deref(), "void");
                functions.insert(name.clone(), (param_types, ret));
            }
        }
        Self { program, class_name, functions }
    }

    pub fn generate(&self) -> Result<String> {
        let mut source = String::new();
        source.push_str("// Generated by Roze compiler v0.1\n");
        source.push_str(&format!("public class {} {{\n", self.class_name));

        let mut main_found = false;
        for stmt in &self.program.statements {
            if let Statement::Function { name, params, body, .. } = stmt {
                // Core intrinsics are handled entirely at each call site
                // (see generate_call) and never emitted as a real method,
                // even if a same-named function happens to be declared.
                if is_intrinsic(name) {
                    continue;
                }

                if name == "main" {
                    source.push_str("    public static void main(String[] args) {\n");
                    let mut scope: Scope = vec![HashMap::new()];
                    self.generate_statement(&mut source, body, 2, &mut scope)?;
                    source.push_str("    }\n");
                    main_found = true;
                } else {
                    let (param_types, ret_java) = self.functions.get(name).cloned().unwrap_or_default();
                    let params_sig: Vec<String> = params.iter().zip(param_types.iter())
                        .map(|(p, t)| format!("{} {}", t, p.name))
                        .collect();

                    source.push_str(&format!(
                        "    public static {} {}({}) {{\n",
                        ret_java, name, params_sig.join(", ")
                    ));

                    let mut scope: Scope = vec![HashMap::new()];
                    {
                        let frame = scope.last_mut().expect("scope always has a frame");
                        for (p, t) in params.iter().zip(param_types.iter()) {
                            frame.insert(p.name.clone(), t.clone());
                        }
                    }
                    self.generate_statement(&mut source, body, 2, &mut scope)?;
                    source.push_str("    }\n");
                }
            }
        }

        if !main_found {
            source.push_str("    public static void main(String[] args) {\n");
            source.push_str("        System.out.println(\"No main function found!\");\n");
            source.push_str("    }\n");
        }

        source.push_str(IO_HELPER_METHODS);
        source.push_str("}\n");

        Ok(source)
    }

    fn generate_statement(&self, source: &mut String, stmt: &Statement, indent: usize, scope: &mut Scope) -> Result<()> {
        let indent_str = "    ".repeat(indent);

        match stmt {
            Statement::Block { statements, .. } => {
                for stmt in statements {
                    self.generate_statement(source, stmt, indent, scope)?;
                }
            }
            Statement::Expression { expr, .. } => {
                if let Expression::Call { function, arguments, .. } = expr.as_ref() {
                    if let Expression::Identifier { name, .. } = function.as_ref() {
                        if name == "println" {
                            source.push_str(&format!("{}System.out.println(", indent_str));
                            self.generate_println_args(source, arguments, scope)?;
                            source.push_str(");\n");
                            return Ok(());
                        }
                    }
                }
                source.push_str(&indent_str);
                self.generate_expression(source, expr, scope)?;
                source.push_str(";\n");
            }
            Statement::Let { name, value, .. } => {
                let java_type = self.infer_type(value, scope);

                source.push_str(&format!("{}{} {} = ", indent_str, java_type, name));
                self.generate_expression(source, value, scope)?;
                source.push_str(";\n");

                scope.last_mut().expect("scope always has a frame").insert(name.clone(), java_type);
            }
            Statement::Return { value, .. } => {
                if let Some(expr) = value {
                    source.push_str(&format!("{}return ", indent_str));
                    self.generate_expression(source, expr, scope)?;
                    source.push_str(";\n");
                } else {
                    source.push_str(&format!("{}return;\n", indent_str));
                }
            }
            Statement::Function { .. } => {
                // Nested function declarations aren't supported; top-level
                // functions are collected and emitted directly in `generate`.
            }
            Statement::Import { .. } => {
                // Imports are already resolved into real functions before
                // this stage even runs (see imports::resolve_imports),
                // so in practice there's nothing left to do here -- this
                // arm only exists in case that ever changes.
            }
            Statement::Assign { name, value, .. } => {
                source.push_str(&format!("{}{} = ", indent_str, name));
                self.generate_expression(source, value, scope)?;
                source.push_str(";\n");
            }
            Statement::If { condition, then_branch, else_branch, .. } => {
                self.generate_if_chain(source, condition, then_branch, else_branch.as_deref(), indent, scope, true)?;
            }
            Statement::While { condition, body, .. } => {
                source.push_str(&format!("{}while (", indent_str));
                self.generate_expression(source, condition, scope)?;
                source.push_str(") {\n");
                scope.push(HashMap::new());
                self.generate_statement(source, body, indent + 1, scope)?;
                scope.pop();
                source.push_str(&format!("{}}}\n", indent_str));
            }
            Statement::For { init, condition, update, body, .. } => {
                source.push_str(&format!("{}for (", indent_str));
                scope.push(HashMap::new());
                self.generate_for_clause(source, init, scope)?;
                source.push_str("; ");
                self.generate_expression(source, condition, scope)?;
                source.push_str("; ");
                self.generate_for_clause(source, update, scope)?;
                source.push_str(") {\n");
                self.generate_statement(source, body, indent + 1, scope)?;
                scope.pop();
                source.push_str(&format!("{}}}\n", indent_str));
            }
        }

        Ok(())
    }

    /// Renders a for-loop's init or update clause inline inside the
    /// Java `for (...)` header -- i.e. like `generate_statement`'s
    /// `Let`/`Assign` cases, but without the indent prefix or trailing
    /// `;\n` those emit as full statements. The parser guarantees a
    /// for-loop's init/update are always `Let` or `Assign`.
    fn generate_for_clause(&self, source: &mut String, stmt: &Statement, scope: &mut Scope) -> Result<()> {
        match stmt {
            Statement::Let { name, value, .. } => {
                let java_type = self.infer_type(value, scope);
                source.push_str(&format!("{} {} = ", java_type, name));
                self.generate_expression(source, value, scope)?;
                scope.last_mut().expect("scope always has a frame").insert(name.clone(), java_type);
            }
            Statement::Assign { name, value, .. } => {
                source.push_str(&format!("{} = ", name));
                self.generate_expression(source, value, scope)?;
            }
            _ => unreachable!("for-loop init/update is always Let or Assign (enforced by the parser)"),
        }
        Ok(())
    }

    /// Emits `if (cond) { ... }`, followed by any `else` / `else if` chain,
    /// ending with exactly one trailing newline.
    fn generate_if_chain(
        &self,
        source: &mut String,
        condition: &Expression,
        then_branch: &Statement,
        else_branch: Option<&Statement>,
        indent: usize,
        scope: &mut Scope,
        write_indent: bool,
    ) -> Result<()> {
        let indent_str = "    ".repeat(indent);

        if write_indent {
            source.push_str(&indent_str);
        }
        source.push_str("if (");
        self.generate_expression(source, condition, scope)?;
        source.push_str(") {\n");
        scope.push(HashMap::new());
        self.generate_statement(source, then_branch, indent + 1, scope)?;
        scope.pop();
        source.push_str(&format!("{}}}", indent_str));

        match else_branch {
            None => source.push('\n'),
            Some(Statement::If { condition, then_branch, else_branch, .. }) => {
                source.push_str(" else ");
                self.generate_if_chain(source, condition, then_branch, else_branch.as_deref(), indent, scope, false)?;
            }
            Some(other) => {
                source.push_str(" else {\n");
                scope.push(HashMap::new());
                self.generate_statement(source, other, indent + 1, scope)?;
                scope.pop();
                source.push_str(&format!("{}}}\n", indent_str));
            }
        }

        Ok(())
    }

    fn generate_println_args(&self, source: &mut String, arguments: &[Expression], scope: &Scope) -> Result<()> {
        if arguments.is_empty() {
            return Ok(());
        }
        if arguments.len() == 1 {
            return self.generate_expression(source, &arguments[0], scope);
        }
        // println with several arguments concatenates them, so
        // `println(a, b)` behaves like `println(a + b)`.
        source.push('(');
        for (i, arg) in arguments.iter().enumerate() {
            if i > 0 {
                source.push_str(" + ");
            }
            self.generate_expression(source, arg, scope)?;
        }
        source.push(')');
        Ok(())
    }

    /// Infers the Java type an expression will evaluate to, using tracked
    /// local/parameter types instead of guessing from variable names.
    fn infer_type(&self, expr: &Expression, scope: &Scope) -> String {
        match expr {
            Expression::Number { .. } => "int".to_string(),
            Expression::String { .. } => "String".to_string(),
            Expression::Boolean { .. } => "boolean".to_string(),
            Expression::Null { .. } => OBJECT_TYPE.to_string(),
            Expression::Identifier { name, .. } => {
                for frame in scope.iter().rev() {
                    if let Some(t) = frame.get(name) {
                        return t.clone();
                    }
                }
                OBJECT_TYPE.to_string()
            }
            Expression::Unary { operand, .. } => self.infer_type(operand, scope),
            Expression::Binary { left, operator, right, .. } => {
                let left_type = self.infer_type(left, scope);
                let right_type = self.infer_type(right, scope);

                match operator {
                    BinaryOperator::Add => {
                        if left_type == "String" || right_type == "String" {
                            "String".to_string()
                        } else if left_type == "int" && right_type == "int" {
                            "int".to_string()
                        } else {
                            // One side is some Object-typed/unknown value;
                            // Java's `+` between an Object and anything
                            // else always means string concatenation, so
                            // this is the only type that's always valid.
                            "String".to_string()
                        }
                    }
                    BinaryOperator::Subtract | BinaryOperator::Multiply | BinaryOperator::Divide => "int".to_string(),
                    BinaryOperator::Equal | BinaryOperator::NotEqual |
                    BinaryOperator::LessThan | BinaryOperator::GreaterThan |
                    BinaryOperator::LessEqual | BinaryOperator::GreaterEqual |
                    BinaryOperator::And | BinaryOperator::Or => "boolean".to_string(),
                }
            }
            Expression::Call { function, .. } => {
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    if name == "println" {
                        return "void".to_string();
                    }
                    if let Some(ret) = intrinsic_return_type(name) {
                        return ret.to_string();
                    }
                    if let Some((_, ret)) = self.functions.get(name) {
                        return ret.clone();
                    }
                }
                OBJECT_TYPE.to_string()
            }
        }
    }

    fn generate_expression(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        match expr {
            Expression::String { value, .. } => {
                source.push_str(&format!("\"{}\"", escape_for_java_string_literal(value)));
            }
            Expression::Number { value, .. } => {
                source.push_str(value);
            }
            Expression::Identifier { name, .. } => {
                source.push_str(name);
            }
            Expression::Boolean { value, .. } => {
                source.push_str(&format!("{}", value));
            }
            Expression::Null { .. } => {
                source.push_str("null");
            }
            Expression::Call { function, arguments, .. } => {
                if let Expression::Identifier { name, .. } = function.as_ref() {
                    self.generate_call(source, name, arguments, scope)?;
                }
            }
            Expression::Binary { left, operator, right, .. } => {
                let left_type = self.infer_type(left, scope);
                let right_type = self.infer_type(right, scope);
                let is_primitive = |t: &str| t == "int" || t == "boolean";

                let use_equals_method = matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
                    && (!is_primitive(&left_type) || !is_primitive(&right_type));

                if use_equals_method {
                    // Java's `==`/`!=` on a non-primitive type is
                    // reference identity, not content equality -- two
                    // distinct String objects with identical content
                    // (e.g. the result of runtime concatenation vs. a
                    // literal) would silently compare unequal. Route
                    // through Objects.equals instead, which is
                    // content-based and null-safe.
                    if matches!(operator, BinaryOperator::NotEqual) {
                        source.push('!');
                    }
                    source.push_str("java.util.Objects.equals(");
                    self.generate_expression(source, left, scope)?;
                    source.push_str(", ");
                    self.generate_expression(source, right, scope)?;
                    source.push(')');
                } else {
                    let is_string_concat = matches!(operator, BinaryOperator::Add)
                        && (left_type == "String" || right_type == "String");

                    source.push('(');
                    self.generate_expression(source, left, scope)?;
                    if is_string_concat {
                        source.push_str(" + ");
                    } else {
                        let op = match operator {
                            BinaryOperator::Add => " + ",
                            BinaryOperator::Subtract => " - ",
                            BinaryOperator::Multiply => " * ",
                            BinaryOperator::Divide => " / ",
                            BinaryOperator::Equal => " == ",
                            BinaryOperator::NotEqual => " != ",
                            BinaryOperator::LessThan => " < ",
                            BinaryOperator::GreaterThan => " > ",
                            BinaryOperator::LessEqual => " <= ",
                            BinaryOperator::GreaterEqual => " >= ",
                            BinaryOperator::And => " && ",
                            BinaryOperator::Or => " || ",
                        };
                        source.push_str(op);
                    }
                    self.generate_expression(source, right, scope)?;
                    source.push(')');
                }
            }
            Expression::Unary { operator, operand, .. } => {
                let op = match operator {
                    UnaryOperator::Negate => "-",
                    UnaryOperator::Not => "!",
                };
                source.push_str(op);
                self.generate_expression(source, operand, scope)?;
            }
        }

        Ok(())
    }

    /// Generates a call, rewriting Core (string/math) intrinsics to real
    /// JVM standard-library calls, since Roze has no method-call syntax of
    /// its own yet to express `s.length()` directly.
    fn generate_call(&self, source: &mut String, name: &str, arguments: &[Expression], scope: &Scope) -> Result<()> {
        match name {
            "string_length" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")).length()");
            }
            "string_to_upper" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")).toUpperCase()");
            }
            "string_to_lower" if arguments.len() == 1 => {
                source.push_str("((String)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(")).toLowerCase()");
            }
            "string_concat" if arguments.len() == 2 => {
                source.push('(');
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(" + ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "abs" if arguments.len() == 1 => {
                source.push_str("Math.abs(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push(')');
            }
            "max" if arguments.len() == 2 => {
                source.push_str("Math.max(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "min" if arguments.len() == 2 => {
                source.push_str("Math.min(");
                self.generate_int_arg(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_int_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "to_string" if arguments.len() == 1 => {
                source.push_str("String.valueOf(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "to_int" if arguments.len() == 1 => {
                source.push_str("Integer.parseInt(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "is_number" if arguments.len() == 1 => {
                source.push_str("((Object)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(") instanceof Integer)");
            }
            "is_string" if arguments.len() == 1 => {
                source.push_str("((Object)(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(") instanceof String)");
            }

            // ---- Collections: List ----
            "list_new" if arguments.is_empty() => {
                source.push_str("new java.util.ArrayList()");
            }
            "list_push" if arguments.len() == 2 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".add(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "list_get" if arguments.len() == 2 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".get(");
                self.generate_index_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "list_set" if arguments.len() == 3 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".set(");
                self.generate_index_arg(source, &arguments[1], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2], scope)?;
                source.push(')');
            }
            "list_remove" if arguments.len() == 2 => {
                // Must be a genuine primitive int (see generate_index_arg):
                // List.remove has both remove(int) and remove(Object)
                // overloads, and a boxed Integer argument binds to
                // remove(Object) -- remove-by-equality, not by index.
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".remove(");
                self.generate_index_arg(source, &arguments[1], scope)?;
                source.push(')');
            }
            "list_length" if arguments.len() == 1 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".size()");
            }
            "list_is_empty" if arguments.len() == 1 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".isEmpty()");
            }

            // ---- Collections: Map ----
            "map_new" if arguments.is_empty() => {
                source.push_str("new java.util.HashMap()");
            }
            "map_put" if arguments.len() == 3 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".put(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[2], scope)?;
                source.push(')');
            }
            "map_get" if arguments.len() == 2 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".get(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "map_has" if arguments.len() == 2 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".containsKey(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "map_remove" if arguments.len() == 2 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".remove(");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "map_size" if arguments.len() == 1 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".size()");
            }
            "map_is_empty" if arguments.len() == 1 => {
                self.generate_receiver(source, &arguments[0], scope)?;
                source.push_str(".isEmpty()");
            }

            // ---- IO: file ----
            // Each delegates to a fixed helper method (see
            // IO_HELPER_METHODS) that wraps the underlying checked
            // exception and rethrows unchecked.
            "read_file" if arguments.len() == 1 => {
                source.push_str("__roze_read_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "write_file" if arguments.len() == 2 => {
                source.push_str("__roze_write_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "append_file" if arguments.len() == 2 => {
                source.push_str("__roze_append_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }
            "file_exists" if arguments.len() == 1 => {
                // Files.exists doesn't throw, so no helper needed.
                source.push_str("java.nio.file.Files.exists(java.nio.file.Paths.get(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str("))");
            }
            "delete_file" if arguments.len() == 1 => {
                source.push_str("__roze_delete_file(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "read_lines" if arguments.len() == 1 => {
                source.push_str("__roze_read_lines(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }

            // ---- IO: network ----
            "http_get" if arguments.len() == 1 => {
                source.push_str("__roze_http_get(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push(')');
            }
            "http_post" if arguments.len() == 2 => {
                source.push_str("__roze_http_post(");
                self.generate_expression(source, &arguments[0], scope)?;
                source.push_str(", ");
                self.generate_expression(source, &arguments[1], scope)?;
                source.push(')');
            }

            "println" => {
                source.push_str("System.out.println(");
                self.generate_println_args(source, arguments, scope)?;
                source.push(')');
            }
            _ => {
                source.push_str(name);
                source.push('(');
                for (i, arg) in arguments.iter().enumerate() {
                    if i > 0 {
                        source.push_str(", ");
                    }
                    self.generate_expression(source, arg, scope)?;
                }
                source.push(')');
            }
        }
        Ok(())
    }

    /// Generates an expression known to be used in an `int`-typed position
    /// (a Math.* argument). If the expression's static type isn't already
    /// `int`, it's coming from an Object-typed (untyped-parameter) value,
    /// so we unbox it via Integer -- valid whenever the runtime value
    /// really is a boxed Integer, matching the declared `int` parameter.
    fn generate_int_arg(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        let t = self.infer_type(expr, scope);
        if t == "int" {
            self.generate_expression(source, expr, scope)
        } else {
            source.push_str("((Integer)(Object)(");
            self.generate_expression(source, expr, scope)?;
            source.push_str("))");
            Ok(())
        }
    }

    /// Generates an expression used as the receiver of a method call
    /// (e.g. the list in `list.add(x)`), wrapped in parens as a cheap,
    /// always-safe guard against needing to reason about whether the
    /// receiver expression needs them for any particular case.
    fn generate_receiver(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        source.push('(');
        self.generate_expression(source, expr, scope)?;
        source.push(')');
        Ok(())
    }

    /// Like `generate_int_arg`, but guarantees a genuine primitive `int`
    /// (via an explicit `.intValue()` unboxing call) rather than
    /// potentially leaving a boxed `Integer`. Needed specifically for
    /// `List` index arguments: `List.remove` has both `remove(int)` and
    /// `remove(Object)` overloads, and a boxed `Integer` argument binds
    /// to `remove(Object)` (remove-by-equality) since that's reachable
    /// without any unboxing at all -- silently doing the wrong thing
    /// rather than failing to compile. `List.get`/`set` don't have that
    /// particular ambiguity, but using the same guaranteed-primitive
    /// helper for all three keeps index handling uniform.
    fn generate_index_arg(&self, source: &mut String, expr: &Expression, scope: &Scope) -> Result<()> {
        let t = self.infer_type(expr, scope);
        if t == "int" {
            self.generate_expression(source, expr, scope)
        } else {
            source.push_str("((Integer)(Object)(");
            self.generate_expression(source, expr, scope)?;
            source.push_str(")).intValue()");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn generate(src: &str) -> String {
        let program = parse(tokenize(src)).expect("fixture should parse");
        JavaSourceGenerator::new(program, "Fixture".to_string())
            .generate()
            .expect("fixture should generate")
    }

    #[test]
    fn emits_every_function_not_just_main() {
        let java = generate("func greet() { println(\"hi\"); } func main() { greet(); }");
        assert!(java.contains("static void greet"), "greet() should be emitted:\n{}", java);
        assert!(java.contains("static void main"), "main() should be emitted:\n{}", java);
        assert!(java.contains("greet();"), "main() should call greet():\n{}", java);
    }

    #[test]
    fn uses_declared_types_not_name_guessing() {
        // Previously, codegen guessed a variable's Java type from its
        // *name* (only x/y/z/i/j/k were ever treated as int). A variable
        // holding a string, named like one of those letters, must still
        // come out as String.
        let java = generate("func main() { let x = \"hello\"; println(x); }");
        assert!(java.contains("String x ="), "expected 'String x =', got:\n{}", java);
    }

    #[test]
    fn if_else_if_chain_generates_valid_shape() {
        let java = generate("func f(x: int) -> int { if x > 0 { return 1; } else if x < 0 { return -1; } else { return 0; } } func main() { }");
        assert!(java.contains("if (") && java.contains("} else if (") && java.contains("} else {"), "unexpected if/else shape:\n{}", java);
    }

    #[test]
    fn while_loop_generates_java_while() {
        let java = generate("func main() { let i = 0; while i < 3 { i = i + 1; } }");
        assert!(java.contains("while ("), "expected a while loop:\n{}", java);
    }

    #[test]
    fn for_loop_generates_java_for() {
        let java = generate("func main() { for let i = 0; i < 3; i = i + 1 { println(i); } }");
        assert!(java.contains("for (int i = 0;"), "expected a Java for-loop header:\n{}", java);
    }

    #[test]
    fn string_intrinsics_map_to_real_java_calls() {
        let java = generate("func main() { println(string_to_upper(\"hi\")); println(string_length(\"hi\")); }");
        assert!(java.contains(".toUpperCase()"), "expected toUpperCase():\n{}", java);
        assert!(java.contains(".length()"), "expected length():\n{}", java);
    }

    #[test]
    fn math_intrinsics_map_to_java_math() {
        let java = generate("func main() { println(abs(-5)); println(max(1, 2)); println(min(1, 2)); }");
        assert!(java.contains("Math.abs("));
        assert!(java.contains("Math.max("));
        assert!(java.contains("Math.min("));
    }

    #[test]
    fn intrinsic_named_function_is_never_emitted_as_a_real_method() {
        // Even if a program somehow declares its own `abs`, it must not
        // get emitted as a real Java method -- the call-site rewrite to
        // Math.abs() always takes priority.
        let java = generate("func abs(x: int) -> int { return x; } func main() { println(abs(-5)); }");
        assert!(!java.contains("static int abs"), "an 'abs' method should never be emitted:\n{}", java);
        assert!(java.contains("Math.abs("), "the call site should still use Math.abs():\n{}", java);
    }

    #[test]
    fn string_equality_compares_content_not_reference() {
        // Java's == on Strings is reference identity; a naive codegen
        // would emit `combined == "Hello World"` which silently gives the
        // wrong answer at runtime for a non-interned (e.g. runtime
        // concatenation) result. Must route through content equality.
        let java = generate("func main() { let a = \"Hello\"; let b = a + \"!\"; println(b == \"Hello!\"); }");
        assert!(java.contains("Objects.equals("), "expected content equality via Objects.equals:\n{}", java);
        assert!(!java.contains("== \"Hello!\""), "must not use reference equality (==) on strings:\n{}", java);
    }

    #[test]
    fn int_equality_still_uses_primitive_operator() {
        // No need to pay for Objects.equals boxing when both sides are
        // already primitives -- plain == is correct and cheaper.
        let java = generate("func main() { let a = 5; println(a == 5); }");
        assert!(java.contains("(a == 5)"), "expected plain == for int comparison:\n{}", java);
    }

    #[test]
    fn no_main_falls_back_to_placeholder() {
        let java = generate("func helper() { println(\"hi\"); }");
        assert!(java.contains("No main function found"));
    }

    // ---- Collections ----

    #[test]
    fn list_operations_map_to_java_list_methods() {
        let java = generate(
            "func main() { let l = list_new(); list_push(l, 1); println(list_get(l, 0)); list_set(l, 0, 2); list_remove(l, 0); println(list_length(l)); println(list_is_empty(l)); }"
        );
        assert!(java.contains("new java.util.ArrayList()"), "expected list_new -> ArrayList:\n{}", java);
        assert!(java.contains(").add("), "expected list_push -> .add(:\n{}", java);
        assert!(java.contains(").get("), "expected list_get -> .get(:\n{}", java);
        assert!(java.contains(").set("), "expected list_set -> .set(:\n{}", java);
        assert!(java.contains(").remove("), "expected list_remove -> .remove(:\n{}", java);
        assert!(java.contains(").size()"), "expected list_length -> .size():\n{}", java);
        assert!(java.contains(").isEmpty()"), "expected list_is_empty -> .isEmpty():\n{}", java);
    }

    #[test]
    fn list_remove_forces_primitive_int_to_avoid_overload_ambiguity() {
        // List.remove(int) vs remove(Object): an untyped (Object-typed)
        // index must be explicitly unboxed with .intValue(), or Java
        // silently picks remove(Object) -- remove-by-equality instead of
        // remove-by-index.
        let java = generate("func f(idx) { let l = list_new(); list_remove(l, idx); } func main() { }");
        assert!(java.contains(".intValue()"), "expected an explicit .intValue() unboxing for a non-int index:\n{}", java);
    }

    #[test]
    fn map_operations_map_to_java_map_methods() {
        let java = generate(
            "func main() { let m = map_new(); map_put(m, \"a\", 1); println(map_get(m, \"a\")); println(map_has(m, \"a\")); map_remove(m, \"a\"); println(map_size(m)); println(map_is_empty(m)); }"
        );
        assert!(java.contains("new java.util.HashMap()"), "expected map_new -> HashMap:\n{}", java);
        assert!(java.contains(").put("), "expected map_put -> .put(:\n{}", java);
        assert!(java.contains(").get("), "expected map_get -> .get(:\n{}", java);
        assert!(java.contains(").containsKey("), "expected map_has -> .containsKey(:\n{}", java);
        assert!(java.contains(").remove("), "expected map_remove -> .remove(:\n{}", java);
        assert!(java.contains(").size()"), "expected map_size -> .size():\n{}", java);
        assert!(java.contains(").isEmpty()"), "expected map_is_empty -> .isEmpty():\n{}", java);
    }

    #[test]
    fn list_and_map_declared_types_use_java_util() {
        let java = generate("func main() { let l = list_new(); let m = map_new(); }");
        assert!(java.contains("java.util.List l ="), "expected 'java.util.List l =':\n{}", java);
        assert!(java.contains("java.util.Map m ="), "expected 'java.util.Map m =':\n{}", java);
    }

    // ---- IO: file ----

    #[test]
    fn file_intrinsics_delegate_to_helper_methods() {
        let java = generate(
            "func main() { write_file(\"a\", \"b\"); append_file(\"a\", \"c\"); read_file(\"a\"); delete_file(\"a\"); read_lines(\"a\"); }"
        );
        assert!(java.contains("__roze_write_file("));
        assert!(java.contains("__roze_append_file("));
        assert!(java.contains("__roze_read_file("));
        assert!(java.contains("__roze_delete_file("));
        assert!(java.contains("__roze_read_lines("));
        // Every helper method must actually be defined in the class.
        assert!(java.contains("private static void __roze_write_file"));
        assert!(java.contains("private static void __roze_append_file"));
        assert!(java.contains("private static String __roze_read_file"));
        assert!(java.contains("private static boolean __roze_delete_file"));
        assert!(java.contains("private static java.util.List __roze_read_lines"));
    }

    #[test]
    fn file_exists_does_not_need_a_helper() {
        // Files.exists doesn't throw a checked exception, so it can be
        // called directly without a try/catch-wrapping helper.
        let java = generate("func main() { file_exists(\"a\"); }");
        assert!(java.contains("java.nio.file.Files.exists("));
        assert!(!java.contains("__roze_file_exists"));
    }

    // ---- IO: network ----

    #[test]
    fn network_intrinsics_delegate_to_helper_methods() {
        let java = generate("func main() { http_get(\"http://x\"); http_post(\"http://x\", \"body\"); }");
        assert!(java.contains("__roze_http_get("));
        assert!(java.contains("__roze_http_post("));
        assert!(java.contains("private static String __roze_http_get"));
        assert!(java.contains("private static String __roze_http_post"));
        assert!(java.contains("java.net.http.HttpClient"));
    }

    // ---- String escaping ----

    #[test]
    fn string_escapes_are_re_escaped_for_java() {
        // The Roze lexer resolves "\n" in source into a real newline
        // byte; codegen must re-escape it back to "\n" in the generated
        // Java text, or the .java file contains a raw newline inside a
        // string literal (invalid Java: "unclosed string literal").
        let java = generate("func main() { println(\"a\\nb\\tc\"); }");
        assert!(java.contains("\"a\\nb\\tc\""), "expected re-escaped \\n and \\t, got:\n{}", java);
        assert!(!java.contains("\"a\nb"), "must not contain a raw newline inside the Java string literal:\n{}", java);
    }

    #[test]
    fn backslash_and_quote_are_still_escaped() {
        let java = generate(r#"func main() { println("a\\b\"c"); }"#);
        assert!(java.contains(r#""a\\b\"c""#), "expected escaped backslash and quote, got:\n{}", java);
    }
}
