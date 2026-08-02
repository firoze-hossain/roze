# Memory model for the native backend: a decision, not a default

**Decided: ARC.** Approved and a real implementation now exists for
three heap types (`string`, `list`, and `map`) -- see
`compiler/src/codegen/native.rs` and the "What's actually built"
sections further down this document. The analysis and recommendation
below are kept as-is (including the original framing of this as an
open decision) since they're still the record of *why* ARC was chosen,
and the same reasoning applies to whatever comes next (a user-defined
`class`) as ARC gets extended further.

**Update history**:
- A real Cranelift-based native backend spike first proved the typed-
  IR-to-native pipeline end-to-end for a deliberately small subset
  (int/bool functions, arithmetic, control flow, no heap types).
- ARC was then approved as the memory model, and implemented for
  `string` -- the first and simplest heap type -- as the next concrete
  step past that spike.
- ARC was then extended to `list` (int/bool elements only for now).
- ARC was then extended to `map` (int/bool keys/values only for now,
  same reasoning as `list`'s elements) -- see below.

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

## What's actually built (strings, under ARC)

The `class` question above is still open -- what's built so far is
narrower and doesn't need it: ARC for Roze's one built-in heap type
that already existed conceptually, `string`, without yet introducing
any new user-facing type-declaration syntax.

**Representation**: a Roze string value is a pointer to its bytes,
with a 16-byte ARC header immediately *before* that pointer (refcount,
then length) -- so the value Roze code holds is always already a
valid, NUL-terminated C string, usable directly with `printf`'s `%s`
with no adjustment. A literal is a specially-marked *immortal* string
(refcount sentinel `-1`, living in static data, never allocated or
freed) -- retain/release are no-ops on it, so the overwhelmingly common
case of using a literal costs no heap traffic at all.

**Ownership convention**: a *fresh* value (a literal, a concatenation
result, a function's return value) already carries a properly-owned
reference nobody else holds a claim on. A bare *identifier* reference
aliases an existing owned binding, so creating a second independent
owner from it needs an explicit retain. Every scope releases its own
string locals on normal exit; an early `return` releases every
*active* scope (the whole function is being exited, not just the
innermost block), protecting the returned value with a retain first if
it's a bare identifier alias.

**What works**: `string` parameters/return types, `let`/reassignment,
concatenation (`+`), content equality (`==`/`!=`, real `memcmp`-based
comparison, not pointer identity), and `println` of any string value
(not just a literal anymore).

**Verified for real**: beyond the golden tests (which check program
*output*), the test suite runs a string-heavy program through
`valgrind --leak-check=full` and asserts zero leaks and zero errors --
this is what actually caught a real bug during development (a
temporary string consumed by concat/equality/println and then
discarded needs an explicit release, since no named binding ever owns
it) and is what continues to guard against that class of regression.
Manually stress-tested beyond the automated suite too: nested early
returns several scopes deep, a `for`-loop whose own init variable is a
heap-allocated (non-literal) string with an early return from inside
the loop body, and a high-volume loop performing ~700 concatenations
in a row -- all clean under Valgrind (0 leaks, 0 errors, allocation
count exactly matches free count).

## What's actually built (list, under the same ARC convention)

**Representation**: unlike a string, a native `list` value's identity
has to stay stable across mutation -- `list_push` can need to grow the
backing storage, and `realloc` can return a different address. Two-
level indirection solves this: the pointer Roze code holds points at a
small, *fixed*-address header (refcount, length, capacity, and a
pointer to a *separately* allocated data buffer); only that inner data
pointer ever moves when the buffer grows, never the header itself, so
every binding aliasing the same list keeps working correctly across an
arbitrary number of pushes.

**Ownership convention**: identical to strings -- a fresh list (from
`list_new()`, or handed back as a function's return value) is already
owned; a bare identifier alias needs an explicit retain to create a
second independent owner; every scope releases its own list locals;
early `return` releases every active scope. Elements themselves are
plain i64 words with no ARC of their own, which is why this is
deliberately scoped to int/bool elements only -- a string or another
list stored as an element would compile but silently do the wrong
thing at runtime (never retained/released as part of the container),
so that's rejected at compile time instead, with a message naming the
restriction.

**What works**: `list_new/push/get/set/remove/length/is_empty`, growth
past initial capacity (via `realloc`), shrinking via `remove` (via a
single `memmove` call, correct even when removing the last element,
since a zero-length `memmove` is a defined no-op), and safe out-of-
bounds handling -- a clear message and a controlled `exit(1)`, never a
crash or a silent wrong read from walking off the allocated buffer.

**Found a second real bug this way, not just a first**: extending the
ownership convention to `list` immediately exposed that `Return`,
`Assign`, and bare `Expression`-statement cleanup had each
independently written their own `if ty == Type::String` check -- three
separate, narrow copies of the same logic, none of which knew about
`list`. Caught by Valgrind: a list was being freed by its own function
before it ever reached the caller it was returned to. Fixed by
consolidating all three into one shared release-by-type dispatch
function, specifically so the *next* ARC type (whatever extends this
after `list`) can't reintroduce the same class of bug by the same
mechanism -- there's now exactly one place that needs to know about a
new ARC type's release call, not three.

**Verified the same way**: output-correctness tests, plus Valgrind-
gated tests for the trickiest scenarios -- multiple live lists nested
several scopes deep with an early return from the deepest one, and a
high-volume run (1000 pushes forcing several `realloc` growths, 500
`memmove`-based removals, 200 nested list allocations) -- all clean (0
leaks, 0 errors, allocation count exactly matching free count).

**What's not built yet**: `map` on the native backend still doesn't
exist -- a hash table's collision-resolution and resizing logic is
meaningfully harder to get right (and to verify with the same
confidence) than a growable array was, so it's being treated as its
own increment rather than rushed alongside list. No `class`/user-
defined reference types. No `weak` escape hatch (moot until something
can form a reference cycle, which needs `class` first). Every Core/
Collections/IO/Web/Database intrinsic remains JVM-only.

## What's actually built (map, under the same ARC convention)

**Representation**: same two-level-indirection idea as `list` and for
the same reason (growth has to be able to relocate the backing storage
without changing the map's own identity) -- a fixed-address header
(refcount, count, capacity, and a pointer to a *separately* allocated
slots array). Each slot holds a state (empty / occupied / tombstone),
a key, and a value. Open addressing with linear probing: an insert or
lookup starts at `hash(key) & (capacity - 1)` (capacity is always a
power of 2, so this is a cheap bitmask, and works correctly for
negative keys too, since AND operates on the bit pattern regardless of
sign) and scans forward, wrapping at the end, until it finds the right
kind of slot. A tombstone -- rather than resetting a removed slot
straight back to empty -- is load-bearing for correctness, not just an
optimization: resetting to empty could break the probe sequence for
some *other* key that hashed to the same start index and had to skip
past this slot to find its own, making a later lookup for that other
key stop early and incorrectly report it missing.

**Managing the added complexity**: a hash table's collision-resolution
and resizing logic is genuinely harder to get right than a growable
array's. Rather than writing it directly in Cranelift IR and hoping,
the whole algorithm (probing, growth, tombstones) was prototyped in
plain C first -- checked for correctness with a real test covering
insert/update/remove/missing-key/negative-key/post-growth-still-
correct scenarios, and checked for leaks under Valgrind -- *before*
being translated into IR. This separates "is the algorithm right" from
"is the IR construction right" instead of debugging both at once.

**Ownership convention**: identical to `string`/`list` -- a fresh map
is already owned, a bare identifier alias needs an explicit retain,
every scope releases its own map locals, early `return` releases every
active scope. Keys and values are plain i64 words with no ARC of their
own, same restriction and same reasoning as `list`'s elements -- scoped
to int/bool for now, rejected at compile time (naming whether it was
the key or the value) otherwise.

**What works**: `map_new/put/get/has/remove/size/is_empty`. `put`
returns the old value on an update, 0 on a new key (Roze has no
null-distinct-from-0 representation on this backend yet, so `has`
is how you tell "absent" from "present with value 0"). Growth doubles
capacity and rehashes every live entry into the new table once the
load factor would exceed 75%, reusing the exact same probing function
insertion already uses (rather than a second, subtly-different copy of
that logic) for each entry's new slot.

**Verified the same way as `string`/`list`**: output-correctness
tests, plus Valgrind-gated tests for the trickiest scenarios --
high-volume growth (1000 puts forcing several doublings and full
rehashes) together with negative keys and post-removal tombstone
probing, and multiple live maps nested several scopes deep with an
early return from the deepest one -- all clean (0 leaks, 0 errors,
allocation count exactly matching free count).

**What's not built yet**: no `class`/user-defined reference types (so
no way to define a *new* heap type beyond the three built-in ones
above). No `weak` escape hatch (moot until something can form a
reference cycle, which needs `class` first). No string or nested-
container keys/values in `list`/`map` (would need the same recursive
retain/release treatment applied through a container, not a given just
because the container itself is ARC-managed). Every Core/Collections/
IO/Web/Database intrinsic remains JVM-only.
