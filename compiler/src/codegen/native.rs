// compiler/src/codegen/native.rs
//
// A native (non-JVM) backend, built on Cranelift, targeting the host
// machine directly -- no JVM/JDK involved at any point. This exists to
// prove the typed IR (see ir.rs) is genuinely backend-agnostic, and as
// the starting point for the systems/embedded half of the project's
// goals that the JVM backend structurally can't reach (see
// ROADMAP.md's "The bigger picture" section).
//
// STATUS: extending past the original int/bool-only spike now that the
// memory model decision (see docs/MEMORY_MODEL_DECISION.md) has been
// made: ARC. Strings are the first heap-allocated type implemented
// under it -- real `malloc`-backed allocation, real reference counting,
// real `free` when a count hits zero. `list`/`map` aren't ported yet
// (each element/key/value would need its own retain/release, which is
// a real next increment, not a given just because strings work), and
// every Core/Collections/IO/Web/Database intrinsic is still JVM-only.
//
// ARC ownership convention used throughout this file (the same rule
// applied uniformly to `let`, reassignment, `return`, and call
// arguments): a *fresh* string value -- a literal, a concatenation
// result, or a function call's return value -- already carries a
// properly-owned reference nobody else holds a claim on, so handing it
// off needs no extra work. A bare *identifier* reference aliases an
// existing owned binding that will release its own reference at scope
// exit, so creating a second independent owner from it needs an
// explicit retain first (see `retain_if_aliasing`) -- otherwise the
// first owner's release would free the value while the second owner
// still thinks it's valid.
//
// Every scope (a `{ ... }` block, and a function's combined parameter +
// top-level-body scope) releases whichever of its own local bindings
// are strings when it exits normally; an early `return` releases every
// *active* scope's string locals (the whole function is being exited,
// not just the innermost block), protecting the returned value first
// with a retain if it's a bare identifier alias per the rule above.
//
// What IS supported, for real: functions with `int`/`bool`/`string`
// parameters and return types, arithmetic, comparisons, boolean logic
// (with real short-circuit evaluation), string concatenation and
// content equality, `if`/`else`/`while`/`for`, calling other Roze
// functions (including recursion), and `println` of any int/bool/
// string value (not just literals anymore).
use crate::ir::{TypedExpression, TypedExpressionKind, TypedProgram, TypedStatement};
use crate::parser::ast::{BinaryOperator, UnaryOperator};
use crate::semantic::Type;
use anyhow::{anyhow, Result};
use cranelift::prelude::*;
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::HashMap;
use std::fs;
use std::process::Command;

/// Every Roze value on the native backend is represented as a 64-bit
/// integer: a real integer for `int`, 0/1 for `bool`, and a pointer
/// (see `StringRuntime`'s doc comment for the layout it points at) for
/// `string`. A real implementation would want a narrower type for
/// `bool`; using one uniform type everywhere keeps code generation
/// simple.
const VALUE_TYPE: types::Type = types::I64;

/// Size, in bytes, of a Roze string's header (see `StringRuntime`).
const STRING_HEADER_SIZE: i64 = 16;
/// Refcount value marking a string as immortal (a compile-time literal,
/// living in static data, never allocated or freed) -- retain/release
/// treat this as a permanent no-op, so the overwhelmingly common case
/// of using a literal costs no heap allocation or refcount traffic at
/// all.
const IMMORTAL_SENTINEL: i64 = -1;

/// Compiles `program` (the same typed IR the JVM backend consumes) to a
/// native object file, then links it into an executable using the
/// system's C compiler -- present on essentially every dev machine
/// already, and already the same kind of reliance the JVM backend has
/// on an external tool (`javac`).
pub fn compile_to_native(program: TypedProgram, input_file: &str) -> Result<()> {
    let output_name = crate::codegen::class_name_from_path(input_file);

    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").map_err(|e| anyhow!(e.to_string()))?;
    flag_builder.set("is_pic", "true").map_err(|e| anyhow!(e.to_string()))?;
    let isa_builder = cranelift_native::builder()
        .map_err(|e| anyhow!("unsupported host platform for the native backend: {}", e))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;

    let obj_builder = ObjectBuilder::new(isa, output_name.clone(), cranelift_module::default_libcall_names())
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut module = ObjectModule::new(obj_builder);

    {
        let mut generator = NativeGenerator::new(&mut module)?;
        generator.declare_all_functions(&program)?;
        generator.compile_all_functions(&program)?;
    }

    let product = module.finish();
    let bytes = product.emit().map_err(|e| anyhow!(e.to_string()))?;
    let obj_path = format!("{}.o", output_name);
    fs::write(&obj_path, &bytes)?;
    println!("📝 Generated native object file: {}", obj_path);

    link_object_file(&obj_path, &output_name)?;

    Ok(())
}

/// Links `obj_path` into an executable at `output_name`, trying each of
/// the standard C compiler/linker driver names in turn. This covers
/// Linux/Mac (where `cc` is virtually always present) and Windows with
/// MSYS2's MinGW-w64 toolchain installed (which provides `gcc`, and
/// often `cc` too).
///
/// Deliberately doesn't attempt MSVC's `cl.exe`: its command-line
/// syntax for this is different enough from `cc`/`gcc`/`clang`'s that
/// getting it right without a Windows machine to verify against felt
/// riskier than pointing at a well-trodden, already-documented
/// alternative (MSYS2) in the error message below.
fn link_object_file(obj_path: &str, output_name: &str) -> Result<()> {
    const CANDIDATES: [&str; 3] = ["cc", "gcc", "clang"];

    for candidate in CANDIDATES {
        match Command::new(candidate).arg(obj_path).arg("-o").arg(output_name).status() {
            Ok(status) if status.success() => {
                println!("✅ Linked native executable: {}", output_name);
                return Ok(());
            }
            Ok(status) => {
                return Err(anyhow!("linking failed via '{}' (exit status: {})", candidate, status));
            }
            Err(_) => continue, // this one isn't installed -- try the next candidate
        }
    }

    Err(anyhow!(
        "couldn't find a C compiler to link the native executable (tried: {}).\n\n\
         The native backend (--target native) needs one to produce a final executable -- \
         this is separate from whatever toolchain Rust itself used to build the `roze` \
         compiler, so having that working already doesn't mean this step will.\n\n\
         On Windows, the most reliable option is MSYS2's MinGW-w64 toolchain:\n\
         \x20 1. winget install -e --id MSYS2.MSYS2\n\
         \x20 2. Open \"MSYS2 MinGW x64\" (not plain MSYS2) from the Start menu\n\
         \x20 3. pacman -S mingw-w64-x86_64-toolchain\n\
         \x20 4. Add C:\\msys64\\mingw64\\bin to your PATH, then open a new terminal\n\n\
         On Linux: sudo apt install build-essential (or your distro's equivalent).\n\
         On macOS: xcode-select --install",
        CANDIDATES.join(", ")
    ))
}

/// Runs a compiled native executable by path (as opposed to
/// `codegen::run_java`, which runs a JVM class by name through `java`).
pub fn run_native(name: &str) -> Result<()> {
    let exe_path = if name.starts_with('.') || name.contains('/') || name.contains('\\') {
        name.to_string()
    } else {
        format!("./{}", name)
    };
    let status = Command::new(&exe_path)
        .status()
        .map_err(|e| anyhow!("failed to run '{}': {}", exe_path, e))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("native program exited with a non-zero status"))
    }
}

/// Rejects any type this backend doesn't support yet, with a message
/// pointing at why, rather than either panicking or silently
/// miscompiling.
fn require_supported_type(ty: &Type, context: &str) -> Result<()> {
    match ty {
        Type::Int | Type::Bool | Type::Void | Type::Unknown | Type::String => Ok(()),
        Type::List | Type::Map => Err(anyhow!(
            "the native backend doesn't support '{}' yet -- each element/key/value would need its own ARC retain/release, which hasn't been ported from the string implementation yet -- {}",
            ty, context
        )),
        Type::Function { .. } => Err(anyhow!("the native backend doesn't support function values -- {}", context)),
    }
}

/// The FuncIds for the small ARC runtime backing `string` values,
/// declared/built once per compilation and shared by every function.
///
/// Layout of a Roze string: the pointer Roze code actually holds points
/// at the *data*, with an ARC header immediately before it:
///
///   [-16..-8)  i64  refcount (or -1 == IMMORTAL_SENTINEL for a literal)
///   [-8..0)    i64  length (not including the implicit NUL terminator)
///   [0..)           the bytes themselves, NUL-terminated
///
/// so that pointer is always *also* a valid, NUL-terminated C string --
/// directly usable with printf's `%s` or any libc call with no
/// adjustment, the same trick already used for the fixed format/true/
/// false strings below.
///
/// (Assumes a little-endian target when literals are assembled as raw
/// bytes in `FunctionCompiler::compile_string_literal` -- true for the
/// x86_64 hosts this has actually been run on; a big-endian native
/// target would need that specific spot adjusted.)
struct StringRuntime {
    retain_id: FuncId,
    release_id: FuncId,
    concat_id: FuncId,
    eq_id: FuncId,
}

fn declare_string_runtime(module: &mut ObjectModule) -> Result<StringRuntime> {
    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let malloc_id = module.declare_function("malloc", Linkage::Import, &malloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(VALUE_TYPE));
    let free_id = module.declare_function("free", Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut memcpy_sig = module.make_signature();
    memcpy_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcpy_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcpy_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcpy_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let memcpy_id = module.declare_function("memcpy", Linkage::Import, &memcpy_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut memcmp_sig = module.make_signature();
    memcmp_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcmp_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcmp_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcmp_sig.returns.push(AbiParam::new(types::I32));
    let memcmp_id = module.declare_function("memcmp", Linkage::Import, &memcmp_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut ctx = module.make_context();
    let retain_id = build_string_retain(module, &mut ctx)?;
    let release_id = build_string_release(module, &mut ctx, free_id)?;
    let concat_id = build_string_concat(module, &mut ctx, malloc_id, memcpy_id)?;
    let eq_id = build_string_eq(module, &mut ctx, memcmp_id)?;

    Ok(StringRuntime { retain_id, release_id, concat_id, eq_id })
}

/// `__roze_string_retain(ptr: i64) -> void`: increments the refcount,
/// unless `ptr` is immortal (a no-op in that case).
fn build_string_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_retain", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, -(STRING_HEADER_SIZE as i32));
        let sentinel = builder.ins().iconst(VALUE_TYPE, IMMORTAL_SENTINEL);
        let is_immortal = builder.ins().icmp(IntCC::Equal, refcount, sentinel);

        let retain_block = builder.create_block();
        let done_block = builder.create_block();
        builder.ins().brif(is_immortal, done_block, &[], retain_block, &[]);

        builder.switch_to_block(retain_block);
        builder.seal_block(retain_block);
        let incremented = builder.ins().iadd_imm(refcount, 1);
        builder.ins().store(MemFlags::new(), incremented, ptr, -(STRING_HEADER_SIZE as i32));
        builder.ins().jump(done_block, &[]);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_string_release(ptr: i64) -> void`: decrements the refcount
/// and frees the allocation if it reaches zero, unless `ptr` is
/// immortal (a no-op in that case).
fn build_string_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, free_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_release", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, -(STRING_HEADER_SIZE as i32));
        let sentinel = builder.ins().iconst(VALUE_TYPE, IMMORTAL_SENTINEL);
        let is_immortal = builder.ins().icmp(IntCC::Equal, refcount, sentinel);

        let mutate_block = builder.create_block();
        let done_block = builder.create_block();
        builder.ins().brif(is_immortal, done_block, &[], mutate_block, &[]);

        builder.switch_to_block(mutate_block);
        builder.seal_block(mutate_block);
        let decremented = builder.ins().iadd_imm(refcount, -1);
        builder.ins().store(MemFlags::new(), decremented, ptr, -(STRING_HEADER_SIZE as i32));

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let should_free = builder.ins().icmp(IntCC::Equal, decremented, zero);
        let free_block = builder.create_block();
        builder.ins().brif(should_free, free_block, &[], done_block, &[]);

        builder.switch_to_block(free_block);
        builder.seal_block(free_block);
        let block_ptr = builder.ins().iadd_imm(ptr, -STRING_HEADER_SIZE);
        let free_ref = module.declare_func_in_func(free_id, builder.func);
        builder.ins().call(free_ref, &[block_ptr]);
        builder.ins().jump(done_block, &[]);

        // done_block has three predecessors (is_immortal's true-edge,
        // should_free's false-edge, and free_block's jump) -- only seal
        // it now that all three exist.
        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_string_concat(a: i64, b: i64) -> i64`: allocates a new owned
/// (refcount = 1) string containing `a`'s bytes followed by `b`'s.
fn build_string_concat(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, malloc_id: FuncId, memcpy_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_concat", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let a = builder.block_params(entry)[0];
        let b = builder.block_params(entry)[1];

        let length_offset = -(STRING_HEADER_SIZE as i32) + 8;
        let len_a = builder.ins().load(VALUE_TYPE, MemFlags::new(), a, length_offset);
        let len_b = builder.ins().load(VALUE_TYPE, MemFlags::new(), b, length_offset);
        let total_len = builder.ins().iadd(len_a, len_b);

        let header_and_nul = builder.ins().iconst(VALUE_TYPE, STRING_HEADER_SIZE + 1);
        let alloc_size = builder.ins().iadd(header_and_nul, total_len);

        let malloc_ref = module.declare_func_in_func(malloc_id, builder.func);
        let call = builder.ins().call(malloc_ref, &[alloc_size]);
        let block_ptr = builder.inst_results(call)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().store(MemFlags::new(), one, block_ptr, 0);
        builder.ins().store(MemFlags::new(), total_len, block_ptr, 8);

        let data_ptr = builder.ins().iadd_imm(block_ptr, STRING_HEADER_SIZE);

        let memcpy_ref = module.declare_func_in_func(memcpy_id, builder.func);
        builder.ins().call(memcpy_ref, &[data_ptr, a, len_a]);
        let dest_b = builder.ins().iadd(data_ptr, len_a);
        builder.ins().call(memcpy_ref, &[dest_b, b, len_b]);

        let zero_byte = builder.ins().iconst(types::I8, 0);
        let term_addr = builder.ins().iadd(data_ptr, total_len);
        builder.ins().store(MemFlags::new(), zero_byte, term_addr, 0);

        builder.ins().return_(&[data_ptr]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_string_eq(a: i64, b: i64) -> i64` (0 or 1): content equality
/// -- compares length first, then bytes via `memcmp` (not `strcmp`, so
/// this would still be correct if Roze strings ever allow embedded NUL
/// bytes).
fn build_string_eq(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, memcmp_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_eq", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let a = builder.block_params(entry)[0];
        let b = builder.block_params(entry)[1];

        let length_offset = -(STRING_HEADER_SIZE as i32) + 8;
        let len_a = builder.ins().load(VALUE_TYPE, MemFlags::new(), a, length_offset);
        let len_b = builder.ins().load(VALUE_TYPE, MemFlags::new(), b, length_offset);
        let same_len = builder.ins().icmp(IntCC::Equal, len_a, len_b);

        let compare_block = builder.create_block();
        let not_equal_block = builder.create_block();
        let merge_block = builder.create_block();
        builder.append_block_param(merge_block, VALUE_TYPE);

        builder.ins().brif(same_len, compare_block, &[], not_equal_block, &[]);

        builder.switch_to_block(compare_block);
        builder.seal_block(compare_block);
        let memcmp_ref = module.declare_func_in_func(memcmp_id, builder.func);
        let call = builder.ins().call(memcmp_ref, &[a, b, len_a]);
        let cmp_result = builder.inst_results(call)[0];
        let zero_i32 = builder.ins().iconst(types::I32, 0);
        let bytes_equal = builder.ins().icmp(IntCC::Equal, cmp_result, zero_i32);
        let bytes_equal_i64 = builder.ins().uextend(VALUE_TYPE, bytes_equal);
        builder.ins().jump(merge_block, &[bytes_equal_i64]);

        builder.switch_to_block(not_equal_block);
        builder.seal_block(not_equal_block);
        let false_val = builder.ins().iconst(VALUE_TYPE, 0);
        builder.ins().jump(merge_block, &[false_val]);

        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        let result = builder.block_params(merge_block)[0];
        builder.ins().return_(&[result]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

struct NativeGenerator<'a> {
    module: &'a mut ObjectModule,
    functions: HashMap<String, FuncId>,
    printf_id: FuncId,
    fmt_int: DataId,
    fmt_str: DataId,
    true_str: DataId,
    false_str: DataId,
    strings: StringRuntime,
    literal_counter: usize,
}

impl<'a> NativeGenerator<'a> {
    fn new(module: &'a mut ObjectModule) -> Result<Self> {
        // printf's Cranelift-side signature only needs to reflect what
        // every call site here actually passes: a format-string pointer
        // plus exactly one more 64-bit value (unused by the callee when
        // the format string has no '%' specifier -- harmless, and it
        // means every call site can share one signature instead of
        // needing a distinct one per arg count for a variadic function).
        let mut printf_sig = module.make_signature();
        printf_sig.params.push(AbiParam::new(VALUE_TYPE));
        printf_sig.params.push(AbiParam::new(VALUE_TYPE));
        printf_sig.returns.push(AbiParam::new(types::I32));
        let printf_id = module.declare_function("printf", Linkage::Import, &printf_sig)
            .map_err(|e| anyhow!(e.to_string()))?;

        let fmt_int = Self::declare_c_string(module, "__roze_fmt_int", b"%lld\n")?;
        let fmt_str = Self::declare_c_string(module, "__roze_fmt_str", b"%s\n")?;
        let true_str = Self::declare_c_string(module, "__roze_true_str", b"true")?;
        let false_str = Self::declare_c_string(module, "__roze_false_str", b"false")?;

        let strings = declare_string_runtime(module)?;

        Ok(Self {
            module,
            functions: HashMap::new(),
            printf_id,
            fmt_int,
            fmt_str,
            true_str,
            false_str,
            strings,
            literal_counter: 0,
        })
    }

    fn declare_c_string(module: &mut ObjectModule, name: &str, bytes: &[u8]) -> Result<DataId> {
        let mut owned = bytes.to_vec();
        owned.push(0); // NUL-terminate: this is handed straight to printf as a C string.
        let mut data_ctx = DataDescription::new();
        data_ctx.define(owned.into_boxed_slice());
        let data_id = module.declare_data(name, Linkage::Local, false, false)
            .map_err(|e| anyhow!(e.to_string()))?;
        module.define_data(data_id, &data_ctx).map_err(|e| anyhow!(e.to_string()))?;
        Ok(data_id)
    }

    /// Registers every top-level function's signature before compiling
    /// any bodies, so forward references and mutual recursion work (the
    /// same reason the type checker and the JVM backend each do their
    /// own first pass). Roze's `main` is renamed to `__roze_main` here
    /// so it doesn't collide with the real C-ABI `main` synthesized in
    /// `emit_c_main`.
    fn declare_all_functions(&mut self, program: &TypedProgram) -> Result<()> {
        for stmt in &program.statements {
            if let TypedStatement::Function { name, params, return_type, .. } = stmt {
                for param in params {
                    require_supported_type(&param.type_, &format!("parameter '{}' of function '{}'", param.name, name))?;
                }
                require_supported_type(return_type, &format!("the return type of function '{}'", name))?;

                let mut sig = self.module.make_signature();
                for _ in params {
                    sig.params.push(AbiParam::new(VALUE_TYPE));
                }
                if !matches!(return_type, Type::Void) {
                    sig.returns.push(AbiParam::new(VALUE_TYPE));
                }

                let func_name = if name == "main" { "__roze_main" } else { name.as_str() };
                let func_id = self.module.declare_function(func_name, Linkage::Local, &sig)
                    .map_err(|e| anyhow!(e.to_string()))?;
                self.functions.insert(name.clone(), func_id);
            }
        }
        Ok(())
    }

    fn compile_all_functions(&mut self, program: &TypedProgram) -> Result<()> {
        let mut ctx = self.module.make_context();

        for stmt in &program.statements {
            if let TypedStatement::Function { name, params, body, .. } = stmt {
                let func_id = *self.functions.get(name)
                    .expect("declare_all_functions registers every function before this runs");

                ctx.func.signature = self.module.declarations().get_function_decl(func_id).signature.clone();

                {
                    let mut func_ctx = FunctionBuilderContext::new();
                    let builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

                    let mut fc = FunctionCompiler {
                        module: self.module,
                        builder,
                        functions: &self.functions,
                        printf_id: self.printf_id,
                        fmt_int: self.fmt_int,
                        fmt_str: self.fmt_str,
                        true_str: self.true_str,
                        false_str: self.false_str,
                        string_retain_id: self.strings.retain_id,
                        string_release_id: self.strings.release_id,
                        string_concat_id: self.strings.concat_id,
                        string_eq_id: self.strings.eq_id,
                        literal_counter: &mut self.literal_counter,
                        scopes: vec![HashMap::new()],
                        next_var_index: 0,
                    };

                    let entry = fc.builder.create_block();
                    fc.builder.append_block_params_for_function_params(entry);
                    fc.builder.switch_to_block(entry);
                    fc.builder.seal_block(entry);

                    for (i, param) in params.iter().enumerate() {
                        let var = fc.declare_local(&param.name, param.type_.clone());
                        let value = fc.builder.block_params(entry)[i];
                        fc.builder.def_var(var, value);
                        // Parameters arrive already "owned" by convention
                        // (the caller retained before passing anything
                        // that aliased one of its own bindings -- see
                        // `retain_if_aliasing`), so no extra retain here.
                    }

                    // Inline the body's top-level statements into this
                    // same frame (rather than letting the generic Block
                    // handler push a second, nested one) so parameters
                    // and top-level `let`s release together in one pass
                    // when the function falls off the end normally.
                    let terminated = if let TypedStatement::Block { statements, .. } = body.as_ref() {
                        let mut terminated = false;
                        for s in statements {
                            if terminated {
                                break;
                            }
                            terminated = fc.compile_statement(s)?;
                        }
                        terminated
                    } else {
                        fc.compile_statement(body)?
                    };

                    if !terminated {
                        fc.release_frame(0)?;
                        fc.builder.ins().return_(&[]);
                    }
                    fc.builder.finalize();
                }

                self.module.define_function(func_id, &mut ctx).map_err(|e| anyhow!(e.to_string()))?;
                self.module.clear_context(&mut ctx);
            }
        }

        self.emit_c_main(&mut ctx)?;
        Ok(())
    }

    /// Emits the real C-ABI `main() -> i32` entry point, which just
    /// calls Roze's `main` (compiled above as `__roze_main`) and returns
    /// 0 -- the same reason the JVM backend hard-codes `main`'s Java
    /// signature regardless of what Roze's own `main` declares.
    fn emit_c_main(&mut self, ctx: &mut cranelift::codegen::Context) -> Result<()> {
        let mut main_sig = self.module.make_signature();
        main_sig.returns.push(AbiParam::new(types::I32));
        let main_id = self.module.declare_function("main", Linkage::Export, &main_sig)
            .map_err(|e| anyhow!(e.to_string()))?;

        ctx.func.signature = main_sig;
        {
            let mut func_ctx = FunctionBuilderContext::new();
            let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
            let block = builder.create_block();
            builder.switch_to_block(block);
            builder.seal_block(block);

            if let Some(&roze_main_id) = self.functions.get("main") {
                let main_ref = self.module.declare_func_in_func(roze_main_id, builder.func);
                builder.ins().call(main_ref, &[]);
            }
            // Else: no `main` in the Roze program -- still emit a valid
            // (trivial) executable rather than failing to link, matching
            // the JVM backend's "No main function found!" fallback.

            let zero = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[zero]);
            builder.finalize();
        }
        self.module.define_function(main_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
        self.module.clear_context(ctx);
        Ok(())
    }
}

/// Compiles the body of a single function. Holds a live `FunctionBuilder`
/// for the whole duration, plus everything it needs to emit calls to
/// other Roze functions or the printf/ARC-based intrinsics -- all
/// declared once on `NativeGenerator` and threaded through here by
/// reference.
struct FunctionCompiler<'a, 'b> {
    module: &'a mut ObjectModule,
    builder: FunctionBuilder<'b>,
    functions: &'a HashMap<String, FuncId>,
    printf_id: FuncId,
    fmt_int: DataId,
    fmt_str: DataId,
    true_str: DataId,
    false_str: DataId,
    string_retain_id: FuncId,
    string_release_id: FuncId,
    string_concat_id: FuncId,
    string_eq_id: FuncId,
    /// Shared across every function being compiled (not reset per
    /// function), so two string literals never collide on the same
    /// generated data symbol name.
    literal_counter: &'a mut usize,
    /// Each frame maps a local's name to (its Cranelift Variable, its
    /// Roze type) -- the type is what lets scope-exit cleanup know
    /// which locals are strings needing a release call.
    scopes: Vec<HashMap<String, (Variable, Type)>>,
    next_var_index: usize,
}

impl<'a, 'b> FunctionCompiler<'a, 'b> {
    fn declare_local(&mut self, name: &str, ty: Type) -> Variable {
        let var = Variable::new(self.next_var_index);
        self.next_var_index += 1;
        self.builder.declare_var(var, VALUE_TYPE);
        self.scopes.last_mut().expect("at least one scope").insert(name.to_string(), (var, ty));
        var
    }

    fn lookup_local(&self, name: &str) -> (Variable, Type) {
        for scope in self.scopes.iter().rev() {
            if let Some((var, ty)) = scope.get(name) {
                return (*var, ty.clone());
            }
        }
        panic!(
            "undefined variable '{}' reached native codegen -- the type checker should have rejected this before codegen ever ran",
            name
        );
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    /// Releases every string-typed local declared directly in the given
    /// frame (not its ancestors). Called at a scope's normal exit (not
    /// after an early return -- see `release_all_active_scopes`, used
    /// there instead, and never both for the same exit).
    fn release_frame(&mut self, frame_index: usize) -> Result<()> {
        let string_vars: Vec<Variable> = self.scopes[frame_index]
            .values()
            .filter(|(_, ty)| *ty == Type::String)
            .map(|(var, _)| *var)
            .collect();
        for var in string_vars {
            let val = self.builder.use_var(var);
            self.emit_string_release(val)?;
        }
        Ok(())
    }

    /// Releases every string-typed local across *every* active scope --
    /// used at an early `return`, since that exits the whole function,
    /// not just the innermost block.
    fn release_all_active_scopes(&mut self) -> Result<()> {
        for i in 0..self.scopes.len() {
            self.release_frame(i)?;
        }
        Ok(())
    }

    /// Per this backend's ownership convention (see the module doc
    /// comment): a *fresh* expression (a literal, a concatenation, a
    /// function call result) already produces a properly-owned
    /// reference nobody else holds a claim on, so handing it off needs
    /// no extra work. A bare *identifier* aliases an existing tracked
    /// binding that will release its own reference at scope exit, so
    /// creating a second independent owner from it needs an explicit
    /// retain first.
    fn retain_if_aliasing(&mut self, expr: &TypedExpression, val: Value) -> Result<()> {
        if expr.type_ == Type::String {
            if let TypedExpressionKind::Identifier(_) = &expr.kind {
                self.emit_string_retain(val)?;
            }
        }
        Ok(())
    }

    fn emit_string_retain(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.string_retain_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_string_release(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.string_release_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_string_concat(&mut self, a: Value, b: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.string_concat_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[a, b]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_string_eq(&mut self, a: Value, b: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.string_eq_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[a, b]);
        Ok(self.builder.inst_results(call)[0])
    }

    /// Compiles a string literal into a static, immortal (never
    /// allocated, never freed, refcount = -1) data blob with the same
    /// ARC header layout a heap-allocated string has, and returns a
    /// pointer to its data -- indistinguishable, from any code that
    /// consumes it, from a real heap-allocated string except that
    /// retain/release are no-ops on it.
    fn compile_string_literal(&mut self, s: &str) -> Result<Value> {
        *self.literal_counter += 1;
        let name = format!("__roze_str_lit_{}", self.literal_counter);
        let bytes = s.as_bytes();

        let mut blob = Vec::with_capacity(STRING_HEADER_SIZE as usize + bytes.len() + 1);
        blob.extend_from_slice(&IMMORTAL_SENTINEL.to_le_bytes());
        blob.extend_from_slice(&(bytes.len() as i64).to_le_bytes());
        blob.extend_from_slice(bytes);
        blob.push(0);

        let mut data_ctx = DataDescription::new();
        data_ctx.define(blob.into_boxed_slice());
        let data_id = self.module.declare_data(&name, Linkage::Local, false, false)
            .map_err(|e| anyhow!(e.to_string()))?;
        self.module.define_data(data_id, &data_ctx).map_err(|e| anyhow!(e.to_string()))?;

        let gv = self.module.declare_data_in_func(data_id, self.builder.func);
        let header_ptr = self.builder.ins().global_value(VALUE_TYPE, gv);
        Ok(self.builder.ins().iadd_imm(header_ptr, STRING_HEADER_SIZE))
    }

    /// Compiles a statement. Returns whether control flow is guaranteed
    /// to have already left the function by the time this statement
    /// finishes (e.g. because it always executes a `return`) -- callers
    /// use this to avoid emitting an unreachable extra jump/return (or
    /// scope-exit release call) after a block that already terminated,
    /// since Cranelift requires every block to end in exactly one
    /// terminator instruction.
    fn compile_statement(&mut self, stmt: &TypedStatement) -> Result<bool> {
        match stmt {
            TypedStatement::Block { statements, .. } => {
                self.push_scope();
                let mut terminated = false;
                for s in statements {
                    if terminated {
                        break; // dead code after an unconditional return
                    }
                    terminated = self.compile_statement(s)?;
                }
                if !terminated {
                    self.release_frame(self.scopes.len() - 1)?;
                }
                self.scopes.pop();
                Ok(terminated)
            }
            TypedStatement::Let { name, value, .. } => {
                require_supported_type(&value.type_, &format!("variable '{}'", name))?;
                let val = self.compile_expression(value)?;
                self.retain_if_aliasing(value, val)?;
                let var = self.declare_local(name, value.type_.clone());
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Assign { name, value, .. } => {
                let val = self.compile_expression(value)?;
                self.retain_if_aliasing(value, val)?;
                let (var, ty) = self.lookup_local(name);
                if ty == Type::String {
                    let old_val = self.builder.use_var(var);
                    self.emit_string_release(old_val)?;
                }
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Expression { expr, .. } => {
                let val = self.compile_expression(expr)?;
                if expr.type_ == Type::String && !matches!(expr.kind, TypedExpressionKind::Identifier(_)) {
                    self.emit_string_release(val)?;
                }
                Ok(false)
            }
            TypedStatement::Return { value, .. } => {
                match value {
                    Some(expr) => {
                        let val = self.compile_expression(expr)?;
                        if expr.type_ == Type::String {
                            self.retain_if_aliasing(expr, val)?;
                        }
                        self.release_all_active_scopes()?;
                        self.builder.ins().return_(&[val]);
                    }
                    None => {
                        self.release_all_active_scopes()?;
                        self.builder.ins().return_(&[]);
                    }
                }
                Ok(true)
            }
            TypedStatement::If { condition, then_branch, else_branch, .. } => {
                let cond_val = self.compile_expression(condition)?;

                let then_block = self.builder.create_block();
                let merge_block = self.builder.create_block();
                let else_block = if else_branch.is_some() { self.builder.create_block() } else { merge_block };

                self.builder.ins().brif(cond_val, then_block, &[], else_block, &[]);

                self.builder.switch_to_block(then_block);
                self.builder.seal_block(then_block);
                let then_terminated = self.compile_statement(then_branch)?;
                if !then_terminated {
                    self.builder.ins().jump(merge_block, &[]);
                }

                let else_terminated = if let Some(else_stmt) = else_branch {
                    self.builder.switch_to_block(else_block);
                    self.builder.seal_block(else_block);
                    let terminated = self.compile_statement(else_stmt)?;
                    if !terminated {
                        self.builder.ins().jump(merge_block, &[]);
                    }
                    terminated
                } else {
                    false
                };

                self.builder.switch_to_block(merge_block);
                self.builder.seal_block(merge_block);

                Ok(else_branch.is_some() && then_terminated && else_terminated)
            }
            TypedStatement::While { condition, body, .. } => {
                let header = self.builder.create_block();
                let loop_body = self.builder.create_block();
                let exit = self.builder.create_block();

                self.builder.ins().jump(header, &[]);

                self.builder.switch_to_block(header);
                let cond_val = self.compile_expression(condition)?;
                self.builder.ins().brif(cond_val, loop_body, &[], exit, &[]);

                self.builder.switch_to_block(loop_body);
                self.builder.seal_block(loop_body);
                let body_terminated = self.compile_statement(body)?;
                if !body_terminated {
                    self.builder.ins().jump(header, &[]);
                }
                self.builder.seal_block(header);

                self.builder.switch_to_block(exit);
                self.builder.seal_block(exit);
                Ok(false)
            }
            TypedStatement::For { init, condition, update, body, .. } => {
                self.push_scope();
                self.compile_statement(init)?;

                let header = self.builder.create_block();
                let loop_body = self.builder.create_block();
                let exit = self.builder.create_block();

                self.builder.ins().jump(header, &[]);

                self.builder.switch_to_block(header);
                let cond_val = self.compile_expression(condition)?;
                self.builder.ins().brif(cond_val, loop_body, &[], exit, &[]);

                self.builder.switch_to_block(loop_body);
                self.builder.seal_block(loop_body);
                let body_terminated = self.compile_statement(body)?;
                if !body_terminated {
                    self.compile_statement(update)?;
                    self.builder.ins().jump(header, &[]);
                }
                self.builder.seal_block(header);

                self.builder.switch_to_block(exit);
                self.builder.seal_block(exit);
                self.release_frame(self.scopes.len() - 1)?;
                self.scopes.pop();
                Ok(false)
            }
            TypedStatement::Function { .. } => {
                // Nested functions aren't supported; handled at the top level.
                Ok(false)
            }
        }
    }

    fn compile_expression(&mut self, expr: &TypedExpression) -> Result<Value> {
        match &expr.kind {
            TypedExpressionKind::Number(s) => {
                let n: i64 = s.parse().map_err(|_| anyhow!("invalid integer literal '{}'", s))?;
                Ok(self.builder.ins().iconst(VALUE_TYPE, n))
            }
            TypedExpressionKind::Boolean(b) => {
                Ok(self.builder.ins().iconst(VALUE_TYPE, if *b { 1 } else { 0 }))
            }
            TypedExpressionKind::Null => Ok(self.builder.ins().iconst(VALUE_TYPE, 0)),
            TypedExpressionKind::String(s) => self.compile_string_literal(s),
            TypedExpressionKind::Identifier(name) => {
                let (var, _ty) = self.lookup_local(name);
                Ok(self.builder.use_var(var))
            }
            TypedExpressionKind::Unary { operator, operand } => {
                let val = self.compile_expression(operand)?;
                match operator {
                    UnaryOperator::Negate => Ok(self.builder.ins().ineg(val)),
                    UnaryOperator::Not => {
                        let zero = self.builder.ins().iconst(VALUE_TYPE, 0);
                        Ok(self.compile_cmp(IntCC::Equal, val, zero))
                    }
                }
            }
            TypedExpressionKind::Binary { left, operator, right } => self.compile_binary(left, operator, right),
            TypedExpressionKind::Call { function, arguments } => self.compile_call(function, arguments),
        }
    }

    fn compile_cmp(&mut self, cc: IntCC, l: Value, r: Value) -> Value {
        let cmp = self.builder.ins().icmp(cc, l, r);
        self.builder.ins().uextend(VALUE_TYPE, cmp)
    }

    fn compile_binary(&mut self, left: &TypedExpression, operator: &BinaryOperator, right: &TypedExpression) -> Result<Value> {
        // And/Or need real short-circuit control flow (the right-hand
        // side must not be evaluated when the left already decides the
        // result), not just a plain bitwise instruction.
        match operator {
            BinaryOperator::And => return self.compile_short_circuit(left, right, true),
            BinaryOperator::Or => return self.compile_short_circuit(left, right, false),
            _ => {}
        }

        let is_string_op = left.type_ == Type::String && right.type_ == Type::String;

        let l = self.compile_expression(left)?;
        let r = self.compile_expression(right)?;

        if is_string_op {
            let result = match operator {
                BinaryOperator::Add => self.emit_string_concat(l, r)?,
                BinaryOperator::Equal => self.emit_string_eq(l, r)?,
                BinaryOperator::NotEqual => {
                    let eq = self.emit_string_eq(l, r)?;
                    let zero = self.builder.ins().iconst(VALUE_TYPE, 0);
                    self.compile_cmp(IntCC::Equal, eq, zero)
                }
                other => return Err(anyhow!("'{:?}' isn't supported between strings on the native backend", other)),
            };
            // l/r were only *read* here (bytes copied or compared),
            // never stored anywhere lasting. An existing binding
            // (Identifier) still owns its own reference and will
            // release it elsewhere (its own scope exit) -- but a fresh
            // temporary (a nested concat, a call result) has nothing
            // else that will ever release it, so it must happen here,
            // or it leaks. Releasing a literal is always safe too:
            // it's immortal, so this is a no-op for it specifically.
            if !matches!(left.kind, TypedExpressionKind::Identifier(_)) {
                self.emit_string_release(l)?;
            }
            if !matches!(right.kind, TypedExpressionKind::Identifier(_)) {
                self.emit_string_release(r)?;
            }
            return Ok(result);
        }

        Ok(match operator {
            BinaryOperator::Add => self.builder.ins().iadd(l, r),
            BinaryOperator::Subtract => self.builder.ins().isub(l, r),
            BinaryOperator::Multiply => self.builder.ins().imul(l, r),
            BinaryOperator::Divide => self.builder.ins().sdiv(l, r),
            BinaryOperator::Equal => self.compile_cmp(IntCC::Equal, l, r),
            BinaryOperator::NotEqual => self.compile_cmp(IntCC::NotEqual, l, r),
            BinaryOperator::LessThan => self.compile_cmp(IntCC::SignedLessThan, l, r),
            BinaryOperator::GreaterThan => self.compile_cmp(IntCC::SignedGreaterThan, l, r),
            BinaryOperator::LessEqual => self.compile_cmp(IntCC::SignedLessThanOrEqual, l, r),
            BinaryOperator::GreaterEqual => self.compile_cmp(IntCC::SignedGreaterThanOrEqual, l, r),
            BinaryOperator::And | BinaryOperator::Or => unreachable!("handled above"),
        })
    }

    fn compile_short_circuit(&mut self, left: &TypedExpression, right: &TypedExpression, is_and: bool) -> Result<Value> {
        let l = self.compile_expression(left)?;

        let rhs_block = self.builder.create_block();
        let skip_block = self.builder.create_block();
        let merge_block = self.builder.create_block();
        self.builder.append_block_param(merge_block, VALUE_TYPE);

        if is_and {
            self.builder.ins().brif(l, rhs_block, &[], skip_block, &[]);
        } else {
            self.builder.ins().brif(l, skip_block, &[], rhs_block, &[]);
        }

        self.builder.switch_to_block(rhs_block);
        self.builder.seal_block(rhs_block);
        let r = self.compile_expression(right)?;
        self.builder.ins().jump(merge_block, &[r]);

        self.builder.switch_to_block(skip_block);
        self.builder.seal_block(skip_block);
        let short_circuit_val = self.builder.ins().iconst(VALUE_TYPE, if is_and { 0 } else { 1 });
        self.builder.ins().jump(merge_block, &[short_circuit_val]);

        self.builder.switch_to_block(merge_block);
        self.builder.seal_block(merge_block);
        Ok(self.builder.block_params(merge_block)[0])
    }

    fn compile_call(&mut self, name: &str, arguments: &[TypedExpression]) -> Result<Value> {
        if name == "println" {
            return self.compile_println(arguments);
        }

        if super::jvm::is_intrinsic(name) {
            return Err(anyhow!(
                "'{}' is a Core/Collections/IO/Web/Database intrinsic, which is only available on the JVM backend today (see docs/MEMORY_MODEL_DECISION.md for why the native backend doesn't have these yet)",
                name
            ));
        }

        let func_id = *self.functions.get(name).ok_or_else(|| {
            anyhow!(
                "undefined function '{}' reached native codegen -- the type checker should have rejected this before codegen ever ran",
                name
            )
        })?;
        let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);

        let mut arg_values = Vec::with_capacity(arguments.len());
        for arg in arguments {
            let val = self.compile_expression(arg)?;
            // The callee will release this reference at its own scope
            // exit (parameters are tracked exactly like locals -- see
            // `compile_all_functions`), so it needs to receive a
            // reference it's safe to consume that way.
            self.retain_if_aliasing(arg, val)?;
            arg_values.push(val);
        }

        let call = self.builder.ins().call(func_ref, &arg_values);
        let results = self.builder.inst_results(call);
        if results.is_empty() {
            // A void-returning function used where a value is
            // syntactically expected (only reachable as a bare
            // Expression-statement in practice, since the type checker
            // wouldn't allow using a void value otherwise) -- return an
            // unused placeholder so callers don't need special-casing.
            Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
        } else {
            Ok(results[0])
        }
    }

    fn compile_println(&mut self, arguments: &[TypedExpression]) -> Result<Value> {
        if arguments.len() != 1 {
            return Err(anyhow!("the native backend's println only supports exactly one argument for now"));
        }
        let arg = &arguments[0];

        match &arg.type_ {
            Type::String => {
                let val = self.compile_expression(arg)?;
                let result = self.emit_printf(self.fmt_str, val)?;
                if !matches!(arg.kind, TypedExpressionKind::Identifier(_)) {
                    self.emit_string_release(val)?;
                }
                Ok(result)
            }
            Type::Bool => {
                let val = self.compile_expression(arg)?;
                let true_gv = self.module.declare_data_in_func(self.true_str, self.builder.func);
                let false_gv = self.module.declare_data_in_func(self.false_str, self.builder.func);
                let true_ptr = self.builder.ins().global_value(VALUE_TYPE, true_gv);
                let false_ptr = self.builder.ins().global_value(VALUE_TYPE, false_gv);
                let selected_ptr = self.builder.ins().select(val, true_ptr, false_ptr);
                self.emit_printf(self.fmt_str, selected_ptr)
            }
            Type::Int | Type::Unknown => {
                let val = self.compile_expression(arg)?;
                self.emit_printf(self.fmt_int, val)
            }
            other => Err(anyhow!("println of a {} isn't supported on the native backend yet", other)),
        }
    }

    fn emit_printf(&mut self, fmt_data: DataId, arg: Value) -> Result<Value> {
        let fmt_gv = self.module.declare_data_in_func(fmt_data, self.builder.func);
        let fmt_ptr = self.builder.ins().global_value(VALUE_TYPE, fmt_gv);
        let printf_ref = self.module.declare_func_in_func(self.printf_id, self.builder.func);
        let call = self.builder.ins().call(printf_ref, &[fmt_ptr, arg]);
        let _ = self.builder.inst_results(call);
        Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
    }
}
