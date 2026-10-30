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

### 2.1 Primitive & Numeric Types
Roze features a comprehensive, fixed-width numeric suite with explicit signedness, bit-width guarantees, and character representation matching Rust and modern low-level systems languages:

| Type | Representation | Description |
| :--- | :--- | :--- |
| `i8` | 8-bit signed integer | Value range: `[-128, 127]` |
| `u8` | 8-bit unsigned integer | Value range: `[0, 255]`, raw byte |
| `i16` | 16-bit signed integer | Value range: `[-32,768, 32,767]` |
| `u16` | 16-bit unsigned integer | Value range: `[0, 65,535]` |
| `i32` | 32-bit signed integer | Value range: `[-2,147,483,648, 2,147,483,647]` |
| `u32` | 32-bit unsigned integer | Value range: `[0, 4,294,967,295]` |
| `i64` / `int` | 64-bit signed integer | Value range: `[-9,223,372,036,854,775,808, 9,223,372,036,854,775,807]`. Default integer type. |
| `u64` | 64-bit unsigned integer | Value range: `[0, 18,446,744,073,709,551,615]` |
| `i128` | 128-bit signed integer | Value range: `[-2^127, 2^127 - 1]` |
| `u128` | 128-bit unsigned integer | Value range: `[0, 2^128 - 1]` |
| `isize` | Pointer-width signed int | 64-bit signed offset on 64-bit targets |
| `usize` | Pointer-width unsigned int | 64-bit unsigned pointer index on 64-bit targets |
| `f32` | 32-bit IEEE 754 | Single-precision floating point |
| `f64` / `float` | 64-bit IEEE 754 | Double-precision floating point. Default float type. |
| `char` | 32-bit Unicode scalar | Character literal (`'a'`, `'\n'`, `'\t'`, `'\\'`, `'\''`) |
| `bool` | 1-bit boolean | `true` or `false` |
| `string` | UTF-8 ARC string | Immutable refcounted string buffer |
| `void` | Unit type | Function without return value |
| `rawpointer` | 64-bit address | Unmanaged memory address |

#### Numeric Literals with Type Suffixes
Literals can be explicitly annotated with their target primitive type using suffixes:
```roze
let b: u8 = 255u8;
let s: i16 = -1000i16;
let w: u32 = 4000000000u32;
let big: i128 = 170141183460469231731687303715884105727i128;
let pi32: f32 = 3.14159f32;
let ch: char = 'R';
```

#### Explicit Type Conversions & Casting
Explicit conversions between numeric types are invoked using type constructor syntax:
```roze
let x: int = 120;
let y: u8 = u8(x);
let z: f64 = f64(y);
let c: char = char(65); // 'A'
let code: int = int(c); // 65
```

#### Numeric Type Bounds Intrinsics
Type minimum and maximum limits are accessible via built-in bound intrinsics:
- `i8_min()`, `i8_max()`, `u8_min()`, `u8_max()`
- `i16_min()`, `i16_max()`, `u16_min()`, `u16_max()`
- `i32_min()`, `i32_max()`, `u32_min()`, `u32_max()`
- `i64_min()`, `i64_max()`, `u64_min()`, `u64_max()`
- `i128_min()`, `i128_max()`, `u128_min()`, `u128_max()`
- `isize_min()`, `isize_max()`, `usize_min()`, `usize_max()`
- `f32_min()`, `f32_max()`, `f64_min()`, `f64_max()`

#### Wrapping and Saturating Arithmetic
For systems programming, cryptography, and game development where overflow semantics must be explicitly controlled, Roze provides standard wrapping and saturating operations:
```roze
// Wrapping arithmetic (two's complement wrap-around)
let max_u8: u8 = 255u8;
let wrapped: u8 = max_u8.wrapping_add(1u8); // 0u8
let wrapped_sub: u8 = (0u8).wrapping_sub(1u8); // 255u8

// Saturating arithmetic (clamps at type boundaries)
let clamped_add: u8 = (250u8).saturating_add(20u8); // 255u8
let clamped_sub: u8 = (5u8).saturating_sub(10u8); // 0u8
let clamped_mul: i8 = (100i8).saturating_mul(2i8); // 127i8
```
Both method syntax (`a.wrapping_add(b)`) and procedural syntax (`wrapping_add(a, b)`) are supported across all fixed-width integer types (`i8` through `u128`, `isize`, `usize`).

### 2.2 Hardware Acceleration: SIMD Vector Intrinsics Engine
Roze includes first-class hardware SIMD vector types and an intrinsics execution engine offering 8x–16x data-parallel acceleration for AI/ML tensor workloads, game physics, audio/video processing, and high-performance computing:

| SIMD Type | Width | Lane Layout | Target Instruction Sets |
| :--- | :--- | :--- | :--- |
| `Vec4f` | 128-bit | 4x 32-bit single-precision floats (`f32`) | SSE4.1, AVX2, ARM NEON |
| `Vec8f` | 256-bit | 8x 32-bit single-precision floats (`f32`) | AVX, AVX2, AVX-512, ARM SVE/NEON |
| `Vec4i` | 128-bit | 4x 32-bit signed integers (`i32`) | SSE4.1, AVX2, ARM NEON |
| `Vec16u8` | 128-bit | 16x 8-bit unsigned bytes (`u8`) | SSSE3, AVX2, ARM NEON |

#### Construction & Splatting
Vectors can be constructed component-wise, splatted from scalars, or zero-initialized:
```roze
// Component-wise construction
let v4 = Vec4f(1.0, 2.0, 3.0, 4.0);
let v8 = Vec8f(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0);
let vi = Vec4i(10, 20, 30, 40);
let vu = Vec16u8_splat(200);

// Splatting and zeroing
let splat4 = Vec4f_splat(3.5);
let zero4 = Vec4f_zero();
```

#### Vector Arithmetic & Operators
Standard operators (`+`, `-`, `*`, `/`) work symmetrically for vector-vector and vector-scalar operations:
```roze
let v_sum = v1 + v2;
let v_diff = v2 - v1;
let v_prod = v1 * v2;
let v_quot = v1 / v2;

// Vector-scalar broadcasting
let v_scaled = v1 * 2.0;
let v_offset = v1 + 10.0;
```

#### Vector Math Intrinsics
Hardware-accelerated mathematical intrinsics:
- **Dot Product & Reduction**: `v1.dot(v2)` (dot product scalar), `v1.sum()` (horizontal sum reduction)
- **Component-wise Min/Max**: `v1.min(v2)`, `v1.max(v2)`
- **Square Root & Absolute Value**: `v.sqrt()`, `v.abs()`
- **Fused Multiply-Add (FMA)**: `a.fma(b, c)` computing `(a * b) + c` in a single hardware instruction
- **Saturating Arithmetic on `Vec16u8`**: `vu1.saturating_add(vu2)`, `vu1.saturating_sub(vu2)` clamping to `[0, 255]`
- **Lane Access**: `v.get(idx)`, `v.set(idx, val)` (pure, returns modified vector)

#### Direct Memory Load & Store
Unmanaged pointer interactions for zero-copy bulk tensor memory streaming:
```roze
unsafe {
    let ptr = alloc(64);
    vec4f_store(ptr, v1);
    let loaded: Vec4f = vec4f_load(ptr);
    free(ptr);
}
```

### 2.3 Standard Collections: Advanced High-Performance Collections
Roze provides a comprehensive standard library of 7 high-performance, specialized collections achieving amortized O(1) performance where applicable, matching Rust `std::collections` and the Java 25 `java.util` hierarchy with 100% behavioral parity across both JVM and Native Cranelift backends:

| Collection | Internal Architecture | Time Complexity | Primary Capabilities |
| :--- | :--- | :--- | :--- |
| `VecDeque<T>` | Contiguous circular ring buffer with geometric 2x growth | Amortized O(1) push/pop both ends, O(1) indexed lookup | `push_front`, `push_back`, `pop_front`, `pop_back`, `front`, `back`, `get`, `set`, `len`, `is_empty`, `clear` |
| `BTreeMap<K, V>` | Self-balancing ordered AVL binary search tree | O(log N) lookup, insertion, and deletion; sorted order | `insert`, `get`, `contains_key`, `remove`, `first_key`, `last_key`, `keys`, `values`, `len`, `is_empty`, `clear` |
| `BTreeSet<T>` | Unique sorted set wrapping AVL tree | O(log N) membership and insertion; ascending traversal | `insert`, `contains`, `remove`, `first`, `last`, `to_list`, `len`, `is_empty`, `clear` |
| `BinaryHeap<T>` | Max-heap binary array with sift-up and sift-down | O(1) peek, O(log N) push and pop; priority queue | `push`, `pop`, `peek`, `len`, `is_empty`, `clear` |
| `IndexMap<K, V>` | Insertion-ordered hash map with dense array and hash index | Amortized O(1) key lookup and O(1) indexed access | `insert`, `get`, `get_index_key`, `get_index_value`, `contains_key`, `remove`, `keys`, `values`, `len`, `is_empty`, `clear` |
| `RingBuffer<T>` | Fixed-capacity circular bounded ring buffer | O(1) non-allocating push/pop with overwrite semantics | `push`, `push_overwrite`, `pop`, `peek`, `capacity`, `is_full`, `is_empty`, `len`, `clear` |
| `BitSet` | Dense 64-bit word array with SIMD popcount (`__builtin_popcountll`) | O(1) bit mutation and testing, O(W) bitwise set algebra | `set`, `clear_bit`, `test`, `flip`, `count_ones`, `union_with`, `intersect_with`, `difference_with`, `len`, `is_empty`, `clear` |

#### Dual Method & Procedural APIs
Collections support both object-oriented method calling syntax and procedural standard library functions:
```roze
// Object-oriented method calls
let dq = new VecDeque<int>();
dq.push_back(10);
dq.push_front(5);
let first = dq.pop_front(); // 5

// Procedural standard library calls
let m = new BTreeMap<int, string>();
btree_map_insert(m, 1, "one");
let has_key = btree_map_contains_key(m, 1); // true
```

### 2.4 Algebraic Data Types (Enums)
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
With `--target freestanding`, Roze compiles standalone operating system kernels and embedded binaries without libc, glibc, JVM, or OS dependencies (parity with Rust `#![no_std]` and Zig freestanding target):

#### Zero-Libc & Zero-Heap Execution
- **Zero-Libc**: Generates static ELF64 binaries with 0 dynamic sections and 0 undefined symbols.
- **Zero-Heap Mode (`--no-heap`)**:
  - Completely eliminates the 1MB BSS heap pool (`__roze_freestanding_heap_pool`), reducing memory footprint for memory-constrained microcontrollers and early boot stages.
  - Compile-time rejection of dynamic heap allocations (`new Class()` and `alloc(...)` are disallowed at compile time).
  - Runtime queries: `freestanding_is_heap_enabled() -> bool` (returns `false` under `--no-heap`) and `freestanding_get_heap_capacity() -> int` (returns `0`).

#### Custom Linker Script Integration (`-T / --linker-script`)
- Integrates custom GNU linker scripts to define arbitrary memory layouts, physical/virtual address spaces, and section alignments (e.g., loading at 1MB, 2MB, or higher-half kernel addresses).
- Structured ELF section output:
  - `.boot`: Multiboot 1 and Multiboot 2 headers with 32-bit protected mode trampoline.
  - `.text`: 4KB page-aligned kernel code.
  - `.rodata`: Read-only string literals and constants.
  - `.data`: Initialized kernel data structures.
  - `.bss`: Uninitialized zeroed memory.

#### Freestanding Runtime Entry (`_start` & `--entry`)
- **Default Multiboot Trampoline**:
  - Multiboot 1 (`0x1BADB002`) and Multiboot 2 (`0xE85250D6`) boot headers compatible with GRUB and QEMU `-kernel`.
  - Sets up initial 32-bit stack, builds 1GB identity page tables with 2MB huge pages (PML4, PDPT, PD).
  - Enables PAE (CR4), Long Mode (EFER), and Paging (CR0).
  - Loads 64-bit GDT (`gdt64`) and executes far jump to 64-bit Long Mode.
  - Initializes 64-bit segment registers, switches to 64-bit stack (`stack_top`), initializes COM1 UART and VGA text buffer, and calls `main` (`__roze_main`).
  - Halts with `cli; hlt` upon kernel return.
- **Direct Custom Entry**:
  - Defining `func _start()` in Roze overrides the default weak `_start` symbol with `Linkage::Export`.
  - Specifying `--entry <symbol>` exports `<symbol>` with `Linkage::Export` and instructs the linker to set it as the ELF entry point address.

#### Disabling Float Registers for ISR Handlers
- **ISR Safety**:
  - x86_64 hardware interrupts do not preserve FPU/SSE/AVX registers (`xmm0`-`xmm15`) on the interrupt stack. Using float or SIMD operations inside an Interrupt Service Routine (ISR) silently corrupts interrupted thread state.
  - Roze disables the 128-byte red zone (`-mno-red-zone`) to prevent asynchronous interrupt frames from clobbering temporary stack variables.
  - Functions named `isr_*` or `interrupt_*` are validated at compile time: any use of `f32`, `f64`, `float`, SIMD vector types (`Vec4f`, `Vec8f`), float literals, or SIMD intrinsics is rejected with a compiler error.
  - CLI flag `--disable-float-registers` (or `--no-float`): compiles the entire freestanding binary and runtime with `-mno-red-zone -mno-sse -mno-sse2 -mno-mmx -mno-80387 -DROZE_NO_FLOAT`, disabling all float registers.
  - Query: `freestanding_is_float_enabled() -> bool` returns `false` when float registers are disabled.

#### Built-in Hardware Intrinsics (`import "freestanding";`)
- `multiboot2_get_magic() -> int`: Returns multiboot protocol magic value (`0x36D76289`).
- `multiboot2_get_info_ptr() -> *raw u8`: Pointer to multiboot information structure tags.
- `outb(port: int, val: int)` / `inb(port: int) -> int`: Hardware I/O port read/write.
- `serial_print(s: string)` / `serial_putchar(ch: int)`: 115200 8N1 COM1 serial UART driver.
- `freestanding_halt()`: Enters low-power CPU halt state (`cli; hlt`).
- `qemu_exit(code: int)`: Signals QEMU exit device (`isa-debug-exit`, port `0xf4`).

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

### 3.9 Memory Architecture: Pluggable Allocator API & Smart Pointers

Roze provides pluggable memory allocators and high-level smart pointers matching Rust's `alloc` and C++ polymorphic memory allocators (PMR):

#### Smart Pointers
- **`Box<T>`**: Unique ownership heap pointer.
  - `new_box(value: T) -> Box<T>`: Allocates and boxes a value on the heap.
  - `b.get() -> T`: Dereferences the boxed value.
  - `b.set(val: T)`: Mutates the boxed value.
  - `b.into_raw() -> int`: Consumes the box and yields the raw heap pointer address.
  - `new_box_from_raw(raw: int) -> Box<T>`: Reconstitutes a `Box<T>` from a raw pointer address.
- **`Arc<T>`**: Thread-safe atomically reference-counted pointer.
  - `new_arc(value: T) -> Arc<T>`: Allocates an atomically reference-counted object with a unified control block.
  - `a.get() -> T`: Retrieves the underlying value.
  - `a.clone() -> Arc<T>`: Increments the strong reference count atomically and returns a new `Arc<T>` reference.
  - `a.strong_count() -> int`: Returns the current strong reference count.
  - `a.weak_count() -> int`: Returns the current weak reference count.
  - `a.downgrade() -> Weak<T>`: Creates a non-owning `Weak<T>` pointer referencing the control block.
- **`Weak<T>`**: Non-owning weak pointer for cyclic reference avoidance and cache eviction.
  - `w.upgrade() -> Option<Arc<T>>`: Upgrades to `Some(Arc<T>)` if strong count > 0, or `None` if expired.
  - `w.strong_count() -> int`: Returns the strong reference count of the target.
  - `w.weak_count() -> int`: Returns the weak reference count of the target.
  - `w.is_expired() -> bool`: Returns true if the referenced value has been dropped (`strong_count == 0`).
  - `w.is_alive() -> bool`: Returns true if the referenced value is still active (`strong_count > 0`).

#### Pluggable Allocator API
Allocators implement the core `Allocator` trait defined in `stdlib/src/memory.roze`:
- **`BumpAllocator`**: High-performance linear bump allocator with monotonic pointer bump and bulk reset.
  - `new_bump_allocator(capacity: int) -> BumpAllocator`: Initializes a contiguous memory arena of given capacity.
  - `alloc(size: int) -> int`: Allocates `size` bytes linearly; returns raw offset address or `0` on out-of-memory.
  - `alloc_aligned(size: int, align: int) -> int`: Allocates with power-of-two alignment padding.
  - `reset()`: O(1) bulk reset that resets the bump pointer to 0 without freeing the underlying arena.
  - Metrics: `allocated_bytes()`, `remaining_bytes()`, `capacity()`, `is_empty()`.
- **`ArenaAllocator`**: Chunked arena allocator that allocates fixed-size memory blocks dynamically.
  - `new_arena_allocator(chunk_size: int) -> ArenaAllocator`: Initializes an arena with default chunk size.
  - `alloc(size: int) -> int` / `alloc_aligned(size: int, align: int) -> int`: Allocates from current chunk or links a new chunk.
  - `reset()`: Resets all active chunks to empty without deallocating OS memory.
  - `clear()`: Deallocates all chunks back to the global heap.
  - Metrics: `allocated_bytes()`, `chunk_count()`.
- **`PoolAllocator`**: Fixed-size block pool allocator with O(1) free-list recycling.
  - `new_pool_allocator(block_size: int, capacity: int) -> PoolAllocator`: Pre-allocates a fixed pool of uniform blocks.
  - `alloc() -> int`: Pops a block from the singly-linked free-list in O(1) time; returns 0 if exhausted.
  - `dealloc(ptr: int)`: Pushes a returned block address back onto the free-list in O(1) time.
  - `reset()`: Re-links all blocks into the free-list.
  - Metrics: `block_size()`, `capacity()`, `available_blocks()`, `is_full()`, `is_empty()`.

#### Memory Alignment Utilities
- `align_to(value: int, align: int) -> int` / `align_address(addr: int, align: int) -> int`: Computes power-of-two address alignment using bitwise formula `(val + align - 1) & ~(align - 1)`.
- `cache_line_size() -> int` / `get_cache_line_size() -> int`: Returns standard CPU cache-line size (64 bytes).

---

### 3.10 Kernel Memory: 4-Level Paging & Virtual Memory Manager

Roze provides a complete, low-level OS memory management stack implemented in pure Roze (`import "paging";` or `import "vmm";`):

#### 4-Level x86_64 Paging Architecture
Canonical 48-bit virtual addresses are decomposed across 4 hierarchical levels:
- **PML4** (Page Map Level 4): bits 47..39 (Index 0..511)
- **PDPT** (Page Directory Pointer Table): bits 38..30 (Index 0..511)
- **PD** (Page Directory): bits 29..21 (Index 0..511)
- **PT** (Page Table): bits 20..12 (Index 0..511)
- **Page Offset**: bits 11..0 (0..4095 for 4KB standard pages; bits 20..0 for 2MB huge pages)

#### Page Table Entry (PTE) Flags
```roze
const PAGE_PRESENT: int       = 1;          // Bit 0: Page present in memory
const PAGE_WRITABLE: int      = 2;          // Bit 1: Read/write permissions (1 = writable)
const PAGE_USER: int          = 4;          // Bit 2: User-mode accessible (0 = supervisor only)
const PAGE_WRITE_THROUGH: int = 8;          // Bit 3: Write-through caching
const PAGE_NO_CACHE: int      = 16;         // Bit 4: Cache disable
const PAGE_ACCESSED: int      = 32;         // Bit 5: Accessed by CPU read/write
const PAGE_DIRTY: int         = 64;         // Bit 6: Written to (dirty)
const PAGE_HUGE: int          = 128;        // Bit 7: 2MB huge page (in PDE) or 1GB huge page (in PDPTE)
const PAGE_GLOBAL: int        = 256;        // Bit 8: Global TLB pin across CR3 reloads
const PAGE_NX: int            = 0x80000000; // Bit 63: No-Execute bit
```

#### Page Mapping & Translation Operations
- `vmm_map_page(pml4_root: int, vaddr: int, paddr: int, flags: int, frame_alloc_fn: func() -> int) -> int`: Traverses/allocates intermediate tables and maps a 4KB virtual page to a physical frame.
- `vmm_map_huge_page(pml4_root: int, vaddr: int, paddr: int, flags: int, frame_alloc_fn: func() -> int) -> int`: Maps a contiguous 2MB huge page directly in the Page Directory entry (`PAGE_HUGE`).
- `vmm_unmap_page(pml4_root: int, vaddr: int) -> int`: Clears PTE and invalidates TLB entry via `invlpg`.
- `vmm_unmap_huge_page(pml4_root: int, vaddr: int) -> int`: Clears PDE huge entry and invalidates TLB.
- `vmm_translate(pml4_root: int, vaddr: int) -> int`: Walks page tables returning physical address or -1 if unmapped.
- `vmm_get_page_flags(pml4_root: int, vaddr: int) -> int` / `vmm_set_page_flags(...) -> int`: Inspects and updates entry flags.
- `vmm_identity_map_range(pml4_root: int, start: int, end: int, flags: int, alloc_fn) -> int`: Maps `[start, end)` where virtual address equals physical address.

#### Physical Frame Allocator (Bitmap Allocator)
- `BitmapAllocator`: Manages frame allocation across physical memory via bit arrays:
  - `alloc() -> int`: Finds next free bit, marks it used, and returns 4KB-aligned frame base address.
  - `dealloc(frame_addr: int)`: Clears bit corresponding to physical frame.
  - `reserve(paddr: int, count: int)`: Marks frame range as permanently reserved (e.g. kernel code, ACPI, BIOS).
  - Metrics: `total_frames()`, `used_frames()`, `free_frames()`, `is_frame_used(paddr: int) -> bool`.

#### Physical Frame Allocator (Buddy Allocator)
- `BuddyAllocator`: Binary buddy system managing power-of-two frame blocks (orders 0..`max_order`):
  - `alloc(order: int) -> int`: Allocates a block of $2^{\text{order}}$ frames (Order 0 = 4KB, Order 9 = 2MB huge page), recursively splitting higher-order blocks as required.
  - `dealloc(addr: int, order: int)`: Returns block to free list, recursively coalescing adjacent free buddy pairs up to `max_order`.
  - `alloc_huge_page() -> int` / `free_huge_page(addr: int)`: Dedicated Order 9 (2MB) helpers.

#### Hardware TLB & Control Register Intrinsics
- `invlpg(vaddr: int)` / `tlb_invalidate_page(vaddr: int)`: Invalidates TLB translation for a single page using `invlpg [vaddr]`.
- `tlb_flush_all()`: Full TLB flush by reloading `CR3`.
- `read_cr3() -> int`: Reads current page table root address from `CR3`.
- `write_cr3(val: int)`: Writes new PML4 root address into `CR3`.

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
