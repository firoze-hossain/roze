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
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Roze's bundled Core module (see stdlib/src/core.roze), embedded
/// directly into the `roze` binary at compile time so `import "core";`
/// always works, regardless of the current working directory or how
/// `roze` itself was installed.
const CORE_SOURCE: &str = include_str!("../../stdlib/src/core.roze");
const CORE_DISPLAY_NAME: &str = "core (built-in)";

const CONCURRENCY_SOURCE: &str = include_str!("../../stdlib/src/concurrency.roze");
const CONCURRENCY_DISPLAY_NAME: &str = "concurrency (built-in)";

const SYNC_SOURCE: &str = include_str!("../../stdlib/src/sync.roze");
const SYNC_DISPLAY_NAME: &str = "sync (built-in)";

const WEB_SOURCE: &str = include_str!("../../stdlib/src/web.roze");
const WEB_DISPLAY_NAME: &str = "web (built-in)";

const VGA_SOURCE: &str = include_str!("../../stdlib/src/vga.roze");
const VGA_DISPLAY_NAME: &str = "vga (built-in)";

const RAYLIB_SOURCE: &str = include_str!("../../stdlib/src/raylib.roze");
const RAYLIB_DISPLAY_NAME: &str = "raylib (built-in)";

const AI_SOURCE: &str = include_str!("../../stdlib/src/ai.roze");
const AI_DISPLAY_NAME: &str = "ai (built-in)";

const COLLECTIONS_SOURCE: &str = include_str!("../../stdlib/src/collections.roze");
const COLLECTIONS_DISPLAY_NAME: &str = "collections (built-in)";

const MEMORY_SOURCE: &str = include_str!("../../stdlib/src/memory.roze");
const MEMORY_DISPLAY_NAME: &str = "memory (built-in)";

const FREESTANDING_SOURCE: &str = include_str!("../../stdlib/src/freestanding.roze");
const FREESTANDING_DISPLAY_NAME: &str = "freestanding (built-in)";

const PAGING_SOURCE: &str = include_str!("../../stdlib/src/paging.roze");
const PAGING_DISPLAY_NAME: &str = "paging (built-in)";

const INTERRUPTS_SOURCE: &str = include_str!("../../stdlib/src/interrupts.roze");
const INTERRUPTS_DISPLAY_NAME: &str = "interrupts (built-in)";

const DRIVERS_SOURCE: &str = include_str!("../../stdlib/src/drivers.roze");
const DRIVERS_DISPLAY_NAME: &str = "drivers (built-in)";

const BINARY_SOURCE: &str = include_str!("../../stdlib/src/binary.roze");
const BINARY_DISPLAY_NAME: &str = "binary (built-in)";

const FORENSICS_SOURCE: &str = include_str!("../../stdlib/src/forensics.roze");
const FORENSICS_DISPLAY_NAME: &str = "forensics (built-in)";

const FFI_SOURCE: &str = include_str!("../../stdlib/src/ffi.roze");
const FFI_DISPLAY_NAME: &str = "ffi (built-in)";

const PTR_SOURCE: &str = include_str!("../../stdlib/src/ptr.roze");
const PTR_DISPLAY_NAME: &str = "ptr (built-in)";

const FS_SOURCE: &str = include_str!("../../stdlib/src/fs.roze");
const FS_DISPLAY_NAME: &str = "fs (built-in)";

const IO_SOURCE: &str = include_str!("../../stdlib/src/io.roze");
const IO_DISPLAY_NAME: &str = "io (built-in)";

const PROCESS_SOURCE: &str = include_str!("../../stdlib/src/process.roze");
const PROCESS_DISPLAY_NAME: &str = "process (built-in)";

const ENV_SOURCE: &str = include_str!("../../stdlib/src/env.roze");
const ENV_DISPLAY_NAME: &str = "env (built-in)";

const TIME_SOURCE: &str = include_str!("../../stdlib/src/time.roze");
const TIME_DISPLAY_NAME: &str = "time (built-in)";

const REGEX_SOURCE: &str = include_str!("../../stdlib/src/regex.roze");
const REGEX_DISPLAY_NAME: &str = "regex (built-in)";

const SERDE_SOURCE: &str = include_str!("../../stdlib/src/serde.roze");
const SERDE_DISPLAY_NAME: &str = "serde (built-in)";

const REFLECT_SOURCE: &str = include_str!("../../stdlib/src/reflect.roze");
const REFLECT_DISPLAY_NAME: &str = "reflect (built-in)";

const LIBC_SOURCE: &str = include_str!("../../stdlib/src/libc.roze");
const LIBC_DISPLAY_NAME: &str = "libc (built-in)";

const SQLITE_SOURCE: &str = include_str!("../../stdlib/src/sqlite.roze");
const SQLITE_DISPLAY_NAME: &str = "sqlite (built-in)";

fn statement_defined_name(stmt: &Statement) -> Option<String> {
    match stmt {
        Statement::Function { name, .. } | Statement::ExternFunction { name, .. } | Statement::ClassDecl { name, .. } => Some(name.clone()),
        Statement::Annotated { statement, .. } => statement_defined_name(statement),
        _ => None,
    }
}

/// Walks `program`'s top-level statements, replacing every `Import` with
/// the functions it pulls in. `base_dir` is the directory of the file
/// being compiled, used to resolve relative import paths.
pub fn resolve_imports(program: Program, base_dir: &Path) -> Result<Program> {
    let mut defined_names: HashSet<String> = program.statements.iter()
        .filter_map(statement_defined_name)
        .collect();

    let mut visited_paths: HashSet<String> = HashSet::new();
    let mut imported_functions: Vec<Statement> = Vec::new();
    let mut rest: Vec<Statement> = Vec::new();

    let mut queue: VecDeque<(Statement, PathBuf)> = VecDeque::new();
    for stmt in program.statements {
        match stmt {
            Statement::Import { .. } => queue.push_back((stmt, base_dir.to_path_buf())),
            other => rest.push(other),
        }
    }

    while let Some((stmt, current_base_dir)) = queue.pop_front() {
        if let Statement::Import { path, location } = stmt {
            if visited_paths.contains(&path) {
                continue;
            }
            visited_paths.insert(path.clone());

            let (source, display_name) = load_module_source(&path, &current_base_dir, &location)?;

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

            let next_base_dir = if display_name.contains("(built-in)") {
                current_base_dir.clone()
            } else {
                Path::new(&display_name).parent().unwrap_or(&current_base_dir).to_path_buf()
            };

            for module_stmt in module_program.statements {
                match module_stmt {
                    Statement::Import { .. } => {
                        queue.push_back((module_stmt, next_base_dir.clone()));
                    }
                    stmt if statement_defined_name(&stmt).is_some() => {
                        let name = statement_defined_name(&stmt).unwrap();
                        if defined_names.contains(&name) {
                            // Either the importing file itself, or an
                            // earlier import, already provided this name
                            // -- that one wins, so skip this one rather
                            // than emitting a duplicate Java method/class.
                            continue;
                        }
                        defined_names.insert(name);
                        imported_functions.push(stmt);
                    }
                    other => {
                        imported_functions.push(other);
                    }
                }
            }
        }
    }

    let mut statements = imported_functions;
    statements.extend(rest);
    Ok(Program { statements })
}

/// Attempts to locate and load a modular bsr package (e.g. rozeboot, roze-orm, roze-net, roze-crypto)
/// from the workspace `packages/` directory, project `libs/` directory, cache, or environment.
pub fn resolve_modular_package(path: &str, base_dir: &Path) -> Option<(String, String)> {
    let pkg_subpath = match path {
        "rozeboot" | "rozeboot2" | "roze::boot" | "roze-boot" | "roze_boot" | "ioc" | "annotations" => {
            Some("rozeboot/src/lib.roze".to_string())
        }
        "enterprise_web" | "roze::web" | "http" | "http2" | "http3" | "quic" | "h2" | "h3" => {
            Some("rozeboot/src/enterprise_web.roze".to_string())
        }
        "cloud_security" | "security" | "roze::security" | "oauth2" | "jwt" | "metrics" | "telemetry" | "openapi" | "swagger" => {
            Some("rozeboot/src/cloud_security.roze".to_string())
        }
        "orm" | "data" | "roze::data" | "roze::orm" | "roze-orm" | "roze_orm" | "sql_orm" | "jpa" | "diesel" | "seaorm" => {
            Some("roze-orm/src/lib.roze".to_string())
        }
        "raw_net" | "net" | "packet" | "pcap" | "roze::net::raw" | "packet_crafting" | "protocol" | "dissector" | "dns" | "dhcp"
        | "scanner" | "port_scanner" | "net_scan" | "roze::net::scanner" | "roze-net" | "roze_net" => {
            Some("roze-net/src/lib.roze".to_string())
        }
        "crypto" | "roze::crypto" | "roze-crypto" | "roze_crypto" | "tls" | "sha" | "aes" | "chacha" | "rsa" => {
            Some("roze-crypto/src/lib.roze".to_string())
        }
        _ => None,
    };

    let subpaths = match pkg_subpath {
        Some(sp) => vec![sp],
        None => {
            let sanitized = path.replace("::", "_").replace('-', "_");
            let mut list = Vec::new();
            list.push(format!("{}/src/lib.roze", path));
            if sanitized != path {
                list.push(format!("{}/src/lib.roze", sanitized));
            }
            list
        }
    };

    for subpath in &subpaths {
        let mut candidates = Vec::new();

        // 1. Direct relative check
        candidates.push(base_dir.join("libs").join(subpath));
        candidates.push(base_dir.join("packages").join(subpath));
        candidates.push(base_dir.join(subpath));

        // 2. Ascending parent directories
        let mut curr = Some(base_dir);
        let mut depth = 0;
        while let Some(dir) = curr {
            if depth > 5 {
                break;
            }
            candidates.push(dir.join("libs").join(subpath));
            candidates.push(dir.join("packages").join(subpath));
            curr = dir.parent();
            depth += 1;
        }

        // 3. Workspace repository root via CARGO_MANIFEST_DIR
        if let Some(parent) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
            candidates.push(parent.join("packages").join(subpath));
        }

        // 4. Environment variable ROZE_PACKAGES_DIR / BSR_PACKAGES_DIR
        if let Ok(dir) = std::env::var("ROZE_PACKAGES_DIR").or_else(|_| std::env::var("BSR_PACKAGES_DIR")) {
            candidates.push(PathBuf::from(dir).join(subpath));
        }

        // 5. Relative to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                if let Some(root) = exe_dir.parent() {
                    candidates.push(root.join("packages").join(subpath));
                    candidates.push(root.join("share").join("roze").join("packages").join(subpath));
                }
            }
        }

        // 6. User package cache (~/.bsr/cache/packages/...)
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(PathBuf::from(home).join(".bsr").join("cache").join("packages").join(subpath));
        }

        // 7. Fallback to stdlib directory on disk if present
        if let Some(parent) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
            let stdlib_file = match path {
                "enterprise_web" => Some("enterprise_web.roze"),
                "cloud_security" => Some("cloud_security.roze"),
                "rozeboot" => Some("rozeboot.roze"),
                "orm" => Some("orm.roze"),
                "raw_net" => Some("raw_net.roze"),
                "crypto" => Some("crypto.roze"),
                _ => None,
            };
            if let Some(sf) = stdlib_file {
                candidates.push(parent.join("stdlib").join("src").join(sf));
            }
        }

        for candidate in candidates {
            if candidate.exists() && candidate.is_file() {
                if let Ok(source) = std::fs::read_to_string(&candidate) {
                    return Some((source, candidate.display().to_string()));
                }
            }
        }
    }

    None
}

/// Resolves an import path to its source text and a human-readable name
/// for error reporting. `"core"` is special-cased to the bundled
/// standard library; anything else is a filesystem path relative to the
/// importing file's own directory (`.roze` is appended if not already
/// present).
fn load_module_source(path: &str, base_dir: &Path, location: &Location) -> Result<(String, String)> {
    // 1. First, check modular bsr packages (rozeboot, roze-orm, roze-net, roze-crypto)
    if let Some((source, display_name)) = resolve_modular_package(path, base_dir) {
        return Ok((source, display_name));
    }

    if path == "core" {
        return Ok((CORE_SOURCE.to_string(), CORE_DISPLAY_NAME.to_string()));
    }
    if path == "concurrency" {
        return Ok((CONCURRENCY_SOURCE.to_string(), CONCURRENCY_DISPLAY_NAME.to_string()));
    }
    if path == "sync" {
        return Ok((SYNC_SOURCE.to_string(), SYNC_DISPLAY_NAME.to_string()));
    }
    if path == "web" {
        return Ok((WEB_SOURCE.to_string(), WEB_DISPLAY_NAME.to_string()));
    }
    if path == "vga" {
        return Ok((VGA_SOURCE.to_string(), VGA_DISPLAY_NAME.to_string()));
    }
    if path == "raylib" || path == "std::raylib" || path == "roze::raylib" || path == "game::raylib" {
        return Ok((RAYLIB_SOURCE.to_string(), RAYLIB_DISPLAY_NAME.to_string()));
    }
    if path == "ai" {
        return Ok((AI_SOURCE.to_string(), AI_DISPLAY_NAME.to_string()));
    }
    if path == "collections" {
        return Ok((COLLECTIONS_SOURCE.to_string(), COLLECTIONS_DISPLAY_NAME.to_string()));
    }
    if path == "memory" {
        return Ok((MEMORY_SOURCE.to_string(), MEMORY_DISPLAY_NAME.to_string()));
    }
    if path == "freestanding" {
        return Ok((FREESTANDING_SOURCE.to_string(), FREESTANDING_DISPLAY_NAME.to_string()));
    }
    if path == "paging" || path == "vmm" {
        return Ok((PAGING_SOURCE.to_string(), PAGING_DISPLAY_NAME.to_string()));
    }
    if path == "interrupts" || path == "idt" || path == "gdt" || path == "apic" || path == "pic" {
        return Ok((INTERRUPTS_SOURCE.to_string(), INTERRUPTS_DISPLAY_NAME.to_string()));
    }
    if path == "drivers" || path == "uart" || path == "keyboard" || path == "mouse" || path == "ps2" || path == "timer" || path == "pit" || path == "hpet" || path == "gop" || path == "framebuffer" {
        return Ok((DRIVERS_SOURCE.to_string(), DRIVERS_DISPLAY_NAME.to_string()));
    }
    if path == "binary" || path == "roze::binary" || path == "disasm" || path == "roze::disasm" || path == "elf" || path == "pe" || path == "macho" {
        return Ok((BINARY_SOURCE.to_string(), BINARY_DISPLAY_NAME.to_string()));
    }
    if path == "forensics" || path == "roze::forensics" || path == "mem_forensics" || path == "memory_forensics" || path == "rop" {
        return Ok((FORENSICS_SOURCE.to_string(), FORENSICS_DISPLAY_NAME.to_string()));
    }
    if path == "ffi" || path == "roze::ffi" || path == "c_ffi" || path == "cabi" || path == "panama" || path == "dl" {
        return Ok((FFI_SOURCE.to_string(), FFI_DISPLAY_NAME.to_string()));
    }
    if path == "ptr" || path == "roze::ptr" || path == "core::ffi" || path == "ffi::ptr" || path == "std::ptr" || path == "memory_layout" {
        return Ok((PTR_SOURCE.to_string(), PTR_DISPLAY_NAME.to_string()));
    }
    if path == "fs" || path == "std::fs" || path == "roze::fs" || path == "filesystem" || path == "path" || path == "std::path" {
        return Ok((FS_SOURCE.to_string(), FS_DISPLAY_NAME.to_string()));
    }
    if path == "io" || path == "std::io" || path == "roze::io" {
        return Ok((IO_SOURCE.to_string(), IO_DISPLAY_NAME.to_string()));
    }
    if path == "process" || path == "std::process" || path == "roze::process" {
        return Ok((PROCESS_SOURCE.to_string(), PROCESS_DISPLAY_NAME.to_string()));
    }
    if path == "env" || path == "std::env" || path == "roze::env" || path == "os" || path == "std::os" {
        return Ok((ENV_SOURCE.to_string(), ENV_DISPLAY_NAME.to_string()));
    }
    if path == "time" || path == "std::time" || path == "roze::time" || path == "clock" || path == "instant" || path == "duration" {
        return Ok((TIME_SOURCE.to_string(), TIME_DISPLAY_NAME.to_string()));
    }
    if path == "regex" || path == "std::regex" || path == "roze::regex" || path == "text::regex" || path == "re" {
        return Ok((REGEX_SOURCE.to_string(), REGEX_DISPLAY_NAME.to_string()));
    }
    if path == "serde" || path == "std::serde" || path == "roze::serde" || path == "serialization" || path == "json" || path == "toml" || path == "yaml" {
        return Ok((SERDE_SOURCE.to_string(), SERDE_DISPLAY_NAME.to_string()));
    }
    if path == "reflect" || path == "std::reflect" || path == "roze::reflect" || path == "introspection" {
        return Ok((REFLECT_SOURCE.to_string(), REFLECT_DISPLAY_NAME.to_string()));
    }
    if path == "sqlite" || path == "sqlite3" || path == "std::sqlite" || path == "roze::sqlite" || path == "db::sqlite" {
        return Ok((SQLITE_SOURCE.to_string(), SQLITE_DISPLAY_NAME.to_string()));
    }
    if path == "libc" || path == "posix" || path == "std::libc" || path == "std::posix" || path == "roze::libc" {
        return Ok((LIBC_SOURCE.to_string(), LIBC_DISPLAY_NAME.to_string()));
    }

    let primary_path: PathBuf = if path.ends_with(".roze") {
        base_dir.join(path)
    } else {
        base_dir.join(format!("{}.roze", path))
    };

    let candidates = {
        let mut list = Vec::new();
        // 1. Direct path relative to base_dir
        list.push(primary_path.clone());

        // 2. Search in `libs/` and `packages/` relative to base_dir and its parent directories (project root)
        let mut curr = Some(base_dir);
        let mut depth = 0;
        while let Some(dir) = curr {
            if depth > 5 {
                break;
            }
            list.push(dir.join("libs").join(path).join("src").join("lib.roze"));
            list.push(dir.join("libs").join(path).join("lib.roze"));
            list.push(dir.join("libs").join(format!("{}.roze", path)));
            list.push(dir.join("packages").join(path).join("src").join("lib.roze"));
            let sanitized = path.replace("::", "_");
            if sanitized != path {
                list.push(dir.join("libs").join(&sanitized).join("src").join("lib.roze"));
                list.push(dir.join("libs").join(&sanitized).join("lib.roze"));
                list.push(dir.join("libs").join(format!("{}.roze", sanitized)));
                list.push(dir.join("packages").join(&sanitized).join("src").join("lib.roze"));
            }
            let hyphenated = path.replace('_', "-");
            if hyphenated != path {
                list.push(dir.join("packages").join(&hyphenated).join("src").join("lib.roze"));
            }
            curr = dir.parent();
            depth += 1;
        }
        list
    };

    for candidate in &candidates {
        if let Ok(source) = std::fs::read_to_string(candidate) {
            return Ok((source, candidate.display().to_string()));
        }
    }

    Err(RozeError::module(
        format!("Could not find module '{}' (looked for {})", path, primary_path.display()),
        location.line,
        location.column,
    )
    .with_hint("built-in modules: \"core\", \"concurrency\", \"sync\", \"web\", \"fs\", \"io\", \"process\", \"time\", \"serde\", \"reflect\" -- modular packages: \"rozeboot\", \"roze-orm\", \"roze-net\", \"roze-crypto\", \"packages/<pkg>\", or \"libs/<pkg>/lib.roze\"")
    .into())
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

    #[test]
    fn importing_from_libs_directory_pulls_in_functions() {
        let temp_dir = std::env::temp_dir().join(format!("roze_test_import_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);
        let libs_pkg = temp_dir.join("libs").join("my_math");
        std::fs::create_dir_all(&libs_pkg).unwrap();
        std::fs::write(
            libs_pkg.join("lib.roze"),
            "func custom_add(a: int, b: int) -> int { return a + b; }\n",
        ).unwrap();

        let src_dir = temp_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let code = "import \"my_math\"; func main() -> int { return custom_add(10, 20); }";
        let program = parse(tokenize(code)).unwrap();
        let resolved = resolve_imports(program, &src_dir).unwrap();

        let names = function_names(&resolved);
        assert!(names.contains(&"custom_add".to_string()));
        assert!(names.contains(&"main".to_string()));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn importing_vga_pulls_in_vga_functions() {
        let program = resolve("import \"vga\"; func main() { }").unwrap();
        let names = function_names(&program);
        assert!(names.contains(&"vga_color_entry".to_string()));
        assert!(names.contains(&"main".to_string()));
    }
}
