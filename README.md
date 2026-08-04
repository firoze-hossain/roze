# 🌹 Roze

Roze is a programming language project targeting the JVM, with an eventual
goal of covering everything from scripting and web backends to systems
programming. It's early-stage and under active development.

```roze
func classify(x: int) -> string {
    if x > 0 {
        return "positive";
    } else if x < 0 {
        return "negative";
    } else {
        return "zero";
    }
}

func main() {
    let name = "Roze";
    println("Hello, " + name + "!");
    println(string_to_upper(name));
    println(abs(-42));
    println(classify(-5));
}
```

## Status

Roze compiles to Java source, which is then run through `javac`/`java`.
Currently working:

- Variables (`let`), functions (with typed parameters and return types),
  `if`/`else`/`else if`, `while`, `for` (C-style), assignment
- A minimal module system: `import "core";` pulls in a handful of real
  utility functions (`clamp`, `sign`, `square`, ...); `import "your_file";`
  pulls in another `.roze` file's functions
- Arithmetic, comparison, and boolean expressions (string `==`/`!=`
  compare content, not object identity)
- Real error messages: a plain-English message, a `-->` pointer at
  `file:line:column`, the offending source line, and a `^^^` underline --
  for lexer, parser, and type errors alike
- Type checking that enforces return types (including on reassignment)
- A small built-in **Core** library, always available with no import:
  `string_length`, `string_concat`, `string_to_upper`, `string_to_lower`,
  `abs`, `max`, `min`, `to_string`, `to_int`, `is_number`, `is_string`
- **Collections**: `list_new`/`push`/`get`/`set`/`remove`/`length`/`is_empty`
  and `map_new`/`put`/`get`/`has`/`remove`/`size`/`is_empty`, backed by
  real `java.util.ArrayList`/`HashMap` (no array-literal syntax or
  generics yet, so these are plain functions -- see
  `stdlib/src/collections.roze`)
- **IO**: `read_file`/`write_file`/`append_file`/`file_exists`/
  `delete_file`/`read_lines`, and `http_get`/`http_post` -- see
  `stdlib/src/io.roze`
- **Web**: `json_encode`/`json_decode` (working directly with
  `list`/`map` values), plus a synchronous HTTP server
  (`http_server_start`/`accept`/`respond`/`stop`) -- see
  `stdlib/src/web.roze`
- **Database**: `sql_connect`/`sql_query`/`sql_execute`/`sql_close` on
  top of `java.sql` -- bring your own JDBC driver via `roze run
  app.roze --classpath driver.jar` (the JDK ships no driver for any
  database) -- see `stdlib/src/sql.roze`
- `println`

Not yet implemented: array-literal syntax, generics, structs/classes,
and a Result/Option type for error handling (file/network errors
currently surface as a runtime crash). Also, Roze identifiers that
happen to match a Java reserved word (e.g. a function named `assert`)
currently fail to compile -- see ROADMAP.md. See
[ROADMAP.md](./ROADMAP.md) for the full picture, including which pieces
are genuinely done vs. in progress.

## Building

Requires a Rust toolchain and a JDK (`javac`/`java`) on your `PATH`.

```bash
cargo build --release
```

This builds the `roze` compiler binary (and the other workspace tools:
`roze-build`, `roze-pkg`, `roze-lsp`).

Run the test suite (unit tests plus end-to-end tests that build and run
real `.roze` programs -- needs the JDK from above too):

```bash
cargo test --workspace
```

## Using the compiler

```bash
# Compile a .roze file to a .class file
./target/release/roze build examples/core_demo.roze

# Compile and immediately run it
./target/release/roze run examples/core_demo.roze
```

See [examples/core_demo.roze](./examples/core_demo.roze) for a working
example exercising control flow and the Core library.

### Native backend

`--target native` compiles to a real, standalone executable via
Cranelift -- no JVM/JDK involved at all. Supports int/bool/`string`/
`list`/`map`/user-defined `class` functions, arithmetic, `if`/`while`/
`for`, calling other Roze functions, and (as of the ARC memory model
decision -- see
[docs/MEMORY_MODEL_DECISION.md](./docs/MEMORY_MODEL_DECISION.md))
real, general-purpose strings, lists, maps, and classes, all backed by
real reference counting: string parameters/return values/`let`/
reassignment/concatenation/content equality; `list_new/push/get/set/
remove/length/is_empty` with int/bool elements (growing automatically
past initial capacity, safe out-of-bounds handling); `map_new/put/
get/has/remove/size/is_empty` with int/bool keys/values (a real hash
table with automatic growth); and `class Name { field: type, ... }`
with `new`/field read/field write, where a field can be a string, a
list, a map, or another class (not restricted to int/bool the way
`list`/`map` elements are, since a class's field types are statically
declared). Not supported yet: string/list/map elements or keys/values
*inside* a `list`/`map` specifically, a `weak` reference (so a
reference cycle between two classes will leak), and every Core/
Collections/IO/Web/Database intrinsic (JVM-specific today). Needs a C
compiler (`cc`, `gcc`, or `clang` -- tried in that
order) on `PATH` for linking; this is separate from whatever toolchain
Rust itself used to build `roze`. On Linux/Mac this is virtually always
already present; on Windows, MSYS2's MinGW-w64 toolchain is the most
reliable option (see
the error message from `--target native` for exact install steps if
none is found).

```bash
./target/release/roze run examples/native_demo.roze --target native
```

## Project layout

```
compiler/     Lexer, parser, type checker, typed IR, and JVM code generator (the `roze` binary)
runtime/      Runtime support crate
stdlib/       Standard library reference/source (see ROADMAP.md for current status)
docs/         Design/decision documents (e.g. the memory model decision)
tools/
  roze-build/   Build system (`roze-build`)
  roze-pkg/     Package manager (`roze-pkg`)
  roze-lsp/     Language Server Protocol implementation
ide/vscode/   VS Code extension
examples/     Example .roze programs
```

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

MIT — see [LICENSE](./LICENSE).
