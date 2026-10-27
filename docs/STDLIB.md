# 🌹 Roze Standard Library Reference (v1.0.0)

The Roze Standard Library (`stdlib/`) provides production-ready, memory-safe abstractions across core algorithms, data structures, networking, systems programming, and AI.

---

## 1. `core.roze` - Fundamentals & Math

Fundamental utilities included by default across all targets:

```roze
func abs(x: int) -> int
func min(a: int, b: int) -> int
func max(a: int, b: int) -> int
func clamp(val: int, low: int, high: int) -> int
func sign(x: int) -> int
func string_length(s: string) -> int
func string_to_upper(s: string) -> string
func string_to_lower(s: string) -> string
func string_substring(s: string, start: int, end: int) -> string
func string_contains(s: string, sub: string) -> bool
```

---

## 2. `collections.roze` - Dynamic Data Structures

### `List<T>`
Dynamically resizing array backed by contiguous memory:
```roze
let list = list_new();
list_push(list, 10);
list_push(list, 20);
let val = list_get(list, 0);       // 10
list_set(list, 1, 99);             // replaces 20 with 99
let len = list_length(list);       // 2
let removed = list_remove(list, 0); // removes 10
let empty = list_is_empty(list);   // false
```

### `Map<K, V>`
Hash map supporting fast insertion, key lookup, and removal:
```roze
let map = map_new();
map_put(map, "port", 8080);
let has = map_has(map, "port");    // true
let port = map_get(map, "port");   // 8080
map_remove(map, "port");
let size = map_size(map);          // 0
```

---

## 3. `io.roze` - File & Stream Operations

```roze
func read_file(path: string) -> string
func write_file(path: string, content: string) -> void
func append_file(path: string, content: string) -> void
func delete_file(path: string) -> bool
func file_exists(path: string) -> bool
func read_lines(path: string) -> List<string>
```

---

## 4. `web.roze` - HTTP Client & Server

```roze
// HTTP Client:
func http_get(url: string) -> string
func http_post(url: string, body: string) -> string

// Multithreaded HTTP Server:
func http_server_start(port: int) -> int
func http_server_accept(server_fd: int) -> Map<string, string>
func http_server_respond(req: Map<string, string>, status: int, body: string) -> void
func http_server_stop(server_fd: int) -> void

// JSON:
func json_encode(obj: Map<string, any>) -> string
func json_decode(raw: string) -> Map<string, any>
```

---

## 5. `concurrency.roze` - Threads & Channels

Safe concurrency with native OS threads and multi-producer multi-consumer channels:
```roze
let chan = channel_new();

let thread = thread_spawn(() => {
    channel_send(chan, 42);
});

let message = channel_recv(chan); // Receives 42
thread_join(thread);
```

---

## 6. `sql.roze` - Enterprise Database Access

```roze
func sql_connect(jdbc_url: string) -> Connection
func sql_query(conn: Connection, query: string) -> List<Map<string, any>>
func sql_execute(conn: Connection, sql: string) -> int
func sql_close(conn: Connection) -> void
```

---

## 7. `raylib.roze` - 2D & 3D Game Engine

Complete bindings to Raylib for real-time graphics, sound, and physics:
```roze
func InitWindow(width: int, height: int, title: string) -> void
func WindowShouldClose() -> bool
func CloseWindow() -> void
func BeginDrawing() -> void
func EndDrawing() -> void
func ClearBackground(color: int) -> void
func DrawCircle(x: int, y: int, radius: float, color: int) -> void
func DrawRectangle(x: int, y: int, width: int, height: int, color: int) -> void
func DrawText(text: string, x: int, y: int, fontSize: int, color: int) -> void
func IsKeyDown(key: int) -> bool
func PlaySound(sound: Sound) -> void
```

---

## 8. `ai.roze` - Machine Learning & Tensor Engine

High-performance multidimensional tensors and neural network execution:
```roze
func tensor_create_1d(data: List<float>, size: int) -> Tensor
func tensor_create_2d(rows: int, cols: int) -> Tensor
func tensor_matmul(a: Tensor, b: Tensor) -> Tensor
func tensor_add(a: Tensor, b: Tensor) -> Tensor
func tensor_relu(a: Tensor) -> Tensor
func tensor_softmax(a: Tensor) -> Tensor
func tensor_transformer_forward(input_tokens: List<int>) -> List<float>
```
