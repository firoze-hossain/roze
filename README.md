# 🌹 The Roze Programming Language (v1.0.0 Stable)

[![CI](https://github.com/firoze/roze/actions/workflows/ci.yml/badge.svg)](https://github.com/firoze/roze/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Release](https://img.shields.io/badge/release-v1.0.0-rose.svg)](https://github.com/firoze/roze/releases/tag/v1.0.0)
[![Documentation](https://img.shields.io/badge/docs-rozehub-blue.svg)](https://rozehub.roze-lang.org)

**Roze** is an expressive, fast, statically typed programming language designed for both high-performance systems and enterprise cloud applications. It features a dual-backend architecture (**Native Cranelift/C** & **JVM**), zero-pause atomic reference counting (ARC), algebraic data types, traits, first-class concurrency, and full self-hosting independence.

---

## ⚡ Quickstart

### Install via One-Line Script
```bash
# Linux & macOS:
curl -fsSL https://roze-lang.org/install.sh | bash

# Windows (PowerShell):
iwr -useb https://roze-lang.org/install.ps1 | iex
```

### Hello, World!
```roze
enum Status {
    Success(code: int),
    Failed(reason: string)
}

func main() {
    let greeting = "Hello, Roze v1.0.0! 🌹";
    println(greeting);

    let status = Status::Success(200);
    match status {
        Status::Success(code) => println("Status: " + code),
        Status::Failed(err) => println("Failed: " + err)
    }
}
```

### Compile & Run
```bash
# Compile to a standalone native ELF/Mach-O/PE binary via Cranelift:
roze build main.roze --target native

# Or run via JVM bytecode:
roze run main.roze
```

---

## 🌟 Key Language Features

- **Dual-Backend Versatility**:
  - `--target native`: Pure Cranelift backend producing standalone executables with zero JVM or runtime dependencies.
  - `--target jvm`: Direct translation to optimized JVM bytecode for enterprise interoperability.
  - `--target freestanding`: Bare-metal kernel mode with zero libc/OS dependencies, multiboot2 header, and VGA text buffer drivers.
- **Deterministic ARC Memory**: Thread-safe atomic reference counting (`atomic_rmw`) with zero garbage collection pauses.
- **Expressive Safety**:
  - Sum types (`enum`) with exhaustive pattern matching (`match`).
  - `Result<T, E>` and `Option<T>` with the `?` try operator for error propagation without exceptions.
  - Parametric polymorphism (Generics `<T>`) across functions, classes, and enums.
  - Polymorphic dispatch with Traits (`trait` and `impl Trait for Type`).
  - First-class closures and lambda expressions (`(x) => x * 2`).
- **Systems & Concurrency**:
  - Direct C-ABI FFI bindings (`extern "C"`).
  - Native OS threads (`thread_spawn`) and thread-safe MPMC channels (`channel_new`, `channel_send`, `channel_recv`).
  - Unsafe blocks (`unsafe { alloc(64); ... }`) for low-level memory operations.
- **Pure Self-Hosting**: The compiler is written in Roze and verified through 3-stage bootstrap fixpoint closure (`diff roze_stage2.c roze_stage3.c` is zero).

---

## 📦 Toolchain & Ecosystem

- **`roze`**: Compiler driver supporting native, JVM, and freestanding targets.
- **`roze-pkg`**: Package manager and dependency resolver supporting semver, lockfiles, and registry publishing.
- **`roze-lsp`**: Language Server Protocol engine with diagnostics, syntax checking, and code hover.
- **`roze-build`**: Incremental workspace build tool.
- **VS Code Extension**: Syntax highlighting, snippet library, and LSP integration ([Marketplace VSIX](ide/vscode/roze-language-1.0.0.vsix)).

---

## 📚 Documentation & Specifications

- [Language Reference Specification](docs/SPECIFICATION.md)
- [Formal EBNF Grammar](docs/GRAMMAR.md)
- [Installation Guide](docs/INSTALLATION.md)
- [Standard Library Reference](docs/STDLIB.md)
- [Production Cookbook](docs/COOKBOOK.md)
- [Self-Hosted Compiler Architecture](demos/self_hosted/README.md)
- [Changelog](CHANGELOG.md)

---

## 📜 License

Roze is open-source software licensed under the [MIT License](LICENSE).
