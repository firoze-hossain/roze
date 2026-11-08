// tools/rozedoc/tests/integration_tests.rs

use std::fs;
use tempfile::tempdir;
use rozedoc::*;

#[test]
fn test_full_package_documentation_generation() {
    let tmp = tempdir().unwrap();
    let pkg_dir = tmp.path().join("my_app");
    fs::create_dir_all(pkg_dir.join("src/math")).unwrap();

    // Create bsr.mfz
    let mfz_content = r#"[package]
name = "my_app"
version = "2.1.0"
description = "A comprehensive math and scientific computing package for Roze"
authors = ["Mohammad Firoze <firoze@roze.org>", "Roze Core Team"]
license = "MIT"
repository = "https://github.com/roze-lang/my_app"
"#;
    fs::write(pkg_dir.join("bsr.mfz"), mfz_content).unwrap();

    // Create README.md
    let readme_content = r#"# My App

Welcome to `my_app`! This package provides high-performance math routines.

> [!NOTE]
> Tested with Roze 2026 edition.

## Quickstart

```roze
import math::vector;

func main() {
    let v = Vector::new(1.0, 2.0);
    println(v);
}
```
"#;
    fs::write(pkg_dir.join("README.md"), readme_content).unwrap();

    // Create src/lib.roze
    let lib_roze = r#"
//! Root library module for my_app.
//!
//! Provides top-level exports and version constant.

/// Current version of the my_app library.
let APP_VERSION: string = "2.1.0";

/// Initializes the application runtime.
///
/// @param debug Enable verbose debug logs
/// @return Returns 0 on success
/// @throws InitError If initialization fails
/// @example
/// init_app(true);
func init_app(debug: bool) -> int {
    return 0;
}
"#;
    fs::write(pkg_dir.join("src/lib.roze"), lib_roze).unwrap();

    // Create src/math/vector.roze
    let vector_roze = r#"
//! Vector mathematics and linear algebra.

/// Represents a 2D Euclidean vector.
class Vector {
    x: float,
    y: float,
}

/// Represents the dimension space.
enum Space {
    TwoD,
    ThreeD(int),
}

/// Computes the dot product of two vectors.
///
/// @param a First vector
/// @param b Second vector
/// @return The scalar dot product
func dot_product(a: Vector, b: Vector) -> float {
    return a.x * b.x + a.y * b.y;
}
"#;
    fs::write(pkg_dir.join("src/math/vector.roze"), vector_roze).unwrap();

    // Output directory
    let out_dir = tmp.path().join("doc_output");

    let package = generate_docs(&pkg_dir, &out_dir.display().to_string(), true, true).unwrap();

    assert_eq!(package.name, "my_app");
    assert_eq!(package.version, "2.1.0");
    assert_eq!(package.authors.len(), 2);
    assert_eq!(package.license.as_deref(), Some("MIT"));
    assert!(package.modules.len() >= 2);

    // Verify generated static files
    assert!(out_dir.join("index.html").exists());
    assert!(out_dir.join("rozedoc.css").exists());
    assert!(out_dir.join("search.js").exists());
    assert!(out_dir.join("search-index.js").exists());
    assert!(out_dir.join("playground.js").exists());
    assert!(out_dir.join("doc.json").exists());

    // Verify index.html content
    let index_html = fs::read_to_string(out_dir.join("index.html")).unwrap();
    assert!(index_html.contains("my_app"));
    assert!(index_html.contains("v2.1.0"));
    assert!(index_html.contains("Mohammad Firoze"));
    assert!(index_html.contains("callout-note"));
    assert!(index_html.contains("Tested with Roze 2026 edition."));

    // Verify module HTML pages
    let search_index_js = fs::read_to_string(out_dir.join("search-index.js")).unwrap();
    assert!(search_index_js.contains("init_app"));
    assert!(search_index_js.contains("dot_product"));
    assert!(search_index_js.contains("Vector"));
    assert!(search_index_js.contains("Space"));

    // Verify JSON model deserialization
    let json_content = fs::read_to_string(out_dir.join("doc.json")).unwrap();
    let deserialized: DocPackage = serde_json::from_str(&json_content).unwrap();
    assert_eq!(deserialized.name, "my_app");
    assert_eq!(deserialized.version, "2.1.0");
}

#[test]
fn test_single_file_documentation_generation() {
    let tmp = tempdir().unwrap();
    let file_path = tmp.path().join("standalone.roze");

    let content = r#"
//! Standalone script documentation.

/// Prints a greeting message.
/// @param name The name to greet
func greet(name: string) -> string {
    return "Hello, " + name;
}
"#;
    fs::write(&file_path, content).unwrap();

    let out_dir = tmp.path().join("standalone_doc");
    let package = generate_docs(&file_path, &out_dir.display().to_string(), true, false).unwrap();

    assert_eq!(package.name, "standalone");
    assert_eq!(package.modules.len(), 1);
    assert_eq!(package.modules[0].functions.len(), 1);
    assert_eq!(package.modules[0].functions[0].name, "greet");
    assert!(out_dir.join("index.html").exists());
}
