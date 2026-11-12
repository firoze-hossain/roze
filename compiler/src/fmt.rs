// compiler/src/fmt.rs
//
// 🌹 Roze Source Code Formatter (roze-fmt)
// Automated, deterministic code formatting with AST/token awareness.
// Parity with 'rustfmt' and 'gofmt'.

use std::fs;
use std::path::{Path, PathBuf};
use colored::*;
use anyhow::{Result, anyhow};
use walkdir::WalkDir;

/// Configuration options for the Roze source code formatter.
#[derive(Debug, Clone)]
pub struct FmtOptions {
    pub indent_size: usize,
    pub check: bool,
    pub verbose: bool,
}

impl Default for FmtOptions {
    fn default() -> Self {
        Self {
            indent_size: 4,
            check: false,
            verbose: false,
        }
    }
}

/// Result summary of formatting operations across multiple files.
#[derive(Debug, Default, Clone)]
pub struct FmtSummary {
    pub files_checked: usize,
    pub files_formatted: usize,
    pub unformatted_files: Vec<PathBuf>,
}

/// Formats Roze source code string according to standard formatting style.
/// Completely deterministic and idempotent: format_source(&format_source(s), n) == format_source(s, n).
pub fn format_source(source: &str, indent_size: usize) -> String {
    let mut formatted = String::with_capacity(source.len() + 128);
    let mut indent_level: usize = 0;
    let mut in_block_comment = false;
    let mut consecutive_blank_lines = 0;

    let lines: Vec<&str> = source.lines().collect();

    for raw_line in lines {
        let trimmed = raw_line.trim();

        // 1. Handle blank lines
        if trimmed.is_empty() {
            if !formatted.is_empty() && consecutive_blank_lines < 1 && !in_block_comment {
                formatted.push('\n');
                consecutive_blank_lines += 1;
            }
            continue;
        }
        consecutive_blank_lines = 0;

        // 2. Handle block comment states
        if in_block_comment {
            let indent = " ".repeat(indent_level * indent_size);
            formatted.push_str(&indent);
            formatted.push_str(trimmed);
            formatted.push('\n');
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.starts_with("/*") && !trimmed.contains("*/") {
            in_block_comment = true;
            let indent = " ".repeat(indent_level * indent_size);
            formatted.push_str(&indent);
            formatted.push_str(trimmed);
            formatted.push('\n');
            continue;
        }

        // 3. Count closing braces at start of line to de-indent before printing
        let closing_braces = count_leading_closing_braces(trimmed);
        if closing_braces > 0 && indent_level >= closing_braces {
            indent_level -= closing_braces;
        }

        // 4. Format the line content (preserving string literals & comments)
        let formatted_line = format_line_tokens(trimmed);

        // 5. Apply indentation
        let indent = " ".repeat(indent_level * indent_size);
        formatted.push_str(&indent);
        formatted.push_str(&formatted_line);
        formatted.push('\n');

        // 6. Update indent level based on braces in line (excluding strings/comments)
        let (open_count, close_count) = count_braces_in_line(trimmed);
        indent_level += open_count;
        let trailing_close = close_count.saturating_sub(closing_braces);
        if indent_level >= trailing_close {
            indent_level -= trailing_close;
        }
    }

    // Ensure single trailing newline
    if !formatted.is_empty() && !formatted.ends_with('\n') {
        formatted.push('\n');
    }

    formatted
}

/// Formats a single line while preserving string literals and comments.
fn format_line_tokens(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 16);
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    let mut in_str = false;
    let mut str_quote = '"';

    while i < len {
        let ch = chars[i];

        // Handle string literals
        if in_str {
            out.push(ch);
            if ch == '\\' && i + 1 < len {
                i += 1;
                out.push(chars[i]);
            } else if ch == str_quote {
                in_str = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_str = true;
            str_quote = ch;
            out.push(ch);
            i += 1;
            continue;
        }

        // Handle line comment `//`
        if ch == '/' && i + 1 < len && chars[i + 1] == '/' {
            // Ensure space before comment if not at start of line
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            let rest: String = chars[i..].iter().collect();
            // Ensure space after `//` if missing (e.g. `//comment` -> `// comment`)
            if rest.starts_with("//") && rest.len() > 2 && chars[i + 2] != ' ' && chars[i + 2] != '/' && chars[i + 2] != '!' {
                out.push_str("// ");
                out.push_str(&rest[2..]);
            } else {
                out.push_str(&rest);
            }
            break;
        }

        // Handle block comment `/* ... */` on same line
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            let rest: String = chars[i..].iter().collect();
            out.push_str(&rest);
            break;
        }

        // Spacing around operators and punctuation
        match ch {
            ',' => {
                // Strip any space before comma
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(',');
                out.push(' ');
                // Skip subsequent spaces in input
                while i + 1 < len && chars[i + 1] == ' ' {
                    i += 1;
                }
            }
            ';' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(';');
                if i + 1 < len && chars[i + 1] != ' ' && chars[i + 1] != '}' && chars[i + 1] != '/' {
                    out.push(' ');
                }
            }
            ':' => {
                // Check if this is `::`
                if i + 1 < len && chars[i + 1] == ':' {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    out.push_str("::");
                    i += 1;
                } else {
                    // Type annotation colon or dict colon: `name: type`
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    out.push(':');
                    out.push(' ');
                    while i + 1 < len && chars[i + 1] == ' ' {
                        i += 1;
                    }
                }
            }
            '-' if i + 1 < len && chars[i + 1] == '>' => {
                // Arrow `->`
                if !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str("-> ");
                i += 1;
                while i + 1 < len && chars[i + 1] == ' ' {
                    i += 1;
                }
            }
            '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '&' | '|' => {
                // Check for multi-char operators: `==`, `!=`, `<=`, `>=`, `+=`, `-=`, `*=`, `/=`, `&&`, `||`
                let next_ch = if i + 1 < len { chars[i + 1] } else { '\0' };
                let is_compound = (ch == '=' && next_ch == '=')
                    || (ch == '<' && next_ch == '=')
                    || (ch == '>' && next_ch == '=')
                    || (ch == '+' && next_ch == '=')
                    || (ch == '-' && next_ch == '=')
                    || (ch == '*' && next_ch == '=')
                    || (ch == '/' && next_ch == '=')
                    || (ch == '&' && next_ch == '&')
                    || (ch == '|' && next_ch == '|');

                // Check for unary minus: e.g. `-10` or `let x = -5`
                let is_unary_minus = ch == '-' && !is_compound && (out.is_empty() || out.ends_with('(') || out.ends_with('[') || out.ends_with(',') || out.ends_with('='));

                if is_unary_minus {
                    out.push('-');
                } else if is_compound {
                    if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('(') {
                        out.push(' ');
                    }
                    out.push(ch);
                    out.push(next_ch);
                    out.push(' ');
                    i += 1;
                    while i + 1 < len && chars[i + 1] == ' ' {
                        i += 1;
                    }
                } else {
                    if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('(') {
                        out.push(' ');
                    }
                    out.push(ch);
                    if i + 1 < len && chars[i + 1] != ' ' && chars[i + 1] != ';' && chars[i + 1] != ')' && chars[i + 1] != ',' {
                        out.push(' ');
                    }
                }
            }
            '!' if i + 1 < len && chars[i + 1] == '=' => {
                // `!=`
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str("!= ");
                i += 1;
                while i + 1 < len && chars[i + 1] == ' ' {
                    i += 1;
                }
            }
            '(' | '[' => {
                if out.ends_with(' ') {
                    let trimmed_end = out.trim_end();
                    if let Some(last_ch) = trimmed_end.chars().last() {
                        if last_ch.is_alphanumeric() || last_ch == '_' {
                            let last_word = trimmed_end.split(|c: char| !c.is_alphanumeric() && c != '_').last().unwrap_or("");
                            let is_kw = matches!(last_word, "if" | "while" | "for" | "match" | "switch" | "return");
                            if !is_kw {
                                while out.ends_with(' ') {
                                    out.pop();
                                }
                            }
                        }
                    }
                }
                out.push(ch);
                // Skip following whitespace in input
                while i + 1 < len && (chars[i + 1] == ' ' || chars[i + 1] == '\t') {
                    i += 1;
                }
            }
            ')' | ']' => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(ch);
            }
            '{' => {
                if !out.is_empty() && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push('{');
                if i + 1 < len && chars[i + 1] != ' ' && chars[i + 1] != '}' {
                    out.push(' ');
                }
            }
            '}' => {
                if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('{') {
                    out.push(' ');
                }
                out.push('}');
            }
            ' ' | '\t' => {
                if !out.is_empty() && !out.ends_with(' ') && !out.ends_with('(') && !out.ends_with('[') {
                    out.push(' ');
                }
            }
            _ => {
                out.push(ch);
            }
        }
        i += 1;
    }

    // Clean up keywords like `if(`, `while(`, `for(` to `if (`, `while (`, `for (`
    cleanup_keywords(&out)
}

fn cleanup_keywords(s: &str) -> String {
    let mut res = s.to_string();
    let keywords = ["if", "while", "for", "match", "switch"];
    for kw in keywords {
        let pattern = format!("{}(", kw);
        let replacement = format!("{} (", kw);
        res = res.replace(&pattern, &replacement);
    }
    res
}

fn count_leading_closing_braces(line: &str) -> usize {
    let mut count = 0;
    for ch in line.chars() {
        if ch == '}' {
            count += 1;
        } else if ch == ' ' || ch == '\t' {
            continue;
        } else {
            break;
        }
    }
    count
}

fn count_braces_in_line(line: &str) -> (usize, usize) {
    let mut open_count: usize = 0;
    let mut close_count: usize = 0;
    let mut in_str = false;
    let mut quote_ch = '"';
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if in_str {
            if ch == '\\' && i + 1 < chars.len() {
                i += 2;
                continue;
            }
            if ch == quote_ch {
                in_str = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' || ch == '\'' {
            in_str = true;
            quote_ch = ch;
            i += 1;
            continue;
        }

        if ch == '/' && i + 1 < chars.len() && (chars[i + 1] == '/' || chars[i + 1] == '*') {
            break; // comment starts
        }

        if ch == '{' {
            open_count += 1;
        } else if ch == '}' {
            close_count += 1;
        }
        i += 1;
    }

    (open_count, close_count)
}

/// Formats a single file on disk or checks if it is formatted.
pub fn format_file(path: &Path, opts: &FmtOptions) -> Result<bool> {
    let content = fs::read_to_string(path)?;
    let formatted = format_source(&content, opts.indent_size);

    if content == formatted {
        if opts.verbose {
            println!("  {} {}", "✓".green(), path.display());
        }
        Ok(false) // No change needed
    } else {
        if opts.check {
            println!("  {} {} {}", "✗".red(), path.display(), "(needs formatting)".yellow());
        } else {
            fs::write(path, &formatted)?;
            if opts.verbose {
                println!("  {} {} {}", "⚡".cyan(), path.display(), "(formatted)".bright_green());
            }
        }
        Ok(true) // Was formatted or needs formatting
    }
}

/// Formats all Roze files specified in `paths` (or current directory if empty).
pub fn format_paths(paths: &[PathBuf], opts: &FmtOptions) -> Result<FmtSummary> {
    let target_paths: Vec<PathBuf> = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths.to_vec()
    };

    let mut summary = FmtSummary::default();

    for target in target_paths {
        if target.is_file() {
            if target.extension().map_or(false, |ext| ext == "roze") {
                summary.files_checked += 1;
                let needed = format_file(&target, opts)?;
                if needed {
                    if opts.check {
                        summary.unformatted_files.push(target);
                    } else {
                        summary.files_formatted += 1;
                    }
                }
            }
        } else if target.is_dir() {
            for entry in WalkDir::new(&target)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let p = e.path();
                    // Skip hidden directories, target, vendor, node_modules
                    !p.components().any(|c| {
                        let s = c.as_os_str().to_string_lossy();
                        s.starts_with('.') || s == "target" || s == "vendor" || s == "node_modules"
                    })
                })
            {
                let path = entry.path();
                if path.is_file() && path.extension().map_or(false, |ext| ext == "roze") {
                    summary.files_checked += 1;
                    let needed = format_file(path, opts)?;
                    if needed {
                        if opts.check {
                            summary.unformatted_files.push(path.to_path_buf());
                        } else {
                            summary.files_formatted += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(summary)
}

/// Entry point for `roze fmt` CLI subcommand.
pub fn run_fmt_cli(paths: &[PathBuf], check: bool, indent: usize, verbose: bool) -> Result<()> {
    let opts = FmtOptions {
        indent_size: indent,
        check,
        verbose,
    };

    println!("{}", "🌹 roze fmt - Formatting Roze Source Files...".bold().magenta());

    let summary = format_paths(paths, &opts)?;

    if opts.check {
        if summary.unformatted_files.is_empty() {
            println!(
                "{} All {} Roze source files are properly formatted!",
                "✅".bright_green().bold(),
                summary.files_checked.to_string().bold()
            );
            Ok(())
        } else {
            eprintln!(
                "{} {} of {} files need formatting. Run 'roze fmt' to format them.",
                "❌ Error:".bright_red().bold(),
                summary.unformatted_files.len().to_string().bold().red(),
                summary.files_checked
            );
            Err(anyhow!("Code formatting check failed"))
        }
    } else {
        println!(
            "{} Checked {} files; formatted {} files.",
            "✅".bright_green().bold(),
            summary.files_checked.to_string().bold(),
            summary.files_formatted.to_string().bold().cyan()
        );
        Ok(())
    }
}
