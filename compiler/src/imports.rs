// compiler/src/imports.rs
//
// A minimal module system: resolves `import "...";` statements by
// tokenizing and parsing another file's source and pulling in its
// top-level functions. Intentionally simple:
//
// - One level deep -- an imported file's own `import` statements aren't
//   followed. (Nothing stops you writing them; they're just ignored,
//   same as any other statement type we don't specifically pull in.)
// - No namespacing/aliasing -- an imported `func greet(...)` becomes
//   just `greet` in your program, same as if you'd written it yourself.
// - A name your file already defines always wins over an imported one
//   of the same name, so you can freely shadow/override anything you
//   import instead of getting a duplicate-definition error.
//
// Known limitation: if an imported function has a *type* error (as
// opposed to a *syntax* error, which is caught and reported correctly
// against the module's own file -- see below), it will be reported using
// the top-level file's line numbers once merged in, which won't point at
// the right place. Fully fixing that means tracking a source file per
// statement all the way through the type checker and codegen, which is
// more than a "minimal" module system needs; every function in the
// bundled `core` module is verified to type-check cleanly, so this only
// bites you if you import your own, buggy, multi-file project -- a
// reasonable rough edge for a first version.
use crate::error::{AlreadyReported, RozeError};
use crate::lexer::tokenize;
use crate::parser::ast::*;
use crate::parser::parse;
use anyhow::Result;
use colored::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Roze's bundled Core module (see stdlib/src/core.roze), embedded
/// directly into the `roze` binary at compile time so `import "core";`
/// always works, regardless of the current working directory or how
/// `roze` itself was installed.
const CORE_SOURCE: &str = include_str!("../../stdlib/src/core.roze");
const CORE_DISPLAY_NAME: &str = "core (built-in)";

/// Walks `program`'s top-level statements, replacing every `Import` with
/// the functions it pulls in. `base_dir` is the directory of the file
/// being compiled, used to resolve relative import paths.
pub fn resolve_imports(program: Program, base_dir: &Path) -> Result<Program> {
    let mut defined_names: HashSet<String> = program.statements.iter()
        .filter_map(|stmt| match stmt {
            Statement::Function { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();

    let mut imported_functions: Vec<Statement> = Vec::new();
    let mut rest: Vec<Statement> = Vec::new();

    for stmt in program.statements {
        match stmt {
            Statement::Import { path, location } => {
                let (source, display_name) = load_module_source(&path, base_dir, &location)?;

                let tokens = tokenize(&source);
                let module_program = match parse(tokens) {
                    Ok(p) => p,
                    Err(e) => {
                        // Report this against the *module's* own file and
                        // source, not the file that imported it -- the
                        // line numbers only make sense there.
                        if let Some(roze_err) = e.downcast_ref::<RozeError>() {
                            eprintln!("{}", roze_err.report(&display_name, &source));
                        } else {
                            eprintln!("{} {}", "❌ Error:".bright_red().bold(), e);
                        }
                        return Err(AlreadyReported.into());
                    }
                };

                for module_stmt in module_program.statements {
                    if let Statement::Function { ref name, .. } = module_stmt {
                        if defined_names.contains(name) {
                            // Either the importing file itself, or an
                            // earlier import, already provided this name
                            // -- that one wins, so skip this one rather
                            // than emitting a duplicate Java method.
                            continue;
                        }
                        defined_names.insert(name.clone());
                    }
                    imported_functions.push(module_stmt);
                }
            }
            other => rest.push(other),
        }
    }

    let mut statements = imported_functions;
    statements.extend(rest);
    Ok(Program { statements })
}

/// Resolves an import path to its source text and a human-readable name
/// for error reporting. `"core"` is special-cased to the bundled
/// standard library; anything else is a filesystem path relative to the
/// importing file's own directory (`.roze` is appended if not already
/// present).
fn load_module_source(path: &str, base_dir: &Path, location: &Location) -> Result<(String, String)> {
    if path == "core" {
        return Ok((CORE_SOURCE.to_string(), CORE_DISPLAY_NAME.to_string()));
    }

    let file_path: PathBuf = if path.ends_with(".roze") {
        base_dir.join(path)
    } else {
        base_dir.join(format!("{}.roze", path))
    };

    match std::fs::read_to_string(&file_path) {
        Ok(source) => Ok((source, file_path.display().to_string())),
        Err(_) => Err(RozeError::module(
            format!("Could not find module '{}' (looked for {})", path, file_path.display()),
            location.line,
            location.column,
        )
        .with_hint("built-in modules: \"core\" -- anything else is a path to a .roze file, relative to the importing file")
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use std::path::Path;

    fn resolve(src: &str) -> Result<Program> {
        let program = parse(tokenize(src)).expect("fixture should parse");
        resolve_imports(program, Path::new("."))
    }

    fn function_names(program: &Program) -> Vec<String> {
        program.statements.iter()
            .filter_map(|s| match s {
                Statement::Function { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn importing_core_pulls_in_its_functions() {
        let program = resolve("import \"core\"; func main() { }").unwrap();
        let names = function_names(&program);
        assert!(names.contains(&"clamp".to_string()), "expected 'clamp' to be imported, got {:?}", names);
        assert!(names.contains(&"sign".to_string()), "expected 'sign' to be imported, got {:?}", names);
        assert!(names.contains(&"main".to_string()));
    }

    #[test]
    fn no_import_means_no_core_functions() {
        let program = resolve("func main() { }").unwrap();
        let names = function_names(&program);
        assert_eq!(names, vec!["main".to_string()]);
    }

    #[test]
    fn own_definition_shadows_imported_one_of_the_same_name() {
        let program = resolve("import \"core\"; func square(x: int) -> int { return 999; } func main() { }").unwrap();
        let square_count = program.statements.iter()
            .filter(|s| matches!(s, Statement::Function { name, .. } if name == "square"))
            .count();
        assert_eq!(square_count, 1, "expected exactly one 'square' after shadowing, not a duplicate");
    }

    #[test]
    fn missing_module_is_a_clear_error() {
        let result = resolve("import \"this_module_does_not_exist\"; func main() { }");
        assert!(result.is_err());
    }

    #[test]
    fn import_statements_are_removed_after_resolution() {
        let program = resolve("import \"core\"; func main() { }").unwrap();
        assert!(
            !program.statements.iter().any(|s| matches!(s, Statement::Import { .. })),
            "no Import statements should remain after resolution"
        );
    }
}
