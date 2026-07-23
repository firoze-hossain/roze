// compiler/src/error.rs
//
// Structured compiler errors that know their own source position, so we
// can render a real "Roze-flavored" report -- message, a `-->` pointer at
// file:line:column, the actual offending source line, and a `^^^`
// underline -- instead of a bare error string, and never a Rust panic
// backtrace.
use colored::*;
use std::fmt;

/// Which compiler stage raised the error. Used to choose a label so
/// someone reading the error knows roughly where in the pipeline to look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Lexer,
    Parser,
    Type,
}

impl ErrorKind {
    fn label(&self) -> &'static str {
        match self {
            ErrorKind::Lexer => "Lexer error",
            ErrorKind::Parser => "Parse error",
            ErrorKind::Type => "Type error",
        }
    }
}

/// A single reportable compiler error, with enough position information
/// to point at the exact place in the user's source that caused it.
#[derive(Debug, Clone)]
pub struct RozeError {
    pub kind: ErrorKind,
    pub message: String,
    pub line: usize,
    pub column: usize,
    /// How many characters to underline starting at `column`. Defaults to
    /// 1 (a single caret) when the exact token width isn't known.
    pub length: usize,
    pub hint: Option<String>,
}

impl RozeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            kind,
            message: message.into(),
            line,
            column,
            length: 1,
            hint: None,
        }
    }

    pub fn lexer(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(ErrorKind::Lexer, message, line, column)
    }

    pub fn parser(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(ErrorKind::Parser, message, line, column)
    }

    pub fn type_error(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self::new(ErrorKind::Type, message, line, column)
    }

    pub fn with_length(mut self, length: usize) -> Self {
        self.length = length.max(1);
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Renders a full, friendly report: the message, a `-->` pointer at
    /// file:line:column (mirroring rustc's well-understood format), the
    /// offending source line, a `^^^` underline under the exact span, and
    /// an optional hint.
    ///
    /// `file` and `source` are supplied here rather than stored on the
    /// error itself, since the error is typically constructed deep inside
    /// the lexer/parser/type checker, well before anyone there has (or
    /// needs) the file name or a copy of the full source text.
    pub fn report(&self, file: &str, source: &str) -> String {
        let mut out = String::new();

        out.push_str(&format!("{}: {}", self.kind.label().bright_red().bold(), self.message));
        out.push_str(&format!(
            "\n  {} {}:{}:{}",
            "-->".blue().bold(),
            file,
            self.line,
            self.column
        ));

        let lines: Vec<&str> = source.lines().collect();
        if !lines.is_empty() {
            let (line_no, line_text, at_eof) = if self.line >= 1 && self.line <= lines.len() {
                (self.line, lines[self.line - 1], false)
            } else {
                (lines.len(), lines[lines.len() - 1], true)
            };
            let gutter = format!("{}", line_no);
            let gutter_width = gutter.len();

            out.push_str(&format!("\n{} {}", " ".repeat(gutter_width), "|".blue().bold()));
            out.push_str(&format!(
                "\n{} {} {}",
                gutter.blue().bold(),
                "|".blue().bold(),
                line_text
            ));

            let col = if at_eof {
                line_text.chars().count()
            } else {
                self.column.max(1) - 1
            };
            let underline_len = if at_eof { 1 } else { self.length.max(1) };
            out.push_str(&format!(
                "\n{} {} {}{}",
                " ".repeat(gutter_width),
                "|".blue().bold(),
                " ".repeat(col),
                "^".repeat(underline_len).bright_red().bold()
            ));
            if at_eof {
                out.push_str(&" (end of file)".dimmed().to_string());
            }
        }

        if let Some(hint) = &self.hint {
            out.push_str(&format!("\n  {} {}", "help:".green().bold(), hint));
        }

        out
    }
}

impl fmt::Display for RozeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at line {}, column {}: {}",
            self.kind.label(),
            self.line,
            self.column,
            self.message
        )
    }
}

impl std::error::Error for RozeError {}
