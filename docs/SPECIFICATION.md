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
