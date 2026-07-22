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
  `if`/`else`/`else if`, `while`, assignment
- Arithmetic, comparison, and boolean expressions
- A small built-in **Core** library, always available with no import:
  `string_length`, `string_concat`, `string_to_upper`, `string_to_lower`,
  `abs`, `max`, `min`, `to_string`, `to_int`, `is_number`, `is_string`
- `println`

Not yet implemented: `for` loops, arrays/collections, structs/classes,
a module/import system, and a real standard library beyond Core. See
[ROADMAP.md](./ROADMAP.md) for the full picture, including which pieces
are genuinely done vs. in progress.

## Building

Requires a Rust toolchain and a JDK (`javac`/`java`) on your `PATH`.

```bash
cargo build --release
```

This builds the `roze` compiler binary (and the other workspace tools:
`roze-build`, `roze-pkg`, `roze-lsp`).

## Using the compiler

```bash
# Compile a .roze file to a .class file
./target/release/roze build examples/core_demo.roze

# Compile and immediately run it
./target/release/roze run examples/core_demo.roze
```

See [examples/core_demo.roze](./examples/core_demo.roze) for a working
example exercising control flow and the Core library.

## Project layout

```
compiler/     Lexer, parser, type checker, and JVM code generator (the `roze` binary)
runtime/      Runtime support crate
stdlib/       Standard library reference/source (see ROADMAP.md for current status)
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
