// tools/rozedoc/src/indexer.rs
//
// Search Index Builder and JSON Exporter for Roze documentation.
// Generates:
// - `search-index.js` (client-side instant search usable even over file://)
// - `search.json` (machine-readable search index)
// - `package.json` (complete AST doc model export)

use anyhow::Result;
use super::model::*;

/// Build a list of indexed search items from the documented package.
pub fn build_search_index(package: &DocPackage) -> Vec<SearchIndexItem> {
    let mut items = Vec::new();

    for module in &package.modules {
        let mod_slug = super::markdown::slugify(&module.name);
        let mod_href = format!("{}.html", mod_slug);

        // Index module itself
        items.push(SearchIndexItem {
            id: format!("mod-{}", module.name),
            name: module.name.clone(),
            kind: "module".to_string(),
            module: module.name.clone(),
            signature: format!("module {}", module.name),
            doc_summary: summarize_doc(module.doc.as_deref()),
            href: mod_href.clone(),
        });

        // Functions
        for f in &module.functions {
            let fn_anchor = format!("fn-{}", f.name);
            items.push(SearchIndexItem {
                id: format!("{}-fn-{}", module.name, f.name),
                name: f.name.clone(),
                kind: if f.is_extern { "extern fn" } else { "function" }.to_string(),
                module: module.name.clone(),
                signature: f.signature.clone(),
                doc_summary: summarize_doc(f.doc.as_deref()),
                href: format!("{}#{}", mod_href, fn_anchor),
            });
        }

        // Classes
        for c in &module.classes {
            let class_anchor = format!("class-{}", c.name);
            items.push(SearchIndexItem {
                id: format!("{}-class-{}", module.name, c.name),
                name: c.name.clone(),
                kind: "class".to_string(),
                module: module.name.clone(),
                signature: format!("class {}", c.name),
                doc_summary: summarize_doc(c.doc.as_deref()),
                href: format!("{}#{}", mod_href, class_anchor),
            });

            // Class fields
            for fld in &c.fields {
                let fld_anchor = format!("field-{}-{}", c.name, fld.name);
                let fld_sig = if let Some(ref t) = fld.type_name {
                    format!("{}.{}: {}", c.name, fld.name, t)
                } else {
                    format!("{}.{}", c.name, fld.name)
                };
                items.push(SearchIndexItem {
                    id: format!("{}-{}-{}", module.name, c.name, fld.name),
                    name: format!("{}.{}", c.name, fld.name),
                    kind: "field".to_string(),
                    module: module.name.clone(),
                    signature: fld_sig,
                    doc_summary: summarize_doc(fld.doc.as_deref()),
                    href: format!("{}#{}", mod_href, fld_anchor),
                });
            }

            // Class methods
            for m in &c.methods {
                let m_anchor = format!("method-{}-{}", c.name, m.name);
                items.push(SearchIndexItem {
                    id: format!("{}-{}-method-{}", module.name, c.name, m.name),
                    name: format!("{}.{}", c.name, m.name),
                    kind: "method".to_string(),
                    module: module.name.clone(),
                    signature: m.signature.clone(),
                    doc_summary: summarize_doc(m.doc.as_deref()),
                    href: format!("{}#{}", mod_href, m_anchor),
                });
            }
        }

        // Enums
        for e in &module.enums {
            let enum_anchor = format!("enum-{}", e.name);
            items.push(SearchIndexItem {
                id: format!("{}-enum-{}", module.name, e.name),
                name: e.name.clone(),
                kind: "enum".to_string(),
                module: module.name.clone(),
                signature: format!("enum {}", e.name),
                doc_summary: summarize_doc(e.doc.as_deref()),
                href: format!("{}#{}", mod_href, enum_anchor),
            });

            // Enum variants
            for v in &e.variants {
                let v_anchor = format!("variant-{}-{}", e.name, v.name);
                let v_sig = if v.payload_types.is_empty() {
                    format!("{}::{}", e.name, v.name)
                } else {
                    format!("{}::{}({})", e.name, v.name, v.payload_types.join(", "))
                };
                items.push(SearchIndexItem {
                    id: format!("{}-{}-{}", module.name, e.name, v.name),
                    name: format!("{}::{}", e.name, v.name),
                    kind: "variant".to_string(),
                    module: module.name.clone(),
                    signature: v_sig,
                    doc_summary: summarize_doc(v.doc.as_deref()),
                    href: format!("{}#{}", mod_href, v_anchor),
                });
            }
        }

        // Traits
        for t in &module.traits {
            let trait_anchor = format!("trait-{}", t.name);
            items.push(SearchIndexItem {
                id: format!("{}-trait-{}", module.name, t.name),
                name: t.name.clone(),
                kind: "trait".to_string(),
                module: module.name.clone(),
                signature: format!("trait {}", t.name),
                doc_summary: summarize_doc(t.doc.as_deref()),
                href: format!("{}#{}", mod_href, trait_anchor),
            });

            // Trait methods
            for tm in &t.methods {
                let tm_anchor = format!("trait-method-{}-{}", t.name, tm.name);
                items.push(SearchIndexItem {
                    id: format!("{}-{}-tm-{}", module.name, t.name, tm.name),
                    name: format!("{}::{}", t.name, tm.name),
                    kind: "trait method".to_string(),
                    module: module.name.clone(),
                    signature: tm.signature.clone(),
                    doc_summary: summarize_doc(tm.doc.as_deref()),
                    href: format!("{}#{}", mod_href, tm_anchor),
                });
            }
        }

        // Constants
        for c in &module.constants {
            let c_anchor = format!("const-{}", c.name);
            items.push(SearchIndexItem {
                id: format!("{}-const-{}", module.name, c.name),
                name: c.name.clone(),
                kind: "constant".to_string(),
                module: module.name.clone(),
                signature: if let Some(ref t) = c.type_name {
                    format!("let {}: {}", c.name, t)
                } else {
                    format!("let {}", c.name)
                },
                doc_summary: summarize_doc(c.doc.as_deref()),
                href: format!("{}#{}", mod_href, c_anchor),
            });
        }
    }

    items
}

/// Generate `search-index.js` containing `window.ROZEDOC_SEARCH_INDEX = [...]`.
pub fn generate_search_index_js(package: &DocPackage) -> Result<String> {
    let items = build_search_index(package);
    let json = serde_json::to_string(&items)?;
    Ok(format!("window.ROZEDOC_SEARCH_INDEX = {};\n", json))
}

/// Export package as full JSON AST structure.
pub fn export_package_json(package: &DocPackage) -> Result<String> {
    Ok(serde_json::to_string_pretty(package)?)
}

fn summarize_doc(doc: Option<&str>) -> String {
    if let Some(text) = doc {
        let first_line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        let trimmed = first_line.trim();
        if trimmed.len() > 120 {
            format!("{}...", &trimmed[..117])
        } else {
            trimmed.to_string()
        }
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_index_generation() {
        let mut pkg = DocPackage::new("test_pkg", "1.0.0");
        let mut m = DocModule::new("core::math", "src/math.roze");
        m.doc = Some("Math module summary".into());
        m.functions.push(DocFunction {
            name: "calculate".into(),
            signature: "func calculate(x: int) -> int".into(),
            type_params: Vec::new(),
            params: Vec::new(),
            return_type: Some("int".into()),
            is_async: false,
            is_extern: false,
            doc: Some("Performs calculation".into()),
            return_doc: None,
            throws_doc: Vec::new(),
            examples: Vec::new(),
            line: 10,
        });
        pkg.modules.push(m);

        let items = build_search_index(&pkg);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "module");
        assert_eq!(items[1].name, "calculate");
        assert_eq!(items[1].kind, "function");

        let js = generate_search_index_js(&pkg).unwrap();
        assert!(js.starts_with("window.ROZEDOC_SEARCH_INDEX = ["));
    }
}
