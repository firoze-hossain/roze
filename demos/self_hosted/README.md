# 🌹 Roze Self-Hosted Compiler (Roze-in-Roze)

> **Milestone ADV-06 (Step 17 - Final Milestone)**  
> **Self-Hosting, Full Language Independence, and Bootstrap Fixpoint Verification**

---

## 1. Overview & Significance

A programming language achieves true maturity and independence when its own compiler is written in the language itself, completely eliminating the dependency on the host language used to bootstrap it.

The **Roze Self-Hosted Compiler** (`demos/self_hosted/`) is a full-featured compiler for the Roze programming language, written entirely in pure Roze. It implements:
- A complete **lexical scanner** (`src/lexer.roze`)
- A **recursive descent parser** with operator precedence climbing (`src/parser.roze`)
- A **semantic type checker** with lexical scoping and symbol table validation (`src/typechecker.roze`)
- A **C99 code generator** producing standalone, native binary-compatible C code (`src/codegen.roze`)
- A **driver and CLI** (`src/main.roze`) with argument parsing and execution pipelines
- A lightweight **ARC runtime layer** (`src/runtime.c` & `include/roze_self_hosted.h`)

---

## 2. Compiler Architecture

```
                    ┌─────────────────────────┐
                    │    Roze Source (.roze)   │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │       Lexer Engine       │ (src/lexer.roze)
                    │ Token Stream & Locations │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │  Recursive Descent AST  │ (src/parser.roze)
                    │  Program & Node Trees   │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │   Semantic Typechecker   │ (src/typechecker.roze)
                    │ Scopes & Symbol Tables   │
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │    C99 Code Generator    │ (src/codegen.roze)
                    │  Standalone C Translation│
                    └───────────┬─────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │ Native Binary Executable │ (ELF x86_64)
                    │   Zero Rust Dependency   │
                    └─────────────────────────┘
```

### Module Breakdown
- **`src/ast.roze`**: Defines AST node kinds (`NODE_FUNC_DECL`, `NODE_CLASS_DECL`, `NODE_BIN_OP`, etc.), token kinds, `Token`, `AstNode`, and `Symbol` classes.
- **`src/lexer.roze`**: Scans characters into tokens, handles string escape sequences (`\n`, `\t`, `\"`, `\\`), numbers, identifiers, operators, and comments.
- **`src/parser.roze`**: Parses top-level declarations (`func`, `extern`, `class`, `import`), statements (`if`, `while`, `for`, `let/var`, `return`), and expressions with binary precedence climbing.
- **`src/typechecker.roze`**: Multi-pass type analyzer. Pre-registers builtins and user types, manages lexical symbol scopes, infers variable types, and validates operands.
- **`src/codegen.roze`**: Emits standalone C99 code with forward declarations, structs, constructors, runtime helpers, function prototypes, implementations, and `main()`.
- **`src/main.roze`**: Compiler CLI entry point, argument parsing (`-o`, `-c`, `--test`, `--help`), file reading, error reporting, and native compilation invocation.
- **`src/runtime.c` & `include/roze_self_hosted.h`**: Standalone C runtime support library providing string manipulation, file I/O, process execution, and diagnostics.

---

## 3. The 3-Stage Bootstrap Verification

The ultimate proof of self-hosting is the **3-stage bootstrap fixpoint**:

$$\text{Stage 0 (Rust)} \xrightarrow{\text{compiles}} \text{Stage 1 (Roze)} \xrightarrow{\text{compiles}} \text{Stage 2 (Roze)} \xrightarrow{\text{compiles}} \text{Stage 3 (Roze)}$$

1. **Stage 0 (Bootstrap Host)**: The Rust-based Roze native compiler compiles `src/main.roze` + `src/runtime.c` to produce `bin/roze_stage1`.
2. **Stage 1 (First Self-Host Generation)**: The `bin/roze_stage1` executable compiles `src/roze_compiler.roze` into `bin/roze_stage2.c` and builds `bin/roze_stage2`.
3. **Stage 2 (Second Generation - Pure Roze)**: The `bin/roze_stage2` executable (now compiled entirely by Roze code!) compiles `src/roze_compiler.roze` into `bin/roze_stage3.c` and builds `bin/roze_stage3`.
4. **Fixpoint Idempotence Proof**:
   ```bash
   diff -u bin/roze_stage2.c bin/roze_stage3.c
   ```
   The diff is **zero bytes**! Because Stage 2 and Stage 3 produce 100% byte-for-byte identical output, the compiler has reached fixpoint closure and is completely independent of the original Rust compiler.

---

## 4. Test Suite Programs

The test suite in `tests/` validates language semantics compiled by the self-hosted compiler:

| Test Program | Features Exercised | Expected Result |
| :--- | :--- | :--- |
| `tests/01_hello.roze` | Basic function declaration, string printing | `"Hello from pure Roze!"` |
| `tests/02_arithmetic.roze` | Arithmetic, operator precedence, recursion (`factorial(5)`) | `42`, `12`, `45`, `9`, `120` |
| `tests/03_control_flow.roze` | `for` loops, `if` statements, modulo `%`, boolean predicates | `5` |
| `tests/04_classes.roze` | Class definitions (`Vector2`), constructors (`new`), field access (`->`) | `40`, `70` |
| `tests/05_strings.roze` | String concatenation (`+`), string equality (`==`), conditionals | `"Welcome to Roze Language!"`, `"Match: Roze"` |

---

## 5. Build & Execution Instructions

Run the entire bootstrap and verification suite with a single command:

```bash
# Run full 3-stage bootstrap fixpoint test and all sample test programs:
make all
```

Individual targets:
```bash
# Build Stage 1 using Stage 0 (Rust roze):
make stage1

# Build Stage 2 using Stage 1:
make stage2

# Build Stage 3 using Stage 2:
make stage3

# Verify zero diff between Stage 2 and Stage 3:
make bootstrap_test

# Compile and run test programs with Stage 2:
make test_programs

# Clean build artifacts:
make clean
```

### Compiling Any Roze Program with the Self-Hosted Compiler
```bash
./bin/roze_stage2 my_program.roze -o my_program
./my_program
```
