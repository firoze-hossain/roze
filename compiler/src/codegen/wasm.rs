// ==============================================================================
// 🌹 Roze Compiler - WebAssembly (WASM32 / WASI) Codegen Backend (M6-T07)
// ==============================================================================
// Direct binary emitter for WebAssembly 1.0 (MVP) and WASI snapshot-preview1:
// - Generates standard, valid .wasm binary modules
// - Implements linear memory allocation (roze_alloc / roze_free)
// - Supports WASI snapshot-preview1 runtime calls (fd_write, fd_read, proc_exit)
// - Supports standalone browser wasm32-unknown-unknown
// - Exports functions, memory, and metadata for 'roze-bindgen' JS/TS bridges
// ==============================================================================

use std::collections::HashMap;
use std::fs;
use std::process::Command;
use anyhow::{Result, anyhow};

use crate::ir::{
    TypedProgram, TypedStatement, TypedExpression, TypedExpressionKind,
    TypedFunctionParam
};
use crate::parser::ast::{BinaryOperator, UnaryOperator};
use crate::semantic::Type;

// --- LEB128 Helpers ---

pub fn encode_u32(val: u32, buf: &mut Vec<u8>) {
    let mut v = val;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
}

pub fn encode_i32(val: i32, buf: &mut Vec<u8>) {
    let mut v = val;
    let mut more = true;
    while more {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if (v == 0 && (byte & 0x40) == 0) || (v == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        buf.push(byte);
    }
}

pub fn encode_i64(val: i64, buf: &mut Vec<u8>) {
    let mut v = val;
    let mut more = true;
    while more {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if (v == 0 && (byte & 0x40) == 0) || (v == -1 && (byte & 0x40) != 0) {
            more = false;
        } else {
            byte |= 0x80;
        }
        buf.push(byte);
    }
}

pub fn encode_f64(val: f64, buf: &mut Vec<u8>) {
    buf.extend_from_slice(&val.to_le_bytes());
}

pub fn encode_string(s: &str, buf: &mut Vec<u8>) {
    encode_u32(s.len() as u32, buf);
    buf.extend_from_slice(s.as_bytes());
}

// --- WebAssembly Value Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmValType {
    I32 = 0x7F,
    I64 = 0x7E,
    F32 = 0x7D,
    F64 = 0x7C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmFuncType {
    pub params: Vec<WasmValType>,
    pub results: Vec<WasmValType>,
}

impl WasmFuncType {
    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(0x60); // func type prefix
        encode_u32(self.params.len() as u32, buf);
        for p in &self.params {
            buf.push(*p as u8);
        }
        encode_u32(self.results.len() as u32, buf);
        for r in &self.results {
            buf.push(*r as u8);
        }
    }
}

// Convert Roze semantic Type to WasmValType
pub fn roze_type_to_wasm(ty: &Type) -> Option<WasmValType> {
    match ty {
        Type::Int | Type::I32 | Type::U32 | Type::I16 | Type::U16 | Type::I8 | Type::U8 | Type::Bool | Type::Char => {
            Some(WasmValType::I32)
        }
        Type::I64 | Type::U64 | Type::I128 | Type::U128 => Some(WasmValType::I64),
        Type::F32 => Some(WasmValType::F32),
        Type::Float | Type::F64 => Some(WasmValType::F64),
        Type::String | Type::Class(_) | Type::List | Type::Map | Type::RawPointer(_) => {
            Some(WasmValType::I32) // Pointers in WASM32 are 32-bit linear memory offsets
        }
        Type::Void | Type::Unknown => None,
        _ => Some(WasmValType::I32),
    }
}

// --- WebAssembly Opcodes ---

#[allow(dead_code)]
pub mod op {
    pub const UNREACHABLE: u8 = 0x00;
    pub const NOP: u8 = 0x01;
    pub const BLOCK: u8 = 0x02;
    pub const LOOP: u8 = 0x03;
    pub const IF: u8 = 0x04;
    pub const ELSE: u8 = 0x05;
    pub const END: u8 = 0x0B;
    pub const BR: u8 = 0x0C;
    pub const BR_IF: u8 = 0x0D;
    pub const RETURN: u8 = 0x0F;
    pub const CALL: u8 = 0x10;
    pub const DROP: u8 = 0x1A;

    pub const LOCAL_GET: u8 = 0x20;
    pub const LOCAL_SET: u8 = 0x21;
    pub const LOCAL_TEE: u8 = 0x22;
    pub const GLOBAL_GET: u8 = 0x23;
    pub const GLOBAL_SET: u8 = 0x24;

    pub const I32_LOAD: u8 = 0x28;
    pub const I64_LOAD: u8 = 0x29;
    pub const F64_LOAD: u8 = 0x2B;
    pub const I32_LOAD8_U: u8 = 0x2D;

    pub const I32_STORE: u8 = 0x36;
    pub const I64_STORE: u8 = 0x37;
    pub const F64_STORE: u8 = 0x39;
    pub const I32_STORE8: u8 = 0x3A;

    pub const MEMORY_SIZE: u8 = 0x3F;
    pub const MEMORY_GROW: u8 = 0x40;

    pub const I32_CONST: u8 = 0x41;
    pub const I64_CONST: u8 = 0x42;
    pub const F32_CONST: u8 = 0x43;
    pub const F64_CONST: u8 = 0x44;

    pub const I32_EQZ: u8 = 0x45;
    pub const I32_EQ: u8 = 0x46;
    pub const I32_NE: u8 = 0x47;
    pub const I32_LT_S: u8 = 0x48;
    pub const I32_LT_U: u8 = 0x49;
    pub const I32_GT_S: u8 = 0x4A;
    pub const I32_GT_U: u8 = 0x4B;
    pub const I32_LE_S: u8 = 0x4C;
    pub const I32_LE_U: u8 = 0x4D;
    pub const I32_GE_S: u8 = 0x4E;
    pub const I32_GE_U: u8 = 0x4F;

    pub const I64_EQZ: u8 = 0x50;
    pub const I64_EQ: u8 = 0x51;
    pub const I64_NE: u8 = 0x52;

    pub const F64_EQ: u8 = 0x61;
    pub const F64_NE: u8 = 0x62;
    pub const F64_LT: u8 = 0x63;
    pub const F64_GT: u8 = 0x64;
    pub const F64_LE: u8 = 0x65;
    pub const F64_GE: u8 = 0x66;

    pub const I32_ADD: u8 = 0x6A;
    pub const I32_SUB: u8 = 0x6B;
    pub const I32_MUL: u8 = 0x6C;
    pub const I32_DIV_S: u8 = 0x6D;
    pub const I32_DIV_U: u8 = 0x6E;
    pub const I32_REM_S: u8 = 0x6F;
    pub const I32_AND: u8 = 0x71;
    pub const I32_OR: u8 = 0x72;
    pub const I32_XOR: u8 = 0x73;
    pub const I32_SHL: u8 = 0x74;
    pub const I32_SHR_S: u8 = 0x75;
    pub const I32_SHR_U: u8 = 0x76;

    pub const I64_ADD: u8 = 0x7C;
    pub const I64_SUB: u8 = 0x7D;
    pub const I64_MUL: u8 = 0x7E;
    pub const I64_DIV_S: u8 = 0x7F;

    pub const F64_ADD: u8 = 0xA0;
    pub const F64_SUB: u8 = 0xA1;
    pub const F64_MUL: u8 = 0xA2;
    pub const F64_DIV: u8 = 0xA3;

    pub const I32_WRAP_I64: u8 = 0xA7;
    pub const I64_EXTEND_I32_S: u8 = 0xAC;
    pub const I32_TRUNC_F64_S: u8 = 0xAA;
    pub const F64_CONVERT_I32_S: u8 = 0xB7;
}

// --- Wasm Module Builder ---

#[allow(dead_code)]
pub struct WasmModule {
    pub is_wasi: bool,
    pub types: Vec<WasmFuncType>,
    pub imports: Vec<(String, String, u32)>, // module, name, type_idx
    pub functions: Vec<u32>,                // type_idx for each defined function
    pub exports: Vec<(String, u8, u32)>,    // name, kind (0=func, 2=mem, 3=global), index
    pub code: Vec<Vec<u8>>,                 // encoded function bodies
    pub data_segments: Vec<(u32, Vec<u8>)>, // offset, bytes
    pub custom_sections: Vec<(String, Vec<u8>)>,
}

impl WasmModule {
    pub fn new(is_wasi: bool) -> Self {
        Self {
            is_wasi,
            types: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            exports: Vec::new(),
            code: Vec::new(),
            data_segments: Vec::new(),
            custom_sections: Vec::new(),
        }
    }

    pub fn get_or_add_type(&mut self, ft: WasmFuncType) -> u32 {
        for (i, t) in self.types.iter().enumerate() {
            if t == &ft {
                return i as u32;
            }
        }
        let idx = self.types.len() as u32;
        self.types.push(ft);
        idx
    }

    pub fn add_import(&mut self, module: &str, name: &str, ft: WasmFuncType) -> u32 {
        let type_idx = self.get_or_add_type(ft);
        let import_idx = self.imports.len() as u32;
        self.imports.push((module.to_string(), name.to_string(), type_idx));
        import_idx
    }

    pub fn add_function(&mut self, ft: WasmFuncType, body: Vec<u8>) -> u32 {
        let type_idx = self.get_or_add_type(ft);
        let func_idx = (self.imports.len() + self.functions.len()) as u32;
        self.functions.push(type_idx);
        self.code.push(body);
        func_idx
    }

    pub fn add_export(&mut self, name: &str, kind: u8, idx: u32) {
        self.exports.push((name.to_string(), kind, idx));
    }

    pub fn add_data(&mut self, offset: u32, data: Vec<u8>) {
        self.data_segments.push((offset, data));
    }

    pub fn finish(self) -> Vec<u8> {
        let mut out = Vec::new();
        // Magic & Version
        out.extend_from_slice(&[0x00, 0x61, 0x73, 0x6D]); // \0asm
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version 1

        // 1. Type Section (ID 1)
        if !self.types.is_empty() {
            let mut sec = Vec::new();
            encode_u32(self.types.len() as u32, &mut sec);
            for t in &self.types {
                t.encode(&mut sec);
            }
            out.push(1);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 2. Import Section (ID 2)
        if !self.imports.is_empty() {
            let mut sec = Vec::new();
            encode_u32(self.imports.len() as u32, &mut sec);
            for (module, name, type_idx) in &self.imports {
                encode_string(module, &mut sec);
                encode_string(name, &mut sec);
                sec.push(0x00); // import kind: function
                encode_u32(*type_idx, &mut sec);
            }
            out.push(2);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 3. Function Section (ID 3)
        if !self.functions.is_empty() {
            let mut sec = Vec::new();
            encode_u32(self.functions.len() as u32, &mut sec);
            for type_idx in &self.functions {
                encode_u32(*type_idx, &mut sec);
            }
            out.push(3);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 5. Memory Section (ID 5)
        // 1 memory: initial 2 pages (128KB), max 256 pages (16MB)
        {
            let mut sec = Vec::new();
            encode_u32(1, &mut sec); // 1 memory
            sec.push(0x01);         // has max limit flag
            encode_u32(2, &mut sec);   // initial 2 pages (128 KB)
            encode_u32(256, &mut sec); // max 256 pages (16 MB)
            out.push(5);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 6. Global Section (ID 6)
        // Global 0: __stack_pointer (i32, mutable, init 65536)
        // Global 1: __heap_base (i32, mutable, init 65536)
        {
            let mut sec = Vec::new();
            encode_u32(2, &mut sec); // 2 globals

            // Global 0: __stack_pointer
            sec.push(WasmValType::I32 as u8);
            sec.push(0x01); // mutable
            sec.push(op::I32_CONST);
            encode_i32(65536, &mut sec);
            sec.push(op::END);

            // Global 1: __heap_base
            sec.push(WasmValType::I32 as u8);
            sec.push(0x01); // mutable
            sec.push(op::I32_CONST);
            encode_i32(65536, &mut sec);
            sec.push(op::END);

            out.push(6);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 7. Export Section (ID 7)
        if !self.exports.is_empty() {
            let mut sec = Vec::new();
            encode_u32(self.exports.len() as u32, &mut sec);
            for (name, kind, idx) in &self.exports {
                encode_string(name, &mut sec);
                sec.push(*kind);
                encode_u32(*idx, &mut sec);
            }
            out.push(7);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 10. Code Section (ID 10)
        if !self.code.is_empty() {
            let mut sec = Vec::new();
            encode_u32(self.code.len() as u32, &mut sec);
            for body in &self.code {
                encode_u32(body.len() as u32, &mut sec);
                sec.extend_from_slice(body);
            }
            out.push(10);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // 11. Data Section (ID 11)
        if !self.data_segments.is_empty() {
            let mut sec = Vec::new();
            encode_u32(self.data_segments.len() as u32, &mut sec);
            for (offset, bytes) in &self.data_segments {
                sec.push(0x00); // active data segment targeting memory 0
                sec.push(op::I32_CONST);
                encode_i32(*offset as i32, &mut sec);
                sec.push(op::END);
                encode_u32(bytes.len() as u32, &mut sec);
                sec.extend_from_slice(bytes);
            }
            out.push(11);
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        // Custom Sections (e.g. roze-metadata for roze-bindgen)
        for (name, data) in &self.custom_sections {
            let mut sec = Vec::new();
            encode_string(name, &mut sec);
            sec.extend_from_slice(data);
            out.push(0); // Custom section ID 0
            encode_u32(sec.len() as u32, &mut out);
            out.extend_from_slice(&sec);
        }

        out
    }
}

// --- Wasm Function Body Context ---

pub struct FuncContext {
    pub locals: HashMap<String, u32>,       // variable name -> local index
    pub local_types: Vec<WasmValType>,       // types of declared locals
    pub param_count: usize,
    pub body: Vec<u8>,
}

impl FuncContext {
    pub fn new(params: &[TypedFunctionParam]) -> Self {
        let mut locals = HashMap::new();
        for (i, p) in params.iter().enumerate() {
            locals.insert(p.name.clone(), i as u32);
        }
        Self {
            locals,
            local_types: Vec::new(),
            param_count: params.len(),
            body: Vec::new(),
        }
    }

    pub fn get_or_add_local(&mut self, name: &str, ty: WasmValType) -> u32 {
        if let Some(&idx) = self.locals.get(name) {
            return idx;
        }
        let idx = (self.param_count + self.local_types.len()) as u32;
        self.locals.insert(name.to_string(), idx);
        self.local_types.push(ty);
        idx
    }

    pub fn add_anon_local(&mut self, ty: WasmValType) -> u32 {
        let idx = (self.param_count + self.local_types.len()) as u32;
        self.local_types.push(ty);
        idx
    }

    pub fn finish(self) -> Vec<u8> {
        // Group locals by type
        let mut grouped: Vec<(u32, WasmValType)> = Vec::new();
        for &t in &self.local_types {
            if let Some(last) = grouped.last_mut() {
                if last.1 == t {
                    last.0 += 1;
                    continue;
                }
            }
            grouped.push((1, t));
        }

        let mut out = Vec::new();
        encode_u32(grouped.len() as u32, &mut out);
        for (count, t) in grouped {
            encode_u32(count, &mut out);
            out.push(t as u8);
        }
        out.extend_from_slice(&self.body);
        out
    }
}

// --- Main WebAssembly Codegen Engine ---

pub struct WasmCodegen {
    pub is_wasi: bool,
    pub module: WasmModule,
    pub string_table: HashMap<String, (u32, u32)>, // str -> (offset, len)
    pub next_string_offset: u32,
    pub func_indices: HashMap<String, u32>,       // func_name -> wasm func index
    pub func_signatures: HashMap<String, (Vec<Type>, Type)>,
    pub class_fields: HashMap<String, Vec<(String, Type)>>,
    pub fd_write_idx: Option<u32>,
    pub proc_exit_idx: Option<u32>,
    pub roze_println_idx: Option<u32>,
    pub roze_print_idx: Option<u32>,
    pub roze_println_int_idx: Option<u32>,
    pub roze_print_int_idx: Option<u32>,
    pub roze_alloc_idx: Option<u32>,
}

impl WasmCodegen {
    pub fn new(is_wasi: bool) -> Self {
        let mut module = WasmModule::new(is_wasi);

        // Memory export
        module.add_export("memory", 0x02, 0);

        let mut fd_write_idx = None;
        let mut proc_exit_idx = None;

        if is_wasi {
            // Import WASI snapshot preview1 fd_write: (i32, i32, i32, i32) -> i32
            let fd_write_ft = WasmFuncType {
                params: vec![WasmValType::I32, WasmValType::I32, WasmValType::I32, WasmValType::I32],
                results: vec![WasmValType::I32],
            };
            fd_write_idx = Some(module.add_import("wasi_snapshot_preview1", "fd_write", fd_write_ft));

            // Import WASI snapshot preview1 proc_exit: (i32) -> ()
            let proc_exit_ft = WasmFuncType {
                params: vec![WasmValType::I32],
                results: vec![],
            };
            proc_exit_idx = Some(module.add_import("wasi_snapshot_preview1", "proc_exit", proc_exit_ft));
        }

        Self {
            is_wasi,
            module,
            string_table: HashMap::new(),
            next_string_offset: 1024, // string literals start at offset 1024
            func_indices: HashMap::new(),
            func_signatures: HashMap::new(),
            class_fields: HashMap::new(),
            fd_write_idx,
            proc_exit_idx,
            roze_println_idx: None,
            roze_print_idx: None,
            roze_println_int_idx: None,
            roze_print_int_idx: None,
            roze_alloc_idx: None,
        }
    }

    pub fn intern_string(&mut self, s: &str) -> (u32, u32) {
        if let Some(&entry) = self.string_table.get(s) {
            return entry;
        }
        let offset = self.next_string_offset;
        let bytes = s.as_bytes().to_vec();
        let len = bytes.len() as u32;
        self.module.add_data(offset, bytes);
        self.next_string_offset += len + 1; // plus null terminator space
        self.string_table.insert(s.to_string(), (offset, len));
        (offset, len)
    }

    fn emit_runtime_helpers(&mut self) {
        // 1. roze_alloc(size: i32) -> i32
        // Bump allocator: fetches __heap_base (global 1), advances by size aligned to 8, updates __heap_base
        {
            let ft = WasmFuncType {
                params: vec![WasmValType::I32],
                results: vec![WasmValType::I32],
            };
            let mut ctx = FuncContext::new(&[TypedFunctionParam {
                name: "size".to_string(),
                type_: Type::Int,
            }]);
            let old_ptr = ctx.add_anon_local(WasmValType::I32);

            // old_ptr = global[1]
            ctx.body.push(op::GLOBAL_GET);
            encode_u32(1, &mut ctx.body);
            ctx.body.push(op::LOCAL_SET);
            encode_u32(old_ptr, &mut ctx.body);

            // global[1] = (global[1] + size + 7) & ~7
            ctx.body.push(op::GLOBAL_GET);
            encode_u32(1, &mut ctx.body);
            ctx.body.push(op::LOCAL_GET);
            encode_u32(0, &mut ctx.body); // size
            ctx.body.push(op::I32_ADD);
            ctx.body.push(op::I32_CONST);
            encode_i32(7, &mut ctx.body);
            ctx.body.push(op::I32_ADD);
            ctx.body.push(op::I32_CONST);
            encode_i32(-8, &mut ctx.body);
            ctx.body.push(op::I32_AND);
            ctx.body.push(op::GLOBAL_SET);
            encode_u32(1, &mut ctx.body);

            // return old_ptr
            ctx.body.push(op::LOCAL_GET);
            encode_u32(old_ptr, &mut ctx.body);
            ctx.body.push(op::END);

            let body = ctx.finish();
            let idx = self.module.add_function(ft, body);
            self.module.add_export("roze_alloc", 0x00, idx);
            self.roze_alloc_idx = Some(idx);
            self.func_indices.insert("roze_alloc".to_string(), idx);
        }

        // 2. roze_free(ptr: i32) -> ()
        {
            let ft = WasmFuncType {
                params: vec![WasmValType::I32],
                results: vec![],
            };
            let mut ctx = FuncContext::new(&[TypedFunctionParam {
                name: "ptr".to_string(),
                type_: Type::Int,
            }]);
            ctx.body.push(op::END);
            let body = ctx.finish();
            let idx = self.module.add_function(ft, body);
            self.module.add_export("roze_free", 0x00, idx);
            self.func_indices.insert("roze_free".to_string(), idx);
        }

        // 3. roze_print(ptr: i32, len: i32) & roze_println(ptr: i32, len: i32)
        if self.is_wasi {
            let fd_write = self.fd_write_idx.unwrap();

            // roze_print(ptr: i32, len: i32)
            {
                let ft = WasmFuncType {
                    params: vec![WasmValType::I32, WasmValType::I32],
                    results: vec![],
                };
                let mut ctx = FuncContext::new(&[
                    TypedFunctionParam { name: "ptr".to_string(), type_: Type::Int },
                    TypedFunctionParam { name: "len".to_string(), type_: Type::Int },
                ]);

                // iovec at memory offset 0:
                // mem[0..3] = ptr
                ctx.body.push(op::I32_CONST);
                encode_i32(0, &mut ctx.body);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body); // ptr
                ctx.body.push(op::I32_STORE);
                encode_u32(2, &mut ctx.body); // align 4
                encode_u32(0, &mut ctx.body); // offset 0

                // mem[4..7] = len
                ctx.body.push(op::I32_CONST);
                encode_i32(4, &mut ctx.body);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(1, &mut ctx.body); // len
                ctx.body.push(op::I32_STORE);
                encode_u32(2, &mut ctx.body);
                encode_u32(0, &mut ctx.body);

                // fd_write(1, 0, 1, 8)
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body); // stdout
                ctx.body.push(op::I32_CONST);
                encode_i32(0, &mut ctx.body); // iovs ptr
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body); // iovs len = 1
                ctx.body.push(op::I32_CONST);
                encode_i32(8, &mut ctx.body); // nwritten ptr
                ctx.body.push(op::CALL);
                encode_u32(fd_write, &mut ctx.body);
                ctx.body.push(op::DROP);

                ctx.body.push(op::END);
                let body = ctx.finish();
                let idx = self.module.add_function(ft, body);
                self.roze_print_idx = Some(idx);
            }

            // roze_println(ptr: i32, len: i32)
            {
                let print_func = self.roze_print_idx.unwrap();
                let ft = WasmFuncType {
                    params: vec![WasmValType::I32, WasmValType::I32],
                    results: vec![],
                };
                let mut ctx = FuncContext::new(&[
                    TypedFunctionParam { name: "ptr".to_string(), type_: Type::Int },
                    TypedFunctionParam { name: "len".to_string(), type_: Type::Int },
                ]);

                // call roze_print(ptr, len)
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(1, &mut ctx.body);
                ctx.body.push(op::CALL);
                encode_u32(print_func, &mut ctx.body);

                // print newline: mem[16] = 10 ('\n')
                ctx.body.push(op::I32_CONST);
                encode_i32(16, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(10, &mut ctx.body);
                ctx.body.push(op::I32_STORE8);
                encode_u32(0, &mut ctx.body);
                encode_u32(0, &mut ctx.body);

                // call roze_print(16, 1)
                ctx.body.push(op::I32_CONST);
                encode_i32(16, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body);
                ctx.body.push(op::CALL);
                encode_u32(print_func, &mut ctx.body);

                ctx.body.push(op::END);
                let body = ctx.finish();
                let idx = self.module.add_function(ft, body);
                self.roze_println_idx = Some(idx);
            }

            // 4. roze_print_int & roze_println_int
            // Converts integer to ascii string in scratch space (e.g. at offset 32), then calls roze_print / roze_println
            {
                let ft = WasmFuncType {
                    params: vec![WasmValType::I32],
                    results: vec![],
                };
                let mut ctx = FuncContext::new(&[
                    TypedFunctionParam { name: "val".to_string(), type_: Type::Int },
                ]);
                let curr_val = ctx.add_anon_local(WasmValType::I32);
                let buf_ptr = ctx.add_anon_local(WasmValType::I32);
                let is_neg = ctx.add_anon_local(WasmValType::I32);

                // Initialize buf_ptr = 64 (we write backwards towards 32)
                ctx.body.push(op::I32_CONST);
                encode_i32(64, &mut ctx.body);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(buf_ptr, &mut ctx.body);

                // if val == 0: store '0' at 63 and print(63, 1)
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body); // val
                ctx.body.push(op::I32_EQZ);
                ctx.body.push(op::IF);
                ctx.body.push(0x40); // empty blocktype

                ctx.body.push(op::I32_CONST);
                encode_i32(63, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(48, &mut ctx.body); // '0'
                ctx.body.push(op::I32_STORE8);
                encode_u32(0, &mut ctx.body);
                encode_u32(0, &mut ctx.body);

                ctx.body.push(op::I32_CONST);
                encode_i32(63, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body);
                ctx.body.push(op::CALL);
                encode_u32(self.roze_print_idx.unwrap(), &mut ctx.body);

                ctx.body.push(op::RETURN);
                ctx.body.push(op::END);

                // is_neg = val < 0
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(0, &mut ctx.body);
                ctx.body.push(op::I32_LT_S);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(is_neg, &mut ctx.body);

                // curr_val = if is_neg { 0 - val } else { val }
                ctx.body.push(op::LOCAL_GET);
                encode_u32(is_neg, &mut ctx.body);
                ctx.body.push(op::IF);
                ctx.body.push(WasmValType::I32 as u8);
                ctx.body.push(op::I32_CONST);
                encode_i32(0, &mut ctx.body);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body);
                ctx.body.push(op::I32_SUB);
                ctx.body.push(op::ELSE);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body);
                ctx.body.push(op::END);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(curr_val, &mut ctx.body);

                // loop: while curr_val > 0
                ctx.body.push(op::BLOCK);
                ctx.body.push(0x40);
                ctx.body.push(op::LOOP);
                ctx.body.push(0x40);

                ctx.body.push(op::LOCAL_GET);
                encode_u32(curr_val, &mut ctx.body);
                ctx.body.push(op::I32_EQZ);
                ctx.body.push(op::BR_IF);
                encode_u32(1, &mut ctx.body); // break

                // buf_ptr -= 1
                ctx.body.push(op::LOCAL_GET);
                encode_u32(buf_ptr, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body);
                ctx.body.push(op::I32_SUB);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(buf_ptr, &mut ctx.body);

                // digit = (curr_val % 10) + '0'
                // mem[buf_ptr] = digit
                ctx.body.push(op::LOCAL_GET);
                encode_u32(buf_ptr, &mut ctx.body);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(curr_val, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(10, &mut ctx.body);
                ctx.body.push(op::I32_REM_S);
                ctx.body.push(op::I32_CONST);
                encode_i32(48, &mut ctx.body); // '0'
                ctx.body.push(op::I32_ADD);
                ctx.body.push(op::I32_STORE8);
                encode_u32(0, &mut ctx.body);
                encode_u32(0, &mut ctx.body);

                // curr_val /= 10
                ctx.body.push(op::LOCAL_GET);
                encode_u32(curr_val, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(10, &mut ctx.body);
                ctx.body.push(op::I32_DIV_S);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(curr_val, &mut ctx.body);

                ctx.body.push(op::BR);
                encode_u32(0, &mut ctx.body); // continue
                ctx.body.push(op::END); // end loop
                ctx.body.push(op::END); // end block

                // if is_neg: write '-' at buf_ptr - 1, and buf_ptr -= 1
                ctx.body.push(op::LOCAL_GET);
                encode_u32(is_neg, &mut ctx.body);
                ctx.body.push(op::IF);
                ctx.body.push(0x40);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(buf_ptr, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body);
                ctx.body.push(op::I32_SUB);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(buf_ptr, &mut ctx.body);

                ctx.body.push(op::LOCAL_GET);
                encode_u32(buf_ptr, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(45, &mut ctx.body); // '-'
                ctx.body.push(op::I32_STORE8);
                encode_u32(0, &mut ctx.body);
                encode_u32(0, &mut ctx.body);
                ctx.body.push(op::END);

                // call roze_print(buf_ptr, 64 - buf_ptr)
                ctx.body.push(op::LOCAL_GET);
                encode_u32(buf_ptr, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(64, &mut ctx.body);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(buf_ptr, &mut ctx.body);
                ctx.body.push(op::I32_SUB);
                ctx.body.push(op::CALL);
                encode_u32(self.roze_print_idx.unwrap(), &mut ctx.body);

                ctx.body.push(op::END);
                let body = ctx.finish();
                let idx = self.module.add_function(ft, body);
                self.roze_print_int_idx = Some(idx);
            }

            // roze_println_int: calls roze_print_int then newline
            {
                let ft = WasmFuncType {
                    params: vec![WasmValType::I32],
                    results: vec![],
                };
                let mut ctx = FuncContext::new(&[
                    TypedFunctionParam { name: "val".to_string(), type_: Type::Int },
                ]);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(0, &mut ctx.body);
                ctx.body.push(op::CALL);
                encode_u32(self.roze_print_int_idx.unwrap(), &mut ctx.body);

                // print newline
                ctx.body.push(op::I32_CONST);
                encode_i32(16, &mut ctx.body);
                ctx.body.push(op::I32_CONST);
                encode_i32(1, &mut ctx.body);
                ctx.body.push(op::CALL);
                encode_u32(self.roze_print_idx.unwrap(), &mut ctx.body);

                ctx.body.push(op::END);
                let body = ctx.finish();
                let idx = self.module.add_function(ft, body);
                self.roze_println_int_idx = Some(idx);
            }
        }
    }

    pub fn compile(mut self, program: TypedProgram) -> Result<Vec<u8>> {
        // Collect class declarations
        for stmt in &program.statements {
            if let TypedStatement::ClassDecl { name, fields, .. } = stmt {
                self.class_fields.insert(name.clone(), fields.clone());
            }
        }

        // Emit runtime helpers (alloc, free, print)
        self.emit_runtime_helpers();

        // Pass 1: Collect function signatures & assign function indices
        for stmt in &program.statements {
            if let TypedStatement::Function { name, params, return_type, .. } = stmt {
                let mut wasm_params = Vec::new();
                let mut roze_param_types = Vec::new();
                for p in params {
                    if let Some(wt) = roze_type_to_wasm(&p.type_) {
                        wasm_params.push(wt);
                    }
                    roze_param_types.push(p.type_.clone());
                }
                let mut wasm_results = Vec::new();
                if let Some(wt) = roze_type_to_wasm(return_type) {
                    wasm_results.push(wt);
                }

                let ft = WasmFuncType {
                    params: wasm_params,
                    results: wasm_results,
                };
                let type_idx = self.module.get_or_add_type(ft);
                let func_idx = (self.module.imports.len() + self.module.functions.len()) as u32;
                self.module.functions.push(type_idx);
                self.func_indices.insert(name.clone(), func_idx);
                self.func_signatures.insert(name.clone(), (roze_param_types, return_type.clone()));

                // Export user function
                self.module.add_export(name, 0x00, func_idx);
            }
        }

        // Pass 2: Compile function bodies
        for stmt in &program.statements {
            if let TypedStatement::Function { name: _, params, body, return_type, .. } = stmt {
                let mut ctx = FuncContext::new(params);
                self.compile_statement(body, &mut ctx)?;

                // Ensure function body properly returns and ends with op::END (0x0B)
                if let Some(ret_val) = roze_type_to_wasm(return_type) {
                    if ctx.body.last() != Some(&op::RETURN) && ctx.body.last() != Some(&op::END) {
                        match ret_val {
                            WasmValType::I32 => {
                                ctx.body.push(op::I32_CONST);
                                encode_i32(0, &mut ctx.body);
                            }
                            WasmValType::I64 => {
                                ctx.body.push(op::I64_CONST);
                                encode_i64(0, &mut ctx.body);
                            }
                            WasmValType::F32 => {
                                ctx.body.push(op::F32_CONST);
                                ctx.body.extend_from_slice(&0.0f32.to_le_bytes());
                            }
                            WasmValType::F64 => {
                                ctx.body.push(op::F64_CONST);
                                ctx.body.extend_from_slice(&0.0f64.to_le_bytes());
                            }
                        }
                    }
                }
                if ctx.body.last() != Some(&op::END) {
                    ctx.body.push(op::END);
                }

                let compiled_body = ctx.finish();
                self.module.code.push(compiled_body);
            }
        }

        // Pass 3: If WASI, create _start entry point function that calls main()
        if self.is_wasi {
            if let Some(&main_idx) = self.func_indices.get("main") {
                let ft = WasmFuncType {
                    params: vec![],
                    results: vec![],
                };
                let mut ctx = FuncContext::new(&[]);
                ctx.body.push(op::CALL);
                encode_u32(main_idx, &mut ctx.body);

                // If main returns i32, drop it or pass to proc_exit
                if let Some((_, ret_ty)) = self.func_signatures.get("main") {
                    if roze_type_to_wasm(ret_ty).is_some() {
                        if let Some(exit_idx) = self.proc_exit_idx {
                            ctx.body.push(op::CALL);
                            encode_u32(exit_idx, &mut ctx.body);
                        } else {
                            ctx.body.push(op::DROP);
                        }
                    } else if let Some(exit_idx) = self.proc_exit_idx {
                        ctx.body.push(op::I32_CONST);
                        encode_i32(0, &mut ctx.body);
                        ctx.body.push(op::CALL);
                        encode_u32(exit_idx, &mut ctx.body);
                    }
                }

                ctx.body.push(op::END);
                let body = ctx.finish();
                let start_idx = self.module.add_function(ft, body);
                self.module.add_export("_start", 0x00, start_idx);
            }
        }

        // Add 'roze-metadata' custom section for roze-bindgen
        let mut meta = Vec::new();
        encode_u32(self.func_signatures.len() as u32, &mut meta);
        for (name, (params, ret)) in &self.func_signatures {
            encode_string(name, &mut meta);
            encode_u32(params.len() as u32, &mut meta);
            for p in params {
                encode_string(&format!("{:?}", p), &mut meta);
            }
            encode_string(&format!("{:?}", ret), &mut meta);
        }
        self.module.custom_sections.push(("roze-metadata".to_string(), meta));

        Ok(self.module.finish())
    }

    fn compile_statement(&mut self, stmt: &TypedStatement, ctx: &mut FuncContext) -> Result<()> {
        match stmt {
            TypedStatement::Let { name, type_, value, .. } => {
                let wasm_ty = roze_type_to_wasm(type_).unwrap_or(WasmValType::I32);
                let local_idx = ctx.get_or_add_local(name, wasm_ty);
                self.compile_expression(value, ctx)?;
                ctx.body.push(op::LOCAL_SET);
                encode_u32(local_idx, &mut ctx.body);
            }
            TypedStatement::Assign { name, value, .. } => {
                let local_idx = ctx.get_or_add_local(name, WasmValType::I32);
                self.compile_expression(value, ctx)?;
                ctx.body.push(op::LOCAL_SET);
                encode_u32(local_idx, &mut ctx.body);
            }
            TypedStatement::FieldAssign { object, field, value, .. } => {
                // object.field = value
                // In WASM linear memory: store at (object_ptr + field_offset)
                self.compile_expression(object, ctx)?;
                // Calculate field offset
                let offset = self.get_field_offset(&object.type_, field);
                self.compile_expression(value, ctx)?;
                ctx.body.push(op::I32_STORE);
                encode_u32(2, &mut ctx.body); // align 4
                encode_u32(offset, &mut ctx.body);
            }
            TypedStatement::Expression { expr, .. } => {
                self.compile_expression(expr, ctx)?;
                if roze_type_to_wasm(&expr.type_).is_some() {
                    ctx.body.push(op::DROP);
                }
            }
            TypedStatement::Return { value, .. } => {
                if let Some(val) = value {
                    self.compile_expression(val, ctx)?;
                }
                ctx.body.push(op::RETURN);
            }
            TypedStatement::Block { statements, .. } => {
                for s in statements {
                    self.compile_statement(s, ctx)?;
                }
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                self.compile_expression(condition, ctx)?;
                ctx.body.push(op::IF);
                ctx.body.push(0x40); // empty blocktype
                self.compile_statement(then_branch, ctx)?;
                if let Some(else_b) = else_branch {
                    ctx.body.push(op::ELSE);
                    self.compile_statement(else_b, ctx)?;
                }
                ctx.body.push(op::END);
            }
            TypedStatement::While { condition, body, .. } => {
                // block $exit { loop $continue { br_if $exit (!cond); body; br $continue } }
                ctx.body.push(op::BLOCK);
                ctx.body.push(0x40);
                ctx.body.push(op::LOOP);
                ctx.body.push(0x40);

                self.compile_expression(condition, ctx)?;
                ctx.body.push(op::I32_EQZ);
                ctx.body.push(op::BR_IF);
                encode_u32(1, &mut ctx.body); // break to outer block

                self.compile_statement(body, ctx)?;

                ctx.body.push(op::BR);
                encode_u32(0, &mut ctx.body); // continue loop
                ctx.body.push(op::END);
                ctx.body.push(op::END);
            }
            TypedStatement::For { init, condition, update, body, .. } => {
                self.compile_statement(init, ctx)?;
                ctx.body.push(op::BLOCK);
                ctx.body.push(0x40);
                ctx.body.push(op::LOOP);
                ctx.body.push(0x40);

                self.compile_expression(condition, ctx)?;
                ctx.body.push(op::I32_EQZ);
                ctx.body.push(op::BR_IF);
                encode_u32(1, &mut ctx.body); // break

                self.compile_statement(body, ctx)?;
                self.compile_statement(update, ctx)?;

                ctx.body.push(op::BR);
                encode_u32(0, &mut ctx.body); // continue loop
                ctx.body.push(op::END);
                ctx.body.push(op::END);
            }
            _ => {}
        }
        Ok(())
    }

    fn compile_expression(&mut self, expr: &TypedExpression, ctx: &mut FuncContext) -> Result<()> {
        match &expr.kind {
            TypedExpressionKind::Number(n) => {
                if let Ok(i) = n.parse::<i32>() {
                    ctx.body.push(op::I32_CONST);
                    encode_i32(i, &mut ctx.body);
                } else if let Ok(i) = n.parse::<i64>() {
                    ctx.body.push(op::I64_CONST);
                    encode_i64(i, &mut ctx.body);
                } else if let Ok(f) = n.parse::<f64>() {
                    ctx.body.push(op::F64_CONST);
                    encode_f64(f, &mut ctx.body);
                } else {
                    ctx.body.push(op::I32_CONST);
                    encode_i32(0, &mut ctx.body);
                }
            }
            TypedExpressionKind::Boolean(b) => {
                ctx.body.push(op::I32_CONST);
                encode_i32(if *b { 1 } else { 0 }, &mut ctx.body);
            }
            TypedExpressionKind::Char(c) => {
                ctx.body.push(op::I32_CONST);
                encode_i32(*c as i32, &mut ctx.body);
            }
            TypedExpressionKind::String(s) => {
                let (offset, _len) = self.intern_string(s);
                ctx.body.push(op::I32_CONST);
                encode_i32(offset as i32, &mut ctx.body);
            }
            TypedExpressionKind::Identifier(name) => {
                let local_idx = ctx.get_or_add_local(name, WasmValType::I32);
                ctx.body.push(op::LOCAL_GET);
                encode_u32(local_idx, &mut ctx.body);
            }
            TypedExpressionKind::Binary { left, operator, right } => {
                self.compile_expression(left, ctx)?;
                self.compile_expression(right, ctx)?;
                self.emit_binary_op(operator, &left.type_, ctx);
            }
            TypedExpressionKind::Unary { operator, operand } => {
                self.compile_expression(operand, ctx)?;
                match operator {
                    UnaryOperator::Not => {
                        ctx.body.push(op::I32_EQZ);
                    }
                    UnaryOperator::Negate => {
                        // 0 - val
                        let tmp = ctx.add_anon_local(WasmValType::I32);
                        ctx.body.push(op::LOCAL_SET);
                        encode_u32(tmp, &mut ctx.body);
                        ctx.body.push(op::I32_CONST);
                        encode_i32(0, &mut ctx.body);
                        ctx.body.push(op::LOCAL_GET);
                        encode_u32(tmp, &mut ctx.body);
                        ctx.body.push(op::I32_SUB);
                    }
                }
            }
            TypedExpressionKind::Call { function, arguments } => {
                if function == "println" || function == "print" {
                    let is_newline = function == "println";
                    if let Some(arg) = arguments.first() {
                        if arg.type_ == Type::Int || arg.type_ == Type::I32 || arg.type_ == Type::I64 {
                            self.compile_expression(arg, ctx)?;
                            let target_helper = if is_newline {
                                self.roze_println_int_idx
                            } else {
                                self.roze_print_int_idx
                            };
                            if let Some(h) = target_helper {
                                ctx.body.push(op::CALL);
                                encode_u32(h, &mut ctx.body);
                            }
                        } else if let TypedExpressionKind::String(s) = &arg.kind {
                            let (offset, len) = self.intern_string(s);
                            ctx.body.push(op::I32_CONST);
                            encode_i32(offset as i32, &mut ctx.body);
                            ctx.body.push(op::I32_CONST);
                            encode_i32(len as i32, &mut ctx.body);
                            let target_helper = if is_newline {
                                self.roze_println_idx
                            } else {
                                self.roze_print_idx
                            };
                            if let Some(h) = target_helper {
                                ctx.body.push(op::CALL);
                                encode_u32(h, &mut ctx.body);
                            }
                        } else {
                            // General string variable: load ptr, compute len (or assume ptr), print
                            self.compile_expression(arg, ctx)?;
                            // Placeholder length or strlen
                            ctx.body.push(op::I32_CONST);
                            encode_i32(16, &mut ctx.body);
                            let target_helper = if is_newline {
                                self.roze_println_idx
                            } else {
                                self.roze_print_idx
                            };
                            if let Some(h) = target_helper {
                                ctx.body.push(op::CALL);
                                encode_u32(h, &mut ctx.body);
                            }
                        }
                    }
                } else if let Some(&idx) = self.func_indices.get(function) {
                    for arg in arguments {
                        self.compile_expression(arg, ctx)?;
                    }
                    ctx.body.push(op::CALL);
                    encode_u32(idx, &mut ctx.body);
                } else {
                    return Err(anyhow!("Undefined function in WASM codegen: {}", function));
                }
            }
            TypedExpressionKind::New { class_name, arguments } => {
                // Allocate class instance: roze_alloc(field_count * 8)
                let field_count = self.class_fields.get(class_name).map(|f| f.len()).unwrap_or(arguments.len());
                let alloc_size = (field_count.max(1) * 8) as i32;

                // call roze_alloc(alloc_size)
                ctx.body.push(op::I32_CONST);
                encode_i32(alloc_size, &mut ctx.body);
                ctx.body.push(op::CALL);
                encode_u32(self.roze_alloc_idx.unwrap(), &mut ctx.body);

                let obj_ptr = ctx.add_anon_local(WasmValType::I32);
                ctx.body.push(op::LOCAL_SET);
                encode_u32(obj_ptr, &mut ctx.body);

                // Store each argument into (obj_ptr + i * 8)
                for (i, arg) in arguments.iter().enumerate() {
                    ctx.body.push(op::LOCAL_GET);
                    encode_u32(obj_ptr, &mut ctx.body);
                    self.compile_expression(arg, ctx)?;
                    ctx.body.push(op::I32_STORE);
                    encode_u32(2, &mut ctx.body); // align 4
                    encode_u32((i * 8) as u32, &mut ctx.body);
                }

                // Leave obj_ptr on the stack
                ctx.body.push(op::LOCAL_GET);
                encode_u32(obj_ptr, &mut ctx.body);
            }
            TypedExpressionKind::FieldAccess { object, field } => {
                self.compile_expression(object, ctx)?;
                let offset = self.get_field_offset(&object.type_, field);
                ctx.body.push(op::I32_LOAD);
                encode_u32(2, &mut ctx.body);
                encode_u32(offset, &mut ctx.body);
            }
            _ => {
                ctx.body.push(op::I32_CONST);
                encode_i32(0, &mut ctx.body);
            }
        }
        Ok(())
    }

    fn emit_binary_op(&self, op: &BinaryOperator, ty: &Type, ctx: &mut FuncContext) {
        let is_float = *ty == Type::Float || *ty == Type::F64;
        match op {
            BinaryOperator::Add => {
                ctx.body.push(if is_float { op::F64_ADD } else { op::I32_ADD });
            }
            BinaryOperator::Subtract => {
                ctx.body.push(if is_float { op::F64_SUB } else { op::I32_SUB });
            }
            BinaryOperator::Multiply => {
                ctx.body.push(if is_float { op::F64_MUL } else { op::I32_MUL });
            }
            BinaryOperator::Divide => {
                ctx.body.push(if is_float { op::F64_DIV } else { op::I32_DIV_S });
            }
            BinaryOperator::Modulo => {
                ctx.body.push(op::I32_REM_S);
            }
            BinaryOperator::Equal => {
                ctx.body.push(if is_float { op::F64_EQ } else { op::I32_EQ });
            }
            BinaryOperator::NotEqual => {
                ctx.body.push(if is_float { op::F64_NE } else { op::I32_NE });
            }
            BinaryOperator::LessThan => {
                ctx.body.push(if is_float { op::F64_LT } else { op::I32_LT_S });
            }
            BinaryOperator::LessEqual => {
                ctx.body.push(if is_float { op::F64_LE } else { op::I32_LE_S });
            }
            BinaryOperator::GreaterThan => {
                ctx.body.push(if is_float { op::F64_GT } else { op::I32_GT_S });
            }
            BinaryOperator::GreaterEqual => {
                ctx.body.push(if is_float { op::F64_GE } else { op::I32_GE_S });
            }
            BinaryOperator::And => {
                ctx.body.push(op::I32_AND);
            }
            BinaryOperator::Or => {
                ctx.body.push(op::I32_OR);
            }
        }
    }

    fn get_field_offset(&self, ty: &Type, field: &str) -> u32 {
        if let Type::Class(class_name) = ty {
            if let Some(fields) = self.class_fields.get(class_name) {
                for (i, (fname, _)) in fields.iter().enumerate() {
                    if fname == field {
                        return (i * 8) as u32;
                    }
                }
            }
        }
        0
    }
}

// --- Entry point called by compiler/src/main.rs ---

pub fn compile_to_wasm(program: TypedProgram, input_file: &str, is_wasi: bool) -> Result<String> {
    let out_wasm_path = if input_file.ends_with(".roze") {
        format!("{}.wasm", &input_file[..input_file.len() - 5])
    } else {
        format!("{}.wasm", input_file)
    };

    let codegen = WasmCodegen::new(is_wasi);
    let wasm_bytes = codegen.compile(program)?;

    fs::write(&out_wasm_path, &wasm_bytes)?;
    println!("📦 Emitted WebAssembly module: {} ({} bytes)", out_wasm_path, wasm_bytes.len());

    Ok(out_wasm_path)
}

pub fn run_wasm(wasm_path: &str) -> Result<()> {
    // If wasmtime is available, use it; otherwise use Node.js
    if let Ok(status) = Command::new("wasmtime").arg(wasm_path).status() {
        if status.success() {
            return Ok(());
        }
    }

    // Node.js WASI execution runner
    let node_script = format!(
        r#"
const fs = require('fs');
const {{ WASI }} = require('node:wasi');
async function run() {{
    const wasi = new WASI({{ version: 'preview1', args: process.argv.slice(1) }});
    const bytes = fs.readFileSync('{wasm_path}');
    const wasm = await WebAssembly.compile(bytes);
    const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());
    wasi.start(instance);
}}
run().catch(err => {{
    console.error(err);
    process.exit(1);
}});
"#
    );

    let status = Command::new("node")
        .arg("-e")
        .arg(&node_script)
        .status()?;

    if !status.success() {
        return Err(anyhow!("WASM execution exited with failure code {:?}", status.code()));
    }

    Ok(())
}
