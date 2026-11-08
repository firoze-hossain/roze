// tools/rozedoc/src/markdown.rs
//
// Markdown parser and docstring extractor for Roze documentation.
// Supports:
// - doc comments (/// and //!)
// - GitHub-style alerts (> [!NOTE], > [!TIP], etc.)
// - Javadoc / Roze doc tags (@param, @return, @throws, @example, @deprecated, @see)
// - Fenced code blocks with Roze syntax highlighting and Playground runner integration
// - Markdown tables, lists, links, bold, italics, inline code.

#[derive(Debug, Clone, Default)]
pub struct ExtractedDoc {
    pub raw: String,
    pub summary: String,
    pub params: Vec<(String, String)>,
    pub return_doc: Option<String>,
    pub throws: Vec<String>,
    pub examples: Vec<String>,
    pub deprecated: Option<String>,
    pub see_also: Vec<String>,
}

/// Extract doc tags and clean markdown summary from docstring text.
pub fn parse_docstring(input: &str) -> ExtractedDoc {
    let mut extracted = ExtractedDoc {
        raw: input.to_string(),
        ..Default::default()
    };

    let mut summary_lines = Vec::new();
    let mut in_example = false;
    let mut current_example = String::new();

    for line in input.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("@param") {
            if in_example {
                extracted.examples.push(current_example.trim().to_string());
                current_example.clear();
                in_example = false;
            }
            let rest = trimmed["@param".len()..].trim();
            if let Some((name, desc)) = rest.split_once(char::is_whitespace) {
                extracted.params.push((name.trim().to_string(), desc.trim().to_string()));
            } else if !rest.is_empty() {
                extracted.params.push((rest.to_string(), String::new()));
            }
        } else if trimmed.starts_with("@return") || trimmed.starts_with("@returns") {
            if in_example {
                extracted.examples.push(current_example.trim().to_string());
                current_example.clear();
                in_example = false;
            }
            let prefix = if trimmed.starts_with("@returns") { "@returns" } else { "@return" };
            let desc = trimmed[prefix.len()..].trim().to_string();
            extracted.return_doc = Some(desc);
        } else if trimmed.starts_with("@throws") {
            if in_example {
                extracted.examples.push(current_example.trim().to_string());
                current_example.clear();
                in_example = false;
            }
            let desc = trimmed["@throws".len()..].trim().to_string();
            extracted.throws.push(desc);
        } else if trimmed.starts_with("@example") {
            if in_example {
                extracted.examples.push(current_example.trim().to_string());
                current_example.clear();
            }
            in_example = true;
            let after = trimmed["@example".len()..].trim();
            if !after.is_empty() {
                current_example.push_str(after);
                current_example.push('\n');
            }
        } else if trimmed.starts_with("@deprecated") {
            if in_example {
                extracted.examples.push(current_example.trim().to_string());
                current_example.clear();
                in_example = false;
            }
            let reason = trimmed["@deprecated".len()..].trim().to_string();
            extracted.deprecated = Some(if reason.is_empty() { "Deprecated".into() } else { reason });
        } else if trimmed.starts_with("@see") {
            if in_example {
                extracted.examples.push(current_example.trim().to_string());
                current_example.clear();
                in_example = false;
            }
            let link = trimmed["@see".len()..].trim().to_string();
            extracted.see_also.push(link);
        } else if in_example {
            current_example.push_str(line);
            current_example.push('\n');
        } else {
            summary_lines.push(line);
        }
    }

    if in_example && !current_example.trim().is_empty() {
        extracted.examples.push(current_example.trim().to_string());
    }

    extracted.summary = summary_lines.join("\n").trim().to_string();
    extracted
}

/// Convert markdown string into safe HTML with syntax highlighting and interactive features.
pub fn markdown_to_html(md: &str, enable_playground: bool) -> String {
    let mut html = String::new();
    let lines: Vec<&str> = md.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Empty line
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Fenced code block: ```[lang]
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim();
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                if lines[i].trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            let raw_code = code_lines.join("\n");
            html.push_str(&render_code_block(&raw_code, lang, enable_playground));
            continue;
        }

        // GitHub alert / Callout: > [!NOTE], > [!TIP], > [!IMPORTANT], > [!WARNING], > [!CAUTION]
        if trimmed.starts_with("> [!") && trimmed.contains(']') {
            let alert_type_end = trimmed.find(']').unwrap();
            let alert_tag = &trimmed[4..alert_type_end].to_uppercase();
            let mut callout_lines = Vec::new();
            let first_line_content = trimmed[alert_type_end + 1..].trim();
            if !first_line_content.is_empty() {
                callout_lines.push(first_line_content);
            }
            i += 1;
            while i < lines.len() {
                let cl = lines[i].trim();
                if cl.starts_with('>') {
                    let content = cl.trim_start_matches('>').trim();
                    callout_lines.push(content);
                    i += 1;
                } else if cl.is_empty() {
                    break;
                } else {
                    break;
                }
            }
            html.push_str(&render_callout(alert_tag, &callout_lines.join("\n")));
            continue;
        }

        // Standard blockquote: >
        if trimmed.starts_with('>') {
            let mut quote_lines = Vec::new();
            while i < lines.len() {
                let ql = lines[i].trim();
                if ql.starts_with('>') {
                    quote_lines.push(ql.trim_start_matches('>').trim());
                    i += 1;
                } else {
                    break;
                }
            }
            html.push_str("<blockquote>");
            let inner_md = quote_lines.join("\n");
            html.push_str(&markdown_to_html(&inner_md, enable_playground));
            html.push_str("</blockquote>\n");
            continue;
        }

        // Headings: #, ##, ###, ####, #####, ######
        if trimmed.starts_with('#') {
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            if level <= 6 && trimmed.chars().nth(level) == Some(' ') {
                let content = trimmed[level..].trim();
                let heading_id = slugify(content);
                html.push_str(&format!(
                    "<h{} id=\"{}\"><a class=\"header-anchor\" href=\"#{}\">#</a> {}</h{}>\n",
                    level, heading_id, heading_id, render_inline(content), level
                ));
                i += 1;
                continue;
            }
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            html.push_str("<hr/>\n");
            i += 1;
            continue;
        }

        // Markdown Table: lines starting with '|'
        if trimmed.starts_with('|') && trimmed.ends_with('|') && i + 1 < lines.len() && is_table_delimiter(lines[i + 1]) {
            let header_row = parse_table_row(trimmed);
            i += 2; // skip header and delimiter row
            let mut rows = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') && lines[i].trim().ends_with('|') {
                rows.push(parse_table_row(lines[i].trim()));
                i += 1;
            }
            html.push_str(&render_table(&header_row, &rows));
            continue;
        }

        // Unordered list: - or *
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            html.push_str("<ul>\n");
            while i < lines.len() {
                let item_line = lines[i].trim();
                if item_line.starts_with("- ") || item_line.starts_with("* ") {
                    let item_content = &item_line[2..];
                    html.push_str(&format!("  <li>{}</li>\n", render_inline(item_content)));
                    i += 1;
                } else if item_line.is_empty() {
                    break;
                } else {
                    break;
                }
            }
            html.push_str("</ul>\n");
            continue;
        }

        // Ordered list: 1. , 2. , etc.
        if is_ordered_list_item(trimmed) {
            html.push_str("<ol>\n");
            while i < lines.len() {
                let item_line = lines[i].trim();
                if let Some(content) = strip_ordered_list_prefix(item_line) {
                    html.push_str(&format!("  <li>{}</li>\n", render_inline(content)));
                    i += 1;
                } else {
                    break;
                }
            }
            html.push_str("</ol>\n");
            continue;
        }

        // Normal paragraph: gather contiguous lines until empty or special block
        let mut para_lines = Vec::new();
        while i < lines.len() {
            let l = lines[i].trim();
            if l.is_empty()
                || l.starts_with('#')
                || l.starts_with("```")
                || l.starts_with(">")
                || l.starts_with("- ")
                || l.starts_with("* ")
                || is_ordered_list_item(l)
                || (l.starts_with('|') && l.ends_with('|'))
                || l == "---"
            {
                break;
            }
            para_lines.push(l);
            i += 1;
        }
        if !para_lines.is_empty() {
            html.push_str(&format!("<p>{}</p>\n", render_inline(&para_lines.join(" "))));
        }
    }

    html
}

fn is_table_delimiter(line: &str) -> bool {
    let t = line.trim();
    if !t.starts_with('|') || !t.ends_with('|') {
        return false;
    }
    t.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ')
}

fn parse_table_row(line: &str) -> Vec<String> {
    let stripped = line.trim_matches('|');
    stripped.split('|').map(|cell| cell.trim().to_string()).collect()
}

fn render_table(headers: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::from("<div class=\"table-wrapper\"><table class=\"doc-table\">\n<thead><tr>");
    for h in headers {
        out.push_str(&format!("<th>{}</th>", render_inline(h)));
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for row in rows {
        out.push_str("<tr>");
        for cell in row {
            out.push_str(&format!("<td>{}</td>", render_inline(cell)));
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table></div>\n");
    out
}

fn is_ordered_list_item(line: &str) -> bool {
    strip_ordered_list_prefix(line).is_some()
}

fn strip_ordered_list_prefix(line: &str) -> Option<&str> {
    let mut chars = line.chars();
    let mut num_len = 0;
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            num_len += 1;
        } else if c == '.' && num_len > 0 {
            if chars.next() == Some(' ') {
                return Some(&line[num_len + 2..]);
            }
            return None;
        } else {
            return None;
        }
    }
    None
}

/// Slugify title for heading anchor ID.
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
        }
    }
    slug.trim_end_matches('-').to_string()
}

/// Render inline markdown: `code`, **bold**, *italic*, [link](url).
pub fn render_inline(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Escaped characters: \&, \<, \>, etc.
        if chars[i] == '&' {
            out.push_str("&amp;");
            i += 1;
            continue;
        } else if chars[i] == '<' {
            out.push_str("&lt;");
            i += 1;
            continue;
        } else if chars[i] == '>' {
            out.push_str("&gt;");
            i += 1;
            continue;
        }

        // Inline code `...`
        if chars[i] == '`' {
            let mut j = i + 1;
            while j < len && chars[j] != '`' {
                j += 1;
            }
            if j < len {
                let code_content: String = chars[i + 1..j].iter().collect();
                out.push_str("<code>");
                out.push_str(&html_escape(&code_content));
                out.push_str("</code>");
                i = j + 1;
                continue;
            }
        }

        // Bold **...**
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            let mut j = i + 2;
            while j + 1 < len && !(chars[j] == '*' && chars[j + 1] == '*') {
                j += 1;
            }
            if j + 1 < len {
                let bold_content: String = chars[i + 2..j].iter().collect();
                out.push_str("<strong>");
                out.push_str(&render_inline(&bold_content));
                out.push_str("</strong>");
                i = j + 2;
                continue;
            }
        }

        // Italic *...*
        if chars[i] == '*' {
            let mut j = i + 1;
            while j < len && chars[j] != '*' {
                j += 1;
            }
            if j < len {
                let italic_content: String = chars[i + 1..j].iter().collect();
                out.push_str("<em>");
                out.push_str(&render_inline(&italic_content));
                out.push_str("</em>");
                i = j + 1;
                continue;
            }
        }

        // Link [text](url)
        if chars[i] == '[' {
            let mut j = i + 1;
            while j < len && chars[j] != ']' {
                j += 1;
            }
            if j < len && j + 1 < len && chars[j + 1] == '(' {
                let mut k = j + 2;
                while k < len && chars[k] != ')' {
                    k += 1;
                }
                if k < len {
                    let link_text: String = chars[i + 1..j].iter().collect();
                    let link_url: String = chars[j + 2..k].iter().collect();
                    out.push_str(&format!(
                        "<a href=\"{}\">{}</a>",
                        html_escape(&link_url),
                        render_inline(&link_text)
                    ));
                    i = k + 1;
                    continue;
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Render GitHub-style alert / callout box.
fn render_callout(tag: &str, content: &str) -> String {
    let (class_name, title, icon) = match tag {
        "NOTE" => ("callout-note", "Note", "ℹ️"),
        "TIP" => ("callout-tip", "Tip", "💡"),
        "IMPORTANT" => ("callout-important", "Important", "❗"),
        "WARNING" => ("callout-warning", "Warning", "⚠️"),
        "CAUTION" => ("callout-caution", "Caution", "🛑"),
        _ => ("callout-note", tag, "📌"),
    };

    format!(
        "<div class=\"callout {}\">\n  <div class=\"callout-header\"><span class=\"callout-icon\">{}</span> <strong>{}</strong></div>\n  <div class=\"callout-content\">{}</div>\n</div>\n",
        class_name, icon, title, markdown_to_html(content, false)
    )
}

/// Render code block with syntax highlighting, copy button, and optional Playground button.
fn render_code_block(code: &str, lang: &str, enable_playground: bool) -> String {
    let effective_lang = if lang.is_empty() { "roze".to_string() } else { lang.to_lowercase() };
    let is_roze = effective_lang == "roze" || effective_lang == "rz";

    let highlighted = if is_roze {
        highlight_roze(code)
    } else {
        html_escape(code)
    };

    let mut actions = String::new();
    actions.push_str("<button class=\"code-btn copy-btn\" onclick=\"rozedocCopyCode(this)\" title=\"Copy code\">📋 Copy</button>");

    if is_roze && enable_playground {
        actions.push_str("<button class=\"code-btn play-btn\" onclick=\"rozedocOpenPlayground(this)\" title=\"Run in Live Playground\">▶ Run in Playground</button>");
    }

    format!(
        "<div class=\"code-block\" data-lang=\"{}\">\n  <div class=\"code-block-header\"><span class=\"lang-tag\">{}</span><div class=\"code-block-actions\">{}</div></div>\n  <pre><code class=\"language-{}\">{}</code></pre>\n</div>\n",
        effective_lang, effective_lang, actions, effective_lang, highlighted
    )
}

/// Basic syntax highlighter for Roze code.
pub fn highlight_roze(code: &str) -> String {
    let keywords = [
        "func", "let", "mut", "if", "else", "while", "for", "in", "return",
        "class", "enum", "trait", "impl", "async", "await", "extern",
        "unsafe", "comptime", "asm", "match", "import", "as", "pub",
        "true", "false", "null", "self", "Super", "break", "continue",
    ];

    let types = [
        "int", "float", "string", "bool", "void", "char", "byte",
        "Option", "Result", "List", "Map", "Set", "Vec", "Array", "None", "Some", "Ok", "Err",
    ];

    let mut out = String::new();
    let lines: Vec<&str> = code.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }

        let mut i = 0;
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();

        while i < len {
            // Comments: // ...
            if i + 1 < len && chars[i] == '/' && chars[i + 1] == '/' {
                let comment: String = chars[i..].iter().collect();
                out.push_str("<span class=\"token-comment\">");
                out.push_str(&html_escape(&comment));
                out.push_str("</span>");
                break;
            }

            // String literals: "..."
            if chars[i] == '"' {
                let mut j = i + 1;
                while j < len {
                    if chars[j] == '\\' && j + 1 < len {
                        j += 2;
                    } else if chars[j] == '"' {
                        j += 1;
                        break;
                    } else {
                        j += 1;
                    }
                }
                let str_lit: String = chars[i..j].iter().collect();
                out.push_str("<span class=\"token-string\">");
                out.push_str(&html_escape(&str_lit));
                out.push_str("</span>");
                i = j;
                continue;
            }

            // Numbers: digits
            if chars[i].is_ascii_digit() {
                let mut j = i;
                while j < len && (chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_') {
                    j += 1;
                }
                let num_lit: String = chars[i..j].iter().collect();
                out.push_str("<span class=\"token-number\">");
                out.push_str(&html_escape(&num_lit));
                out.push_str("</span>");
                i = j;
                continue;
            }

            // Identifiers / keywords
            if chars[i].is_alphabetic() || chars[i] == '_' {
                let mut j = i;
                while j < len && (chars[j].is_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                let ident: String = chars[i..j].iter().collect();

                // Check if followed by '(' -> function call
                let is_fn_call = {
                    let mut k = j;
                    while k < len && chars[k].is_whitespace() {
                        k += 1;
                    }
                    k < len && chars[k] == '('
                };

                if keywords.contains(&ident.as_str()) {
                    out.push_str("<span class=\"token-keyword\">");
                    out.push_str(&ident);
                    out.push_str("</span>");
                } else if types.contains(&ident.as_str()) {
                    out.push_str("<span class=\"token-type\">");
                    out.push_str(&ident);
                    out.push_str("</span>");
                } else if is_fn_call {
                    out.push_str("<span class=\"token-fn\">");
                    out.push_str(&ident);
                    out.push_str("</span>");
                } else {
                    out.push_str(&ident);
                }
                i = j;
                continue;
            }

            // Escape HTML chars
            if chars[i] == '&' {
                out.push_str("&amp;");
            } else if chars[i] == '<' {
                out.push_str("&lt;");
            } else if chars[i] == '>' {
                out.push_str("&gt;");
            } else {
                out.push(chars[i]);
            }
            i += 1;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docstring_tag_extraction() {
        let input = "Calculates the sum of two integers.\n\n@param a First integer\n@param b Second integer\n@return The sum of a and b\n@throws OverflowError If sum exceeds integer range\n@example\nlet s = add(10, 20);\nprintln(s);\n@deprecated Use add_checked instead";
        let doc = parse_docstring(input);

        assert_eq!(doc.summary, "Calculates the sum of two integers.");
        assert_eq!(doc.params.len(), 2);
        assert_eq!(doc.params[0], ("a".into(), "First integer".into()));
        assert_eq!(doc.params[1], ("b".into(), "Second integer".into()));
        assert_eq!(doc.return_doc.as_deref(), Some("The sum of a and b"));
        assert_eq!(doc.throws.len(), 1);
        assert_eq!(doc.throws[0], "OverflowError If sum exceeds integer range");
        assert_eq!(doc.examples.len(), 1);
        assert!(doc.examples[0].contains("add(10, 20)"));
        assert_eq!(doc.deprecated.as_deref(), Some("Use add_checked instead"));
    }

    #[test]
    fn test_markdown_alerts() {
        let md = "> [!NOTE]\n> This is an important note.\n";
        let html = markdown_to_html(md, true);
        assert!(html.contains("callout-note"));
        assert!(html.contains("Note"));
        assert!(html.contains("This is an important note."));
    }

    #[test]
    fn test_code_block_with_playground() {
        let md = "```roze\nfunc main() {\n    println(\"hello\");\n}\n```";
        let html = markdown_to_html(md, true);
        assert!(html.contains("token-keyword"));
        assert!(html.contains("Run in Playground"));
        assert!(html.contains("copy-btn"));
    }

    #[test]
    fn test_markdown_table_and_inline() {
        let md = "| Option | Description |\n|---|---|\n| `-o` | Output file |\n\nHere is `inline code` and **bold** text.";
        let html = markdown_to_html(md, false);
        assert!(html.contains("<table class=\"doc-table\">"));
        assert!(html.contains("<code>inline code</code>"));
        assert!(html.contains("<strong>bold</strong>"));
    }
}
