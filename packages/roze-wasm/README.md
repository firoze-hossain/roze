# 🌹 Roze-Wasm: WebAssembly & WASI 0.2 Component Model Suite

`roze-wasm` delivers first-class WebAssembly 1.0 (MVP) and WASI 0.2 Component Model capabilities in pure Roze:

- **WASM 1.0 & WASI 0.2 Binary Module Emitter**: Direct standard `.wasm` bytecode generation with LEB128 encoding and full section structure.
- **Linear Memory & Dynamic Heap Allocator**: 64 KB page-aligned linear memory buffer, `roze_alloc` and `roze_free` bump and free-list allocators, zero-copy string views.
- **WASI 0.2 & Snapshot-Preview1 Runtime**: System calls (`fd_write`, `fd_read`, `proc_exit`, `clock_time_get`, `random_get`) for Node.js, Deno, Bun, and standalone wasmtime runtimes.
- **Serverless Cloud Edge Workers**: Cloudflare Workers and Fastly Compute@Edge handler abstractions with HTTP request parsing, header management, and response stream generation.
- **DOM Event Bindings & Browser Bridge**: DOM element tree representation, event dispatchers (`click`, `input`, `keydown`), attribute binding, and JavaScript host interop.
- **WebGL 2.0 GPU Canvas Context**: Shader creation and compilation (vertex, fragment), program linking, VBO/IBO buffer binding, and hardware-accelerated draw calls.
- **`roze-bindgen` Automated JS/TS Bridge**: Generation of TypeScript definitions (`.d.ts`), modern ESM JavaScript wrappers, `package.json`, and interactive browser harnesses.
