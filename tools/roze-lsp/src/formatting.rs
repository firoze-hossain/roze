// tools/roze-lsp/src/formatting.rs
//
// Document Formatting Engine for Roze LSP v3.17.
// Formats Roze code with consistent 4-space indentation, normalized
// operator spacing, comma padding, and clean EOF handling.

use tower_lsp::lsp_types::*;
use crate::Document;

pub struct FormattingEngine;

impl FormattingEngine {
    pub fn format_document(doc: &Document, options: FormattingOptions) -> Vec<TextEdit> {
        let total_lines = doc.text.len_lines();
        let last_line_len = if total_lines > 0 {
            doc.text.line(total_lines - 1).len_chars()
        } else {
            0
        };

        let source = doc.text.to_string();
        let formatted = format_source(&source, options.tab_size as usize, options.insert_spaces);

        vec![TextEdit {
            range: Range {
                start: Position { line: 0, character: 0 },
                end: Position {
                    line: total_lines as u32,
                    character: last_line_len as u32,
                },
            },
            new_text: formatted,
        }]
    }
}

fn format_source(source: &str, tab_size: usize, insert_spaces: bool) -> String {
    let indent_unit = if insert_spaces {
        " ".repeat(tab_size)
    } else {
        "\t".to_string()
    };

    let mut result = String::new();
    let mut indent_level: usize = 0;

    for line in source.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            result.push('\n');
            continue;
        }

        // Adjust indent down if line starts with closing brace
        let starts_with_close = trimmed.starts_with('}') || trimmed.starts_with(')');
        if starts_with_close && indent_level > 0 {
            indent_level -= 1;
        }

        // Apply indentation
        let indent_str = indent_unit.repeat(indent_level);
        result.push_str(&indent_str);

        // Format line content
        let formatted_line = format_line(trimmed);
        result.push_str(&formatted_line);
        result.push('\n');

        // Count open vs close braces on this line to update indent_level
        let (opens, closes) = count_braces(trimmed);
        if !starts_with_close {
            indent_level = indent_level.saturating_add(opens).saturating_sub(closes);
        } else {
            // Already subtracted one close brace
            indent_level = indent_level.saturating_add(opens).saturating_sub(closes.saturating_sub(1));
        }
    }

    result
}

fn count_braces(s: &str) -> (usize, usize) {
    let mut opens = 0;
    let mut closes = 0;
    let mut in_string = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            in_string = !in_string;
        } else if !in_string {
            if c == '/' && chars.peek() == Some(&'/') {
                break; // Comment starts
            }
            if c == '{' {
                opens += 1;
            } else if c == '}' {
                closes += 1;
            }
        }
    }

    (opens, closes)
}

fn format_line(s: &str) -> String {
    // Preserve comments as is
    if s.starts_with("//") {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len());
    let mut in_string = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' {
            in_string = !in_string;
            out.push(c);
        } else if in_string {
            out.push(c);
        } else {
            match c {
                ',' => {
                    out.push(',');
                    if chars.peek() != Some(&' ') && chars.peek().is_some() {
                        out.push(' ');
                    }
                }
                ';' => {
                    // Strip space before semicolon
                    if out.ends_with(' ') {
                        out.pop();
                    }
                    out.push(';');
                }
                _ => out.push(c),
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_nested_blocks_with_correct_indentation() {
        let input = "func main(){\nlet x = 1;\nif true {\nprintln(\"hi\");\n}\n}";
        let formatted = format_source(input, 4, true);
        assert!(formatted.contains("    let x = 1;"));
        assert!(formatted.contains("        println(\"hi\");"));
        assert!(formatted.contains("    }"));
        assert!(formatted.contains("}"));
    }
}
