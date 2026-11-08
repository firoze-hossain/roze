// tools/rozedoc/src/generator.rs
//
// Static Site HTML / CSS / JS Generator for Roze documentation.
// Generates:
// - index.html (package overview, readme, modules list)
// - <module>.html (deep module documentation with signatures, param tables, examples)
// - rozedoc.css (clean modern light/dark themed stylesheet)
// - search.js (instant client-side search engine)
// - search-index.js (precomputed search index)
// - playground.js (live interactive in-browser playground simulator)

use std::fs;
use std::path::Path;
use anyhow::{Context, Result};

use crate::indexer::{export_package_json, generate_search_index_js};
use crate::markdown::{html_escape, markdown_to_html, slugify};
use crate::model::*;

pub struct DocGeneratorOptions {
    pub output_dir: String,
    pub enable_playground: bool,
    pub export_json: bool,
}

impl Default for DocGeneratorOptions {
    fn default() -> Self {
        Self {
            output_dir: "target/doc".to_string(),
            enable_playground: true,
            export_json: false,
        }
    }
}

pub struct DocGenerator {
    options: DocGeneratorOptions,
}

impl DocGenerator {
    pub fn new(options: DocGeneratorOptions) -> Self {
        Self { options }
    }

    /// Generate complete static documentation site into target directory.
    pub fn generate(&self, package: &DocPackage) -> Result<()> {
        let out_dir = Path::new(&self.options.output_dir);
        fs::create_dir_all(out_dir)
            .with_context(|| format!("Failed to create output directory: {}", out_dir.display()))?;

        // 1. Write CSS, Search JS, and Playground JS
        fs::write(out_dir.join("rozedoc.css"), ROZEDOC_CSS)?;
        fs::write(out_dir.join("search.js"), SEARCH_JS)?;
        fs::write(out_dir.join("playground.js"), PLAYGROUND_JS)?;

        // 2. Write search-index.js
        let search_index_js = generate_search_index_js(package)?;
        fs::write(out_dir.join("search-index.js"), search_index_js)?;

        // 3. Write index.html
        let index_html = self.render_index_page(package);
        fs::write(out_dir.join("index.html"), index_html)?;

        // 4. Write module pages
        for module in &package.modules {
            let mod_slug = slugify(&module.name);
            let filename = format!("{}.html", mod_slug);
            let mod_html = self.render_module_page(package, module);
            fs::write(out_dir.join(&filename), mod_html)?;
        }

        // 5. Optional JSON export
        if self.options.export_json {
            let json = export_package_json(package)?;
            fs::write(out_dir.join("doc.json"), json)?;
        }

        Ok(())
    }

    fn render_index_page(&self, package: &DocPackage) -> String {
        let title = format!("{} v{} - Roze Documentation", package.name, package.version);
        let mut content = String::new();

        content.push_str("<div class=\"package-header\">\n");
        content.push_str(&format!("  <div class=\"package-badge\">ROZE CRATE</div>\n"));
        content.push_str(&format!("  <h1 class=\"package-title\">{} <span class=\"package-version\">v{}</span></h1>\n", html_escape(&package.name), html_escape(&package.version)));
        if let Some(ref desc) = package.description {
            content.push_str(&format!("  <p class=\"package-desc\">{}</p>\n", html_escape(desc)));
        }

        content.push_str("  <div class=\"package-metadata\">\n");
        if !package.authors.is_empty() {
            content.push_str(&format!("    <span class=\"meta-item\"><strong>Authors:</strong> {}</span>\n", html_escape(&package.authors.join(", "))));
        }
        if let Some(ref lic) = package.license {
            content.push_str(&format!("    <span class=\"meta-item\"><strong>License:</strong> {}</span>\n", html_escape(lic)));
        }
        if let Some(ref repo) = package.repository {
            content.push_str(&format!("    <span class=\"meta-item\"><strong>Repository:</strong> <a href=\"{}\" target=\"_blank\">{}</a></span>\n", html_escape(repo), html_escape(repo)));
        }
        content.push_str(&format!("    <span class=\"meta-item\"><strong>Total Items:</strong> {}</span>\n", package.total_items()));
        content.push_str("  </div>\n");
        content.push_str("</div>\n");

        // Modules Section
        content.push_str("<section class=\"doc-section\">\n");
        content.push_str("  <h2>Modules</h2>\n");
        content.push_str("  <div class=\"module-grid\">\n");
        for m in &package.modules {
            let mod_slug = slugify(&m.name);
            let mod_href = format!("{}.html", mod_slug);
            let summary = m.doc.as_deref().and_then(|d| d.lines().find(|l| !l.trim().is_empty())).unwrap_or("Module");
            content.push_str("    <a class=\"module-card\" href=\"");
            content.push_str(&mod_href);
            content.push_str("\">\n");
            content.push_str(&format!("      <div class=\"module-card-title\"><code>{}</code></div>\n", html_escape(&m.name)));
            content.push_str(&format!("      <div class=\"module-card-desc\">{}</div>\n", html_escape(summary)));
            content.push_str(&format!("      <div class=\"module-card-stats\">{} fn &bull; {} classes &bull; {} enums &bull; {} traits</div>\n",
                m.functions.len(), m.classes.len(), m.enums.len(), m.traits.len()));
            content.push_str("    </a>\n");
        }
        content.push_str("  </div>\n");
        content.push_str("</section>\n");

        // Readme or Overview Section
        if let Some(ref readme_content) = package.readme {
            content.push_str("<section class=\"doc-section readme-section\">\n");
            content.push_str("  <h2>README</h2>\n");
            content.push_str("<div class=\"markdown-body\">\n");
            content.push_str(&markdown_to_html(readme_content, self.options.enable_playground));
            content.push_str("</div>\n");
            content.push_str("</section>\n");
        }

        self.wrap_html_page(&title, package, None, &content)
    }

    fn render_module_page(&self, package: &DocPackage, module: &DocModule) -> String {
        let title = format!("{}::{} - Roze Documentation", package.name, module.name);
        let mut content = String::new();

        // Breadcrumbs & Title
        content.push_str("<div class=\"module-header\">\n");
        content.push_str(&format!(
            "  <div class=\"breadcrumbs\"><a href=\"index.html\">{}</a> &raquo; <span class=\"current\">{}</span></div>\n",
            html_escape(&package.name),
            html_escape(&module.name)
        ));
        content.push_str(&format!("  <h1 class=\"module-title\">Module <code>{}</code></h1>\n", html_escape(&module.name)));
        content.push_str("</div>\n");

        // Module Docstring (//!)
        if let Some(ref doc) = module.doc {
            content.push_str("<div class=\"module-doc markdown-body\">\n");
            content.push_str(&markdown_to_html(doc, self.options.enable_playground));
            content.push_str("</div>\n");
        }

        // Quick Table of Contents for module
        content.push_str("<div class=\"toc-box\">\n");
        content.push_str("  <div class=\"toc-title\">In this module:</div>\n  <ul class=\"toc-links\">\n");
        if !module.functions.is_empty() {
            content.push_str("    <li><a href=\"#functions\">Functions</a></li>\n");
        }
        if !module.classes.is_empty() {
            content.push_str("    <li><a href=\"#classes\">Classes</a></li>\n");
        }
        if !module.enums.is_empty() {
            content.push_str("    <li><a href=\"#enums\">Enums</a></li>\n");
        }
        if !module.traits.is_empty() {
            content.push_str("    <li><a href=\"#traits\">Traits</a></li>\n");
        }
        if !module.constants.is_empty() {
            content.push_str("    <li><a href=\"#constants\">Constants</a></li>\n");
        }
        content.push_str("  </ul>\n</div>\n");

        // Functions Section
        if !module.functions.is_empty() {
            content.push_str("<section id=\"functions\" class=\"doc-section\">\n");
            content.push_str("  <h2>Functions</h2>\n");
            for f in &module.functions {
                content.push_str(&self.render_function_item(f));
            }
            content.push_str("</section>\n");
        }

        // Classes Section
        if !module.classes.is_empty() {
            content.push_str("<section id=\"classes\" class=\"doc-section\">\n");
            content.push_str("  <h2>Classes</h2>\n");
            for c in &module.classes {
                content.push_str(&self.render_class_item(c));
            }
            content.push_str("</section>\n");
        }

        // Enums Section
        if !module.enums.is_empty() {
            content.push_str("<section id=\"enums\" class=\"doc-section\">\n");
            content.push_str("  <h2>Enums</h2>\n");
            for e in &module.enums {
                content.push_str(&self.render_enum_item(e));
            }
            content.push_str("</section>\n");
        }

        // Traits Section
        if !module.traits.is_empty() {
            content.push_str("<section id=\"traits\" class=\"doc-section\">\n");
            content.push_str("  <h2>Traits</h2>\n");
            for t in &module.traits {
                content.push_str(&self.render_trait_item(t));
            }
            content.push_str("</section>\n");
        }

        // Constants Section
        if !module.constants.is_empty() {
            content.push_str("<section id=\"constants\" class=\"doc-section\">\n");
            content.push_str("  <h2>Constants</h2>\n");
            for c in &module.constants {
                content.push_str(&self.render_constant_item(c));
            }
            content.push_str("</section>\n");
        }

        self.wrap_html_page(&title, package, Some(module), &content)
    }

    fn render_function_item(&self, f: &DocFunction) -> String {
        let mut out = String::new();
        let anchor = format!("fn-{}", f.name);
        out.push_str(&format!("<div class=\"doc-item\" id=\"{}\">\n", anchor));
        out.push_str("  <div class=\"doc-item-header\">\n");
        out.push_str(&format!("    <h3 class=\"doc-item-title\"><a href=\"#{}\" class=\"anchor-link\">#</a> func <code>{}</code></h3>\n", anchor, html_escape(&f.name)));
        out.push_str("    <div class=\"badges\">\n");
        if f.is_async {
            out.push_str("      <span class=\"badge badge-async\">async</span>\n");
        }
        if f.is_extern {
            out.push_str("      <span class=\"badge badge-extern\">extern</span>\n");
        }
        out.push_str(&format!("      <span class=\"badge-line\">line {}</span>\n", f.line));
        out.push_str("    </div>\n");
        out.push_str("  </div>\n");

        // Code Signature
        out.push_str(&format!(
            "  <div class=\"sig-block\"><pre><code class=\"language-roze\">{}</code></pre></div>\n",
            crate::markdown::highlight_roze(&f.signature)
        ));

        // Description
        if let Some(ref doc) = f.doc {
            out.push_str("<div class=\"doc-desc markdown-body\">\n");
            out.push_str(&markdown_to_html(doc, self.options.enable_playground));
            out.push_str("</div>\n");
        }

        // Parameters Table
        if !f.params.is_empty() {
            out.push_str("<div class=\"param-section\">\n");
            out.push_str("  <h4>Parameters</h4>\n");
            out.push_str("  <table class=\"param-table\">\n<thead><tr><th>Name</th><th>Type</th><th>Description</th></tr></thead>\n<tbody>\n");
            for p in &f.params {
                out.push_str(&format!(
                    "    <tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
                    html_escape(&p.name),
                    p.type_name.as_deref().unwrap_or("infer"),
                    p.description.as_deref().unwrap_or("-")
                ));
            }
            out.push_str("  </tbody>\n  </table>\n</div>\n");
        }

        // Returns
        if let Some(ref ret) = f.return_doc {
            out.push_str("<div class=\"return-section\">\n");
            out.push_str(&format!("  <h4>Returns</h4>\n  <p>{}</p>\n", html_escape(ret)));
            out.push_str("</div>\n");
        }

        // Throws
        if !f.throws_doc.is_empty() {
            out.push_str("<div class=\"throws-section\">\n");
            out.push_str("  <h4>Throws</h4>\n  <ul>\n");
            for th in &f.throws_doc {
                out.push_str(&format!("    <li><code>{}</code></li>\n", html_escape(th)));
            }
            out.push_str("  </ul>\n</div>\n");
        }

        // Examples
        if !f.examples.is_empty() {
            out.push_str("<div class=\"examples-section\">\n");
            out.push_str("  <h4>Examples</h4>\n");
            for ex in &f.examples {
                out.push_str(&markdown_to_html(&format!("```roze\n{}\n```", ex), self.options.enable_playground));
            }
            out.push_str("</div>\n");
        }

        out.push_str("</div>\n");
        out
    }

    fn render_class_item(&self, c: &DocClass) -> String {
        let mut out = String::new();
        let anchor = format!("class-{}", c.name);
        out.push_str(&format!("<div class=\"doc-item\" id=\"{}\">\n", anchor));
        out.push_str("  <div class=\"doc-item-header\">\n");
        out.push_str(&format!("    <h3 class=\"doc-item-title\"><a href=\"#{}\" class=\"anchor-link\">#</a> class <code>{}</code></h3>\n", anchor, html_escape(&c.name)));
        out.push_str("  </div>\n");

        if let Some(ref doc) = c.doc {
            out.push_str("<div class=\"doc-desc markdown-body\">\n");
            out.push_str(&markdown_to_html(doc, self.options.enable_playground));
            out.push_str("</div>\n");
        }

        // Fields Table
        if !c.fields.is_empty() {
            out.push_str("<div class=\"param-section\">\n");
            out.push_str("  <h4>Fields</h4>\n");
            out.push_str("  <table class=\"param-table\">\n<thead><tr><th>Field</th><th>Type</th><th>Description</th></tr></thead>\n<tbody>\n");
            for fld in &c.fields {
                out.push_str(&format!(
                    "    <tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
                    html_escape(&fld.name),
                    fld.type_name.as_deref().unwrap_or("infer"),
                    fld.doc.as_deref().unwrap_or("-")
                ));
            }
            out.push_str("  </tbody>\n  </table>\n</div>\n");
        }

        // Methods
        if !c.methods.is_empty() {
            out.push_str("<div class=\"methods-section\">\n");
            out.push_str("  <h4>Methods</h4>\n");
            for m in &c.methods {
                out.push_str(&self.render_function_item(m));
            }
            out.push_str("</div>\n");
        }

        out.push_str("</div>\n");
        out
    }

    fn render_enum_item(&self, e: &DocEnum) -> String {
        let mut out = String::new();
        let anchor = format!("enum-{}", e.name);
        out.push_str(&format!("<div class=\"doc-item\" id=\"{}\">\n", anchor));
        out.push_str("  <div class=\"doc-item-header\">\n");
        out.push_str(&format!("    <h3 class=\"doc-item-title\"><a href=\"#{}\" class=\"anchor-link\">#</a> enum <code>{}</code></h3>\n", anchor, html_escape(&e.name)));
        out.push_str("  </div>\n");

        if let Some(ref doc) = e.doc {
            out.push_str("<div class=\"doc-desc markdown-body\">\n");
            out.push_str(&markdown_to_html(doc, self.options.enable_playground));
            out.push_str("</div>\n");
        }

        // Variants Table
        if !e.variants.is_empty() {
            out.push_str("<div class=\"param-section\">\n");
            out.push_str("  <h4>Variants</h4>\n");
            out.push_str("  <table class=\"param-table\">\n<thead><tr><th>Variant</th><th>Payload</th><th>Description</th></tr></thead>\n<tbody>\n");
            for v in &e.variants {
                let payload = if v.payload_types.is_empty() { "-".into() } else { format!("({})", v.payload_types.join(", ")) };
                out.push_str(&format!(
                    "    <tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td></tr>\n",
                    html_escape(&v.name),
                    html_escape(&payload),
                    v.doc.as_deref().unwrap_or("-")
                ));
            }
            out.push_str("  </tbody>\n  </table>\n</div>\n");
        }

        out.push_str("</div>\n");
        out
    }

    fn render_trait_item(&self, t: &DocTrait) -> String {
        let mut out = String::new();
        let anchor = format!("trait-{}", t.name);
        out.push_str(&format!("<div class=\"doc-item\" id=\"{}\">\n", anchor));
        out.push_str("  <div class=\"doc-item-header\">\n");
        out.push_str(&format!("    <h3 class=\"doc-item-title\"><a href=\"#{}\" class=\"anchor-link\">#</a> trait <code>{}</code></h3>\n", anchor, html_escape(&t.name)));
        out.push_str("  </div>\n");

        if let Some(ref doc) = t.doc {
            out.push_str("<div class=\"doc-desc markdown-body\">\n");
            out.push_str(&markdown_to_html(doc, self.options.enable_playground));
            out.push_str("</div>\n");
        }

        if !t.methods.is_empty() {
            out.push_str("<div class=\"methods-section\">\n");
            out.push_str("  <h4>Required Methods</h4>\n");
            for m in &t.methods {
                out.push_str(&self.render_function_item(m));
            }
            out.push_str("</div>\n");
        }

        out.push_str("</div>\n");
        out
    }

    fn render_constant_item(&self, c: &DocConstant) -> String {
        let mut out = String::new();
        let anchor = format!("const-{}", c.name);
        out.push_str(&format!("<div class=\"doc-item\" id=\"{}\">\n", anchor));
        out.push_str("  <div class=\"doc-item-header\">\n");
        out.push_str(&format!("    <h3 class=\"doc-item-title\"><a href=\"#{}\" class=\"anchor-link\">#</a> const <code>{}</code></h3>\n", anchor, html_escape(&c.name)));
        out.push_str("  </div>\n");

        let sig = if let Some(ref t) = c.type_name {
            format!("let {}: {}", c.name, t)
        } else {
            format!("let {}", c.name)
        };
        out.push_str(&format!(
            "  <div class=\"sig-block\"><pre><code class=\"language-roze\">{}</code></pre></div>\n",
            crate::markdown::highlight_roze(&sig)
        ));

        if let Some(ref doc) = c.doc {
            out.push_str("<div class=\"doc-desc markdown-body\">\n");
            out.push_str(&markdown_to_html(doc, self.options.enable_playground));
            out.push_str("</div>\n");
        }

        out.push_str("</div>\n");
        out
    }

    fn wrap_html_page(&self, title: &str, package: &DocPackage, active_module: Option<&DocModule>, main_content: &str) -> String {
        let active_mod_name = active_module.map(|m| m.name.as_str()).unwrap_or("");

        let mut sidebar_modules = String::new();
        for m in &package.modules {
            let is_active = m.name == active_mod_name;
            let active_cls = if is_active { " active" } else { "" };
            let mod_slug = slugify(&m.name);
            sidebar_modules.push_str(&format!(
                "        <li class=\"nav-item{}\"><a href=\"{}.html\">{}</a></li>\n",
                active_cls, mod_slug, html_escape(&m.name)
            ));
        }

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{}</title>
  <link rel="stylesheet" href="rozedoc.css">
</head>
<body>
  <div class="rozedoc-app">
    <!-- Sidebar -->
    <aside class="rozedoc-sidebar">
      <div class="sidebar-header">
        <a href="index.html" class="brand">
          <span class="brand-logo">🌹</span>
          <div class="brand-info">
            <span class="brand-name">{}</span>
            <span class="brand-version">v{}</span>
          </div>
        </a>
      </div>

      <div class="search-box">
        <input type="text" id="rozedoc-search-input" placeholder="Search docs... (Press '/' to focus)" autocomplete="off">
        <div id="rozedoc-search-results" class="search-dropdown hidden"></div>
      </div>

      <nav class="sidebar-nav">
        <div class="nav-section-title">Navigation</div>
        <ul class="nav-list">
          <li class="nav-item{}"><a href="index.html">Overview</a></li>
        </ul>

        <div class="nav-section-title">Modules</div>
        <ul class="nav-list">
{}        </ul>
      </nav>

      <div class="sidebar-footer">
        <button id="theme-toggle" class="theme-btn" title="Toggle Light/Dark Theme">🌓 Theme</button>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="rozedoc-main">
      <div class="main-container">
{}      </div>
      <footer class="rozedoc-footer">
        Generated by <a href="https://github.com/roze-lang/roze" target="_blank">rozedoc</a> &bull; Roze Documentation Engine
      </footer>
    </main>
  </div>

  <!-- Interactive Playground Modal -->
  <div id="playground-modal" class="playground-modal hidden">
    <div class="playground-content">
      <div class="playground-header">
        <div class="playground-title">🌹 Roze Live Playground</div>
        <div class="playground-actions">
          <button id="playground-run-btn" class="pg-btn pg-run" onclick="executePlaygroundCode()">▶ Run</button>
          <button id="playground-close-btn" class="pg-btn pg-close" onclick="closePlaygroundModal()">✕ Close</button>
        </div>
      </div>
      <div class="playground-body">
        <div class="playground-editor-pane">
          <div class="pane-label">Roze Code</div>
          <textarea id="playground-editor" spellcheck="false"></textarea>
        </div>
        <div class="playground-console-pane">
          <div class="pane-label">Terminal Console Output</div>
          <pre id="playground-console"></pre>
        </div>
      </div>
    </div>
  </div>

  <script src="search-index.js"></script>
  <script src="search.js"></script>
  <script src="playground.js"></script>
</body>
</html>
"#,
            html_escape(title),
            html_escape(&package.name),
            html_escape(&package.version),
            if active_module.is_none() { " active" } else { "" },
            sidebar_modules,
            main_content
        )
    }
}

// ---------------------------------------------------------------------------
// Static Assets (Embedded CSS, Search JS, Playground JS)
// ---------------------------------------------------------------------------

pub const ROZEDOC_CSS: &str = r#"/* rozedoc.css - Roze Documentation Theme */
:root {
  --bg-primary: #ffffff;
  --bg-secondary: #f8fafc;
  --bg-sidebar: #0f172a;
  --text-sidebar: #cbd5e1;
  --text-sidebar-active: #ffffff;
  --bg-sidebar-hover: #1e293b;
  --sidebar-active-bg: #e11d48;
  --text-primary: #0f172a;
  --text-secondary: #64748b;
  --border-color: #e2e8f0;
  --code-bg: #f1f5f9;
  --accent: #e11d48;
  --accent-hover: #be123c;
  --callout-note: #0284c7;
  --callout-tip: #16a34a;
  --callout-important: #9333ea;
  --callout-warning: #d97706;
  --callout-caution: #dc2626;
  --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  --font-mono: ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace;
}

[data-theme="dark"] {
  --bg-primary: #0b0f19;
  --bg-secondary: #111827;
  --bg-sidebar: #080c14;
  --text-sidebar: #94a3b8;
  --text-sidebar-active: #ffffff;
  --bg-sidebar-hover: #1f2937;
  --sidebar-active-bg: #e11d48;
  --text-primary: #f8fafc;
  --text-secondary: #94a3b8;
  --border-color: #1f2937;
  --code-bg: #161e2e;
  --accent: #fb7185;
  --accent-hover: #f43f5e;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

body {
  font-family: var(--font-sans);
  background-color: var(--bg-primary);
  color: var(--text-primary);
  line-height: 1.6;
}

.rozedoc-app {
  display: flex;
  min-height: 100vh;
}

/* Sidebar */
.rozedoc-sidebar {
  width: 290px;
  background-color: var(--bg-sidebar);
  color: var(--text-sidebar);
  display: flex;
  flex-direction: column;
  position: fixed;
  top: 0;
  bottom: 0;
  left: 0;
  overflow-y: auto;
  border-right: 1px solid var(--border-color);
  z-index: 100;
}

.sidebar-header {
  padding: 20px;
  border-bottom: 1px solid #1e293b;
}

.brand {
  display: flex;
  align-items: center;
  gap: 12px;
  text-decoration: none;
  color: #fff;
}

.brand-logo {
  font-size: 28px;
}

.brand-name {
  font-size: 18px;
  font-weight: 700;
  display: block;
}

.brand-version {
  font-size: 12px;
  color: #94a3b8;
}

.search-box {
  padding: 16px;
  position: relative;
}

.search-box input {
  width: 100%;
  padding: 10px 14px;
  border-radius: 6px;
  border: 1px solid #334155;
  background-color: #1e293b;
  color: #fff;
  font-size: 13px;
  outline: none;
}

.search-box input:focus {
  border-color: var(--accent);
}

.search-dropdown {
  position: absolute;
  top: 55px;
  left: 16px;
  right: 16px;
  background-color: #1e293b;
  border: 1px solid #334155;
  border-radius: 6px;
  max-height: 380px;
  overflow-y: auto;
  z-index: 200;
  box-shadow: 0 10px 25px rgba(0, 0, 0, 0.4);
}

.search-dropdown.hidden {
  display: none;
}

.search-item {
  display: block;
  padding: 10px 12px;
  text-decoration: none;
  color: #cbd5e1;
  border-bottom: 1px solid #283548;
}

.search-item:hover, .search-item.selected {
  background-color: #334155;
  color: #fff;
}

.search-item-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.search-item-name {
  font-weight: 600;
  color: #38bdf8;
}

.search-item-kind {
  font-size: 10px;
  padding: 2px 6px;
  border-radius: 4px;
  background: #0f172a;
  text-transform: uppercase;
}

.search-item-sig {
  font-family: var(--font-mono);
  font-size: 11px;
  color: #94a3b8;
  margin-top: 3px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sidebar-nav {
  flex: 1;
  padding: 12px 0;
}

.nav-section-title {
  padding: 10px 20px 4px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: #64748b;
  letter-spacing: 0.05em;
}

.nav-list {
  list-style: none;
}

.nav-item a {
  display: block;
  padding: 8px 20px;
  color: var(--text-sidebar);
  text-decoration: none;
  font-size: 13.5px;
}

.nav-item a:hover {
  background-color: var(--bg-sidebar-hover);
  color: #fff;
}

.nav-item.active a {
  background-color: var(--sidebar-active-bg);
  color: #fff;
  font-weight: 600;
}

.sidebar-footer {
  padding: 16px 20px;
  border-top: 1px solid #1e293b;
}

.theme-btn {
  background: transparent;
  border: 1px solid #334155;
  color: var(--text-sidebar);
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12px;
}

.theme-btn:hover {
  border-color: #cbd5e1;
  color: #fff;
}

/* Main Content */
.rozedoc-main {
  margin-left: 290px;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.main-container {
  max-width: 960px;
  margin: 0 auto;
  padding: 40px 32px 80px;
  width: 100%;
}

.package-badge {
  display: inline-block;
  background-color: #ffe4e6;
  color: #e11d48;
  font-size: 11px;
  font-weight: 700;
  padding: 3px 8px;
  border-radius: 4px;
  letter-spacing: 0.08em;
  margin-bottom: 8px;
}

[data-theme="dark"] .package-badge {
  background-color: #3b0716;
  color: #fb7185;
}

.package-title {
  font-size: 36px;
  font-weight: 800;
  margin-bottom: 8px;
}

.package-version {
  font-size: 20px;
  color: var(--text-secondary);
  font-weight: 400;
}

.package-desc {
  font-size: 18px;
  color: var(--text-secondary);
  margin-bottom: 20px;
}

.package-metadata {
  display: flex;
  flex-wrap: wrap;
  gap: 20px;
  padding: 14px 20px;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  margin-bottom: 36px;
  font-size: 13.5px;
}

.package-metadata a {
  color: var(--accent);
  text-decoration: none;
}

.breadcrumbs {
  font-size: 14px;
  color: var(--text-secondary);
  margin-bottom: 8px;
}

.breadcrumbs a {
  color: var(--accent);
  text-decoration: none;
}

.module-title {
  font-size: 32px;
  font-weight: 800;
  margin-bottom: 24px;
}

.module-title code {
  color: var(--accent);
}

.toc-box {
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 16px 20px;
  margin-bottom: 32px;
}

.toc-title {
  font-size: 12px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--text-secondary);
  margin-bottom: 8px;
}

.toc-links {
  display: flex;
  gap: 16px;
  list-style: none;
}

.toc-links a {
  color: var(--accent);
  text-decoration: none;
  font-size: 14px;
  font-weight: 600;
}

.module-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: 20px;
  margin-top: 16px;
  margin-bottom: 40px;
}

.module-card {
  display: block;
  padding: 20px;
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  text-decoration: none;
  color: inherit;
  transition: transform 0.15s ease, border-color 0.15s ease;
}

.module-card:hover {
  transform: translateY(-2px);
  border-color: var(--accent);
}

.module-card-title code {
  font-size: 16px;
  font-weight: 700;
  color: var(--accent);
}

.module-card-desc {
  font-size: 13.5px;
  color: var(--text-secondary);
  margin: 8px 0 12px;
}

.module-card-stats {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.doc-section {
  margin-bottom: 48px;
}

.doc-section h2 {
  font-size: 24px;
  border-bottom: 2px solid var(--border-color);
  padding-bottom: 8px;
  margin-bottom: 24px;
}

.doc-item {
  background-color: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 24px;
  margin-bottom: 24px;
}

.doc-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}

.doc-item-title {
  font-size: 20px;
  font-weight: 700;
}

.anchor-link {
  color: var(--text-secondary);
  text-decoration: none;
  margin-right: 4px;
  opacity: 0.3;
}

.anchor-link:hover {
  opacity: 1;
  color: var(--accent);
}

.badges {
  display: flex;
  gap: 8px;
  align-items: center;
}

.badge {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  padding: 2px 7px;
  border-radius: 4px;
}

.badge-async {
  background: #e0f2fe;
  color: #0284c7;
}

.badge-extern {
  background: #fef3c7;
  color: #d97706;
}

.badge-line {
  font-size: 11.5px;
  color: var(--text-secondary);
}

.sig-block pre {
  background-color: var(--code-bg);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  padding: 12px 16px;
  font-family: var(--font-mono);
  font-size: 14px;
  overflow-x: auto;
  margin-bottom: 16px;
}

.doc-desc {
  margin-bottom: 16px;
}

/* Param & Field Tables */
.param-section, .methods-section, .return-section, .throws-section, .examples-section {
  margin-top: 16px;
}

.param-section h4, .methods-section h4, .return-section h4, .throws-section h4, .examples-section h4 {
  font-size: 13px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-secondary);
  margin-bottom: 8px;
}

.param-table, .doc-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13.5px;
  margin-bottom: 12px;
}

.param-table th, .param-table td, .doc-table th, .doc-table td {
  padding: 8px 12px;
  text-align: left;
  border: 1px solid var(--border-color);
}

.param-table th, .doc-table th {
  background-color: var(--code-bg);
  font-weight: 600;
}

/* Fenced Code Block */
.code-block {
  border: 1px solid var(--border-color);
  border-radius: 6px;
  background-color: var(--code-bg);
  margin: 16px 0;
  overflow: hidden;
}

.code-block-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 12px;
  background-color: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
}

.lang-tag {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--text-secondary);
}

.code-block-actions {
  display: flex;
  gap: 8px;
}

.code-btn {
  background: var(--code-bg);
  border: 1px solid var(--border-color);
  color: var(--text-primary);
  font-size: 11.5px;
  padding: 3px 8px;
  border-radius: 4px;
  cursor: pointer;
}

.code-btn:hover {
  background-color: var(--accent);
  color: #fff;
  border-color: var(--accent);
}

.play-btn {
  font-weight: 600;
}

.code-block pre {
  padding: 12px 16px;
  font-family: var(--font-mono);
  font-size: 13.5px;
  overflow-x: auto;
}

/* Syntax Highlighting */
.token-keyword { color: #d73a49; font-weight: 600; }
.token-type { color: #6f42c1; font-weight: 600; }
.token-string { color: #032f62; }
.token-number { color: #005cc5; }
.token-comment { color: #6a737d; font-style: italic; }
.token-fn { color: #22863a; font-weight: 600; }

[data-theme="dark"] .token-keyword { color: #ff7b72; }
[data-theme="dark"] .token-type { color: #d2a8ff; }
[data-theme="dark"] .token-string { color: #a5d6ff; }
[data-theme="dark"] .token-number { color: #79c0ff; }
[data-theme="dark"] .token-comment { color: #8b949e; }
[data-theme="dark"] .token-fn { color: #7ee787; }

/* GitHub Callouts */
.callout {
  border-left: 4px solid #cbd5e1;
  background-color: var(--bg-secondary);
  padding: 12px 16px;
  border-radius: 0 6px 6px 0;
  margin: 16px 0;
}

.callout-note { border-left-color: var(--callout-note); }
.callout-tip { border-left-color: var(--callout-tip); }
.callout-important { border-left-color: var(--callout-important); }
.callout-warning { border-left-color: var(--callout-warning); }
.callout-caution { border-left-color: var(--callout-caution); }

.callout-header {
  font-size: 13px;
  font-weight: 700;
  margin-bottom: 6px;
  display: flex;
  align-items: center;
  gap: 6px;
}

/* Interactive Playground Modal */
.playground-modal {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.playground-modal.hidden {
  display: none;
}

.playground-content {
  width: 90%;
  max-width: 960px;
  height: 80vh;
  background-color: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: 12px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 25px 50px rgba(0, 0, 0, 0.5);
}

.playground-header {
  padding: 14px 20px;
  background-color: var(--bg-sidebar);
  color: #fff;
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.playground-title {
  font-weight: 700;
  font-size: 16px;
}

.playground-actions {
  display: flex;
  gap: 10px;
}

.pg-btn {
  padding: 6px 14px;
  border-radius: 6px;
  border: none;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
}

.pg-run {
  background-color: #10b981;
  color: #fff;
}

.pg-run:hover {
  background-color: #059669;
}

.pg-close {
  background-color: #ef4444;
  color: #fff;
}

.pg-close:hover {
  background-color: #dc2626;
}

.playground-body {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.playground-editor-pane, .playground-console-pane {
  flex: 1;
  display: flex;
  flex-direction: column;
  border-right: 1px solid var(--border-color);
}

.pane-label {
  padding: 8px 16px;
  background-color: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: var(--text-secondary);
}

#playground-editor {
  flex: 1;
  width: 100%;
  padding: 16px;
  font-family: var(--font-mono);
  font-size: 13.5px;
  background-color: var(--code-bg);
  color: var(--text-primary);
  border: none;
  outline: none;
  resize: none;
}

#playground-console {
  flex: 1;
  padding: 16px;
  background-color: #0f172a;
  color: #38bdf8;
  font-family: var(--font-mono);
  font-size: 13px;
  overflow-y: auto;
  white-space: pre-wrap;
}

.rozedoc-footer {
  text-align: center;
  padding: 24px;
  font-size: 12px;
  color: var(--text-secondary);
  border-top: 1px solid var(--border-color);
}

.rozedoc-footer a {
  color: var(--accent);
  text-decoration: none;
}
"#;

pub const SEARCH_JS: &str = r#"// search.js - Instant client-side search engine for Roze documentation
(function() {
  const searchInput = document.getElementById('rozedoc-search-input');
  const resultsBox = document.getElementById('rozedoc-search-results');
  let selectedIndex = -1;

  if (!searchInput || !resultsBox) return;

  // Shortcut key: press '/' or 's' to focus search
  document.addEventListener('keydown', (e) => {
    if ((e.key === '/' || e.key === 's') && document.activeElement !== searchInput && !['INPUT', 'TEXTAREA'].includes(document.activeElement.tagName)) {
      e.preventDefault();
      searchInput.focus();
      searchInput.select();
    } else if (e.key === 'Escape' && document.activeElement === searchInput) {
      resultsBox.classList.add('hidden');
      searchInput.blur();
    }
  });

  searchInput.addEventListener('input', () => {
    const q = searchInput.value.trim().toLowerCase();
    if (!q) {
      resultsBox.innerHTML = '';
      resultsBox.classList.add('hidden');
      return;
    }

    const index = window.ROZEDOC_SEARCH_INDEX || [];
    const matches = index.filter(item => {
      return item.name.toLowerCase().includes(q) ||
             item.module.toLowerCase().includes(q) ||
             item.kind.toLowerCase().includes(q) ||
             (item.doc_summary && item.doc_summary.toLowerCase().includes(q));
    }).slice(0, 15);

    if (matches.length === 0) {
      resultsBox.innerHTML = '<div style="padding:12px; color:#94a3b8; font-size:12px;">No matching symbols found</div>';
      resultsBox.classList.remove('hidden');
      return;
    }

    selectedIndex = -1;
    let html = '';
    matches.forEach((item, idx) => {
      html += `
        <a class="search-item" data-idx="${idx}" href="${item.href}">
          <div class="search-item-top">
            <span class="search-item-name">${escapeHtml(item.name)}</span>
            <span class="search-item-kind">${escapeHtml(item.kind)}</span>
          </div>
          <div class="search-item-sig">${escapeHtml(item.signature)}</div>
        </a>
      `;
    });

    resultsBox.innerHTML = html;
    resultsBox.classList.remove('hidden');
  });

  // Keyboard navigation through search results
  searchInput.addEventListener('keydown', (e) => {
    const items = resultsBox.querySelectorAll('.search-item');
    if (!items.length) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % items.length;
      updateSelection(items);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + items.length) % items.length;
      updateSelection(items);
    } else if (e.key === 'Enter') {
      if (selectedIndex >= 0 && items[selectedIndex]) {
        e.preventDefault();
        items[selectedIndex].click();
      }
    }
  });

  function updateSelection(items) {
    items.forEach((it, idx) => {
      if (idx === selectedIndex) {
        it.classList.add('selected');
        it.scrollIntoView({ block: 'nearest' });
      } else {
        it.classList.remove('selected');
      }
    });
  }

  // Close dropdown on outside click
  document.addEventListener('click', (e) => {
    if (!searchInput.contains(e.target) && !resultsBox.contains(e.target)) {
      resultsBox.classList.add('hidden');
    }
  });

  function escapeHtml(s) {
    return (s || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }
})();
"#;

pub const PLAYGROUND_JS: &str = r#"// playground.js - Interactive Live Playground and Copy Code for Roze
function rozedocCopyCode(btn) {
  const codeBlock = btn.closest('.code-block');
  if (!codeBlock) return;
  const code = codeBlock.querySelector('code').innerText;
  navigator.clipboard.writeText(code).then(() => {
    const originalText = btn.innerText;
    btn.innerText = '✓ Copied!';
    setTimeout(() => { btn.innerText = originalText; }, 2000);
  });
}

function rozedocOpenPlayground(btn) {
  const codeBlock = btn.closest('.code-block');
  if (!codeBlock) return;
  const code = codeBlock.querySelector('code').innerText;
  const modal = document.getElementById('playground-modal');
  const editor = document.getElementById('playground-editor');
  const consoleEl = document.getElementById('playground-console');

  if (modal && editor && consoleEl) {
    editor.value = code;
    consoleEl.textContent = '[Ready] Press "Run" to execute in browser sandbox.\n';
    modal.classList.remove('hidden');
    editor.focus();
  }
}

function closePlaygroundModal() {
  const modal = document.getElementById('playground-modal');
  if (modal) modal.classList.add('hidden');
}

function executePlaygroundCode() {
  const editor = document.getElementById('playground-editor');
  const consoleEl = document.getElementById('playground-console');
  if (!editor || !consoleEl) return;

  const code = editor.value;
  consoleEl.textContent = '[Compiling & Executing Roze code...]\n';

  // Client-side evaluator / simulator for Roze examples
  try {
    const output = simulateRozeExecution(code);
    consoleEl.textContent = output;
  } catch (err) {
    consoleEl.textContent = `Runtime Error: ${err.message}\n`;
  }
}

function simulateRozeExecution(code) {
  let logs = [];
  const lines = code.split('\n');

  let variables = {};

  for (let line of lines) {
    let trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('//')) continue;

    // Simulate println / print calls
    if (trimmed.startsWith('println(') || trimmed.includes('println(')) {
      const match = trimmed.match(/println\((.*)\)/);
      if (match) {
        let arg = match[1].trim();
        if (arg.startsWith('"') && arg.endsWith('"')) {
          logs.push(arg.slice(1, -1));
        } else if (variables[arg] !== undefined) {
          logs.push(String(variables[arg]));
        } else {
          logs.push(arg);
        }
      }
    } else if (trimmed.startsWith('let ') || trimmed.startsWith('mut ')) {
      const match = trimmed.match(/(let|mut)\s+([a-zA-Z0-9_]+)\s*=\s*(.*);?/);
      if (match) {
        const varName = match[2];
        const valExpr = match[3].replace(';', '').trim();
        try {
          variables[varName] = valExpr.startsWith('"') ? valExpr.slice(1, -1) : valExpr;
        } catch (_) {}
      }
    }
  }

  if (logs.length === 0) {
    logs.push("Program executed successfully. (No stdout output produced)");
  }
  return logs.join('\n') + '\n\n[Finished with exit code 0]';
}

// Light / Dark Theme Toggle
(function() {
  const themeBtn = document.getElementById('theme-toggle');
  const savedTheme = localStorage.getItem('rozedoc-theme') || 'light';
  if (savedTheme === 'dark') {
    document.documentElement.setAttribute('data-theme', 'dark');
  }

  if (themeBtn) {
    themeBtn.addEventListener('click', () => {
      const current = document.documentElement.getAttribute('data-theme');
      const next = current === 'dark' ? 'light' : 'dark';
      if (next === 'dark') {
        document.documentElement.setAttribute('data-theme', 'dark');
      } else {
        document.documentElement.removeAttribute('data-theme');
      }
      localStorage.setItem('rozedoc-theme', next);
    });
  }
})();
"#;
