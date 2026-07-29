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

## Fixed: Windows path handling produced invalid Java class names

Found by an actual Windows user running `roze run tests\test.roze` --
the compiler crashed with `illegal character: '\'` in the generated
Java. Root cause: both `codegen::compile_to_java` and `main.rs`'s
`run_file` derived the Java class name with `input_file.split('/')`,
which only recognizes forward slashes. On Windows, a path like
`tests\test.roze` has no forward slash at all, so it passed straight
through unsplit and the *entire path, backslash and all*, got embedded
as the literal class name (`public class tests\test { ... }`) -- not a
legal Java identifier. This is exactly the kind of bug that's invisible
from a Linux/Mac dev machine or CI runner, since `split('/')` happens
to work by coincidence there, and only surfaces the moment someone on
Windows passes a path with a subdirectory in it.

Fixed by extracting one shared, unit-tested `class_name_from_path`
helper (previously this exact logic was duplicated in two places,
independently) that treats both `/` and `\` as separators
unconditionally -- deliberately not delegating to `std::path::Path`,
whose separator handling is itself platform-conditional (only `/` on
Unix), which would have made this specific bug untestable from a
non-Windows machine. Also fixed a smaller latent bug in the same spot:
`.replace(".roze", "")` stripped every occurrence of that substring
anywhere in the path, not just a trailing extension.

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

## Phase 2: Developer Experience -- done

The original doc marked this whole phase "Not Started," but `roze-build`,
`roze-pkg`, and `roze-lsp` all already had substantial real
implementations. All three items below are now done, and all three
turned up genuine bugs -- not just gaps -- found by actually running
these tools end to end rather than reading the code.

- [x] **Unified `roze-lsp`'s parser with `compiler`'s.** There really
  were two independent Roze parsers in this workspace. The LSP's own was
  a hand-rolled, line-by-line heuristic scanner: naive
  `split_whitespace()` plus manual brace-counting, no real tokenization
  at all. It would misparse a `{`/`}` inside a string literal, couldn't
  handle a multi-line function signature, and "detected" classes via a
  bare `line.starts_with("class")` even though the real compiler doesn't
  parse classes into anything at all. Diagnostics were similarly
  heuristic: "does this line end in `=`?" (false-positives on any
  multi-line expression), "are parens balanced *on this line*?"
  (false-positives on any multi-line call) -- and neither could catch a
  single *real* Roze error (undefined variable, wrong return type,
  actual syntax error). Replaced both with a thin adapter over the real
  `roze_compiler` lexer/parser/type-checker (`tools/roze-lsp/src/parser.rs`,
  `diagnostics.rs`), so every grammar fix made to the compiler (like
  `for` loops) is automatically reflected in the editor experience
  instead of needing to be hand-ported to a second parser that only
  drifts further from reality over time. 13 unit tests.

- [x] **`roze-build` / `roze-pkg` end-to-end smoke test**, which found:
  - `roze-build`'s `find_compiler()` had an off-by-one: it moved to the
    *parent* directory before ever checking anything, so it could never
    find a compiler sitting in the current directory's own
    `target/release/`. That failure was then silently swallowed into a
    useless `"roze"` string fallback, which failed later with a raw
    **Rust panic backtrace** -- the same class of bug fixed in the main
    compiler's `main()` early on, just never applied here.
  - `roze-pkg` had an *independent*, differently-buggy `find_compiler()`
    of its own (hardcoded "go up 3 directories," landing one level too
    high for a normal `cargo build --release` layout) -- the same
    unification problem as the LSP's parser, just for compiler-discovery
    logic instead of grammar. Both are now replaced with one shared,
    tested implementation in `compiler::toolchain`.
  - `roze-pkg`'s `DependencyManager` never loaded existing dependencies
    from `roze.toml` on construction, so every command started from an
    empty map. Practical effect: `roze-pkg add` on a second dependency
    silently **deleted the first one** (since saving always overwrites
    with the in-memory map), and `roze-pkg remove` could never find
    anything, since it was always comparing against a map that had never
    been told what already existed. This was the most serious bug found
    in this pass -- silent data loss, not just a crash.
  - `roze-build` also left a stray generated `.java` file behind in the
    project root instead of moving it to the output directory like the
    `.class` file.
  - 7 regression tests added across both tools (4 for compiler-discovery,
    3 for dependency persistence), plus the `main()` backtrace fix
    applied to both.
  - Minor, not fixed: `roze-pkg remove` doesn't clean up the
    `libs/<name>/` directory `install` created for that dependency, so a
    stale stub lingers on disk after removal. Low severity (doesn't
    affect correctness of what's declared in `roze.toml`), noted here
    rather than left silently unmentioned.

- [x] **VS Code extension smoke test.** The extension itself (see
  `ide/vscode`) is thin boilerplate that spawns the `roze-lsp` binary and
  forwards messages between the editor UI and it, so the meaningful test
  is of the server's actual protocol behavior -- not achievable with a
  real VS Code GUI in this environment, but achievable completely
  otherwise: `tools/roze-lsp/tests/protocol_smoke.rs` speaks genuine LSP
  JSON-RPC-over-stdio to the compiled binary (the exact same transport
  VS Code uses) and exercises a full session -- initialize, a broken
  program's diagnostics (checked against a real position), a valid
  program using `for`/imports (checked for *zero* false positives),
  documentSymbol, hover, completion, and clean shutdown.

## Phase 3: Standard Library

Core (string, math) is done -- see Phase 1. IO and Collections are now
done too. Both turned up a real bug along the way, same as every phase
before this one -- found by actually compiling and running programs
that use them, not just by writing the feature and assuming it works.

- [x] **Collections (`List`, `Map`)**. The original note here was right
  that this is "blocked on the language having arrays/generics at all" --
  rather than wait on that (a much larger language-design undertaking,
  see below), `list_new`/`push`/`get`/`set`/`remove`/`length`/`is_empty`
  and `map_new`/`put`/`get`/`has`/`remove`/`size`/`is_empty` are
  implemented as compiler intrinsics, the same pattern as Core --
  backed by real `java.util.ArrayList`/`HashMap`, with elements/keys/
  values all untyped (Object-boxed), since there's no generics to give
  them a real element type yet. Iterate with a C-style `for` and
  `list_length` (no `for-each` yet). See `stdlib/src/collections.roze`.
  - Found: `List.remove` has both `remove(int)` and `remove(Object)`
    overloads, and a boxed `Integer` index silently binds to
    `remove(Object)` -- remove-by-equality instead of remove-by-index.
    Fixed by forcing index arguments to a genuine primitive `int` via an
    explicit `.intValue()` unboxing call.
- [x] **IO (`file`, `network`)**. `read_file`/`write_file`/`append_file`/
  `file_exists`/`delete_file`/`read_lines`, and `http_get`/`http_post`
  via `java.net.http.HttpClient`. Also intrinsics, also backed by real
  JVM calls. Errors (file not found, a failed request) surface as a
  runtime crash rather than a Roze-level error value -- Roze doesn't
  have a Result/Option type yet (see the "bigger picture" section
  below for where that fits). Java requires checked exceptions
  (`IOException`, `InterruptedException`) to be caught or declared, and
  Roze has no `throws` syntax, so these delegate to a fixed set of
  helper methods (emitted into every generated class) that catch and
  rethrow as unchecked. See `stdlib/src/io.roze`.
  - Found (unrelated to IO specifically, but found while testing it): a
    Roze string containing `\n`/`\t`/`\r` -- e.g.
    `append_file(path, "\nmore text")` -- generated invalid Java. The
    Roze lexer resolves `\n` in source into a real newline byte, and
    codegen was only escaping backslash/quote when re-emitting a string
    into generated Java source text, so the raw newline landed directly
    inside a Java string literal ("unclosed string literal"). Fixed
    with a proper escape function covering every control character the
    lexer can produce.
  - Tested for real, not just structurally: file tests actually
    read/write/append/delete real files and check the content; the
    network test spins up a minimal hand-rolled HTTP/1.1 server on
    localhost (raw `TcpListener`, no new dependency) so `http_get`/
    `http_post` are exercised against genuine network I/O without
    depending on external network access.
- [x] **Web (`HTTP`, `JSON`)**. `http_get`/`http_post` (Phase 3 IO) cover
  the client side. Added:
  - `json_encode`/`json_decode`, working directly with the existing
    `list`/`map` values (a JSON object decodes to a `map`, a JSON array
    to a `list`) rather than a separate JSON-value type. There is no
    JSON support anywhere in the standard JDK -- unlike `java.sql` or
    `java.net.http`, this genuinely doesn't exist as a JDK API at all --
    so this is a small hand-written recursive-descent parser/encoder,
    embedded the same way the file-IO helpers are.
  - An HTTP *server*: `http_server_start`/`accept`/`respond`/`stop`.
    Roze has no closures/lambdas, so there's no way to hand the server
    a per-request callback the way most frameworks (including Spring
    Boot) do -- this is a synchronous accept/respond loop you write
    yourself instead, with the request handed back as a plain `map`
    (reusing the Map intrinsics rather than inventing a "request"
    type). No concurrency: one request is fully handled before the
    next is accepted. A real limitation for anything under load, but a
    genuine, working starting point rather than nothing.
  - Found and fixed a real gap while testing this: `list_get`/`map_get`/
    etc. assumed their receiver was already statically typed
    `java.util.List`/`Map`, which breaks the moment a value's real type
    can't be known until runtime -- exactly the case for `json_decode`
    (statically `Object`, since a JSON array vs. object vs. string isn't
    knowable from the call site alone). Fixed with type-aware casting
    helpers that only cast when the static type isn't already right.
  - Tested for real: the JSON test encodes a nested map (with a list
    inside), decodes it back, and checks the values; the HTTP server
    test spawns the compiled server as a real subprocess, waits for a
    "ready" marker (printed right after the socket binds, so there's no
    arbitrary sleep-based race), then sends genuine GET and POST
    requests via a raw `TcpStream` and checks the responses.
- [x] **Database (`SQL`)**: `sql_connect`/`sql_query`/`sql_execute`/
  `sql_close`, on top of the JDK's own `java.sql` API. The harder
  problem here isn't the intrinsics themselves, it's that **the JDK
  ships no database driver at all, for any database** -- `java.sql` is
  only interfaces. There's no way around this without Roze having some
  form of dependency/classpath management, which it doesn't. Rather
  than fake it, `roze build`/`roze run` gained a `--classpath` flag:
  point it at a JDBC driver jar (H2, SQLite, Postgres, ...) and
  `DriverManager` finds it automatically (JDBC 4+ drivers self-register
  via `META-INF/services`, so Roze never needs to name the driver class).
  This is the first Roze feature that's explicitly BYO-dependency, which
  is an honest reflection of where the language's dependency story
  actually is right now (see `roze-pkg`'s "libs/" stub-generation in
  Phase 2, which doesn't yet fetch or wire in real jars either).
  - Found and fixed the same class of gap as above: `sql_connect`
    returns a statically-`Object`-typed value (there's no dedicated
    Connection type), so `sql_query`/`execute`/`close` need an explicit
    cast to `java.sql.Connection` at each call site.
  - Tested for real against an actual database, not just structurally:
    downloaded a genuine H2 (pure-Java, in-memory) driver jar, created a
    table, inserted rows, queried, updated, and verified the update
    took effect. This test is gated on a `ROZE_TEST_JDBC_JAR`
    environment variable pointing at a driver jar, so it doesn't require
    committing a binary jar to the repo or always having network access
    to fetch one -- it skips with a clear message if the variable isn't
    set, and CI sets it (downloading H2 fresh each run) so this is
    genuinely exercised on every push, not just locally.

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

1. ~~Separate the frontend from the JVM backend now, while it's cheap.~~
   **Done.** `codegen/jvm.rs` used to consume the parser's raw AST
   directly and run its *own*, separate type-inference pass (`infer_type`,
   plus a hand-rolled scope-tracking stack) to recover information the
   type checker had already computed and thrown away -- exactly the kind
   of duplication that drifts, the same problem the parser and the LSP's
   separate parser had before being unified (see Phase 2 above). Added a
   typed IR (`compiler/src/ir.rs`): the type checker now *produces* a
   fully type-annotated tree (`semantic::check_and_lower`) instead of
   just validating and discarding one, and codegen consumes it directly,
   reading `.type_` off each node instead of re-deriving anything.
   `infer_type` and the scope-tracking stack are gone entirely, not just
   hidden.
   - This paid off immediately, not just architecturally: the refactor
     surfaced a real, previously-invisible bug. `http_server_start`'s
     type in the semantic checker was `Unknown` (Object), but codegen's
     old separate inference had independently been returning
     `java.net.ServerSocket` for the same call -- the two systems
     disagreed, silently papered over by codegen never actually
     consulting the type checker's answer. The moment codegen started
     trusting the shared source of truth instead, that mismatch became
     a real compile failure instead of an invisible one. Fixed with an
     explicit cast at the call site (the same pattern already used for
     `sql_connect`'s connection handle).
   - Verified against the full existing test suite (211 tests across
     all four crates, 0 failures) with no behavior changes intended or
     found beyond the bug above -- this was meant to be a pure
     restructuring, and turned out to also be a bug fix.
   - This is also the right shape for a second backend later: a typed
     IR that isn't tied to Java is what a native backend would need to
     consume anyway (see below), so building it now, while there's only
     one backend to migrate, cost far less than untangling it after a
     second backend already existed.

2. ~~Pick a memory model on purpose, not by default.~~ **Decided: ARC**,
   approved and now implemented for the first heap type (`string`) --
   see the Phase 4 entry in "Revised phase order" below for what that
   covers concretely, and
   [`docs/MEMORY_MODEL_DECISION.md`](./docs/MEMORY_MODEL_DECISION.md)
   for the full tradeoff writeup (GC, manual management, ownership/
   borrow-checking, ARC), kept as the permanent record of *why* ARC was
   chosen over the alternatives -- the same reasoning applies as ARC
   gets extended to each next type (`list`/`map`, then eventually a
   user-defined `class`).

3. **Sequence the backends by what's actually reachable first**, rather
   than treating "systems programming" as strictly Phase 4:
   - **JVM backend** (this one): web backends, enterprise/Spring-Boot-style
     servers, Android-adjacent apps. Already the furthest along --
     keep pushing it through Phase 1.5 -> 3.
   - **Native backend via Cranelift**: desktop apps, games, CLI
     tools, systems/security tooling, and eventually OS-level work. A
     real spike exists now (`--target native`, see the Phase 4 entry
     above) proving the pipeline end-to-end for a small supported
     subset -- unlocking the rest of this list is now a matter of
     extending that subset (starting with the memory model decision),
     not proving the architecture can work at all.
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
2. ~~Phase 2~~ -- done: unified the LSP's parser with the compiler's,
   smoke-tested and fixed real bugs in `roze-build`/`roze-pkg`, added a
   genuine LSP protocol test standing in for a VS Code smoke test.
3. ~~Phase 3~~ -- fully done: Core, Collections, IO, Web (JSON + HTTP
   server), and Database, all as compiler intrinsics. SQL is the first
   feature with an external dependency of its own (bring your own JDBC
   driver via `--classpath`), which is worth keeping in mind as a
   precedent for how "real" library dependencies might work in general
   once `roze-pkg` grows past generating stub files.
4. **Native backend design** -- the typed-IR groundwork is done, the
   Cranelift spike is done, the memory model decision itself is
   **decided: ARC** (approved; see
   [`docs/MEMORY_MODEL_DECISION.md`](./docs/MEMORY_MODEL_DECISION.md)
   for the full tradeoff writeup, kept as the record of why), and ARC
   is now implemented for two heap types: `string` and `list`. This
   used to be the most consequential open decision in the whole
   roadmap; it's made now, and the thing worth watching going forward
   is different -- making sure `map`/a real `class` type keep getting
   built out on native at a reasonable pace, rather than the JVM
   backend's existing maturity on those fronts becoming a reason to
   never quite get to it.

   **What the spike proved, concretely**: `roze build foo.roze --target
   native` compiles the exact same typed IR the JVM backend consumes
   into a real, standalone native executable -- an actual ELF binary
   with no JVM/JDK involved anywhere, linked via the system's C
   compiler. `if`/`else`/`while`/`for`, calling other Roze functions
   (including recursion -- `fib(10)` runs correctly), real short-
   circuit `&&`/`||`. The JVM backend is completely unaffected and
   remains the default (`--target jvm` if you want to be explicit).

   **What ARC adds for `string`, concretely**: a real, general-purpose
   type on the native backend -- parameters, return values, `let`/
   reassignment, concatenation, content equality (`==`/`!=`, a real
   `memcmp`, not pointer identity), and `println` of any string value,
   not just a literal. Backed by real `malloc`/`free` and real
   reference counting, with literals specially marked *immortal*
   (refcount sentinel, living in static data, retain/release are
   no-ops) so the overwhelmingly common case -- using a literal --
   costs no heap traffic at all. Verified with more than golden-test
   output-checking: the test suite runs a string-heavy program through
   `valgrind --leak-check=full` and asserts zero leaks/errors, which is
   what actually caught a real bug during development (a temporary
   string consumed by concat/equality/println and discarded needs an
   explicit release, since no named binding owns it) and continues to
   guard against that class of regression. Manually stress-tested
   beyond the automated suite too: nested early returns several scopes
   deep, a `for`-loop whose own init variable is a heap-allocated
   (non-literal) string with an early return from inside the loop
   body, and a ~700-operation high-volume concatenation loop -- all
   clean under Valgrind.

   **What ARC adds for `list`, concretely**: `list_new/push/get/set/
   remove/length/is_empty`, growth past initial capacity via `realloc`,
   shrinking via a single `memmove` call on `remove`, and safe out-of-
   bounds handling (a clear message and a controlled `exit(1)`, never a
   crash). A list's identity stays stable across a `realloc`-driven
   grow by design -- the pointer Roze code holds points at a small,
   fixed-address header; only an internal data pointer inside it ever
   moves. Deliberately scoped to int/bool elements only for now (a
   string or nested list stored as an element would compile but
   silently do the wrong thing at runtime -- never retained/released as
   part of the container -- so that's rejected at compile time with a
   message naming the restriction instead). This is also where a
   *second* real bug turned up: extending the string ownership
   convention to `list` exposed that `Return`/`Assign`/bare-
   `Expression`-statement cleanup had each independently written their
   own `Type::String`-only check -- three narrow copies of the same
   logic, none aware of `list`. Valgrind caught it immediately (a list
   freed by its own function before ever reaching the caller it was
   returned to); fixed by consolidating all three into one shared
   release-by-type dispatch function, specifically so the *next* ARC
   type can't reintroduce the same bug through the same mechanism.
   Verified the same way as strings: output-correctness tests plus
   Valgrind-gated tests for the hardest cases -- multiple live lists
   nested several scopes deep with an early return from the deepest
   one, and a high-volume run (1000 pushes forcing several growths, 500
   removals, 200 nested list allocations) -- all clean.

   **Still not supported, deliberately, and rejected with a clear
   error rather than silently miscompiled**: `map` (a hash table's
   collision-resolution and resizing logic is meaningfully harder to
   get right, and to verify with the same confidence, than a growable
   array was -- its own increment, not bundled in alongside list), any
   user-defined `class`/reference type (doesn't exist as syntax yet --
   `class` is tokenized but never parsed, see Phase 1 above), and every
   Core/Collections/IO/Web/Database intrinsic (JVM-specific today).
5. Everything else (Web/DB stdlib, WASM, embedded, self-hosting) follows
   naturally once the above are in place.
