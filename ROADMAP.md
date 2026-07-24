# Roze Roadmap

This replaces the earlier status table, which had drifted from what the
compiler actually does. Everything marked ✅ below was verified by
compiling and running real `.roze` programs, not just by reading the
code.

## Phase 1: Foundation

| Task | Status | Notes |
|---|---|---|
| Lexer | ✅ Done | Tokenizes comments, strings, numbers, all operators, and keywords including `if`/`else`/`while`/`for`/`class` (the latter two aren't used by the parser yet). |
| Parser | ✅ Done for the currently-supported grammar | Handles `func` (with typed params and `-> ReturnType`), `let`, assignment, `return`, blocks, `if`/`else`/`else if`, `while`, `for` (C-style: `for let i = 0; i < n; i = i + 1 { ... }`), `import`, and expressions with normal precedence. `class` is tokenized but intentionally not parsed yet (no structs/objects in the language yet to back it). |
| Type checker | ✅ Done | Tracks real parameter/return types, scoped variable lookup, and catches undefined-variable/undefined-function errors. Now also enforces that every `return` matches the function's declared return type (including a bare `return;` in a non-void function), that reassignment preserves a variable's original declared type, and that `main` doesn't declare a return type (it's always void, matching what codegen hard-codes). |
| JVM codegen | ✅ Done for the currently-supported grammar | Emits **every** top-level function (previously only `main` was emitted -- calling any second function was a silent miscompile), using real declared/inferred types instead of guessing from variable names. Generates `if`/`while`/`for`/assignment, all as real Java control flow (a Roze `for` compiles to an actual Java `for (init; cond; update)`, not a desugared `while`). |
| Core (string, math) | ✅ Done | `string_length`, `string_concat`, `string_to_upper`, `string_to_lower`, `abs`, `max`, `min`, `to_string`, `to_int`, `is_number`, `is_string` -- implemented as compiler intrinsics mapped to real JVM calls, available in every program with no import. See `stdlib/src/core.roze` for the reference doc. |
| `==`/`!=` on strings | ✅ Fixed | Was a real, silent correctness bug: `==`/`!=` compiled straight to Java's `==`, which is reference identity, not content equality. `let combined = a + b; combined == "some literal"` would silently evaluate to `false` even when the content matched, since a runtime-built string isn't the same object as a literal. Now routes through `java.util.Objects.equals` (null-safe content equality) whenever either side isn't a Java primitive; plain `==` is kept for `int`/`bool` comparisons, where it was already correct. Found by actually running the pre-existing `tests/test_runner.roze` rather than just reading it. |
| Error messages | ✅ Done | Real "Roze-flavored" errors: a plain-English message, a `-->` pointer at `file:line:column`, the offending source line, and a `^^^` underline sized to the actual token -- for lexer, parser, and type errors alike. `main()` never lets a raw Rust `anyhow` Debug dump or panic backtrace reach the user; every failure path is caught and rendered through `RozeError::report`. Fixed two real off-by-one bugs in the lexer's column tracking along the way (the very first character of the file, and the first character after *every* newline, were both reported one column too high) -- these had been silently wrong since before any code actually displayed columns to a person. |

### Phase 1.5 -- done

Everything originally listed here is now done:

- **`for` loops**: C-style, `for <init>; <condition>; <update> { ... }`,
  e.g. `for let i = 0; i < 10; i = i + 1 { println(i); }`. Compiles to a
  real Java `for (...)` loop, not a desugared `while`. The loop variable
  is scoped to the loop (checked by both the type checker and a test).
- **A minimal module system**: `import "core";` and `import "your_file";`
  now genuinely pull in another file's top-level functions, instead of
  silently doing nothing. See `compiler/src/imports.rs` for the exact
  rules (one level deep, no namespacing, your own definition always wins
  over an imported one of the same name). `stdlib/src/core.roze` now has
  real, non-intrinsic utility functions (`clamp`, `sign`, `square`,
  `is_positive`, `is_negative`, `is_empty`, `repeat`) specifically so
  there's something genuine for `import "core";` to move, beyond the
  intrinsics that were already available everywhere. Known limitation: a
  *type* error inside an imported file will currently report against the
  wrong line numbers once merged into the importing program (a *syntax*
  error is reported correctly, against the module's own file); fully
  fixing that means tracking a source file per statement through the
  whole type checker and codegen, more than a "minimal" module system
  needs for now.
- **Test suite**: 49 unit tests across the lexer, parser, type checker,
  codegen, imports, and error formatting, plus 8 end-to-end "golden"
  tests that build and run real `.roze` fixtures through the actual
  compiled `roze` binary and assert on stdout (`compiler/tests/golden.rs`
  + `compiler/tests/fixtures/`). Verified these have real teeth, not just
  tautological passes: deliberately reintroduced the old lexer
  column-tracking bug and confirmed exactly the three tests that should
  catch it fail, then confirmed they pass again once reverted. Run with
  `cargo test --workspace`.

## Known issue found but not yet fixed: Java reserved words as identifiers

While validating the fixes above against the pre-existing
`tests/test_runner.roze`, a function named `assert` failed to compile --
`assert` is a Java reserved word, and codegen emits Roze identifiers
verbatim as Java identifiers with no check against Java's keyword list.
Any Roze variable, parameter, or function name that happens to match a
Java keyword (`assert`, `interface`, `synchronized`, `native`, `package`,
`throws`, `enum`, `default`, `switch`, `case`, `new`... the list is long)
will fail the same way -- a real, if narrow, correctness gap. Renamed
`test_runner.roze`'s `assert` to `check` as a workaround rather than
leaving a known-broken test file in the repo, but the underlying gap is
still open. Fixing it properly means maintaining a Java-reserved-word
list in codegen and escaping any identifier that collides with one
(e.g. emitting `assert_` instead of `assert`) consistently at both the
definition and every use site -- a reasonably small, well-scoped next
fix, not attempted here to keep this pass focused on `for`/imports/tests.

## Phase 2: Developer Experience

The original doc marked this whole phase "Not Started," but `roze-build`,
`roze-pkg`, and `roze-lsp` all already have substantial real
implementations (the LSP tool alone is ~600 lines across `analyzer.rs`,
`diagnostics.rs`, and `parser.rs`). They weren't deeply audited in this
pass -- that's the natural next focus once Phase 1.5 lands, since some of
them (e.g. the LSP's own parser) may need to be reconciled with the fixes
made here rather than duplicating parser logic a second time.

- [ ] Audit `roze-lsp`'s parser vs. `compiler`'s parser (right now there appear to be two independent Roze parsers in this workspace -- worth unifying so fixes only need to happen once)
- [ ] `roze build` / `roze pkg` end-to-end smoke test
- [ ] VS Code extension smoke test against a real `.roze` file

## Phase 3: Standard Library

Core (string, math) is done -- see Phase 1. The module system needed to
actually use a standard library (Phase 1.5) is also done now, so these
are unblocked except where noted:

- [ ] IO (`file`, `network`)
- [ ] Collections (`List`, `Map`) -- also blocked on the language having arrays/generics at all, which don't exist yet
- [ ] Web (`HTTP`, `JSON`)
- [ ] Database (`SQL`)

## The bigger picture: one language, many targets

You described the goal as being able to build *anything* in Roze --
desktop apps, games, security/systems tooling, an OS, embedded software,
web backends and Spring-Boot-style enterprise servers, and AI/ML. That's
a legitimate and well-precedented goal (Kotlin, Swift, and Rust all
target more than one backend), but it changes the shape of the roadmap in
one important way worth calling out explicitly, rather than leaving it as
an implicit Phase 6 afterthought:

**The JVM backend cannot deliver the systems half of that list, no
matter how much you build on top of it.** An OS kernel, embedded/no_std
firmware, and low-level systems or security tooling all need direct
control over memory layout and no mandatory runtime underneath -- a JVM
program can't run without a JVM. This isn't a matter of writing more
Roze code against the JVM backend; it's a different backend entirely.
The original roadmap already knew this (Phase 4 lists an LLVM backend,
unsafe pointers, and no_std support) -- the change worth making now is
architectural, not sequential: **treat "which backend" as a decision made
per-target from early on, not a bolt-on after five other phases.**

Concretely:

1. **Separate the frontend from the JVM backend now, while it's cheap.**
   Right now `codegen/jvm.rs` consumes the parser's AST directly. That's
   fine with one backend; it becomes a real cost with two, because every
   frontend feature (generics, closures, pattern matching, whatever comes
   next) would otherwise need to be re-taught to each backend
   independently, and they *will* drift the way the parser and the LSP's
   separate parser already have. The standard shape here (used by
   rustc, Kotlin, and Swift) is: `source -> AST -> typed IR -> backend`,
   where the typed IR is backend-agnostic and each backend
   (JVM-source-today, LLVM-later, WASM-maybe) only has to consume that
   one shared representation.

2. **Pick a memory model on purpose, not by default.** This is the
   single highest-leverage decision left, and it's currently undecided
   by omission rather than by choice. The JVM backend implies GC. A
   systems backend needs either manual memory management, ownership-style
   compile-time checking (Rust's approach), or reference counting
   (Swift's approach) -- and whichever you pick shapes syntax you haven't
   written yet (how does the language spell "borrow," "move," "unsafe
   block"?). Deciding this before Phase 3's Collections work is
   worthwhile, since `List`/`Map` APIs look different under GC vs.
   ownership.

3. **Sequence the backends by what's actually reachable first**, rather
   than treating "systems programming" as strictly Phase 4:
   - **JVM backend** (this one): web backends, enterprise/Spring-Boot-style
     servers, Android-adjacent apps. Already the furthest along --
     keep pushing it through Phase 1.5 -> 3.
   - **Native backend via LLVM or Cranelift**: desktop apps, games, CLI
     tools, systems/security tooling, and eventually OS-level work. This
     is the one that unlocks most of your list and doesn't yet exist at
     all -- worth starting design work on once Phase 1.5 lands, in
     parallel with Phase 3, rather than waiting for Phase 3-5 to finish
     first.
   - **WASM backend**: browser-side web, and a plausible path for
     portable AI/ML inference code. Lower priority than the native
     backend unless a browser target becomes a concrete goal.
   - **Embedded/no_std**: realistically downstream of the native backend
     and the memory-model decision above, not a separate independent
     track.
   - **AI/ML**: in practice this means "good FFI to existing
     numerical/tensor libraries" (e.g. calling into BLAS/ONNX/PyTorch's C
     API) far more than it means "Roze reimplements tensor math." Worth
     scoping as an FFI story once a native backend exists, rather than
     its own vertical.
   - **"Hacking"**: if this means security/systems tooling (network
     programming, binary analysis, low-level protocol work) --
     straightforwardly the native backend's territory, same as any other
     systems use case. If it means something else, worth clarifying so
     the roadmap reflects the right target.

4. **Self-hosting (Phase 6) is a good end-state test, not a goal to
   chase early.** A compiler written in Roze that can compile itself is
   a strong signal the language is mature enough for real use -- but
   it's a consequence of the above phases landing well, not something to
   sequence before them.

## Revised phase order

1. ~~Phase 1.5~~ -- done: `for`, return-type checking, real errors,
   module system, test suite.
2. **Phase 3 (Core is done; Collections/IO next)**, using the new module
   system, in parallel with:
3. **Native backend design** (memory model decision + LLVM/Cranelift
   spike), so systems/games/desktop work has *somewhere to go* instead of
   being permanently "later."
4. **Phase 2 audit** (reconcile the LSP's parser with the compiler's,
   smoke-test the existing build/package tools) -- this can happen
   whenever there's a lull, since it's mostly validating what already
   exists rather than building something new.
5. Everything else (Web/DB stdlib, WASM, embedded, self-hosting) follows
   naturally once the above are in place.
