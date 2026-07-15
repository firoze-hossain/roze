// compiler/src/error.rs
use colored::*;
use std::fmt;

#[derive(Debug)]
pub struct CompilerError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub file: String,
    pub hint: Option<String>,
}

impl CompilerError {
    pub fn new(message: &str, line: usize, column: usize, file: &str) -> Self {
        Self {
            message: message.to_string(),
            line,
            column,
            file: file.to_string(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: &str) -> Self {
        self.hint = Some(hint.to_string());
        self
    }

    pub fn display(&self, source: &str) -> String {
        let mut output = String::new();
        output.push_str(&format!("{} at {}:{}:{}", "❌ Error".bright_red(), self.file, self.line, self.column));
        output.push_str(&format!("\n  {}", self.message));

        if let Some(hint) = &self.hint {
            output.push_str(&format!("\n  {}", hint));
        }

        // Show source line
        let lines: Vec<&str> = source.lines().collect();
        if self.line <= lines.len() {
            let line = lines[self.line - 1];
            output.push_str(&format!("\n  {}\n", line));

            // Add arrow
            let arrow = "  ".to_string() + &" ".repeat(self.column - 1) + "^";
            output.push_str(&format!("{}\n", arrow));
        }

        output
    }
}