# Memory model for the native backend: a decision, not a default

This is the one item from ["The bigger picture"](../ROADMAP.md#the-bigger-picture-one-language-many-targets)
that genuinely needs your sign-off rather than mine to implement. I'm
laying out the tradeoffs and making a recommendation, but this is a
decision that shapes syntax you haven't written yet, and it's yours to
make deliberately -- not something that should get decided by whichever
option happens to be easiest to prototype first.

**Update**: a real Cranelift-based native backend spike now exists
(`compiler/src/codegen/native.rs`, `roze build foo.roze --target
native`) proving the typed-IR-to-native pipeline works end-to-end for a
deliberately small subset (int/bool functions, arithmetic, control
flow, no heap types). This document's decision is now the *only* thing
standing between that spike and actually reaching the systems/embedded
goal -- extending it to strings/collections/anything heap-allocated
means picking one of the options below first.

## Why this can't be deferred any further

The JVM backend doesn't need this decision -- it's already built, it
already implies GC, and that's a fine, unremarkable choice for
web/enterprise/desktop code. This document is entirely about the
**native backend** (the one that doesn't exist yet, needed for
systems/embedded/OS/games/security -- see the roadmap's "bigger
picture" section for why the JVM backend structurally can't reach that
half of the goal).

Every native-code language needs an answer to "who frees this memory,
and when," and the answer is visible in the language's syntax, not just
its runtime:

- Rust: `fn f(s: &str)` vs `fn f(s: String)` -- borrowing vs. ownership
  is in every function signature.
- Swift: reference types are implicitly ARC'd; `weak`/`unowned` exist
  specifically to break retain cycles.
- C: nothing in the syntax at all -- `free()` is just a function call
  the programmer remembers to make, or doesn't.

Whichever of these shapes Roze picks, it becomes part of the language's
grammar. Deciding this *after* writing a bunch of native-backend
syntax means rewriting that syntax. Deciding it now, before any of that
exists, costs nothing extra.

## The options

### 1. Tracing garbage collection

What the JVM backend already does. A background collector periodically
finds and frees unreachable objects.

- **For**: simplest possible mental model for the language user; zero
  new syntax; Roze already has an implementation of "this" to learn
  from, if you wanted to ship a small bundled GC for the native
  backend too.
- **Against**: this is the one option that's actually disqualified by
  the goal, not just suboptimal for it. A GC needs a runtime, has
  unpredictable pause times, and typically can't run at all in the
  smallest embedded targets (no heap to speak of) or inside an OS
  kernel (no scheduler to run the collector on). Choosing GC for the
  native backend is choosing not to reach OS/embedded, which is the
  entire reason a native backend is on the roadmap at all.

**Verdict**: ruled out for the native backend specifically, precisely
*because* it already covers the JVM backend perfectly well and there's
no reason to build the same thing twice for the half of the goal it
can't reach anyway.

### 2. Manual memory management (malloc/free, C-style)

The programmer allocates and frees explicitly. No compiler help.

- **For**: maximum control, zero runtime overhead, the most direct
  match to "systems programming" in the traditional sense, and by far
  the least compiler-engineering effort to implement -- codegen just
  emits the calls.
- **Against**: this reintroduces, on purpose, the exact category of bug
  (use-after-free, double-free, buffer overflows, memory leaks) that
  the last twenty years of systems-language design has been trying to
  get away from. It's also a poor fit for the *other* half of Roze's
  stated goals -- nobody wants to manually manage memory to write a
  Spring-Boot-style web handler, and a language that requires it there
  would undercut the "approachable" quality Roze has been optimizing
  for so far (no generics yet, everything-is-a-function-call instead of
  deep type-system features).

**Verdict**: matches the systems half of the goal, actively fights the
approachable/enterprise half. Would only make sense if Roze were
*purely* a systems language, which isn't the stated goal.

### 3. Ownership with compile-time borrow checking (Rust's approach)

The compiler tracks who owns a value and when it's valid to use,
rejecting a program at compile time if it might use freed or aliased
memory. No runtime cost at all -- the checking happens once, during
compilation.

- **For**: the only option here with zero runtime overhead *and* memory
  safety. Genuinely proven to scale from embedded to web servers (Rust
  itself spans that whole range with one model). This is the "purest"
  answer to what a systems language should do today.
- **Against**: a borrow checker is one of the most difficult pieces of
  compiler engineering that exists in mainstream language
  implementation -- it took Rust's own team years, multiple false
  starts (the original "regions" system was scrapped and replaced by
  the current NLL borrow checker), and it remains the single biggest
  source of newcomer friction in the language today. It also adds real
  language-surface complexity (lifetimes, `&`/`&mut`, move semantics)
  that would sit uneasily next to Roze's current design philosophy of
  "ship real capability via plain functions, not new type-system
  machinery" (see how Core/Collections/IO all landed as intrinsics
  rather than new syntax).

**Verdict**: the "correct" systems answer, and the wrong scope for
where this project is today. This is realistically a multi-year
undertaking for a solo/small-team project, and choosing it now would
mean the native backend doesn't ship for a very long time.

### 4. Automatic reference counting (Swift's approach)

Every reference-counted value tracks how many live references point to
it; the compiler inserts retain/release calls automatically; the value
is freed the instant its count hits zero. Deterministic (no GC pause),
but does have real per-operation overhead (every assignment/copy touches
a counter) and needs `weak`/`unowned` escape hatches for reference
cycles, or they leak.

- **For**: memory-safe without a tracing GC, so it *can* run in
  embedded/OS contexts a GC can't (Swift itself targets embedded and
  systems-adjacent work, e.g. on microcontrollers via Embedded Swift).
  Meaningfully less compiler-engineering effort than a borrow checker --
  it's "insert a retain/release call at compile time," not "solve a
  whole-program aliasing analysis." Spans Roze's whole stated range
  (Swift itself goes from iOS apps to server-side Vapor to embedded)
  with one model, which is exactly the property this decision needs.
  Closer in spirit to Roze's existing "keep the surface simple" design
  than ownership/borrowing is.
- **Against**: real runtime overhead compared to ownership (retain/
  release traffic; a systems purist would call this disqualifying for
  the tightest embedded targets). Reference cycles are a genuine leak
  risk that needs a manual escape hatch (`weak`) the user has to
  remember to use, similar in spirit (if smaller in blast radius) to
  manual memory management's "the programmer has to remember" problem.

**Verdict**: the pragmatic middle option -- safe, spans the whole
target range, achievable by a small team in a reasonable timeframe.

## Recommendation: ARC

For where Roze is today -- a small/solo project that has consistently
chosen "ship working capability now, via the simplest mechanism that
works" over "build the theoretically ideal feature" (this is the same
reasoning that led Collections/IO/Web/SQL to all land as plain
functions rather than waiting on generics or a module system to mature
first) -- ARC is the option that matches that pattern for memory
management too:

- It's **achievable**. A borrow checker is a multi-year research-grade
  effort; ARC is "walk the IR, insert retain/release calls, handle
  scope exits," which is squarely normal compiler engineering.
- It **reaches the whole goal**, not just half of it. Embedded, OS-
  adjacent, and general application code all work under ARC (that's
  Swift's own range). Ownership would reach the same range in
  principle, but only after the multi-year borker-checker investment;
  manual memory management reaches systems code but actively fights
  the enterprise/web half.
- It **fits the language's existing shape**. Roze's design so far
  avoids asking the user to learn new type-system machinery (no
  generics, no lifetimes, no explicit type annotations required in most
  places). ARC needs exactly one new piece of user-facing surface
  (`weak` for cycles) versus ownership's much larger surface
  (lifetimes, borrow syntax, move semantics throughout).

This is a recommendation, not a fait accompli -- if the appetite is
specifically to build "a serious systems language" and years of
runway exist for a borrow checker, ownership is the more prestigious
and more truly zero-overhead answer, and that's a legitimate choice to
make instead. But absent that explicit appetite, ARC is what actually
ships a working native backend in a reasonable amount of time while
still reaching every target on the original list.

## What changes once this is decided

Whatever gets chosen, here's the concrete syntax/design surface it
touches, so the scope of "deciding this" is visible up front:

- **A `class`/reference-type declaration.** Roze tokenizes `class`
  already but never parses it (see ROADMAP.md's Phase 1 notes) --
  reference-counted (or owned, or GC'd) values need *some* way to
  declare a type with fields, and this is almost certainly it.
- **How mutation through a shared reference works.** Right now every
  Roze value is either a primitive (copied) or one of the Object-boxed
  intrinsic types (`list`/`map`/opaque handles like a SQL connection)
  passed around by reference with no ownership story at all -- that's
  only fine because the JVM's GC is silently doing the real work
  underneath. A native backend makes this an explicit design question.
- **The cycle-breaking escape hatch**, if ARC: a `weak` (or similarly
  named) qualifier, and a rule for what happens when you dereference a
  weak reference whose target is already gone.
- **Whether/how this interacts with the JVM backend's types.** `list`/
  `map`/etc. are JVM-collection-backed today; a native backend
  presumably wants its own concrete representation, which raises the
  question of whether `list`/`map` mean the same thing on both
  backends or become backend-specific.

None of this needs to be designed in this document -- it's scoped here
so that "pick ARC" (or whichever option) is understood as the first
step of a real design task, not the whole of it.
