// src/parser.rs
#[derive(Debug, Clone)]
pub struct Ast {
    pub functions: Vec<Function>,
    pub variables: Vec<Variable>,
    pub classes: Vec<Class>,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub return_type: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub type_: Option<String>,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone)]
pub struct Class {
    pub name: String,
    pub fields: Vec<Variable>,
    pub methods: Vec<Function>,
    pub line: usize,
    pub column: usize,
}

pub fn parse(source: &str) -> Option<Ast> {
    let mut ast = Ast {
        functions: Vec::new(),
        variables: Vec::new(),
        classes: Vec::new(),
        imports: Vec::new(),
    };

    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty() || line.starts_with("//") || line.starts_with("/*") {
            i += 1;
            continue;
        }

        if line.starts_with("import") {
            if let Some(path) = line.strip_prefix("import").map(|s| s.trim().trim_end_matches(';')) {
                ast.imports.push(path.to_string());
            }
            i += 1;
            continue;
        }

        if line.starts_with("func") {
            if let Some(func) = parse_function(&lines, &mut i) {
                ast.functions.push(func);
            }
            continue;
        }

        if line.starts_with("class") {
            if let Some(class) = parse_class(&lines, &mut i) {
                ast.classes.push(class);
            }
            continue;
        }

        if line.starts_with("let") {
            if let Some(var) = parse_variable(line) {
                ast.variables.push(var);
            }
        }

        i += 1;
    }

    Some(ast)
}

fn parse_function(lines: &[&str], index: &mut usize) -> Option<Function> {
    let line = lines[*index].trim();
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    let name = parts[1].to_string();
    let mut params = Vec::new();

    if let Some(param_start) = line.find('(') {
        if let Some(param_end) = line.find(')') {
            let param_str = &line[param_start + 1..param_end];
            if !param_str.is_empty() {
                params = param_str.split(',').map(|s| s.trim().to_string()).collect();
            }
        }
    }

    *index += 1;
    let mut brace_count = 1;
    while *index < lines.len() && brace_count > 0 {
        brace_count += lines[*index].chars().filter(|&c| c == '{').count();
        brace_count -= lines[*index].chars().filter(|&c| c == '}').count();
        *index += 1;
    }

    Some(Function {
        name,
        params,
        return_type: None,
        line: *index - 1,
        column: 0,
    })
}

fn parse_class(lines: &[&str], index: &mut usize) -> Option<Class> {
    let line = lines[*index].trim();
    let parts: Vec<&str> = line.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    let name = parts[1].trim_end_matches('{').to_string();
    let mut fields = Vec::new();
    let mut methods = Vec::new();

    *index += 1;
    let mut brace_count = 1;

    while *index < lines.len() && brace_count > 0 {
        let body_line = lines[*index].trim();

        if body_line.starts_with("let") {
            if let Some(var) = parse_variable(body_line) {
                fields.push(var);
            }
        } else if body_line.starts_with("func") {
            if let Some(func) = parse_function(lines, index) {
                methods.push(func);
                continue;
            }
        }

        brace_count += lines[*index].chars().filter(|&c| c == '{').count();
        brace_count -= lines[*index].chars().filter(|&c| c == '}').count();
        *index += 1;
    }

    Some(Class {
        name,
        fields,
        methods,
        line: *index - 1,
        column: 0,
    })
}

fn parse_variable(line: &str) -> Option<Variable> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    let name = parts[1].trim_end_matches(';').to_string();
    let mut type_ = None;

    if let Some(colon_pos) = line.find(':') {
        let after_colon = &line[colon_pos + 1..].trim();
        if let Some(equal_pos) = after_colon.find('=') {
            type_ = Some(after_colon[..equal_pos].trim().to_string());
        }
    }

    Some(Variable {
        name,
        type_,
        line: 0,
        column: 0,
    })
}