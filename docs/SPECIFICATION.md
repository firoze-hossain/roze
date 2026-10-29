# 🌹 The Roze Language Specification (v1.0.0 Stable)

**Roze** is an expressive, fast, statically typed systems programming language designed for everything from bare-metal OS kernels to cloud microservices, 60 FPS real-time games, and AI inference engines.

---

## 1. Design Principles & Goals

1. **Dual-Backend Versatility**:
   - **Native Backend**: Cranelift-powered native code generation with an atomic ARC runtime, direct C-ABI FFI, and bare-metal `--target freestanding` mode.
   - **JVM Backend**: High-performance enterprise backend running seamlessly on Java 17+ with HotSpot JIT optimization and zero-overhead interop.
2. **Deterministic Memory Safety**:
   - Zero-pause Automatic Reference Counting (`atomic_rmw` ARC) with cycle prevention design.
   - Explicit `unsafe` blocks for low-level systems programming, direct pointer dereference, and hardware memory mapped I/O.
3. **Ergonomic Safety**:
   - First-class `Result<T, E>` and `Option<T>` types with the `?` try operator for error propagation without exceptions.
   - Algebraic Data Types (`enum`) with exhaustive pattern matching (`match`).
4. **Self-Hosting Independence**:
   - The Roze compiler is fully written in Roze and compiles itself with 100% byte-for-byte bootstrap fixpoint reproducibility.

---

## 2. Type System

### 2.1 Primitive Types
| Type | Representation | Description |
| :--- | :--- | :--- |
| `int` | 64-bit signed integer | Default integer type (`i64`) |
| `float` | 64-bit IEEE 754 | Double-precision floating point (`f64`) |
| `bool` | 1-bit boolean | `true` or `false` |
| `string` | UTF-8 ARC string | Immutable refcounted string buffer |
| `void` | Unit type | Function without return value |
| `u8` | 8-bit unsigned byte | Raw byte representation |
| `rawpointer` | 64-bit address | Unmanaged memory address |

### 2.2 Algebraic Data Types (Enums)
Enums represent discriminated unions with payload variants:
```roze
enum Color {
    Red,
    Green,
    Blue,
    Custom(r: int, g: int, b: int)
}
```

### 2.3 Classes and Constructors
Classes define heap-allocated reference types with structured fields:
```roze
class Point {
    x: int,
    y: int
}

let p = new Point(10, 20);
println(p.x + p.y);
```

### 2.4 Traits and Implementation
Traits define behavioral contracts with static and dynamic dispatch:
```roze
trait Printable {
    func display(self) -> string;
}

impl Printable for Point {
    func display(self) -> string {
        return "Point(" + self.x + ", " + self.y + ")";
    }
}
```

### 2.5 Generics & Trait Bounds
Type parameters allow parameterized data types and algorithms with compile-time trait bound validation:
```roze
class Pair<A, B> {
    first: A,
    second: B
}

let pair = new Pair<int, string>(1, "roze");
```

#### Trait Bounds (`<T: Bound1 + Bound2>`)
Generic type parameters can be constrained by one or more traits using the `:` bound syntax and `+` operator:
```roze
trait Printable {
    func print_info() -> int;
}

trait Cloneable {
    func clone_val() -> int;
}

func show_and_clone<T: Printable + Cloneable>(item: T) -> int {
    let copy = item.clone_val();
    item.print_info();
    return copy;
}
```

#### Where Clauses
When generic signatures involve complex relationships or multiple parameters, `where` clauses provide clean and readable constraints:
```roze
func render_elements<A, B>(a: A, b: B) -> int
    where A: Printable, B: Printable + Cloneable
{
    a.print_info();
    b.print_info();
    return 1;
}
```

#### Associated Types in Traits
Traits can specify associated types (`type Item;`), allowing implementations to define concrete associated types without cluttering the trait signature with unnecessary type parameters:
```roze
trait Producer {
    type Item;
    func produce() -> int;
}

class NumberProducer {
    val: int
}

impl Producer for NumberProducer {
    type Item = int;
    func produce() -> int {
        return this.val;
    }
}
```

#### Static Monomorphization Engine
Generic functions and classes undergo compile-time specialization (monomorphization):
- Each distinct concrete type instantiation creates a dedicated, specialized typed AST node and backend code block (e.g. `show_item__User`).
- All trait method invocations on monomorphized types resolve directly to direct static calls or inlined functions with zero vtable pointer chasing.
- Violations of trait bounds are detected at compile-time with clear error diagnostics.

### 2.6 Exhaustive Pattern Matching (`match`)

Roze provides industrial-grade pattern matching with deep destructuring, range patterns, and match guards with complete parity to Rust 1.99 and Java 25 pattern matching:

#### Deep Nested Enum Destructuring
Matches variant hierarchies down to any recursive depth:
```roze
enum Color { Red, Green, Blue }
enum Container { Empty, Item(Color), Pair(Color, Color) }

match c {
    Container::Item(Color::Red) => { println("Container holds Red"); }
    Container::Item(other) => { println("Container holds other color"); }
    Container::Pair(Color::Red, Color::Blue) => { println("Container holds Red and Blue pair"); }
    Container::Empty => { println("Container is empty"); }
}
```

#### Struct & Class Destructuring
Deconstructs class fields by name with optional shorthand bindings:
```roze
class Point { x: int, y: int }

match pt {
    Point { x, y } if x == y => { println("Diagonal: " + x); }
    Point { x: px, y: py } if px > py => { println("px > py: " + px); }
    Point { x, y } => { println("Other: " + x + ", " + y); }
}
```

#### Tuple & Slice Patterns
Supports fixed tuples and dynamic slice patterns with `[head, ..tail]` rest binding:
```roze
match items {
    [] => { println("empty slice"); }
    [head, ..tail] => {
        println("head: " + head);
        println("tail length: " + tail.len());
    }
}

match (a, b) {
    (x, y) if x == y => { println("Identical: " + x); }
    (x, y) => { println("Pair: " + x + ", " + y); }
}
```

#### Range Patterns & Match Guards
Ranges (`start..=end` inclusive and `start..end` exclusive) with arbitrary boolean `if <guard>` predicates:
```roze
match score {
    100 => { println("Perfect Score!"); }
    90..=99 if has_honors => { println("High Honors"); }
    90..=99 => { println("Grade A"); }
    80..90 => { println("Grade B"); }
    70..80 => { println("Grade C"); }
    _ => { println("Needs Improvement"); }
}
```

#### Compile-Time Exhaustiveness Guarantees
For enum matches, the semantic analyzer verifies that every variant is covered. Missing variants in the absence of an unconditional wildcard (`_`) or variable catch-all trigger a compile-time type error pointing to the unhandled variants.

### 2.7 Compile-Time Function Execution (`comptime`) & Type Reflection

Roze integrates compile-time function execution and metaprogramming matching Zig `comptime` and Rust `const fn` capabilities. Expressions and blocks marked with `comptime` are evaluated during the compiler semantic analysis pass, completely eliminating runtime calculation overhead for static algorithms, math tables, and data structures.

#### Constant Math Folding
Arithmetic, floating-point operations, boolean logic, and string concatenations are evaluated at compile time:
```roze
let const_int = comptime (100 * 25 + 500 / 2); // folded to 2750
let const_float = comptime (3.14159 * 2.0);    // folded to 6.28318
let const_bool = comptime (10 > 5 && !false);  // folded to true
let const_str = comptime ("Roze " + "v2.0");   // folded to "Roze v2.0"
```

#### Pure Compile-Time Function Execution
User-defined functions with recursion, loops, and local mutation can be executed entirely during compilation:
```roze
func factorial(n: int) -> int {
    if n <= 1 { return 1; }
    return n * factorial(n - 1);
}

let fact7 = comptime factorial(7); // evaluated to 5040 during compilation
```

#### Precomputed Lookup Tables
Lookup tables for games, neural network tensor weights, and OS kernels are generated at compile time:
```roze
func generate_powers_of_two(count: int) -> list {
    let tbl = list_new();
    let p = 1;
    for let i = 0; i < count; i = i + 1 {
        list_push(tbl, p);
        p = p * 2;
    }
    return tbl;
}

let POWERS = comptime generate_powers_of_two(8);
// At runtime, POWERS is pre-populated with [1, 2, 4, 8, 16, 32, 64, 128]
```

#### Compile-Time Blocks (`comptime { ... }`)
Arbitrary statements can be evaluated within compile-time blocks:
```roze
let block_sum = comptime {
    let acc = 0;
    for let i = 1; i <= 20; i = i + 1 {
        acc = acc + i;
    }
    return acc;
}; // evaluated to 210
```

#### Type Layout Reflection (`size_of`, `align_of`, `offset_of`)
Calculates struct byte sizes, natural alignments, and field offsets matching 64-bit ABI conventions:
```roze
class Particle {
    id: int,
    active: bool,
    weight: float
}

let size = comptime size_of<Particle>();        // 24 bytes
let align = comptime align_of<Particle>();      // 8 bytes
let offset_w = comptime offset_of<Particle>("weight"); // byte offset 16
```

---

## 3. Memory Model & Safety

### 3.1 Automatic Reference Counting (ARC)
- Heap-allocated classes and dynamic strings are managed via thread-safe atomic reference counting.
- When an object reference is created, its refcount is initialized to 1.
- Aliasing bindings increment the refcount (`atomic_add(1)`).
- When a variable leaves scope, its destructor decrements the refcount (`atomic_sub(1)`).
- If the refcount reaches 0, the object memory and all child fields are recursively reclaimed.

### 3.2 Unsafe Blocks & Raw Pointers
Direct memory access is quarantined within `unsafe` blocks:
```roze
unsafe {
    let ptr = alloc(64);
    write_bytes(ptr, 0x42, 64);
    let val = read_bytes(ptr, 0);
    free_ptr(ptr);
}
```

### 3.3 Freestanding Bare-Metal Mode
With `--target freestanding`, Roze compiles without libc, glibc, JVM, or OS dependencies:
- Multiboot2 compliant header for GRUB bootloader.
- Direct port I/O (`inb`, `outb`).
- VGA text buffer access (`vga_print`, `vga_write_char`, `vga_set_color`).

### 3.4 Modern Concurrency: M:N Green Threads (Virtual Threads)
Roze implements a unified high-throughput concurrency runtime with parity to Java 25 Project Loom Virtual Threads and Go goroutines:
- **M:N Cooperative Work-Stealing Scheduler**: User-space green threads (`Fiber`) multiplexed across all CPU carrier worker threads.
- **Zero-Allocation Fiber Lifecycle**: Reusable slab-allocated fiber pools eliminate heap allocations during high-frequency fiber spawning.
- **Cooperative Context Switching**: Fibers yield (`fiber_yield()`) and join (`fiber_join()`) without blocking carrier OS worker threads.
- **Massive Scalability**: Supports hundreds of thousands to 1,000,000+ simultaneous concurrent green threads with sub-millisecond scheduling overhead.
- **Backend Mapping**:
  - **JVM Backend**: Maps to Java 25 Virtual Threads (`Thread.ofVirtual().start(...)`) running on Loom carrier `ForkJoinPool`.
  - **Native Backend**: Multi-core work-stealing scheduler with lock-free/deques, Chase-Lev stealing, and batch task migration.
- **Key Primitives**:
  - `fiber_spawn(worker: func() -> T) -> Fiber<T>`
  - `fiber_join(fiber: Fiber<T>) -> T`
  - `fiber_yield() -> void`
  - `fiber_sleep(ms: int) -> void`
  - `fiber_id() -> int`
  - `fiber_count() -> int`
  - `fiber_stats() -> void`
  - `channel_new(capacity: int) -> Channel<T>`, `send(chan, val)`, `recv(chan)`

```roze
import "concurrency";

func main() {
    let ch = channel_new(1024);

    // Spawn 10,000 cooperative green threads
    for let i = 0; i < 10000; i = i + 1 {
        fiber_spawn(|| {
            fiber_yield();
            send(ch, 1);
            return 0;
        });
    }

    let total = 0;
    for let i = 0; i < 10000; i = i + 1 {
        let v: int = recv(ch);
        total = total + v;
    }
    println(total); // 10000
}
```

### 3.5 Async / Await State-Machine Engine (`Future<T>`)
Roze provides first-class asynchronous programming with parity to Rust async/await and modern Java 25 CompletableFuture / Structured Concurrency:
- **Zero-Allocation State Machine Lowering**: `async func` declarations and `await` expressions are lowered by the compiler semantic pass into asynchronous tasks and state machines with reusable slab pools.
- **Top-Level Await**: `await <expr>` expressions are valid both within async functions and at top-level entrypoints (`main()`).
- **Native Polling Contract**:
  - `await fut` -> $T$: cooperatively awaits completion of a `Future<T>`, resuming with the resolved value.
  - `future_poll(fut: Future<T>) -> bool`: non-blocking poll returning `true` if the future has completed.
  - `future_ready(val: T) -> Future<T>`: creates an immediately completed future with zero context switch.
  - `async_sleep(ms: int) -> Future<int>`: non-blocking timer future integrated with the reactor.
- **Backend Mapping**:
  - **JVM Backend**: Maps to Java 25 `CompletableFuture<Object>` running on Loom Virtual Threads (`Thread.ofVirtual().start(...)`).
  - **Native Backend**: Slab-allocated `RozeFuture` structures with atomic states (`PENDING`, `READY`), mutex/cond signaling, fast-path resolution, and reactor timer integration.

```roze
async func compute(x: int) -> int {
    return x * 2;
}

async func delayed_task(val: int) -> int {
    await async_sleep(10);
    return val + 100;
}

func main() {
    let f = compute(21);
    let is_done = future_poll(f);
    let res = await f;
    println(res); // 42

    let delayed = await delayed_task(50);
    println(delayed); // 150
}
```

### 3.6 Synchronization: Lock-Free Primitives & Channels
Roze implements low-overhead lock-free synchronization primitives, mutual exclusion, and high-throughput MPMC ring-buffer channels matching Rust `std::sync` / `crossbeam-channel` and Java `java.util.concurrent.atomic` / `locks`:

- **Atomic Types**:
  - `AtomicBool`: Lock-free 64-bit aligned atomic boolean with sequential consistency.
  - `AtomicI64`: Lock-free atomic 64-bit integer supporting `load`, `store`, `swap`, `cas`, `add`, `sub`, and atomic bitwise `and`, `or`, `xor`.
  - `AtomicUsize`: Pointer-sized atomic integer for indexes and allocation counters.
  - `AtomicPtr<T>`: Atomic memory address and object reference supporting lock-free pointer manipulation.
- **Mutual Exclusion (`Mutex`)**:
  - Fast-path uncontended acquisition via atomic test-and-set and OS condition/futex fallback.
  - Operations: `mutex_new()`, `mutex_lock()`, `mutex_unlock()`, `mutex_try_lock()`.
  - Supports both procedural and method-call syntax: `m.lock()`, `m.unlock()`, `m.try_lock()`.
- **Reader-Writer Lock (`RwLock`)**:
  - Multiple concurrent readers with single exclusive writer semantics.
  - Operations: `rwlock_new()`, `rwlock_read_lock()`, `rwlock_read_unlock()`, `rwlock_write_lock()`, `rwlock_write_unlock()`, `rwlock_try_read_lock()`, `rwlock_try_write_lock()`.
- **High-Throughput MPMC Channels**:
  - Bounded ring-buffer channels: FIFO blocking queue with fixed buffer capacity.
  - Unbounded channels: Lock-free dynamic buffer growth avoiding deadlocks and eliminating backpressure blocking on senders.
  - Operations: `channel_new(cap)`, `channel_unbounded()`, `channel_send()`, `channel_try_send()`, `channel_recv()`, `channel_try_recv()`, `channel_close()`, `channel_is_closed()`, `channel_len()`, `channel_capacity()`.
- **Backend Mapping**:
  - **JVM Backend**: Maps to `java.util.concurrent.atomic` (`AtomicBoolean`, `AtomicLong`, `AtomicReference`), `ReentrantLock`, `ReentrantReadWriteLock`, and thread-safe MPMC `RozeChannel`.
  - **Native Backend**: C11 `_Atomic` sequentially consistent primitives, `pthread_mutex_t`, `pthread_rwlock_t`, and dynamic circular ring-buffer with condition signaling.

```roze
import "sync";

func main() {
    let atomic_val = atomic_i64_new(10);
    atomic_val.add(5); // 15
    atomic_val.cas(15, 42);

    let m = mutex_new();
    m.lock();
    // Critical section
    m.unlock();

    let uch = channel_unbounded();
    uch.send(100);
    let item: int = uch.recv();
}
```

### 3.7 Structured Concurrency: Nursery, Task Groups & Cancelation
Roze provides full-featured Structured Concurrency matching Java 25 JEP 453 `StructuredTaskScope` and Rust nursery runtimes (`TaskGroup` / `Nursery`), ensuring that child tasks are lifetime-bounded by their parent scope:

- **Nursery / TaskGroup Scope**:
  - `task_group_new()` / `nursery_new()`: Default scope awaiting all spawned child tasks.
  - `tg.fork(|| ...)` / `tg.spawn(|| ...)`: Spawns a concurrent child subtask and returns a `Subtask<T>` handle.
  - `tg.join()` / `tg.wait()`: Awaits task completion according to the configured scope policy.
  - Guarantees zero orphaned child tasks or thread leaks: scope exit waits for all spawned workers.
- **Policies & Error Handling**:
  - `task_group_on_failure()` / `nursery_on_failure()` (ShutdownOnFailure): If any child task fails or cancels, the remaining sibling tasks are automatically cancelled immediately.
  - `task_group_on_success()` / `nursery_on_success()` (ShutdownOnSuccess): Speculative race execution where the first child task to succeed sets the scope result and automatically cancels all remaining sibling tasks.
  - `task_group_with_timeout(ms)` / `nursery_with_timeout(ms)`: Deadline-triggered scope that automatically cancels all subtasks upon timeout expiration.
- **Subtask Handles (`Subtask<T>`)**:
  - `subtask.get()`: Blocks cooperatively until the subtask finishes and extracts its resolved value.
  - `subtask.is_done()`: Returns `true` if the subtask has completed (succeeded, failed, or cancelled).
  - `subtask.is_cancelled()`: Returns `true` if the subtask was cancelled before completion.
- **Unified CancellationToken**:
  - `cancel_token_new()`: Instantiates a cooperatively pollable cancellation token.
  - `tok.cancel()`: Triggers cancellation and broadcasts state change.
  - `tok.is_cancelled()`: Checks cancellation status in long-running loops or nested child nurseries.
  - `tg.token()`: Accesses the cancellation token embedded within a TaskGroup.
- **Backend Mapping**:
  - **JVM Backend**: Implemented via Project Loom virtual threads, `java.util.concurrent.CompletableFuture`, `AtomicInteger`, `AtomicBoolean`, and condition-variable task synchronizers (`RozeTaskGroup`, `RozeSubtask`, `RozeCancellationToken`).
  - **Native Backend**: Implemented via POSIX threads, atomic state flags, timeout watcher threads, and unified ARC retain/release (`RozeTaskGroup`, `RozeSubtask`, `RozeCancellationToken`).

```roze
import "concurrency";

func main() {
    let tg = task_group_new();
    let s1 = tg.fork(|| 10);
    let s2 = tg.fork(|| 20);
    tg.join();
    println(s1.get() + s2.get()); // 30

    // Speculative execution
    let spec = task_group_on_success();
    spec.fork(|| 999);
    let first = spec.join();
    println(first); // 999
}
```

### 3.8 Milestone Gate M1: Integration & Concurrency Benchmark
Milestone Gate M1 serves as the comprehensive integration checkpoint for Month 1 (Sprint 1 & Sprint 2: Core Type System, Advanced Generics, Pattern Matching, Metaprogramming, M:N Green Threads, Async/Await, Lock-Free Synchronization, and Structured Concurrency).

- **100,000 Concurrent Ping-Pong Message Benchmark**:
  - Validates high-throughput message exchange over MPMC channels between M:N green threads (Native work-stealing pool) and Virtual Threads (JVM Loom).
  - Sustains 100,000 loss-free round-trip messages across bounded/unbounded channel buffers with sub-2-second native latency and zero-allocation fiber recycling.
- **Monomorphized Generic Data Structures Stress Suite**:
  - Exercises deeply monomorphized generic structures including multi-parameter pairs (`Pair<A, B>`), nested containers (`Box<Pair<T1, T2>>`), and recursive binary trees (`TreeNode<T>`).
  - Verifies compile-time type substitution, zero-cost specialized memory layouts, and automatic ARC reference counting across both Native and JVM backends.
- **Backend Parity & Fixpoint Verification**:
  - 100% test pass rate across all unit tests and golden integration suites on both JVM and Native Cranelift backends.
  - Three-stage self-hosting compiler bootstrap (`Stage 0 (Rust) -> Stage 1 (Roze) -> Stage 2 (Roze) -> Stage 3 (Roze)`) achieves a bit-for-bit, 0-diff fixpoint verification proof.

---

## 4. Error Handling (`Result`, `Option`, `?`)

Roze replaces unchecked exceptions with explicit sum types:
```roze
func divide(a: int, b: int) -> Result<int, string> {
    if b == 0 {
        return Err("division by zero");
    }
    return Ok(a / b);
}

func calculate() -> Result<int, string> {
    let result = divide(100, 2)?; // Unwraps Ok(50) or returns Err immediately
    return Ok(result + 10);
}
```

---

## 5. Foreign Function Interface (C-ABI FFI)

Roze seamlessly binds existing C, C++, and system libraries via `extern "C"`:
```roze
extern "C" func sin(x: float) -> float;
extern "C" func printf(fmt: string, val: int) -> int;

let y = sin(3.14159);
```

---

## 6. Standard Library Architecture (`stdlib/`)

| Module | Namespace | Capabilities |
| :--- | :--- | :--- |
| **Core** | `core.roze` | Math functions (`abs`, `min`, `max`, `clamp`), string utilities |
| **Collections**| `collections.roze` | Dynamic `List<T>`, Hash `Map<K, V>`, iterators |
| **IO & Files** | `io.roze` | `read_file`, `write_file`, `append_file`, path utilities |
| **Networking** | `web.roze` | HTTP client (`http_get`, `http_post`), multithreaded HTTP server |
| **Database** | `sql.roze` | JDBC connection pooling, query builder, SQL transactions |
| **Threads** | `threading.roze` | OS threads (`thread_spawn`), thread-safe MPMC channels |
| **AI / ML** | `ai.roze` | Tensor multidimensional arrays, activations, inference engine |

---

## 7. Toolchain & Ecosystem

- **`roze`**: Compiler driver supporting `--target native`, `--target jvm`, and `--target freestanding`.
- **`roze-pkg`**: Package manager and dependency resolver supporting semver (`^`, `~`, exact), deterministic lockfile, and package publishing.
- **`roze-lsp`**: Language Server Protocol implementation providing diagnostics, symbol completion, and formatting.
- **`roze-build`**: Build orchestration and incremental workspace compiler.
