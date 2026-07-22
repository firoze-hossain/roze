# Roze Roadmap

This replaces the earlier status table, which had drifted from what the
compiler actually does. Everything marked ✅ below was verified by
compiling and running real `.roze` programs, not just by reading the
code.

## Phase 1: Foundation

| Task | Status | Notes |
|---|---|---|
| Lexer | ✅ Done | Tokenizes comments, strings, numbers, all operators, and keywords including `if`/`else`/`while`/`for`/`class` (the latter two aren't used by the parser yet). |
| Parser | ✅ Done for the currently-supported grammar | Now handles `func` (with typed params and `-> ReturnType`), `let`, assignment, `return`, blocks, `if`/`else`/`else if`, `while`, and expressions with normal precedence. `for` and `class` are tokenized but intentionally not parsed yet (see Phase 1.5). |
| Type checker | ⚠️ Basic, working | Tracks real parameter/return types (previously discarded), scoped variable lookup, and catches undefined-variable/undefined-function errors. Does **not** yet enforce that a `return` matches the function's declared return type, or that reassignment preserves a variable's original type -- both listed below. |
| JVM codegen | ✅ Done for the currently-supported grammar | Emits **every** top-level function (previously only `main` was emitted -- calling any second function was a silent miscompile), using real declared/inferred types instead of guessing from variable names. Generates `if`/`while`/assignment. |
| Core (string, math) | ✅ Done | `string_length`, `string_concat`, `string_to_upper`, `string_to_lower`, `abs`, `max`, `min`, `to_string`, `to_int`, `is_number`, `is_string` -- implemented as compiler intrinsics mapped to real JVM calls, available in every program with no import. See `stdlib/src/core.roze` for the reference doc. |
| Error messages | ⬜ Not started | Errors currently print a Rust `anyhow` message and a Rust stack trace. A real "Roze-flavored" error with a source snippet, a `^^^` pointer, and a plain-English message is Phase 1.5 work, not Phase 2 -- it's higher-leverage now, while the grammar is still small, than after more syntax lands. |

### Phase 1.5 (new -- do this before Phase 2)

These are small, and they unblock everything else more than any Phase 2
item does:

- [ ] `for` loops (tokenized already; needs parser + codegen, same shape as `while`)
- [ ] Return-type enforcement (`check_statement`'s `Return` case currently type-checks the expression but doesn't compare it to `current_return_type`)
- [ ] Human-readable errors with source spans (see above)
- [ ] A minimal module system: right now `import "x";` **parses but does nothing** -- nothing gets pulled in. Without this, `stdlib/` can't actually be used by a program, and "Standard Library" in Phase 3 has nowhere to attach.
- [ ] Test suite: there are currently zero automated tests for the compiler itself (`tests/test_runner.roze` is a `.roze` script, not wired to `cargo test`). Add `cargo test` coverage per compiler stage, plus golden-output tests that compile+run a `.roze` fixture and assert stdout -- exactly the kind of test that would have caught every bug fixed in this pass automatically instead of by manual inspection.

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

Core (string, math) is done -- see Phase 1. Everything else here is
blocked on the module system (Phase 1.5), since a standard library only
matters once a program can actually `import` it:

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

1. **Phase 1.5** -- `for`, return-type checking, real errors, minimal
   module system, test suite. (Small, unblocks everything below.)
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
