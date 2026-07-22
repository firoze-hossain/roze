# Contributing to Roze

Thanks for your interest in Roze! It's early-stage, so there's a lot of
room to make a real difference.

## Getting set up

You'll need a Rust toolchain and a JDK (`javac`/`java`) on your `PATH`.

```bash
git clone https://github.com/firoze-hossain/roze.git
cd roze
cargo build --workspace --release
./target/release/roze run examples/core_demo.roze
```

## Where to look first

- [ROADMAP.md](./ROADMAP.md) has an honest, current picture of what's
  actually implemented vs. planned, including a "Phase 1.5" list of
  small, high-leverage next steps (e.g. `for` loops, a real module
  system) that are good first contributions.
- `compiler/src/` is the compiler itself: `lexer/`, `parser/`,
  `semantic/` (the type checker), and `codegen/jvm.rs`.
- `stdlib/src/core.roze` documents the built-in Core (string/math)
  functions.
- `examples/` has working `.roze` programs you can compile and run.

## Making changes

1. Open an issue or comment on an existing one before starting anything
   non-trivial, so effort doesn't get duplicated.
2. Add a test. There isn't yet a full automated test suite (that's
   tracked in the roadmap), but for compiler changes, please compile and
   *run* a `.roze` program that exercises your change and include the
   command/output in your PR description until `cargo test` coverage
   exists.
3. Run `cargo build --workspace` and `cargo clippy --workspace` before
   opening a PR.
4. Keep PRs focused. Several small PRs are much easier to review than
   one that touches the lexer, parser, and codegen at once.

## Reporting bugs

Please include:
- The `.roze` source that triggers the issue
- The exact command you ran (`roze build ...` / `roze run ...`)
- What you expected vs. what happened (including the full error output)

## Code of Conduct

This project follows the [Code of Conduct](./CODE_OF_CONDUCT.md). Please
read it before participating.
