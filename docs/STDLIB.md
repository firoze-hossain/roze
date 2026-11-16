# 🌹 Roze Standard Library Reference Manual (v2.0.0 'Prometheus')

The **Roze Standard Library** provides zero-overhead, memory-safe, portable systems programming abstractions across 35 standard library modules and 5 reference ecosystem packages.

---

## Table of Contents

1. [Architecture & Design Principles](#1-architecture--design-principles)
2. [Module Index](#2-module-index)
3. [Language Core & Algorithms](#3-language-core--algorithms)
   - `core` • `collections` • `math` • `ptr` • `memory` • `libc`
4. [Concurrency & Synchronization](#4-concurrency--synchronization)
   - `concurrency` • `sync`
5. [I/O, File System & Process Management](#5-io-file-system--process-management)
   - `io` • `fs` • `process` • `env` • `time`
6. [Data Serialization, Text & Reflection](#6-data-serialization-text--reflection)
   - `serde` • `regex` • `reflect`
7. [Enterprise Networking & Security](#7-enterprise-networking--security)
   - `web` • `enterprise_web` • `raw_net` • `crypto` • `cloud_security` • `binary` • `forensics`
8. [Database & Persistence](#8-database--persistence)
   - `sql` • `sqlite` • `orm` • `rozeboot`
9. [Hardware, IoT & Low-Level Systems](#9-hardware-iot--low-level-systems)
   - `embedded_hal` • `freestanding` • `vga` • `paging` • `interrupts` • `drivers` • `ffi`
10. [Reference Ecosystem Packages](#10-reference-ecosystem-packages)
    - `rozeboot` • `roze-orm` • `roze-torch` • `roze-game` • `roze-ui`

---

## 1. Architecture & Design Principles

- **Zero-Cost Abstractions**: Built with standard C-ABI interoperability and Cranelift SSA IR generation. High-level idioms compile down to bare-metal performance.
- **Deterministic Memory Model**: Pure Atomic Reference Counting (ARC) with zero stop-the-world pauses. Deterministic destructors fire immediately when references hit zero.
- **Universal Multi-Target Runtime**: Identical standard library API semantics execute seamlessly across:
  1. Standalone Native ELF / Mach-O / PE binaries (`--target native`).
  2. Enterprise JVM Bytecode (`--target jvm`).
  3. WebAssembly & WASI Serverless (`--target wasm32-wasi`).
  4. ARM Cortex-M & RISC-V Bare-Metal Microcontrollers (`--target thumbv7m / riscv32imc`).
  5. Freestanding x86_64 Multiboot2 OS Kernels (`--target freestanding`).

---

## 2. Module Index

| Module | Canonical Import | Domain / Capability |
| :--- | :--- | :--- |
| `core` | `import "core";` | Fundamental primitives, math functions, string manipulation, conversions |
| `collections` | `import "collections";` | `ArrayList<T>`, `LinkedList<T>`, `HashMap<K, V>`, `LinkedHashMap<K, V>`, `HashSet<T>`, `TreeSet<T>`, `TreeMap<K, V>`, `ArrayDeque<T>`, `PriorityQueue<T>`, `BitSet`, static Collections algorithms (`sort`, `binary_search`, `shuffle`, `reverse`, `rotate`, `unmodifiable_view`) |
| `math` | `import "math";` | Arbitrary-precision `BigInt` (Karatsuba, Miller-Rabin), `BigDecimal` (7 rounding modes), `Complex`, and universal scientific mathematics |
| `concurrency` | `import "concurrency";` | Native threads, M:N fibers, channels, async/await futures, task nurseries |
| `sync` | `import "sync";` | Mutex, RwLock, Semaphore, Barrier, Atomic primitives (`AtomicInt`, `AtomicPtr`) |
| `fs` | `import "fs";` | File metadata, directory walkers, path manipulation, permissions |
| `io` | `import "io";` | Stream I/O, buffered readers/writers, standard streams (`stdin`, `stdout`) |
| `process` | `import "process";` | Child process spawning, exit status, pipes, signal handling |
| `env` | `import "env";` | Command line arguments, environment variables, working directory |
| `time` | `import "time";` | Monotonic clock, duration arithmetic, sleep, timer reactors |
| `regex` | `import "regex";` | Linear-time regex engine (Thompson NFA / DFA), submatch extraction |
| `serde` | `import "serde";` | JSON, TOML, YAML serialization & deserialization, binary encoding |
| `reflect` | `import "reflect";` | Dynamic type introspection, struct field inspection, method invocation |
| `crypto` | `import "crypto";` | SHA-256/512, AES-256-GCM, ChaCha20, Ed25519 signatures, TLS 1.3 |
| `web` | `import "web";` | High-throughput HTTP/1.1 & HTTP/2 async server, router, client |
| `enterprise_web` | `import "enterprise_web";`| Enterprise microservice framework with dependency injection and middleware |
| `sql` | `import "sql";` | SQL database connection abstraction, prepared statements, transactions |
| `sqlite` | `import "sqlite";` | Embedded SQLite C-FFI engine with zero external dependencies |
| `orm` | `import "orm";` | Object-Relational Mapping, query builder, schema migrations |
| `ai` | `import "ai";` | Multidimensional tensors, autograd graph, neural layers, CUDA/BLAS FFI |
| `raylib` | `import "raylib";` | 60+ FPS 2D/3D hardware accelerated graphics, audio, input, windowing |
| `embedded_hal` | `import "embedded_hal";`| Microcontroller GPIO, I2C, SPI, UART, PWM, ADC hardware traits & MMIO |
| `raw_net` | `import "raw_net";` | Promiscuous packet sniffing (`AF_PACKET`), raw Ethernet/IP frame crafting |
| `binary` | `import "binary";` | Binary disassembler, ELF/PE parser, section analyzer, ROP gadgets |
| `forensics` | `import "forensics";` | Process memory dumping, memory map inspection, signature scanning |
| `cloud_security` | `import "cloud_security";`| Cloud auditing, port/banner scanning, certificate validation |
| `freestanding` | `import "freestanding";`| Multiboot2 headers, physical memory allocation, kernel bootloader |
| `vga` | `import "vga";` | VGA text mode buffer driver (80x25), color palettes, scrolling |
| `paging` | `import "paging";` | 4-level x86_64 virtual memory management, page tables, TLB shootdown |
| `interrupts` | `import "interrupts";` | GDT, IDT interrupt gates, PIC 8259, APIC, interrupt service routines |
| `drivers` | `import "drivers";` | 16550A Serial UART, PS/2 Keyboard/Mouse, PIT timer, HPET |
| `ptr` | `import "ptr";` | Unsafe raw pointer arithmetic, memory copying, layout inspection |
| `memory` | `import "memory";` | Pluggable allocators, bump allocator, arena allocator, slab cache |
| `libc` | `import "libc";` | Direct POSIX / C standard library foreign function declarations |
| `ffi` | `import "ffi";` | Universal dynamic symbol loader (`dlopen`/`dlsym`), C calling convention |
| `rozeboot` | `import "rozeboot";` | Declarative enterprise web framework with routing annotations |

---

## 3. Language Core & Algorithms

### 3.1 `core`
```roze
import "core";

// Math & Numerical Primitives
func abs(x: int) -> int
func min(a: int, b: int) -> int
func max(a: int, b: int) -> int
func clamp(val: int, low: int, high: int) -> int
func sign(x: int) -> int

// Floating Point Math
func math_sqrt(x: float) -> float
func math_sin(x: float) -> float
func math_cos(x: float) -> float
func math_tan(x: float) -> float
func math_exp(x: float) -> float
func math_log(x: float) -> float
func math_pow(base: float, exp: float) -> float

// String Manipulation
func string_length(s: string) -> int
func string_concat(a: string, b: string) -> string
func string_to_upper(s: string) -> string
func string_to_lower(s: string) -> string
func string_substring(s: string, start: int, end: int) -> string
func string_contains(s: string, needle: string) -> bool
func string_index_of(s: string, needle: string) -> int
func string_starts_with(s: string, prefix: string) -> bool
func string_ends_with(s: string, suffix: string) -> bool
func string_replace(s: string, target: string, replacement: string) -> string
func string_split(s: string, delim: string) -> List<string>
func string_trim(s: string) -> string

// Type Conversions
func to_string(val: int) -> string
func float_to_string(val: float) -> string
func to_int(s: string) -> int
func to_float(s: string) -> float
```

### 3.2 `collections` (Enterprise Collections Framework & Algorithms)
```roze
import "collections";

// 1. Resizable Array (ArrayList<T> / List<T>) - Java 25 & Rust Parity
let list = new ArrayList<int>();
list.add(10);
list.add(20);
list.add(30);
let val = list.get(1);          // -> 20
list.set(1, 25);
let has = list.contains(25);    // -> true
let idx = list.indexOf(25);     // -> 1
let removed = list.remove(1);   // -> 25
let sz = list.size();           // -> 2
list.clear();
let empty = list.isEmpty();     // -> true

// 2. Doubly-Linked List (LinkedList<T>) - O(1) Head & Tail
let ll = new LinkedList<int>();
ll.add(10);
ll.add_first(5);
ll.add_last(20);
let first = ll.get_first();     // -> 5
let last = ll.get_last();       // -> 20
let p_first = ll.remove_first();// -> 5
let p_last = ll.remove_last();  // -> 20
let as_list = ll.to_list();

// 3. High-Performance Hash Table (HashMap<K, V> / LinkedHashMap<K, V>)
let map = new HashMap<string, int>();
map.put("port", 8080);
map.put("timeout", 30);
let port = map.get("port");                       // -> 8080
let def = map.get_or_default("missing", 5000);    // -> 5000
let has_key = map.containsKey("port");            // -> true
let keys = map.keys();
let values = map.values();

// 4. Deterministic Insertion-Order Map (LinkedHashMap<K, V>)
let ordered = new LinkedHashMap<int, string>();
ordered.put(1, "first");
ordered.put(2, "second");
let k0 = ordered.get_index_key(0);   // -> 1
let v0 = ordered.get_index_value(0); // -> "first"

// 5. Unique Set (HashSet<T> & TreeSet<T>)
let set = new HashSet<int>();
set.add(42);
set.add(42); // duplicate rejected -> false
let in_set = set.contains(42); // -> true

// 6. Ordered Self-Balancing Set & Map (TreeSet<T> & TreeMap<K, V>)
let tree = new TreeSet<int>();
tree.add(50);
tree.add(10);
tree.add(30);
let min_val = tree.first(); // -> 10 (sorted)
let max_val = tree.last();  // -> 50

// 7. Double-Ended Queue (ArrayDeque<T> / VecDeque<T>)
let deque = new ArrayDeque<int>();
deque.addFirst(1);
deque.addLast(2);
let f = deque.pollFirst(); // -> 1
let l = deque.pollLast();  // -> 2

// 8. Max-Heap Priority Queue (PriorityQueue<T> / BinaryHeap<T>)
let pq = new PriorityQueue<int>();
pq.add(10);
pq.add(50);
pq.add(30);
let max_item = pq.poll(); // -> 50 (highest priority first)

// 9. Static Algorithm Suite (Collections static algorithms)
let data = [40, 10, 30, 50, 20];
collections_sort(data);                   // Quicksort -> [10, 20, 30, 40, 50]
let bs_idx = collections_binary_search(data, 30); // -> 2
collections_reverse(data);                // -> [50, 40, 30, 20, 10]
collections_rotate(data, 2);              // Circular rotation
let min_elem = collections_min(data);     // -> 10
let max_elem = collections_max(data);     // -> 50
let freq = collections_frequency(data, 30);// -> 1
collections_fill(data, 0);                // In-place fill
let unmod = collections_unmodifiable_view(data);
collections_shuffle(data);                // Fisher-Yates random shuffle
```

### 3.3 `ptr` & `memory`
```roze
import "ptr";
import "memory";

unsafe {
    let buf: *mut u8 = mem_alloc(1024);
    ptr_write_u8(buf, 0x42);
    let byte = ptr_read_u8(buf);
    ptr_copy(buf, buf + 512, 64);
    mem_free(buf);
}
```

### 3.4 `math` — Universal Arbitrary-Precision Math Engine (BigInt, BigDecimal, Complex)
```roze
import "math";

// 1. Arbitrary-Precision BigInt (Karatsuba multiplication & Miller-Rabin primality)
let a = bigint_from_string("123456789012345678901234567890");
let b = bigint_from_string("987654321098765432109876543210");
let prod = a.mul(b); // O(n^1.585) Karatsuba multiplication
let div_rem = prod.div_rem(a); // [quotient, remainder]
let is_prime = bigint_from_string("2147483647").is_prob_prime(10); // Miller-Rabin test -> true
let gcd_val = bigint_from_int(48).gcd(bigint_from_int(18)); // -> 6
let lcm_val = bigint_from_int(12).lcm(bigint_from_int(15)); // -> 60

// 2. Arbitrary-Precision Fixed-Point BigDecimal & 7 Industry Rounding Modes
let price = bigdecimal_from_string("123.4567");
let tax = bigdecimal_from_string("0.0825");
let total = price.add(price.mul(tax));
// Banker's Rounding (ROUND_HALF_EVEN - Java 25 & Python 3.15 parity):
let rounded_total = total.set_scale(2, round_mode_half_even()); // -> 133.64

// 3. Complex Numbers & Polar / Cartesian Math
let c1 = complex_new(3.0, 4.0);
let c2 = complex_new(1.0, -2.0);
let c_prod = c1.mul(c2); // -> 11.0 - 2.0i
let c_abs = c1.abs_sq(); // -> 25.0

// 4. Universal Number-Theoretic & Scientific Utilities
let f20 = math_factorial(20); // -> 2432902008176640000
let fib50 = math_fibonacci(50); // -> 12586269025
let pi = math_pi(); // -> 3.141592653589793
let e = math_e();   // -> 2.718281828459045
```

---

## 4. Concurrency & Synchronization

### 4.1 Native Threads, Channels & M:N Fibers
```roze
import "concurrency";

// 1. Native OS Threads
let handle = thread_spawn(|| {
    println("Background worker executing");
    return 100;
});
let res = thread_join(handle);

// 2. Multi-Producer Multi-Consumer Thread-Safe Channels
let ch = channel_new(16); // Buffer size 16
thread_spawn(|| {
    send(ch, "job_complete");
    return 0;
});
let msg = recv(ch);

// 3. Structured Concurrency Task Groups (Nurseries)
nursery n {
    n.spawn(|| do_fetch_a());
    n.spawn(|| do_fetch_b());
} // Automatically joins both tasks and propagates any cancellation
```

### 4.2 Synchronization Primitives
```roze
import "sync";

let mtx = mutex_new();
mutex_lock(mtx);
// Critical section
mutex_unlock(mtx);

let atomic = atomic_i64_new(0);
atomic_i64_add(atomic, 1);
let count = atomic_i64_load(atomic);
```

---

## 5. I/O, File System & Process Management

### 5.1 `io` & `fs`
```roze
import "fs";
import "io";

// File System Checks & Operations
if fs_exists("config.json") {
    let size = fs_file_size("config.json");
    let content = read_file("config.json");
    write_file("backup.json", content);
}

// Directory Traversal
let files = fs_read_dir("src");
for f in files {
    println("Source file: " + f);
}
```

### 5.2 `process` & `env`
```roze
import "process";
import "env";

// Environment & Args
let home = env_get("HOME");
let args = env_args();

// Spawning Subprocesses
let exit_code = process_spawn("git", ["status", "--short"]);
```

---

## 6. Data Serialization, Text & Reflection

### 6.1 `serde`
```roze
import "serde";

let user = map_new();
map_put(user, "username", "firoze");
map_put(user, "id", 1001);

let json_str = json_encode(user);
let decoded = json_decode(json_str);
```

### 6.2 `regex`
```roze
import "regex";

let re = regex_compile("^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\\.[a-zA-Z0-9-.]+$");
let is_valid = regex_is_match(re, "user@roze-lang.org");
```

---

## 7. Enterprise Networking & Security

### 7.1 `web` & `enterprise_web`
```roze
import "web";

let server = http_server_new(8080);
http_server_route(server, "GET", "/health", |req| {
    return http_response(200, "{\"status\":\"UP\"}", "application/json");
});
http_server_listen(server);
```

### 7.2 `crypto` & TLS 1.3
```roze
import "crypto";

// SHA-256 Hashing
let hash = sha256_hex("Prometheus v2.0");

// Ed25519 Digital Signatures
let kp = ed25519_keygen();
let sig = ed25519_sign(kp.seed, "release_manifest");
let valid = ed25519_verify(kp.public_key, "release_manifest", sig);

// Authenticated Encryption (AES-256-GCM)
let key = crypto_random_bytes(32);
let nonce = crypto_random_bytes(12);
let ciphertext = aes_gcm_encrypt(key, nonce, "sensitive_data");
```

---

## 8. Database & Persistence

```roze
import "sql";
import "orm";

let pool = sql_pool_connect("sqlite://app.db", 10);
let rows = sql_query(pool, "SELECT id, name FROM users WHERE active = ?", [1]);
for row in rows {
    println("User: " + map_get(row, "name"));
}
```

---

## 9. Hardware, IoT & Low-Level Systems

### 9.1 `embedded_hal`
```roze
import "embedded_hal";

// Type-Safe MMIO Peripheral Access
let pin = gpio_pin_output(GPIOC, 13);
gpio_set_high(pin);
delay_ms(500);
gpio_set_low(pin);
```

### 9.2 `freestanding` & `vga`
```roze
import "freestanding";
import "vga";

func kernel_main() {
    vga_init();
    vga_set_color(10, 0); // Bright green
    vga_print("🌹 Roze Bare-Metal Kernel v2.0 'Prometheus' Online\n");
}
```

---

## 10. Reference Ecosystem Seed Packages

1. **`rozeboot`**: Enterprise web microservices framework with routing and controllers.
2. **`roze-orm`**: High-throughput SQL ORM and query builder.
3. **`roze-torch`**: Multidimensional tensors (`Tensor2D`), Xavier init, GEMM matmul, `LinearLayer`, activations (`ReLU`, `Sigmoid`, `Softmax`), and CUDA/BLAS FFI bridge.
4. **`roze-game`**: 2D/3D Raylib C-FFI wrapper, `Vector2`, `Vector3`, `Color`, `Rectangle`, `Camera2D`, AABB collision detection, and game object lifecycle.
5. **`roze-ui`**: Declarative desktop GUI framework with widgets (`button`, `label`, `input`, `checkbox`, `progress_bar`), reactive state signals, flexbox layout engine, click hit-testing, and tree dumper.
