// tools/rozedoc/src/lib.rs
//
// Automated Documentation Generator for Roze with Live Playground support.
// Parity with rustdoc and javadoc.

pub mod extractor;
pub mod generator;
pub mod indexer;
pub mod markdown;
pub mod model;

pub use extractor::{extract_module, extract_package, extract_single_file};
pub use generator::{DocGenerator, DocGeneratorOptions};
pub use indexer::{build_search_index, export_package_json, generate_search_index_js};
pub use markdown::{markdown_to_html, parse_docstring};
pub use model::*;

use std::path::Path;
use anyhow::Result;

/// Convenience helper to document a package or source file into target output directory.
pub fn generate_docs(
    input_path: &Path,
    output_dir: &str,
    enable_playground: bool,
    export_json: bool,
) -> Result<DocPackage> {
    let package = if input_path.is_file() {
        extract_single_file(input_path)?
    } else {
        extract_package(input_path)?
    };

    let generator = DocGenerator::new(DocGeneratorOptions {
        output_dir: output_dir.to_string(),
        enable_playground,
        export_json,
    });

    generator.generate(&package)?;
    Ok(package)
}
