// ==============================================================================
// 🌹 Roze Bindgen - WebAssembly JS/TS Interface Binding Generator (M6-T07)
// ==============================================================================
// Generates high-level TypeScript definitions (.d.ts) and JavaScript wrappers (.js)
// from Roze-compiled WebAssembly (.wasm) modules.
// Supports:
// - Universal execution in Node.js (with WASI), Browsers, Deno, Bun & Edge
// - String encoding & decoding with UTF-8 TextDecoder/TextEncoder
// - Automatic memory management (roze_alloc / roze_free)
// - NPM package.json & HTML test harness generation
// ==============================================================================

use std::fs;
use std::path::Path;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

impl WasmType {
    pub fn to_ts_type(&self) -> &'static str {
        match self {
            WasmType::I32 | WasmType::F32 | WasmType::F64 => "number",
            WasmType::I64 => "bigint | number",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WasmExportedFunc {
    pub name: String,
    pub params: Vec<WasmType>,
    pub results: Vec<WasmType>,
}

#[derive(Debug, Clone, Default)]
pub struct WasmModuleInfo {
    pub name: String,
    pub has_memory: bool,
    pub has_wasi: bool,
    pub has_alloc: bool,
    pub has_free: bool,
    pub functions: Vec<WasmExportedFunc>,
}

/// Parse WebAssembly binary and extract export signatures
pub fn inspect_wasm_binary(bytes: &[u8], module_name: &str) -> Result<WasmModuleInfo> {
    if bytes.len() < 8 || &bytes[0..4] != b"\0asm" {
        return Err(anyhow!("Invalid WebAssembly magic header"));
    }

    let mut pos = 8;
    let mut func_types: Vec<(Vec<WasmType>, Vec<WasmType>)> = Vec::new();
    let mut import_func_count = 0;
    let mut func_type_indices: Vec<u32> = Vec::new();
    let mut exports: Vec<(String, u8, u32)> = Vec::new();
    let mut has_wasi = false;

    while pos < bytes.len() {
        let section_id = bytes[pos];
        pos += 1;
        let (section_len, len_bytes) = read_u32_leb(&bytes[pos..])?;
        pos += len_bytes;
        let section_end = pos + section_len as usize;
        if section_end > bytes.len() {
            return Err(anyhow!("Section length exceeds file boundary"));
        }

        match section_id {
            1 => {
                // Type section
                let (count, c_bytes) = read_u32_leb(&bytes[pos..])?;
                pos += c_bytes;
                for _ in 0..count {
                    if bytes[pos] != 0x60 {
                        return Err(anyhow!("Expected func type 0x60"));
                    }
                    pos += 1;
                    // params
                    let (p_count, p_bytes) = read_u32_leb(&bytes[pos..])?;
                    pos += p_bytes;
                    let mut params = Vec::new();
                    for _ in 0..p_count {
                        params.push(byte_to_wasm_type(bytes[pos])?);
                        pos += 1;
                    }
                    // results
                    let (r_count, r_bytes) = read_u32_leb(&bytes[pos..])?;
                    pos += r_bytes;
                    let mut results = Vec::new();
                    for _ in 0..r_count {
                        results.push(byte_to_wasm_type(bytes[pos])?);
                        pos += 1;
                    }
                    func_types.push((params, results));
                }
            }
            2 => {
                // Import section
                let (count, c_bytes) = read_u32_leb(&bytes[pos..])?;
                pos += c_bytes;
                for _ in 0..count {
                    let (mod_name, m_bytes) = read_string(&bytes[pos..])?;
                    pos += m_bytes;
                    let (_field_name, f_bytes) = read_string(&bytes[pos..])?;
                    pos += f_bytes;
                    let kind = bytes[pos];
                    pos += 1;
                    if kind == 0x00 {
                        // Function import
                        let (_type_idx, t_bytes) = read_u32_leb(&bytes[pos..])?;
                        pos += t_bytes;
                        import_func_count += 1;
                    } else if kind == 0x01 {
                        // Table
                        pos += 4;
                    } else if kind == 0x02 {
                        // Memory
                        let has_max = bytes[pos];
                        pos += 1;
                        let (_, l1) = read_u32_leb(&bytes[pos..])?;
                        pos += l1;
                        if has_max == 1 {
                            let (_, l2) = read_u32_leb(&bytes[pos..])?;
                            pos += l2;
                        }
                    } else if kind == 0x03 {
                        // Global
                        pos += 2;
                    }

                    if mod_name.contains("wasi") {
                        has_wasi = true;
                    }
                }
            }
            3 => {
                // Function section
                let (count, c_bytes) = read_u32_leb(&bytes[pos..])?;
                pos += c_bytes;
                for _ in 0..count {
                    let (t_idx, t_bytes) = read_u32_leb(&bytes[pos..])?;
                    pos += t_bytes;
                    func_type_indices.push(t_idx);
                }
            }
            7 => {
                // Export section
                let (count, c_bytes) = read_u32_leb(&bytes[pos..])?;
                pos += c_bytes;
                for _ in 0..count {
                    let (exp_name, e_bytes) = read_string(&bytes[pos..])?;
                    pos += e_bytes;
                    let kind = bytes[pos];
                    pos += 1;
                    let (idx, i_bytes) = read_u32_leb(&bytes[pos..])?;
                    pos += i_bytes;
                    exports.push((exp_name, kind, idx));
                }
            }
            _ => {
                // Skip unknown sections
            }
        }
        pos = section_end;
    }

    let mut info = WasmModuleInfo {
        name: module_name.to_string(),
        has_memory: false,
        has_wasi,
        has_alloc: false,
        has_free: false,
        functions: Vec::new(),
    };

    for (name, kind, idx) in exports {
        if kind == 0x02 {
            info.has_memory = true;
        } else if kind == 0x00 {
            if name == "roze_alloc" {
                info.has_alloc = true;
            } else if name == "roze_free" {
                info.has_free = true;
            }

            // Function export
            let func_type = if (idx as usize) < import_func_count {
                // Import func
                continue;
            } else {
                let defined_idx = idx as usize - import_func_count;
                if defined_idx < func_type_indices.len() {
                    let type_idx = func_type_indices[defined_idx] as usize;
                    if type_idx < func_types.len() {
                        func_types[type_idx].clone()
                    } else {
                        (vec![], vec![])
                    }
                } else {
                    (vec![], vec![])
                }
            };

            info.functions.push(WasmExportedFunc {
                name,
                params: func_type.0,
                results: func_type.1,
            });
        }
    }

    Ok(info)
}

fn byte_to_wasm_type(b: u8) -> Result<WasmType> {
    match b {
        0x7F => Ok(WasmType::I32),
        0x7E => Ok(WasmType::I64),
        0x7D => Ok(WasmType::F32),
        0x7C => Ok(WasmType::F64),
        _ => Err(anyhow!("Unknown WASM value type byte: 0x{:02X}", b)),
    }
}

fn read_u32_leb(bytes: &[u8]) -> Result<(u32, usize)> {
    let mut res = 0u32;
    let mut shift = 0;
    let mut count = 0;
    for &b in bytes {
        count += 1;
        res |= ((b & 0x7F) as u32) << shift;
        if (b & 0x80) == 0 {
            return Ok((res, count));
        }
        shift += 7;
        if shift >= 35 {
            return Err(anyhow!("LEB128 integer overflow"));
        }
    }
    Err(anyhow!("Unexpected EOF reading LEB128"))
}

fn read_string(bytes: &[u8]) -> Result<(String, usize)> {
    let (len, len_bytes) = read_u32_leb(bytes)?;
    let total = len_bytes + len as usize;
    if total > bytes.len() {
        return Err(anyhow!("String length out of bounds"));
    }
    let s = std::str::from_utf8(&bytes[len_bytes..total])?;
    Ok((s.to_string(), total))
}

// --- Generator Functions ---

pub fn generate_bindings(wasm_path: &Path, out_dir: &Path) -> Result<()> {
    let wasm_bytes = fs::read(wasm_path)?;
    let file_stem = wasm_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("module");

    let info = inspect_wasm_binary(&wasm_bytes, file_stem)?;

    fs::create_dir_all(out_dir)?;

    // Copy wasm file into out_dir
    let target_wasm_path = out_dir.join(format!("{}.wasm", file_stem));
    fs::write(&target_wasm_path, &wasm_bytes)?;

    // 1. Generate TypeScript definitions (.d.ts)
    let dts_content = generate_typescript_definitions(&info);
    fs::write(out_dir.join(format!("{}.d.ts", file_stem)), dts_content)?;

    // 2. Generate JavaScript ESM module (.js)
    let js_content = generate_javascript_module(&info, file_stem);
    fs::write(out_dir.join(format!("{}.js", file_stem)), js_content)?;

    // 3. Generate package.json
    let pkg_json = generate_package_json(file_stem);
    fs::write(out_dir.join("package.json"), pkg_json)?;

    // 4. Generate Node.js runner (run_node.mjs)
    let node_runner = generate_node_runner(&info, file_stem);
    fs::write(out_dir.join("run_node.mjs"), node_runner)?;

    // 5. Generate browser HTML harness (index.html)
    let html_harness = generate_html_harness(&info, file_stem);
    fs::write(out_dir.join("index.html"), html_harness)?;

    println!("✅ [roze-bindgen] Successfully generated bindings in: {}", out_dir.display());
    println!("   ├── {}.wasm", file_stem);
    println!("   ├── {}.d.ts (TypeScript Type Definitions)", file_stem);
    println!("   ├── {}.js (JavaScript ESM/Node/Browser Wrapper)", file_stem);
    println!("   ├── package.json (NPM Package Manifest)");
    println!("   ├── run_node.mjs (Automated Node.js Runner)");
    println!("   └── index.html (Browser Live Playground)");

    Ok(())
}

pub fn generate_typescript_definitions(info: &WasmModuleInfo) -> String {
    let mut out = String::new();
    out.push_str("/* 🌹 Auto-generated by roze-bindgen. DO NOT EDIT. */\n\n");

    if info.has_memory {
        out.push_str("export const memory: WebAssembly.Memory;\n\n");
    }

    if info.has_alloc {
        out.push_str("export function roze_alloc(size: number): number;\n");
    }
    if info.has_free {
        out.push_str("export function roze_free(ptr: number): void;\n");
    }

    for func in &info.functions {
        if func.name == "roze_alloc" || func.name == "roze_free" || func.name == "_start" {
            continue;
        }
        let params: Vec<String> = func
            .params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("arg{}: {}", i, t.to_ts_type()))
            .collect();
        let ret = if func.results.is_empty() {
            "void".to_string()
        } else {
            func.results[0].to_ts_type().to_string()
        };
        out.push_str(&format!("export function {}({}): {};\n", func.name, params.join(", "), ret));
    }

    out.push_str("\nexport interface RozeModuleOptions {\n");
    out.push_str("    wasi?: boolean;\n");
    out.push_str("    imports?: WebAssembly.Imports;\n");
    out.push_str("}\n\n");

    out.push_str("export class RozeModule {\n");
    out.push_str("    readonly instance: WebAssembly.Instance;\n");
    out.push_str("    readonly memory?: WebAssembly.Memory;\n");
    out.push_str("    static load(wasmSource?: string | BufferSource | Response, options?: RozeModuleOptions): Promise<RozeModule>;\n");
    out.push_str("    readString(ptr: number, len?: number): string;\n");
    out.push_str("    writeString(str: string): { ptr: number; len: number };\n");
    for func in &info.functions {
        if func.name == "_start" { continue; }
        let params: Vec<String> = func
            .params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("arg{}: {}", i, t.to_ts_type()))
            .collect();
        let ret = if func.results.is_empty() {
            "void".to_string()
        } else {
            func.results[0].to_ts_type().to_string()
        };
        out.push_str(&format!("    {}({}): {};\n", func.name, params.join(", "), ret));
    }
    out.push_str("}\n\n");
    out.push_str("export default function init(wasmSource?: string | BufferSource | Response, options?: RozeModuleOptions): Promise<RozeModule>;\n");

    out
}

pub fn generate_javascript_module(_info: &WasmModuleInfo, file_stem: &str) -> String {
    let mut out = String::new();
    out.push_str("/* 🌹 Auto-generated by roze-bindgen. DO NOT EDIT. */\n\n");
    out.push_str(r#"
const textDecoder = new TextDecoder('utf-8');
const textEncoder = new TextEncoder();

export class RozeModule {
    constructor(instance, wasiInstance = null) {
        this.instance = instance;
        this.exports = instance.exports;
        this.memory = instance.exports.memory;
        this.wasi = wasiInstance;

        // Bind all exported functions to instance
        for (const [name, exp] of Object.entries(this.exports)) {
            if (typeof exp === 'function') {
                this[name] = exp;
            }
        }
    }

    readString(ptr, len) {
        if (!this.memory) throw new Error("No memory exported in WASM module");
        const mem = new Uint8Array(this.memory.buffer);
        if (len === undefined) {
            // Null-terminated string search
            let end = ptr;
            while (end < mem.length && mem[end] !== 0) end++;
            len = end - ptr;
        }
        return textDecoder.decode(mem.subarray(ptr, ptr + len));
    }

    writeString(str) {
        if (!this.memory) throw new Error("No memory exported in WASM module");
        if (typeof this.exports.roze_alloc !== 'function') {
            throw new Error("roze_alloc is required to allocate memory for strings");
        }
        const encoded = textEncoder.encode(str);
        const ptr = this.exports.roze_alloc(encoded.length + 1);
        const mem = new Uint8Array(this.memory.buffer);
        mem.set(encoded, ptr);
        mem[ptr + encoded.length] = 0; // null-terminate
        return { ptr, len: encoded.length };
    }

    static async load(wasmSource = null, options = {}) {
        let wasiInstance = null;
        let wasiImports = {};

        const isNode = typeof process !== 'undefined' && process.versions && process.versions.node;

        if (isNode) {
            try {
                const { WASI } = await import('node:wasi');
                wasiInstance = new WASI({ version: 'preview1', args: process.argv, env: process.env });
                wasiImports = wasiInstance.getImportObject();
            } catch (e) {
                // WASI fallback
            }
        } else {
            // Browser WASI snapshot preview1 polyfill
            wasiImports = {
                wasi_snapshot_preview1: {
                    fd_write: (fd, iovs, iovs_len, nwritten) => {
                        return 0;
                    },
                    fd_read: () => 0,
                    proc_exit: (code) => {
                        if (code !== 0) console.warn("WASI proc_exit with code:", code);
                    },
                    clock_time_get: () => 0,
                    random_get: (buf, len) => 0,
                    args_sizes_get: () => 0,
                    args_get: () => 0,
                }
            };
        }

        const imports = Object.assign({}, wasiImports, options.imports || {});

        let wasmBytes;
        if (!wasmSource) {
            if (isNode) {
                const { readFile } = await import('node:fs/promises');
                const { fileURLToPath } = await import('node:url');
                const { dirname, join } = await import('node:path');
                const __dirname = dirname(fileURLToPath(import.meta.url));
                wasmBytes = await readFile(join(__dirname, '"#);
    out.push_str(file_stem);
    out.push_str(r#".wasm'));
            } else {
                wasmSource = '"#);
    out.push_str(file_stem);
    out.push_str(r#".wasm';
            }
        }

        let moduleInstance;
        if (wasmBytes) {
            const compiled = await WebAssembly.compile(wasmBytes);
            moduleInstance = await WebAssembly.instantiate(compiled, imports);
        } else if (typeof wasmSource === 'string' && typeof fetch !== 'undefined') {
            const resp = await fetch(wasmSource);
            const { instance } = await WebAssembly.instantiateStreaming(resp, imports);
            moduleInstance = instance;
        } else {
            const compiled = await WebAssembly.compile(wasmSource);
            moduleInstance = await WebAssembly.instantiate(compiled, imports);
        }

        if (wasiInstance && typeof wasiInstance.start === 'function' && moduleInstance.exports._start) {
            try {
                wasiInstance.start(moduleInstance);
            } catch (err) {
                // Ignore normal exit
            }
        }

        return new RozeModule(moduleInstance, wasiInstance);
    }
}

export default async function init(wasmSource = null, options = {}) {
    return await RozeModule.load(wasmSource, options);
}
"#);

    out
}

pub fn generate_package_json(name: &str) -> String {
    format!(
        r#"{{
  "name": "{name}",
  "version": "1.0.0",
  "description": "WebAssembly package compiled from Roze with roze-bindgen",
  "main": "{name}.js",
  "types": "{name}.d.ts",
  "type": "module",
  "files": [
    "{name}.wasm",
    "{name}.js",
    "{name}.d.ts"
  ]
}}
"#
    )
}

pub fn generate_node_runner(info: &WasmModuleInfo, file_stem: &str) -> String {
    let mut out = String::new();
    out.push_str("/* Automated Node.js verification script */\n");
    out.push_str(&format!("import init from './{}.js';\n\n", file_stem));
    out.push_str("async function test() {\n");
    out.push_str("    console.log('🌹 Initializing WebAssembly module via roze-bindgen...');\n");
    out.push_str("    const module = await init();\n");
    out.push_str("    console.log('✅ WebAssembly Module Loaded Successfully!');\n");

    for func in &info.functions {
        if func.name == "_start" || func.name == "roze_alloc" || func.name == "roze_free" {
            continue;
        }
        let test_args: Vec<String> = func
            .params
            .iter()
            .enumerate()
            .map(|(i, _)| format!("{}", (i + 1) * 10))
            .collect();
        out.push_str(&format!(
            "    if (typeof module.{} === 'function') {{\n        const res = module.{}({});\n        console.log('  -> {}(' + '{}' + ') =', res);\n    }}\n",
            func.name, func.name, test_args.join(", "), func.name, test_args.join(", ")
        ));
    }

    out.push_str("}\n\ntest().catch(console.error);\n");
    out
}

pub fn generate_html_harness(_info: &WasmModuleInfo, file_stem: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>🌹 Roze WebAssembly & 'roze-bindgen' Live Playground</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; margin: 2rem; background: #0f172a; color: #f8fafc; }}
        h1 {{ color: #f43f5e; }}
        pre {{ background: #1e293b; padding: 1rem; border-radius: 8px; border: 1px solid #334155; overflow: auto; }}
        .badge {{ display: inline-block; padding: 0.25rem 0.5rem; border-radius: 4px; background: #e11d48; color: white; font-size: 0.8rem; font-weight: bold; }}
    </style>
</head>
<body>
    <h1>🌹 Roze WebAssembly: <span class="badge">{file_stem}</span></h1>
    <p>Loaded and verified via <code>roze-bindgen</code> browser wrapper.</p>
    <h3>Console Output:</h3>
    <pre id="log">Loading WebAssembly module...</pre>

    <script type="module">
        import init from './{file_stem}.js';
        const logEl = document.getElementById('log');
        function log(msg) {{
            logEl.textContent += '\n' + msg;
        }}

        async function main() {{
            logEl.textContent = 'Initializing {file_stem}.wasm...';
            const module = await init('{file_stem}.wasm');
            log('✅ WebAssembly Module Loaded!');
            log('Memory buffer size: ' + module.memory.buffer.byteLength + ' bytes');
            log('Exported functions: ' + Object.keys(module.exports).filter(k => typeof module.exports[k] === 'function').join(', '));
        }}
        main().catch(err => log('❌ Error: ' + err));
    </script>
</body>
</html>
"#
    )
}
