# 🌹 Roze Standard Library Reference Manual (v2.0.0 'Prometheus')

The **Roze Standard Library** provides zero-overhead, memory-safe, portable systems programming abstractions across 35 standard library modules and 5 reference ecosystem packages.

---

## Table of Contents

1. [Architecture & Design Principles](#1-architecture--design-principles)
2. [Module Index](#2-module-index)
3. [Language Core & Algorithms](#3-language-core--algorithms)
   - `core` • `collections` • `math` • `ptr` • `memory` • `libc`
4. [Concurrency & Synchronization](#4-concurrency--synchronization)
   - `concurrent` • `concurrency` • `sync`
5. [I/O, File System & Process Management](#5-io-file-system--process-management)
   - `io` • `fs` • `process` • `env` • `time` • `nio`
6. [Data Serialization, Text & Reflection](#6-data-serialization-text--reflection)
   - `serde` • `regex` • `reflect` • `data`
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
| `concurrent` | `import "roze.concurrent";` | Enterprise coordination: `CompletableFuture<T>` (monadic composition, `all_of`, `any_of`), `ForkJoinPool` work-stealing engine, `ThreadPoolExecutor`, `CountDownLatch`, `CyclicBarrier`, `Semaphore`, `Exchanger`, `AtomicReference<T>` |
| `concurrency` | `import "concurrency";` | Native threads, M:N fibers, channels, async/await futures, task nurseries |
| `sync` | `import "sync";` | Mutex, RwLock, Semaphore, Barrier, Atomic primitives (`AtomicInt`, `AtomicPtr`) |
| `fs` | `import "fs";` | File metadata, directory walkers, path manipulation, permissions |
| `io` | `import "io";` | Stream I/O, buffered readers/writers, standard streams (`stdin`, `stdout`) |
| `process` | `import "process";` | Child process spawning, exit status, pipes, signal handling |
| `env` | `import "env";` | Command line arguments, environment variables, working directory |
| `time` | `import "roze.time";` | JSR-310 temporal engine: `Instant`, `Duration`, `Period`, `LocalDate`, `LocalTime`, `LocalDateTime`, `ZonedDateTime`, `ZoneId`, `ZoneOffset`, `DateTimeFormatter` |
| `nio` | `import "roze.nio";` | Zero-copy I/O: ByteBuffer (heap and direct off-heap native memory), byte order endianness switching, slicing, duplicate views, memory-mapped files (mmap with PAGE_READONLY, PAGE_READWRITE), I/O multiplexing event selectors (epoll, kqueue, IOCP), Pipes, Rust mio / tokio-core parity |
| `regex` | `import "regex";` | Linear-time regex engine (Thompson NFA / DFA), submatch extraction |
| `data` | `import "roze.data";` | Python 3.15 ergonomics: `itertools` (chain, repeat, cycle, product, permutations, combinations), `functools` (LRU cache, memoize), `statistics` (mean, median, mode, variance, stdev, regression), `pathlib` (`Path`), `csv` (RFC 4180), `argparse` |
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

### 4.3 Enterprise Concurrency: CompletableFuture & Work-Stealing Pools (`roze.concurrent`)
1:1 architectural and functional parity with Java 25 `java.util.concurrent.*` and Rust 1.99 `crossbeam`.

```roze
import "roze.concurrent";

// 1. AtomicReference (Lock-Free CAS Semantics)
let ref = atomic_reference_new("initial");
let updated = ref.compare_and_set("initial", "updated"); // -> true
let val: string = ref.get(); // -> "updated"

// 2. CountDownLatch & CyclicBarrier
let latch = count_down_latch_new(3);
latch.count_down();
latch.await_latch(); // Blocks until count reaches 0

let barrier = cyclic_barrier_new(3, || {
    println("Barrier tripped across all 3 parties!");
});
barrier.await_barrier();

// 3. Semaphore Permits & Exchanger Rendezvous
let sem = semaphore_new(5);
sem.acquire();
let avail = sem.available_permits(); // -> 4
sem.release();

let ex = exchanger_new();
let swapped: string = ex.exchange("data_from_producer");

// 4. CompletableFuture<T> Async Monadic Composition
let future = completable_future_supply_async(|| -> Unknown {
    return 100;
})
.then_apply(|v: Unknown| -> Unknown {
    let n: int = v;
    return n * 2;
})
.then_compose(|v: Unknown| -> CompletableFuture {
    let n: int = v;
    return completable_future_supply_async(|| -> Unknown {
        return n + 50;
    });
});

let result: int = future.get(); // -> 250

// Combinators: all_of, any_of
let f1 = completable_future_completed(10);
let f2 = completable_future_completed(20);
let all_done = completable_future_all_of([f1, f2]);
all_done.join();

// 5. ThreadPoolExecutor & Work-Stealing ForkJoinPool
let pool = thread_pool_executor_new(4, 8);
let fut_task = pool.submit(|| -> Unknown {
    return "task_completed";
});
let res: string = fut_task.get();
pool.shutdown();

let fjp = fork_join_pool_new(4);
let task = fork_join_task_new(|arg: Unknown| -> Unknown {
    let n: int = arg;
    return n * 10;
}, 5);
let fj_res: int = fjp.invoke(task); // -> 50
let stolen = fjp.steal_work(2); // Work-stealing across worker deques
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

### 5.3 `time` (`roze.time` / JSR-310 Temporal Engine)
Full architectural and mathematical parity with Java 25 `java.time.*` (JSR-310) and Python 3.15 `datetime` / `zoneinfo`.
```roze
import "roze.time";

// 1. Civil Calendar Date & Period Math
let today = local_date_of(2026, 11, 16);
let deadline = today.plus_months(3).plus_days(10);
let period = today.until(deadline); // -> P3M10D
let dow = today.day_of_week();      // -> 1 (Monday)
let is_leap = today.is_leap_year(); // -> false

// 2. Civil Time with Nanosecond Precision
let time = local_time_of(14, 30, 0, 500000000);
let evening = time.plus_hours(4);   // -> 18:30:00.500000000

// 3. LocalDateTime Composition
let dt = today.at_time(14, 30, 0);  // -> 2026-11-16T14:30:00

// 4. Timezone-Aware ZonedDateTime & Conversions
let dhaka_zone = zone_id_of("Asia/Dhaka");           // UTC+6
let ny_zone = zone_id_of("America/New_York");        // UTC-5
let zdt_dhaka = dt.at_zone(dhaka_zone);
// Convert to another timezone while keeping identical universal instant:
let zdt_ny = zdt_dhaka.with_zone_same_instant(ny_zone); // 03:30:00-05:00

// 5. RFC 2822 / ISO-8601 Formatting
let rfc_str = zdt_dhaka.format_rfc2822(); // -> Mon, 16 Nov 2026 14:30:00 +0600
let fmt = date_time_formatter_of_pattern("yyyy-MM-dd HH:mm:ss");
let custom_str = fmt.format_date_time(dt); // -> 2026-11-16 14:30:00

// 6. Universal Timeline Instant & Monotonic Timer
let inst = instant_now();
sleep_millis(20);
let elapsed = inst.elapsed(); // Duration with nanosecond precision

// 7. Domain Calendar Entities (YearMonth & MonthDay)
let ym = year_month_of(2024, 2);
let last_day_feb = ym.at_end_of_month(); // -> 2024-02-29
let md = month_day_of(2, 29);
let is_valid = md.is_valid_year(2024);   // -> true

// 8. TemporalAdjusters (Enterprise Business Scheduling)
let next_fri = temporal_adjuster_next(today, time_day_friday());
let fdom = temporal_adjuster_first_day_of_month(today); // -> 2026-11-01
let ldom = temporal_adjuster_last_day_of_month(today);  // -> 2026-11-30

// 9. Clock Universal Timelines & Deterministic Mocking
let clk_fixed = clock_fixed(inst, dhaka_zone);
let mock_millis = clk_fixed.millis();

// 10. ChronoUnit Duration Calculators
let days_diff = time_chrono_unit_days_between(local_date_of(2024, 1, 1), local_date_of(2026, 1, 1)); // -> 731
```

### 5.4 `nio` (`roze.nio` / Zero-Copy Buffers & Memory-Mapped Channels)
Comprehensive Java 25 `java.nio.*` and Rust `mio` / `tokio-core` standard library parity for zero-copy binary I/O, memory-mapped files (`mmap`), non-blocking TCP/IPC channels, and multi-platform reactor event multiplexers (`epoll`, `kqueue`, `IOCP`).

```roze
import "roze.nio";

// 1. Endianness & ByteOrder Switching
let be = byte_buffer_allocate(16);
be.order_set(byte_order_big_endian());
be.put_int(305419896); // 0x12345678 -> bytes: [0x12, 0x34, 0x56, 0x78]

// Switch order on the same buffer to Little Endian
be.order_set(byte_order_little_endian());
let swapped = be.get_int_at(0); // -> 0x78563412 (2018915346)

// 2. Zero-Copy Heap & Direct Off-Heap Native ByteBuffers
let buf = byte_buffer_allocate_direct(1024);
buf.put(42);
buf.put_short(1024);
buf.put_int(123456);
buf.put_long(5000000000);
buf.put_float(3.14159);
buf.put_double(2.718281828);
buf.put_string("Prometheus-NIO");

// Buffer State Traversal: flip, mark, reset, compact
buf.flip();                     // Prepares for reading (limit = position, position = 0)
let b = buf.get();              // -> 42
let s = buf.get_short();        // -> 1024
buf.mark();                     // Remembers current position
let val = buf.get_int();        // -> 123456
buf.reset();                    // Restores position to mark
buf.compact();                  // Shifts unread bytes to index 0, position = remaining()

// Zero-Copy Slices and Views (Shared Memory)
let slice = buf.slice();        // Sub-view sharing exact memory buffer
slice.put_at(0, 99);            // Mutation immediately visible in parent buf!

// 3. Memory-Mapped Files (mmap / FileChannel)
let channel = file_channel_open("data.bin", "rw");
let mbb = channel.map(page_readwrite(), 0, 4096);
mbb.load();                     // Pages in all 4KB memory frames
mbb.put_at(0, 80);              // Direct memory mutation
mbb.force();                    // Flushes dirty pages back to physical disk
channel.close();

// 4. I/O Multiplexing Event Selectors (epoll / kqueue / IOCP Parity)
let selector = selector_open(); // High-performance reactor engine ("epoll" on Linux)

let server = server_socket_channel_open();
server.configure_blocking(false); // Non-blocking mode
server.bind("127.0.0.1", 8080);
server.register(selector, op_accept());

let client = socket_channel_open();
client.configure_blocking(false);
client.connect("127.0.0.1", 8080);
let client_ops = bit_or(op_connect(), bit_or(op_read(), op_write()));
client.register(selector, client_ops);

let ready_channels = selector.select_now(); // Polling without blocking
let keys = selector.selected_keys();

// 5. Unidirectional Pipes (Zero-Copy Inter-Thread Transport)
let pipe = pipe_open();
let msg = byte_buffer_wrap_string("ASYNC_PIPELINE");
pipe.sink().write(msg);

let read_buf = byte_buffer_allocate(64);
let n = pipe.source().read(read_buf); // Reads "ASYNC_PIPELINE"

// 6. Rust mio / tokio-core Parity (Poll & Events)
let poll = mio_poll_new();
let events = mio_events_new(32);
poll.register_socket(client, 1, mio_interest_both());
let num_events = poll.poll(events, 100);
let event0 = events.get(0);
let tok = event0.token();       // -> 1
let can_write = event0.is_writable();
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

### 6.3 `data` (`roze.data` / Python 3.15 Ergonomics)
Comprehensive Python 3.15 parity for functional iteration, caching, statistics, path manipulation, CSV serialization, and CLI argument parsing. Canonical imports: `import "roze.data";` or submodules `import "roze.itertools";`, `import "roze.functools";`, `import "roze.statistics";`, `import "roze.pathlib";`, `import "roze.csv";`, `import "roze.argparse";`.

```roze
import "roze.data";

// 1. roze.itertools: Functional Generators & Combinatorics
let c = itertools_chain([1, 2], [3, 4]);                      // -> [1, 2, 3, 4]
let rep = itertools_repeat(7, 3);                             // -> [7, 7, 7]
let cyc = itertools_cycle([1, 2], 5);                         // -> [1, 2, 1, 2, 1]
let acc = itertools_accumulate_sum([1, 2, 3, 4]);             // -> [1, 3, 6, 10]
let pw = itertools_pairwise([1, 2, 3, 4]);                    // -> [[1, 2], [2, 3], [3, 4]]
let comp = itertools_compress(["a", "b", "c"], [1, 0, 1]);     // -> ["a", "c"]
let prod = itertools_product([1, 2], [10, 20]);               // Cartesian product -> [[1, 10], [1, 20], [2, 10], [2, 20]]
let perms = itertools_permutations([1, 2, 3], 2);             // -> [[1, 2], [1, 3], [2, 1], [2, 3], [3, 1], [3, 2]]
let combs = itertools_combinations([1, 2, 3], 2);             // -> [[1, 2], [1, 3], [2, 3]]
let groups = itertools_groupby_adjacent([1, 1, 2, 3, 3, 3]);   // Run-length grouping

// 2. roze.functools: LRU Cache, Memoization & Reductions
let cache = new LruCache(2);
cache.put("user_1", "Alice");
cache.put("user_2", "Bob");
let val = cache.get("user_1");                                // Cache hit -> "Alice"
cache.put("user_3", "Charlie");                               // Evicts least-recently-used "user_2"
let hits = cache.hits();                                      // -> 1
let misses = cache.misses();                                  // -> 0

let memo = new Memoize();
memo.put("fib_10", 55);
let cached_fib = memo.get("fib_10");                          // -> 55

let sum_val = functools_reduce_add([1, 2, 3, 4, 5], 0);       // -> 15
let prod_val = functools_reduce_mul([1, 2, 3, 4, 5], 1);      // -> 120

// 3. roze.statistics: Mathematical, Statistical & Regression Analysis
let data = [10.0, 20.0, 20.0, 40.0, 50.0];
let avg = stats_mean(data);                                   // -> 28.0
let med = stats_median(data);                                 // -> 20.0
let mod = stats_mode(data);                                   // -> 20.0
let pvar = stats_pvariance(data);                             // Population variance
let pstdev = stats_pstdev(data);                              // Population standard deviation
let s_var = stats_variance(data);                             // Sample variance
let s_std = stats_stdev(data);                                // Sample standard deviation
let q = stats_quantiles(data, 4);                             // Quartiles [Q1, Q2, Q3]

// Linear Regression: y = slope * x + intercept
let xs = [1.0, 2.0, 3.0, 4.0, 5.0];
let ys = [2.0, 4.0, 6.0, 8.0, 10.0];
let reg = stats_linear_regression(xs, ys);
let predicted = reg.predict(6.0);                             // -> 12.0 (slope = 2.0, intercept = 0.0)

// 4. roze.pathlib: Object-Oriented Path Traversal & File I/O
let p = pathlib_path("/var/log/roze/sys.log");
let p_name = p.name();                                        // -> "sys.log"
let p_stem = p.stem();                                        // -> "sys"
let p_ext = p.suffix();                                       // -> ".log"
let p_parent = p.parent().to_string();                        // -> "/var/log/roze"
let p_child = p.parent().joinpath("audit.log");               // -> "/var/log/roze/audit.log"
let is_abs = p.is_absolute();                                 // -> true
p.write_text("Engine boot complete\n");
let content = p.read_text();

// 5. roze.csv: RFC 4180 Parsing, Serializing & Tabular Readers
let raw_csv = "id,name,role\n1,Alice,Admin\n2,\"Bob, Jr.\",Dev\n";
let reader = new CsvDictReader(raw_csv);
let row0 = reader.next_row();
let alice_name = row0.get("name");                            // -> "Alice"

let writer = new CsvWriter();
writer.write_row(["id", "name", "role"]);
writer.write_row(["3", "Charlie", "Security"]);
let output_csv = writer.to_string();

// 6. roze.argparse: Command-Line Option & Flag Parsing
let parser = new ArgumentParser("roze-cli", "Enterprise compiler CLI");
parser.add_argument("--port", "-p", "8080", false, "Server port");
parser.add_argument("--verbose", "-v", "false", true, "Enable verbose logging");
parser.add_argument("input", "", "input.roze", false, "Source file input");

let args = parser.parse_args(["--port", "9090", "--verbose", "main.roze"]);
let port_num = args.get_int("port");                          // -> 9090
let is_v = args.get_bool("verbose");                          // -> true
let input_file = args.get_string("input");                    // -> "main.roze"
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
