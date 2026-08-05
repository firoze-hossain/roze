# Roze: the path to a professional, general-purpose language

This is the honest version of that plan, not the encouraging one. Both
exist for a reason: you asked how many phases it takes to reach
Java/Python/C/C++/Rust-class status, with people building Spring-Boot-
style frameworks, operating systems, desktop apps, embedded firmware,
and AI/ML tooling *on* Roze. That's a real, legitimate goal -- every
language on that list started from nothing. But it's worth being
precise about what actually gets you there, because the honest answer
changes what you should work on next.

## The one thing to internalize before the phase list

**A language doesn't become "professional" by accumulating features.
It becomes professional when other people trust it enough to build on
it, and enough of them do that a real ecosystem forms around it.**

Spring Boot isn't part of Java. It's a separate, enormous open-source
project -- originally built by a company (then acquired into VMware/
Pivotal/Broadcom), with dozens of paid engineers, over more than 20
years, on top of a Java ecosystem that was already mature when Spring
started in 2003. Django isn't part of Python. React isn't part of
JavaScript. Rails isn't part of Ruby. In every case, the *language*
reached a baseline of capability and trust first, and the *framework*
came from someone else, later, motivated by an existing user base.

This means the honest project plan has two tracks that need to run
together, not one:

1. **Engineering track**: get the language, standard library, and
   tooling capable enough that the four target domains are *possible*
   to build on Roze at all.
2. **Adoption track**: get real people using Roze for real (if small)
   things, so that "someone builds a web framework for it" is a thing
   that can actually happen. No amount of solo engineering substitutes
   for this -- it's the part that turned Rust from "Mozilla's systems
   language experiment" (2010) into "a language people build companies
   on" (2020s), and it took a decade with corporate backing and a large
   volunteer community the whole way.

Every phase below is engineering. The adoption track isn't a phase --
it's a parallel, ongoing thing that has to start *now*, with whatever
exists today, not after some future phase completes. More on this at
the end.

## Where Roze actually stands today

This is not a from-scratch pitch. Here's the real, tested, working
inventory, so the phases below are additions to something real, not a
wish list starting at zero:

- **A working compiler** (~9,000 lines of Rust): lexer, parser, a real
  type checker producing a typed intermediate representation, feeding
  two independent, working backends.
- **JVM backend**: compiles to real Java source, then real bytecode.
  Full control flow (`if`/`while`/`for`), functions, a standard
  library covering Core (string/math), Collections (`list`/`map`), IO
  (files, HTTP client and server, JSON), and Database (SQL via any
  JDBC driver). User-defined `class` types compile to real Java
  classes.
- **Native backend** (Cranelift-based): compiles to a real, standalone
  executable with *no* JVM involved. A real, working memory model
  (ARC -- automatic reference counting) covering `string`, `list`,
  `map`, and user-defined `class`, verified leak-free and
  use-after-free-free under Valgrind, including the hard cases
  (reference cycles between classes are the one known gap -- see
  below).
- **Tooling**: a Language Server (real editor diagnostics/completion/
  hover, not heuristic text-scanning), a build tool, and a package
  manager (currently closer to a stub -- see Phase 5).
- **~285 automated tests** across the whole workspace, including
  Valgrind-gated memory-safety tests for the native backend -- this is
  a genuinely unusual level of rigor for a project this young, and
  worth preserving as things scale up, not abandoning under time
  pressure.
- **What's documented as explicitly NOT done yet**, already tracked
  honestly in this repo: generics, closures/first-class functions,
  pattern matching, a real error-handling type (`Result`/`Option`),
  traits/interfaces, a `weak` reference (so class reference cycles
  currently leak), thread safety (ARC's retain/release aren't atomic --
  concurrent access from multiple threads is unsound right now, not
  just unimplemented), `no_std`/freestanding support, FFI (calling C
  libraries), and a real package registry.

That last list is the honest starting point for everything below.

## What "Java/Rust/C/C++-class" actually breaks down into

Four largely independent dimensions, each of which needs its own
sustained work:

1. **Language completeness**: the grammar and type system can express
   what real programs need (generics, closures, error handling,
   polymorphism, concurrency, unsafe/systems escape hatches).
2. **Runtime/platform reach**: the compiler can target what real
   programs need to run on (a real memory model with thread safety, a
   freestanding mode for OS/embedded, FFI to existing native
   libraries, WASM for the browser).
3. **Tooling and developer experience**: the *experience* of using the
   language is trustworthy (a real package registry, a debugger, a
   formatter, a linter, a test framework, documentation generation,
   stable releases with a compatibility policy).
4. **Ecosystem and trust**: other people have used it, written about
   it, built libraries for it, and would recommend it to a colleague.
   This is the one dimension that can't be engineered directly -- it's
   downstream of the other three, plus time, plus visibility.

Every language on your list (Java, Python, C, C++, Rust) is strong on
all four. Roze today has real, tested groundwork on (1) and (2) for two
backends, essentially nothing yet on (3) beyond the basics, and (4) is
zero by definition (nothing gets adopted before it's ready to be).

## What each of your four target domains actually needs

Being concrete about this matters, because "make it professional" reads
very differently depending on which of these you care about most --
they don't all need the same things, and some are much closer than
others.

### Spring-Boot-style web frameworks

- **Language**: closures (for request handlers/middleware), traits/
  interfaces (for the dependency-injection patterns frameworks like
  Spring are built around), generics (for type-safe request/response
  handling), a real error type (so a failed request doesn't crash the
  whole server the way file/network errors currently do).
- **Runtime**: real concurrency (handling many requests at once without
  one blocking the rest) -- Roze has *no* concurrency story at all
  right now, on either backend.
- **Stdlib**: what exists today (HTTP client/server, JSON, SQL) is a
  genuine start, but it's raw -- routing, middleware, connection
  pooling, and templating are all still missing, and are exactly the
  kind of thing a framework (not the language) should provide.
- **Ecosystem**: Spring Boot itself would be *someone else's* project,
  built once the above exists and once there's a reason to build it
  (i.e., people already using Roze for web work).
- **Honest read**: closest of the four to being buildable at a basic
  level today (the JVM backend already has a working HTTP server and
  JSON), but concurrency is a hard blocker for anything beyond a toy.

### Operating systems

- **Language**: unsafe/raw pointer access, inline assembly, no
  implicit runtime dependencies.
- **Runtime**: a `no_std`/freestanding mode -- the native backend
  currently assumes `malloc`/`free`/`printf` from libc are always
  available, which is never true in kernel-space. This needs a real
  design decision (does ARC even make sense inside a kernel without a
  general-purpose allocator already running? Probably needs a
  different, more restricted model for this specific target).
  Cross-compilation to the target architecture, and control over the
  linker (custom entry points, linker scripts) for building something
  bootable.
- **Honest read**: the farthest of the four from where Roze is today.
  Everything about the current native backend's design (ARC via
  `malloc`, calling into libc's `printf`) assumes a hosted environment
  a kernel doesn't have. This is a genuine, separate design track, not
  an incremental extension of what exists.

### Desktop applications

- **Language**: closures (for UI event handlers).
- **Runtime**: **FFI** -- calling into existing native libraries.
  Realistically, nobody writes a whole new GUI toolkit from scratch;
  they bind to GTK, Qt, or the platform's native APIs (Win32, Cocoa).
  Roze has no FFI at all right now, on either backend.
- **Honest read**: this is where FFI earns its keep as the single
  highest-leverage addition on this whole list -- see below.

### Embedded

- **Language/runtime**: the same `no_std` and FFI needs as OS/desktop,
  plus hardware register access and real-time behavior guarantees
  (predictable timing matters more than raw throughput).
- **Honest read**: shares almost all its prerequisites with the OS
  track. Once one is real, the other is much closer.

### AI/ML

- **Runtime**: FFI again -- nobody re-implements BLAS/LAPACK or writes
  a new CUDA kernel compiler from scratch to get a language into ML
  work; they bind to what already exists (this is exactly how Python's
  ML ecosystem works: NumPy/PyTorch/TensorFlow are C/C++/CUDA under a
  thin Python layer).
- **Stdlib**: Roze doesn't have a floating-point/decimal number type at
  all yet (only integers) -- a real gap for any numerical work,
  independent of FFI.
- **Honest read**: FFI-dependent like desktop, plus a language-level
  gap (real number types) that hasn't come up yet because nothing so
  far has needed it.

**The pattern across three of your four domains (desktop, embedded,
AI/ML) is the same missing piece: FFI.** That's not a coincidence, and
it's why the phase plan below treats it as one of the earliest,
highest-priority additions rather than something to get to eventually.

## The full phased plan

Numbered fresh here for clarity, starting after everything in the
current inventory above. Each phase lists what it delivers and roughly
what it depends on. Phases within the same numbered group can often
happen in parallel with more than one contributor; the numbering is
priority order for a small team, not a strict must-happen-in-this-
order sequence.

### Phase 1 -- Language completeness fundamentals

Generics (real parametric polymorphism -- `list<int>` meaning
something, not every element being untyped `Object`), closures/first-
class functions, pattern matching, a real `Result`/`Option`-style error
type (replacing "crash the program" as file/network/parsing's only
failure mode), and traits/interfaces (polymorphism without
inheritance, and the shape most dependency-injection-style framework
patterns are built on). This is the biggest single lift on the whole
list -- it touches the parser, the type checker, and both backends,
and generics specifically is one of the harder features to implement
well in any language's history (see: Java's took until version 5, a
decade after 1.0; Go shipped 1.0 *without* them and added them 12
years later, in 1.18).

*Unlocks*: real type-safe collections, error handling that doesn't
crash the process, and the language-level shape every framework
pattern (in any of your four domains) is built from.

### Phase 2 -- FFI (calling existing C libraries)

The single highest-leverage item on this list for reaching desktop,
embedded, and AI/ML specifically: a way for native-backend Roze code
to declare and call a function from an existing compiled C library.
This doesn't require writing a GUI toolkit, a linear-algebra library,
or a hardware SDK in Roze -- it requires being able to *borrow* the
ones that already exist, the same way Python's `ctypes`/C-extension
story, Rust's `extern "C"`, and literally every other practical
systems-adjacent language does it.

*Unlocks*: binding to GTK/Qt for desktop, BLAS/CUDA for AI/ML, and
vendor hardware SDKs for embedded, all without reimplementing any of
them.

### Phase 3 -- Concurrency and thread safety

Two problems, tightly linked: (a) Roze has no concurrency primitives
at all (no threads, no async/await, no channels) on either backend,
and (b) the ARC implementation that already exists is **not**
thread-safe -- retain/release are plain increments/decrements, not
atomic operations, so using the same `string`/`list`/`map`/`class`
value from two threads at once is undefined behavior today, silently.
This needs an explicit decision (atomic reference counts always, at a
performance cost, the way Objective-C's ARC does it by default; or a
Rust-style ownership check that only some values are shareable across
threads, which is a much bigger type-system feature) before any real
concurrency story is safe to build on top of it.

*Unlocks*: a web server that can actually handle concurrent requests;
safe multi-threaded code in general.

### Phase 4 -- Systems and OS-readiness

`no_std`/freestanding mode (no assumed libc, no assumed heap unless
one is explicitly wired up), raw pointer / `unsafe` access, inline
assembly, cross-compilation to other CPU architectures, and control
over the linker (custom entry points, linker scripts) for anything
that needs to be bootable. This is a genuinely separate design track
from the hosted native backend that exists today, not an incremental
extension of it -- expect this to effectively mean a second, more
restricted native-backend mode, not a flag on the existing one.

*Unlocks*: the OS and embedded tracks specifically; nothing else on
this list needs it.

### Phase 5 -- Tooling maturity

A real package registry (a crates.io/PyPI/npm equivalent -- what
exists today is closer to a stub that doesn't yet fetch or resolve
real third-party dependencies), dependency resolution and lockfiles,
a source-level debugger, a code formatter, a linter, a built-in test
framework (a real `#[test]`-equivalent with assertions, not just what
this project's own internal test suite uses to test the compiler
itself), and a documentation generator. None of this is glamorous, and
none of it is optional -- this is the dimension that determines whether
someone's first hour with Roze feels like a real language or an
experiment.

*Unlocks*: adoption, directly. This is where "professional" is felt
before it's proven by any feature list.

### Phase 6 -- Standard library depth

Real concurrency primitives (once Phase 3 lands), more collection
types (sets, ordered maps, queues), regular expressions, a real
floating-point/decimal number type (a genuine current gap -- Roze only
has integers today), cryptographic primitives, more serialization
formats, and a proper time/date library. Every mature language's
standard library is thousands of functions deep; this phase is never
really "done," it's a continuous, incremental effort that scales with
how many kinds of programs people are actually trying to write.

### Phase 7 -- WASM backend

A third backend, targeting the browser and portable sandboxed
execution generally (also a plausible path for portable AI/ML
inference). Lower priority than the phases above unless a browser
target becomes a concrete, specific goal -- Cranelift (already the
native backend's code generator) has WASM target support, so this is
more tractable than it might sound, but it's still a real backend to
build and maintain.

### Phase 8 -- Platform-specific proof points

Once Phases 1-5 exist, build (or have someone build) one real,
convincing example in each target domain: a minimal bootable "hello
world" kernel, a small desktop app using FFI to a real GUI toolkit, a
"blink an LED" embedded example on real hardware, and a small
numerical/ML example using FFI to a real math library. These aren't
throwaway demos -- they're what makes "you can build an OS in Roze" a
demonstrated fact instead of a claim, and they're what future
adopters in each domain will actually look for before investing their
own time.

### Phase 9 -- Self-hosting

Rewrite the Roze compiler in Roze itself. This is a traditional
"coming of age" milestone (Rust's compiler is written in Rust; Go's
compiler is written in Go), and a genuinely powerful signal of
maturity -- if the language can build a real compiler, it can build
real software. It is not, strictly, required for practical use (C's
first compiler wasn't written in C either, for a while), and it
depends on most of Phase 1 being done first (a compiler is exactly the
kind of program that needs generics, pattern matching, and real error
handling to be pleasant to write). Sequence this after the language is
actually pleasant to write nontrivial software in, not before.

### Phase 10 -- Stability and release engineering

A semantic versioning policy for the language itself, a real
backward-compatibility guarantee, an "edition" mechanism for evolving
the language without breaking existing programs (Rust's approach), a
security vulnerability disclosure process, and long-term-support
releases. Unlike the phases above, **this should start now, in
parallel, not wait its turn** -- the earlier a language commits to "we
won't break your code without warning," the more people will risk
building on it while it's still young.

## The parallel track: adoption, starting today

Everything above is engineering that makes Roze *capable*. None of it
makes Roze *used*, and "used" is the actual destination, not a
side effect. Concretely, starting now, independent of which
engineering phase is active:

- **Publish it.** A real project page, a clear README (this repo
  already has a good one), and a place for people to actually try it
  without cloning a compiler repo and building it themselves --
  install instructions, prebuilt binaries, or a web playground.
- **Write for the domains you care about, honestly.** A blog post
  titled "I tried to write a small web server in Roze, here's what
  worked and what didn't" is worth more right now than a feature list,
  because it's the kind of content that gets a curious systems
  programmer to actually try it.
- **Pick one small, real, finishable thing to build with it yourself**
  in each domain you care about most -- not a framework, just a
  program. A tiny HTTP API. A CLI tool. Real usage surfaces the gaps
  that matter (this whole project's history is full of exactly that:
  the Windows path bug, the reserved-word bug, and both ARC leaks were
  all found by someone actually running real code, not by reading the
  spec).
- **Lower the bar for the first contribution.** A `good-first-issue`
  label, a CONTRIBUTING.md that actually explains the codebase (this
  repo has one, already a good sign), and responding to the first few
  outside contributors fast and well matters disproportionately -- the
  first external contributor to any project is the hardest one to get
  and the most important one to keep.

## Timeline, honestly

Every comparable language took years, with more than one person, often
with corporate backing:

- **Rust**: started at Mozilla in 2006, hit 1.0 in 2015 (9 years),
  reached "companies bet production systems on it" status in the
  2020s (15+ years). Had a dedicated team for most of that time.
- **Go**: started at Google in 2007, 1.0 in 2012 (5 years) -- faster,
  but with a small team of extremely experienced language designers
  (Pike, Thompson, Griesemer) and Google's infrastructure/backing from
  day one.
- **Python**: first released 1991, didn't reach wide industry adoption
  until the 2000s-2010s (10-20+ years), and that timeline includes
  most of the internet's growth happening to align with it.
- **Java**: Sun Microsystems, a large company, from day one (1995),
  with massive marketing and enterprise sales investment -- and it
  still took years to reach "the default enterprise language" status.

None of this means Roze can't get there. It means the honest estimate
for the engineering phases above, done by a small team (a few
dedicated people, not a solo effort, if it's going to move at a
reasonable pace) is **measured in years, not months** -- realistically
3-5 years of sustained work to reach "a capable, adoptable language"
(roughly through Phase 6 above), and the "professional, industry-
trusted" end-state on your original list is closer to **the better
part of a decade**, and that's the *fast* version, contingent on real
adoption happening alongside the engineering, not after it.

## What to actually do next

If the goal is genuine progress rather than a longer roadmap document,
in order:

1. **Decide the thread-safety approach for ARC** (part of Phase 3,
   but worth deciding early since it's a foundational, hard-to-retrofit
   choice -- the same kind of decision the memory model itself was).
2. **Start Phase 1's error-handling piece first**, specifically, ahead
   of generics/closures/traits -- it's the smallest, most contained
   piece of Phase 1, and "file/network operations crash the whole
   program on any error" is the single most user-hostile gap in the
   language as it stands today for literally any real program.
3. **Build FFI (Phase 2) before deciding OS/desktop/embedded/AI-ML
   priority order** -- it's the one piece of engineering that
   directly unlocks three of your four stated domains at once, and
   until it exists, none of those three are realistically buildable
   regardless of anything else on this list.
4. **Start the adoption track this week, not after some phase
   completes.** Write the honest "what worked, what didn't" post about
   using Roze for something real, today, with what already exists.
