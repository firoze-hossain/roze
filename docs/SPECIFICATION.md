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
