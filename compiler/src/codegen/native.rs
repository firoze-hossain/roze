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
use crate::parser::ast::{BinaryOperator, Location, UnaryOperator};
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
/// pointing at why and exactly where in the source, rather than either
/// panicking, silently miscompiling, or (as an earlier version of this
/// function did) giving an error with no position information at all.
fn require_supported_type(ty: &Type, location: &Location, context: &str) -> Result<()> {
    match ty {
        Type::Int | Type::Bool | Type::Void | Type::Unknown | Type::String | Type::List | Type::Map => Ok(()),
        Type::Function { .. } => Err(anyhow!(
            "line {}, column {}: the native backend doesn't support function values -- {}",
            location.line, location.column, context
        )),
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

/// A native `list` value is a pointer to a small, FIXED-size (never
/// reallocated, never moved) header:
///
///   [0..8)   i64  refcount
///   [8..16)  i64  length     (elements currently in use)
///   [16..24) i64  capacity   (elements the data buffer can hold)
///   [24..32) i64  data_ptr   (pointer to a SEPARATELY allocated buffer)
///
/// Two-level indirection (header + separate data buffer) rather than
/// growing the header itself, on purpose: `list_push` can need to grow
/// the backing storage, and `realloc` can return a different address --
/// if the list's own identity (the pointer Roze code holds as "the
/// list") could change out from under it on every push, every other
/// binding aliasing that same list would go stale. Keeping the header
/// itself fixed-address means only `data_ptr` (an internal detail) ever
/// moves; the value Roze code actually holds never does.
///
/// No "immortal" concept here (unlike strings): there's no such thing
/// as a list *literal*, so every list is always a real, live, heap
/// allocation from the moment `list_new()` creates it.
///
/// Elements are plain i64 words with no ARC of their own -- this
/// implementation is deliberately scoped to int/bool elements only
/// (rejected at each call site that would insert an unsupported
/// element), so there's nothing per-element to retain/release, only
/// the list's own header refcount.
const LIST_HEADER_SIZE: i64 = 32;
const LIST_INITIAL_CAPACITY: i64 = 4;

struct ListRuntime {
    new_id: FuncId,
    retain_id: FuncId,
    release_id: FuncId,
    push_id: FuncId,
    get_id: FuncId,
    set_id: FuncId,
    remove_id: FuncId,
    length_id: FuncId,
    is_empty_id: FuncId,
}

fn declare_list_runtime(module: &mut ObjectModule, printf_id: FuncId) -> Result<ListRuntime> {
    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let malloc_id = module.declare_function("malloc", Linkage::Import, &malloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(VALUE_TYPE));
    let free_id = module.declare_function("free", Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut realloc_sig = module.make_signature();
    realloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    realloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    realloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let realloc_id = module.declare_function("realloc", Linkage::Import, &realloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut memmove_sig = module.make_signature();
    memmove_sig.params.push(AbiParam::new(VALUE_TYPE));
    memmove_sig.params.push(AbiParam::new(VALUE_TYPE));
    memmove_sig.params.push(AbiParam::new(VALUE_TYPE));
    memmove_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let memmove_id = module.declare_function("memmove", Linkage::Import, &memmove_sig).map_err(|e| anyhow!(e.to_string()))?;

    module.declare_function("exit", Linkage::Import, &{
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I32));
        sig
    }).map_err(|e| anyhow!(e.to_string()))?;

    let oob_message = NativeGenerator::declare_c_string(module, "__roze_list_oob_msg", b"Roze: list index out of bounds\n")?;

    let mut ctx = module.make_context();
    let new_id = build_list_new(module, &mut ctx, malloc_id)?;
    let retain_id = build_list_retain(module, &mut ctx)?;
    let release_id = build_list_release(module, &mut ctx, free_id)?;
    let push_id = build_list_push(module, &mut ctx, realloc_id)?;
    let get_id = build_list_bounds_checked_access(module, &mut ctx, printf_id, oob_message, AccessKind::Get)?;
    let set_id = build_list_bounds_checked_access(module, &mut ctx, printf_id, oob_message, AccessKind::Set)?;
    let remove_id = build_list_remove(module, &mut ctx, printf_id, oob_message, memmove_id)?;
    let length_id = build_list_length(module, &mut ctx)?;
    let is_empty_id = build_list_is_empty(module, &mut ctx)?;

    Ok(ListRuntime { new_id, retain_id, release_id, push_id, get_id, set_id, remove_id, length_id, is_empty_id })
}

/// Emits `if !(0 <= index < length) { print an error; exit(1); }`,
/// leaving the builder switched into a fresh "in bounds" block for the
/// caller to continue building in. Shared by get/set/remove.
fn emit_bounds_check(
    builder: &mut FunctionBuilder,
    module: &mut ObjectModule,
    printf_id: FuncId,
    oob_message: DataId,
    index: Value,
    length: Value,
) {
    let zero = builder.ins().iconst(VALUE_TYPE, 0);
    let not_negative = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, index, zero);
    let less_than_length = builder.ins().icmp(IntCC::SignedLessThan, index, length);
    let in_bounds = builder.ins().band(not_negative, less_than_length);

    let ok_block = builder.create_block();
    let abort_block = builder.create_block();
    builder.ins().brif(in_bounds, ok_block, &[], abort_block, &[]);

    builder.switch_to_block(abort_block);
    builder.seal_block(abort_block);
    let msg_gv = module.declare_data_in_func(oob_message, builder.func);
    let msg_ptr = builder.ins().global_value(VALUE_TYPE, msg_gv);
    let printf_ref = module.declare_func_in_func(printf_id, builder.func);
    let dummy = builder.ins().iconst(VALUE_TYPE, 0);
    builder.ins().call(printf_ref, &[msg_ptr, dummy]);

    let exit_id = module.declare_function("exit", Linkage::Import, &{
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I32));
        sig
    }).expect("exit was already declared once in declare_list_runtime with an identical signature");
    let exit_ref = module.declare_func_in_func(exit_id, builder.func);
    let one_i32 = builder.ins().iconst(types::I32, 1);
    builder.ins().call(exit_ref, &[one_i32]);
    // exit() never returns in practice; this return only exists so the
    // block has a well-formed terminator Cranelift's verifier accepts.
    builder.ins().return_(&[zero]);

    builder.switch_to_block(ok_block);
    builder.seal_block(ok_block);
}

enum AccessKind {
    Get,
    Set,
}

/// `__roze_list_new() -> i64`: allocates a header plus an initial-
/// capacity data buffer, both independently, and links them together.
fn build_list_new(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, malloc_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_new", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let malloc_ref = module.declare_func_in_func(malloc_id, builder.func);

        let header_size = builder.ins().iconst(VALUE_TYPE, LIST_HEADER_SIZE);
        let call = builder.ins().call(malloc_ref, &[header_size]);
        let header_ptr = builder.inst_results(call)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().store(MemFlags::new(), one, header_ptr, 0);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        builder.ins().store(MemFlags::new(), zero, header_ptr, 8);
        let initial_capacity = builder.ins().iconst(VALUE_TYPE, LIST_INITIAL_CAPACITY);
        builder.ins().store(MemFlags::new(), initial_capacity, header_ptr, 16);

        let data_size = builder.ins().iconst(VALUE_TYPE, LIST_INITIAL_CAPACITY * 8);
        let data_call = builder.ins().call(malloc_ref, &[data_size]);
        let data_ptr = builder.inst_results(data_call)[0];
        builder.ins().store(MemFlags::new(), data_ptr, header_ptr, 24);

        builder.ins().return_(&[header_ptr]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_retain(ptr: i64) -> void`
fn build_list_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_retain", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 0);
        let incremented = builder.ins().iadd_imm(refcount, 1);
        builder.ins().store(MemFlags::new(), incremented, ptr, 0);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_release(ptr: i64) -> void`: frees both the data buffer
/// and the header itself once the refcount reaches zero.
fn build_list_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, free_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_release", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 0);
        let decremented = builder.ins().iadd_imm(refcount, -1);
        builder.ins().store(MemFlags::new(), decremented, ptr, 0);

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let should_free = builder.ins().icmp(IntCC::Equal, decremented, zero);
        let free_block = builder.create_block();
        let done_block = builder.create_block();
        builder.ins().brif(should_free, free_block, &[], done_block, &[]);

        builder.switch_to_block(free_block);
        builder.seal_block(free_block);
        let data_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let free_ref = module.declare_func_in_func(free_id, builder.func);
        builder.ins().call(free_ref, &[data_ptr]);
        builder.ins().call(free_ref, &[ptr]);
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

/// `__roze_list_push(ptr: i64, value: i64) -> i64` (always 1/true,
/// matching `java.util.List.add`'s return convention): doubles the data
/// buffer via `realloc` when full, updating the header's `data_ptr`/
/// `capacity` fields in place -- the header's own address never
/// changes, only what it points at.
fn build_list_push(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, realloc_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_push", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let value = builder.block_params(entry)[1];

        let length = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        let capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);
        let data_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);

        let needs_grow = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, length, capacity);
        let grow_block = builder.create_block();
        let store_block = builder.create_block();
        builder.append_block_param(store_block, VALUE_TYPE); // the data_ptr to actually use

        builder.ins().brif(needs_grow, grow_block, &[], store_block, &[data_ptr]);

        builder.switch_to_block(grow_block);
        builder.seal_block(grow_block);
        let two = builder.ins().iconst(VALUE_TYPE, 2);
        let new_capacity = builder.ins().imul(capacity, two);
        let new_size = builder.ins().imul_imm(new_capacity, 8);
        let realloc_ref = module.declare_func_in_func(realloc_id, builder.func);
        let call = builder.ins().call(realloc_ref, &[data_ptr, new_size]);
        let new_data_ptr = builder.inst_results(call)[0];
        builder.ins().store(MemFlags::new(), new_data_ptr, ptr, 24);
        builder.ins().store(MemFlags::new(), new_capacity, ptr, 16);
        builder.ins().jump(store_block, &[new_data_ptr]);

        builder.switch_to_block(store_block);
        builder.seal_block(store_block);
        let current_data_ptr = builder.block_params(store_block)[0];
        let offset = builder.ins().imul_imm(length, 8);
        let elem_addr = builder.ins().iadd(current_data_ptr, offset);
        builder.ins().store(MemFlags::new(), value, elem_addr, 0);
        let new_length = builder.ins().iadd_imm(length, 1);
        builder.ins().store(MemFlags::new(), new_length, ptr, 8);

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().return_(&[one]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_get(ptr, index) -> i64` / `__roze_list_set(ptr, index,
/// value) -> i64` (returns the *old* value, matching
/// `java.util.List.set`). Both bounds-check first.
fn build_list_bounds_checked_access(
    module: &mut ObjectModule,
    ctx: &mut cranelift::codegen::Context,
    printf_id: FuncId,
    oob_message: DataId,
    kind: AccessKind,
) -> Result<FuncId> {
    let name = match kind {
        AccessKind::Get => "__roze_list_get",
        AccessKind::Set => "__roze_list_set",
    };
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE)); // ptr
    sig.params.push(AbiParam::new(VALUE_TYPE)); // index
    if matches!(kind, AccessKind::Set) {
        sig.params.push(AbiParam::new(VALUE_TYPE)); // value
    }
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function(name, Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let index = builder.block_params(entry)[1];
        let maybe_value = if matches!(kind, AccessKind::Set) { Some(builder.block_params(entry)[2]) } else { None };

        let length = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        emit_bounds_check(&mut builder, module, printf_id, oob_message, index, length);

        let data_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let offset = builder.ins().imul_imm(index, 8);
        let elem_addr = builder.ins().iadd(data_ptr, offset);

        match maybe_value {
            None => {
                let val = builder.ins().load(VALUE_TYPE, MemFlags::new(), elem_addr, 0);
                builder.ins().return_(&[val]);
            }
            Some(new_value) => {
                let old_val = builder.ins().load(VALUE_TYPE, MemFlags::new(), elem_addr, 0);
                builder.ins().store(MemFlags::new(), new_value, elem_addr, 0);
                builder.ins().return_(&[old_val]);
            }
        }
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_remove(ptr, index) -> i64` (returns the removed value):
/// bounds-checks, then shifts every later element down one slot via a
/// single `memmove` call (correct even when the shifted region is
/// empty -- removing the last element -- since a zero-length memmove is
/// a defined no-op).
fn build_list_remove(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, printf_id: FuncId, oob_message: DataId, memmove_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_remove", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let index = builder.block_params(entry)[1];

        let length = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        emit_bounds_check(&mut builder, module, printf_id, oob_message, index, length);

        let data_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let offset = builder.ins().imul_imm(index, 8);
        let elem_addr = builder.ins().iadd(data_ptr, offset);
        let removed = builder.ins().load(VALUE_TYPE, MemFlags::new(), elem_addr, 0);

        // remaining = length - index - 1 (elements after the removed one)
        let after_index = builder.ins().iadd_imm(index, 1);
        let remaining_count = builder.ins().isub(length, after_index);
        let remaining_bytes = builder.ins().imul_imm(remaining_count, 8);
        let src_addr = builder.ins().iadd_imm(elem_addr, 8);
        let memmove_ref = module.declare_func_in_func(memmove_id, builder.func);
        builder.ins().call(memmove_ref, &[elem_addr, src_addr, remaining_bytes]);

        let new_length = builder.ins().iadd_imm(length, -1);
        builder.ins().store(MemFlags::new(), new_length, ptr, 8);

        builder.ins().return_(&[removed]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_length(ptr: i64) -> i64`
fn build_list_length(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_length", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let length = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        builder.ins().return_(&[length]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_is_empty(ptr: i64) -> i64` (0 or 1)
fn build_list_is_empty(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_is_empty", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let length = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_empty = builder.ins().icmp(IntCC::Equal, length, zero);
        let result = builder.ins().uextend(VALUE_TYPE, is_empty);
        builder.ins().return_(&[result]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// A native `map` value is a pointer to a small, FIXED-size (never
/// reallocated, never moved) header -- the same two-level-indirection
/// idea as `list`, for the same reason (growth needs to be able to
/// move the backing storage without changing the map's own identity):
///
///   [0..8)   i64  refcount
///   [8..16)  i64  count      (live entries)
///   [16..24) i64  capacity   (slots -- always a power of 2)
///   [24..32) i64  slots_ptr  (pointer to a SEPARATELY allocated array)
///
/// Each slot is 24 bytes: `[state: i64][key: i64][value: i64]`, where
/// state is 0 (empty, never used), 1 (occupied), or 2 (tombstone -- had
/// an entry, since removed). Open addressing with linear probing:
/// insert/lookup start at `hash(key) & (capacity - 1)` (capacity being
/// a power of 2 makes this a cheap bitmask instead of a real modulo,
/// and works correctly for negative keys too, since AND operates on
/// the bit pattern regardless of sign) and scan forward, wrapping at
/// the end, until the right kind of slot is found. A tombstone (rather
/// than resetting a removed slot straight back to empty) is necessary
/// for correctness, not just an optimization: resetting a slot to
/// empty on removal could break the probe sequence for some *other*
/// key that happens to hash to the same start index and had to skip
/// past this slot to find its own -- a later lookup for that other key
/// would then stop early at the now-empty slot and incorrectly report
/// it missing.
///
/// Keys and values are both plain i64 words with no ARC of their own,
/// same restriction and same reasoning as `list`'s elements (see
/// `LIST_HEADER_SIZE`'s doc comment) -- scoped to int/bool for now.
///
/// The algorithm here (probing, growth, tombstones) was prototyped and
/// verified in plain C first (allocs/frees checked under Valgrind, and
/// exercised well past a resize with a realistic mix of inserts,
/// updates, removals, and negative keys) before being translated to
/// Cranelift IR -- getting the algorithm right is a different problem
/// from getting the IR construction right, and easier to debug
/// separately than at the same time.
const MAP_HEADER_SIZE: i64 = 32;
const MAP_SLOT_SIZE: i64 = 24;
const MAP_INITIAL_CAPACITY: i64 = 8;
/// A 64-bit multiplicative hash constant (2^64 / golden ratio, the same
/// constant Fibonacci hashing uses) -- reinterpreted as a signed i64
/// (so its top bit is set, making the constant itself negative), which
/// is fine: it's used purely to scramble bits via wrapping
/// multiplication, never compared or treated as a magnitude.
const MAP_HASH_MULTIPLIER: i64 = 0x9E3779B97F4A7C15u64 as i64;

struct MapRuntime {
    new_id: FuncId,
    retain_id: FuncId,
    release_id: FuncId,
    put_id: FuncId,
    get_id: FuncId,
    has_id: FuncId,
    remove_id: FuncId,
    size_id: FuncId,
    is_empty_id: FuncId,
}

fn declare_map_runtime(module: &mut ObjectModule) -> Result<MapRuntime> {
    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let malloc_id = module.declare_function("malloc", Linkage::Import, &malloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(VALUE_TYPE));
    let free_id = module.declare_function("free", Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut calloc_sig = module.make_signature();
    calloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    calloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    calloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let calloc_id = module.declare_function("calloc", Linkage::Import, &calloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut ctx = module.make_context();

    // probe_for_insert/probe_for_lookup are shared internally by
    // put/grow and get/has/remove respectively -- declared once here,
    // passed by FuncId to whichever builders need to call them, rather
    // than each duplicating the probing loop.
    let probe_for_insert_id = build_map_probe_for_insert(module, &mut ctx)?;
    let probe_for_lookup_id = build_map_probe_for_lookup(module, &mut ctx)?;
    let grow_id = build_map_grow(module, &mut ctx, calloc_id, free_id, probe_for_insert_id)?;

    let new_id = build_map_new(module, &mut ctx, malloc_id, calloc_id)?;
    let retain_id = build_map_retain(module, &mut ctx)?;
    let release_id = build_map_release(module, &mut ctx, free_id)?;
    let put_id = build_map_put(module, &mut ctx, grow_id, probe_for_insert_id)?;
    let get_id = build_map_get(module, &mut ctx, probe_for_lookup_id)?;
    let has_id = build_map_has(module, &mut ctx, probe_for_lookup_id)?;
    let remove_id = build_map_remove(module, &mut ctx, probe_for_lookup_id)?;
    let size_id = build_map_size(module, &mut ctx)?;
    let is_empty_id = build_map_is_empty(module, &mut ctx)?;

    Ok(MapRuntime { new_id, retain_id, release_id, put_id, get_id, has_id, remove_id, size_id, is_empty_id })
}

/// Computes `hash(key) & (capacity - 1)` -- the starting slot index for
/// probing. Shared by both probe functions and `build_map_grow`.
fn emit_map_hash_index(builder: &mut FunctionBuilder, key: Value, capacity: Value) -> Value {
    let multiplier = builder.ins().iconst(VALUE_TYPE, MAP_HASH_MULTIPLIER);
    let hash = builder.ins().imul(key, multiplier);
    let mask = builder.ins().iadd_imm(capacity, -1);
    builder.ins().band(hash, mask)
}

/// `__roze_map_probe_for_insert(slots_ptr, capacity, key) -> i64`:
/// returns the index of the first slot that's empty, a tombstone, or
/// already holds this exact key (for an in-place update) -- assumes
/// the caller has already ensured room exists (via the load-factor
/// check in `build_map_put`, before growth), so this cannot loop
/// forever in practice; guarded with a probe-count cap regardless, as
/// a defensive fallback that aborts cleanly rather than hanging if that
/// invariant is ever violated by a bug elsewhere.
fn build_map_probe_for_insert(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE)); // slots_ptr
    sig.params.push(AbiParam::new(VALUE_TYPE)); // capacity
    sig.params.push(AbiParam::new(VALUE_TYPE)); // key
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_probe_for_insert", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let slots_ptr = builder.block_params(entry)[0];
        let capacity = builder.block_params(entry)[1];
        let key = builder.block_params(entry)[2];

        let start_index = emit_map_hash_index(&mut builder, key, capacity);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);

        let header = builder.create_block();
        builder.append_block_param(header, VALUE_TYPE); // index
        builder.append_block_param(header, VALUE_TYPE); // probe_count
        builder.ins().jump(header, &[start_index, zero]);

        builder.switch_to_block(header);
        let index = builder.block_params(header)[0];
        let probe_count = builder.block_params(header)[1];

        let abort_block = builder.create_block();
        let check_block = builder.create_block();
        let exceeded = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, probe_count, capacity);
        builder.ins().brif(exceeded, abort_block, &[], check_block, &[]);

        builder.switch_to_block(abort_block);
        builder.seal_block(abort_block);
        // Internal invariant violation (see the doc comment): trap
        // rather than loop forever or silently corrupt the table.
        builder.ins().trap(TrapCode::UnreachableCodeReached);

        builder.switch_to_block(check_block);
        builder.seal_block(check_block);
        let slot_offset = builder.ins().imul_imm(index, MAP_SLOT_SIZE);
        let slot_addr = builder.ins().iadd(slots_ptr, slot_offset);
        let state = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 0);

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let two = builder.ins().iconst(VALUE_TYPE, 2);
        let is_empty = builder.ins().icmp(IntCC::Equal, state, zero);
        let is_tombstone = builder.ins().icmp(IntCC::Equal, state, two);
        let is_occupied = builder.ins().icmp(IntCC::Equal, state, one);

        let existing_key = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 8);
        let key_matches = builder.ins().icmp(IntCC::Equal, existing_key, key);
        let occupied_and_matches = builder.ins().band(is_occupied, key_matches);

        let empty_or_tombstone = builder.ins().bor(is_empty, is_tombstone);
        let should_stop = builder.ins().bor(empty_or_tombstone, occupied_and_matches);

        let found_block = builder.create_block();
        let advance_block = builder.create_block();
        builder.ins().brif(should_stop, found_block, &[], advance_block, &[]);

        builder.switch_to_block(found_block);
        builder.seal_block(found_block);
        builder.ins().return_(&[index]);

        builder.switch_to_block(advance_block);
        builder.seal_block(advance_block);
        let mask = builder.ins().iadd_imm(capacity, -1);
        let advanced = builder.ins().iadd_imm(index, 1);
        let next_index = builder.ins().band(advanced, mask);
        let next_probe_count = builder.ins().iadd_imm(probe_count, 1);
        builder.ins().jump(header, &[next_index, next_probe_count]);

        // header has two predecessors (the initial jump and this
        // back-edge) -- seal only now that both exist.
        builder.seal_block(header);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_probe_for_lookup(slots_ptr, capacity, key) -> i64`:
/// returns the matching slot's index, or -1 if the key is definitely
/// absent (found an empty slot, or exhausted every slot without one).
fn build_map_probe_for_lookup(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE)); // slots_ptr
    sig.params.push(AbiParam::new(VALUE_TYPE)); // capacity
    sig.params.push(AbiParam::new(VALUE_TYPE)); // key
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_probe_for_lookup", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let slots_ptr = builder.block_params(entry)[0];
        let capacity = builder.block_params(entry)[1];
        let key = builder.block_params(entry)[2];

        let start_index = emit_map_hash_index(&mut builder, key, capacity);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);

        let header = builder.create_block();
        builder.append_block_param(header, VALUE_TYPE); // index
        builder.append_block_param(header, VALUE_TYPE); // probe_count
        builder.ins().jump(header, &[start_index, zero]);

        builder.switch_to_block(header);
        let index = builder.block_params(header)[0];
        let probe_count = builder.block_params(header)[1];

        let not_found_block = builder.create_block();
        let check_block = builder.create_block();
        let exceeded = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, probe_count, capacity);
        builder.ins().brif(exceeded, not_found_block, &[], check_block, &[]);

        builder.switch_to_block(check_block);
        builder.seal_block(check_block);
        let slot_offset = builder.ins().imul_imm(index, MAP_SLOT_SIZE);
        let slot_addr = builder.ins().iadd(slots_ptr, slot_offset);
        let state = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 0);
        let is_empty = builder.ins().icmp(IntCC::Equal, state, zero);

        let empty_stop_block = builder.create_block();
        let match_check_block = builder.create_block();
        builder.ins().brif(is_empty, empty_stop_block, &[], match_check_block, &[]);

        builder.switch_to_block(empty_stop_block);
        builder.seal_block(empty_stop_block);
        builder.ins().jump(not_found_block, &[]);

        builder.switch_to_block(match_check_block);
        builder.seal_block(match_check_block);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let is_occupied = builder.ins().icmp(IntCC::Equal, state, one);
        let existing_key = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 8);
        let key_matches = builder.ins().icmp(IntCC::Equal, existing_key, key);
        let found = builder.ins().band(is_occupied, key_matches);

        let found_block = builder.create_block();
        let advance_block = builder.create_block();
        builder.ins().brif(found, found_block, &[], advance_block, &[]);

        builder.switch_to_block(found_block);
        builder.seal_block(found_block);
        builder.ins().return_(&[index]);

        builder.switch_to_block(advance_block);
        builder.seal_block(advance_block);
        let mask = builder.ins().iadd_imm(capacity, -1);
        let advanced = builder.ins().iadd_imm(index, 1);
        let next_index = builder.ins().band(advanced, mask);
        let next_probe_count = builder.ins().iadd_imm(probe_count, 1);
        builder.ins().jump(header, &[next_index, next_probe_count]);

        // header has two predecessors (the initial jump and this
        // back-edge) -- seal only now that both exist.
        builder.seal_block(header);

        // not_found_block has two predecessors (the probe-count-
        // exceeded branch and empty_stop_block's jump) -- seal only now.
        builder.switch_to_block(not_found_block);
        builder.seal_block(not_found_block);
        let neg_one = builder.ins().iconst(VALUE_TYPE, -1);
        builder.ins().return_(&[neg_one]);

        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_new() -> i64`
fn build_map_new(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, malloc_id: FuncId, calloc_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_new", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let malloc_ref = module.declare_func_in_func(malloc_id, builder.func);
        let header_size = builder.ins().iconst(VALUE_TYPE, MAP_HEADER_SIZE);
        let call = builder.ins().call(malloc_ref, &[header_size]);
        let header_ptr = builder.inst_results(call)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().store(MemFlags::new(), one, header_ptr, 0);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        builder.ins().store(MemFlags::new(), zero, header_ptr, 8);
        let initial_capacity = builder.ins().iconst(VALUE_TYPE, MAP_INITIAL_CAPACITY);
        builder.ins().store(MemFlags::new(), initial_capacity, header_ptr, 16);

        let calloc_ref = module.declare_func_in_func(calloc_id, builder.func);
        let slot_size = builder.ins().iconst(VALUE_TYPE, MAP_SLOT_SIZE);
        let slots_call = builder.ins().call(calloc_ref, &[initial_capacity, slot_size]);
        let slots_ptr = builder.inst_results(slots_call)[0];
        builder.ins().store(MemFlags::new(), slots_ptr, header_ptr, 24);

        builder.ins().return_(&[header_ptr]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_retain(ptr: i64) -> void`
fn build_map_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_retain", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 0);
        let incremented = builder.ins().iadd_imm(refcount, 1);
        builder.ins().store(MemFlags::new(), incremented, ptr, 0);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_release(ptr: i64) -> void`
fn build_map_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, free_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_release", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 0);
        let decremented = builder.ins().iadd_imm(refcount, -1);
        builder.ins().store(MemFlags::new(), decremented, ptr, 0);

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let should_free = builder.ins().icmp(IntCC::Equal, decremented, zero);
        let free_block = builder.create_block();
        let done_block = builder.create_block();
        builder.ins().brif(should_free, free_block, &[], done_block, &[]);

        builder.switch_to_block(free_block);
        builder.seal_block(free_block);
        let slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let free_ref = module.declare_func_in_func(free_id, builder.func);
        builder.ins().call(free_ref, &[slots_ptr]);
        builder.ins().call(free_ref, &[ptr]);
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

/// `__roze_map_grow(ptr: i64) -> void`: doubles capacity and rehashes
/// every live entry into the new table, reusing
/// `__roze_map_probe_for_insert` for each reinsertion (the new table
/// starts completely empty, so there's always room -- no load-factor
/// check needed on the way in).
fn build_map_grow(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, calloc_id: FuncId, free_id: FuncId, probe_for_insert_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_grow", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let old_capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);
        let old_slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let new_capacity = builder.ins().imul_imm(old_capacity, 2);

        let calloc_ref = module.declare_func_in_func(calloc_id, builder.func);
        let slot_size = builder.ins().iconst(VALUE_TYPE, MAP_SLOT_SIZE);
        let call = builder.ins().call(calloc_ref, &[new_capacity, slot_size]);
        let new_slots_ptr = builder.inst_results(call)[0];

        // Rehash loop: for old_index in 0..old_capacity.
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let loop_header = builder.create_block();
        builder.append_block_param(loop_header, VALUE_TYPE); // old_index
        builder.ins().jump(loop_header, &[zero]);

        builder.switch_to_block(loop_header);
        let old_index = builder.block_params(loop_header)[0];
        let loop_body = builder.create_block();
        let loop_exit = builder.create_block();
        let done = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, old_index, old_capacity);
        builder.ins().brif(done, loop_exit, &[], loop_body, &[]);

        builder.switch_to_block(loop_body);
        builder.seal_block(loop_body);
        let slot_offset = builder.ins().imul_imm(old_index, MAP_SLOT_SIZE);
        let slot_addr = builder.ins().iadd(old_slots_ptr, slot_offset);
        let state = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 0);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let is_occupied = builder.ins().icmp(IntCC::Equal, state, one);

        let reinsert_block = builder.create_block();
        let next_block = builder.create_block();
        builder.ins().brif(is_occupied, reinsert_block, &[], next_block, &[]);

        builder.switch_to_block(reinsert_block);
        builder.seal_block(reinsert_block);
        let key = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 8);
        let value = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 16);
        let probe_ref = module.declare_func_in_func(probe_for_insert_id, builder.func);
        let probe_call = builder.ins().call(probe_ref, &[new_slots_ptr, new_capacity, key]);
        let new_index = builder.inst_results(probe_call)[0];
        let new_slot_offset = builder.ins().imul_imm(new_index, MAP_SLOT_SIZE);
        let new_slot_addr = builder.ins().iadd(new_slots_ptr, new_slot_offset);
        builder.ins().store(MemFlags::new(), one, new_slot_addr, 0);
        builder.ins().store(MemFlags::new(), key, new_slot_addr, 8);
        builder.ins().store(MemFlags::new(), value, new_slot_addr, 16);
        builder.ins().jump(next_block, &[]);

        builder.switch_to_block(next_block);
        builder.seal_block(next_block);
        let next_index = builder.ins().iadd_imm(old_index, 1);
        builder.ins().jump(loop_header, &[next_index]);

        // loop_header has two predecessors (the initial jump and this
        // back-edge) -- seal only now that both exist.
        builder.seal_block(loop_header);

        builder.switch_to_block(loop_exit);
        builder.seal_block(loop_exit);
        let free_ref = module.declare_func_in_func(free_id, builder.func);
        builder.ins().call(free_ref, &[old_slots_ptr]);
        builder.ins().store(MemFlags::new(), new_slots_ptr, ptr, 24);
        builder.ins().store(MemFlags::new(), new_capacity, ptr, 16);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_put(ptr, key, value) -> i64`: returns the old value if
/// `key` already existed, else 0 (Roze has no null-distinct-from-0
/// representation yet on this backend -- same simplification
/// `__roze_map_get` makes for a missing key).
fn build_map_put(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, grow_id: FuncId, probe_for_insert_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_put", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let key = builder.block_params(entry)[1];
        let value = builder.block_params(entry)[2];

        let count = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        let capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);

        // Grow when (count + 1) * 4 > capacity * 3, i.e. load factor
        // would exceed 75% after this insert.
        let count_plus_1 = builder.ins().iadd_imm(count, 1);
        let lhs = builder.ins().imul_imm(count_plus_1, 4);
        let rhs = builder.ins().imul_imm(capacity, 3);
        let needs_grow = builder.ins().icmp(IntCC::SignedGreaterThan, lhs, rhs);

        let grow_block = builder.create_block();
        let after_grow_block = builder.create_block();
        builder.append_block_param(after_grow_block, VALUE_TYPE); // slots_ptr
        builder.append_block_param(after_grow_block, VALUE_TYPE); // capacity

        let slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        builder.ins().brif(needs_grow, grow_block, &[], after_grow_block, &[slots_ptr, capacity]);

        builder.switch_to_block(grow_block);
        builder.seal_block(grow_block);
        let grow_ref = module.declare_func_in_func(grow_id, builder.func);
        builder.ins().call(grow_ref, &[ptr]);
        let grown_slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let grown_capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);
        builder.ins().jump(after_grow_block, &[grown_slots_ptr, grown_capacity]);

        builder.switch_to_block(after_grow_block);
        builder.seal_block(after_grow_block);
        let current_slots_ptr = builder.block_params(after_grow_block)[0];
        let current_capacity = builder.block_params(after_grow_block)[1];

        let probe_ref = module.declare_func_in_func(probe_for_insert_id, builder.func);
        let probe_call = builder.ins().call(probe_ref, &[current_slots_ptr, current_capacity, key]);
        let index = builder.inst_results(probe_call)[0];
        let slot_offset = builder.ins().imul_imm(index, MAP_SLOT_SIZE);
        let slot_addr = builder.ins().iadd(current_slots_ptr, slot_offset);

        let state = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 0);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let is_new_entry = builder.ins().icmp(IntCC::NotEqual, state, one);
        let old_value = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 16);

        builder.ins().store(MemFlags::new(), one, slot_addr, 0);
        builder.ins().store(MemFlags::new(), key, slot_addr, 8);
        builder.ins().store(MemFlags::new(), value, slot_addr, 16);

        let increment_block = builder.create_block();
        let return_block = builder.create_block();
        builder.append_block_param(return_block, VALUE_TYPE); // result
        builder.ins().brif(is_new_entry, increment_block, &[], return_block, &[old_value]);

        builder.switch_to_block(increment_block);
        builder.seal_block(increment_block);
        let new_count = builder.ins().iadd_imm(count, 1);
        builder.ins().store(MemFlags::new(), new_count, ptr, 8);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        builder.ins().jump(return_block, &[zero]);

        builder.switch_to_block(return_block);
        builder.seal_block(return_block);
        let result = builder.block_params(return_block)[0];
        builder.ins().return_(&[result]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_get(ptr, key) -> i64` (0 if the key is absent -- same
/// no-null-representation simplification as `__roze_map_put`'s "new
/// key" return; use `__roze_map_has` to distinguish "absent" from "0").
fn build_map_get(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, probe_for_lookup_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_get", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let key = builder.block_params(entry)[1];

        let capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);
        let slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let probe_ref = module.declare_func_in_func(probe_for_lookup_id, builder.func);
        let call = builder.ins().call(probe_ref, &[slots_ptr, capacity, key]);
        let index = builder.inst_results(call)[0];

        let neg_one = builder.ins().iconst(VALUE_TYPE, -1);
        let not_found = builder.ins().icmp(IntCC::Equal, index, neg_one);

        let missing_block = builder.create_block();
        let present_block = builder.create_block();
        builder.ins().brif(not_found, missing_block, &[], present_block, &[]);

        builder.switch_to_block(missing_block);
        builder.seal_block(missing_block);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        builder.ins().return_(&[zero]);

        builder.switch_to_block(present_block);
        builder.seal_block(present_block);
        let slot_offset = builder.ins().imul_imm(index, MAP_SLOT_SIZE);
        let slot_addr = builder.ins().iadd(slots_ptr, slot_offset);
        let value = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 16);
        builder.ins().return_(&[value]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_has(ptr, key) -> i64` (0 or 1)
fn build_map_has(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, probe_for_lookup_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_has", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let key = builder.block_params(entry)[1];

        let capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);
        let slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let probe_ref = module.declare_func_in_func(probe_for_lookup_id, builder.func);
        let call = builder.ins().call(probe_ref, &[slots_ptr, capacity, key]);
        let index = builder.inst_results(call)[0];

        let neg_one = builder.ins().iconst(VALUE_TYPE, -1);
        let found = builder.ins().icmp(IntCC::NotEqual, index, neg_one);
        let result = builder.ins().uextend(VALUE_TYPE, found);
        builder.ins().return_(&[result]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_remove(ptr, key) -> i64` (the removed value, or 0 if
/// the key wasn't present)
fn build_map_remove(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, probe_for_lookup_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_remove", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let key = builder.block_params(entry)[1];

        let capacity = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 16);
        let slots_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 24);
        let probe_ref = module.declare_func_in_func(probe_for_lookup_id, builder.func);
        let call = builder.ins().call(probe_ref, &[slots_ptr, capacity, key]);
        let index = builder.inst_results(call)[0];

        let neg_one = builder.ins().iconst(VALUE_TYPE, -1);
        let not_found = builder.ins().icmp(IntCC::Equal, index, neg_one);

        let missing_block = builder.create_block();
        let remove_block = builder.create_block();
        builder.ins().brif(not_found, missing_block, &[], remove_block, &[]);

        builder.switch_to_block(missing_block);
        builder.seal_block(missing_block);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        builder.ins().return_(&[zero]);

        builder.switch_to_block(remove_block);
        builder.seal_block(remove_block);
        let slot_offset = builder.ins().imul_imm(index, MAP_SLOT_SIZE);
        let slot_addr = builder.ins().iadd(slots_ptr, slot_offset);
        let value = builder.ins().load(VALUE_TYPE, MemFlags::new(), slot_addr, 16);
        let two = builder.ins().iconst(VALUE_TYPE, 2);
        builder.ins().store(MemFlags::new(), two, slot_addr, 0);
        let count = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        let new_count = builder.ins().iadd_imm(count, -1);
        builder.ins().store(MemFlags::new(), new_count, ptr, 8);
        builder.ins().return_(&[value]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_size(ptr: i64) -> i64`
fn build_map_size(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_size", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let count = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        builder.ins().return_(&[count]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_is_empty(ptr: i64) -> i64` (0 or 1)
fn build_map_is_empty(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_is_empty", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];
        let count = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_empty = builder.ins().icmp(IntCC::Equal, count, zero);
        let result = builder.ins().uextend(VALUE_TYPE, is_empty);
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
    lists: ListRuntime,
    maps: MapRuntime,
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
        let lists = declare_list_runtime(module, printf_id)?;
        let maps = declare_map_runtime(module)?;

        Ok(Self {
            module,
            functions: HashMap::new(),
            printf_id,
            fmt_int,
            fmt_str,
            true_str,
            false_str,
            strings,
            lists,
            maps,
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
            if let TypedStatement::Function { name, params, return_type, location, .. } = stmt {
                for param in params {
                    require_supported_type(&param.type_, location, &format!("parameter '{}' of function '{}'", param.name, name))?;
                }
                require_supported_type(return_type, location, &format!("the return type of function '{}'", name))?;

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
                        list_new_id: self.lists.new_id,
                        list_retain_id: self.lists.retain_id,
                        list_release_id: self.lists.release_id,
                        list_push_id: self.lists.push_id,
                        list_get_id: self.lists.get_id,
                        list_set_id: self.lists.set_id,
                        list_remove_id: self.lists.remove_id,
                        list_length_id: self.lists.length_id,
                        list_is_empty_id: self.lists.is_empty_id,
                        map_new_id: self.maps.new_id,
                        map_retain_id: self.maps.retain_id,
                        map_release_id: self.maps.release_id,
                        map_put_id: self.maps.put_id,
                        map_get_id: self.maps.get_id,
                        map_has_id: self.maps.has_id,
                        map_remove_id: self.maps.remove_id,
                        map_size_id: self.maps.size_id,
                        map_is_empty_id: self.maps.is_empty_id,
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
    list_new_id: FuncId,
    list_retain_id: FuncId,
    list_release_id: FuncId,
    list_push_id: FuncId,
    list_get_id: FuncId,
    list_set_id: FuncId,
    list_remove_id: FuncId,
    list_length_id: FuncId,
    list_is_empty_id: FuncId,
    map_new_id: FuncId,
    map_retain_id: FuncId,
    map_release_id: FuncId,
    map_put_id: FuncId,
    map_get_id: FuncId,
    map_has_id: FuncId,
    map_remove_id: FuncId,
    map_size_id: FuncId,
    map_is_empty_id: FuncId,
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
        let arc_vars: Vec<(Variable, Type)> = self.scopes[frame_index]
            .values()
            .filter(|(_, ty)| matches!(ty, Type::String | Type::List | Type::Map))
            .cloned()
            .collect();
        for (var, ty) in arc_vars {
            let val = self.builder.use_var(var);
            self.emit_release_for_type(&ty, val)?;
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
        if let TypedExpressionKind::Identifier(_) = &expr.kind {
            match expr.type_ {
                Type::String => self.emit_string_retain(val)?,
                Type::List => self.emit_list_retain(val)?,
                Type::Map => self.emit_map_retain(val)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Native `list` elements and `map` keys/values are plain i64 words
    /// with no ARC of their own (see the module-level doc comments on
    /// `LIST_HEADER_SIZE`/`MAP_HEADER_SIZE`), which is only safe for
    /// int/bool values -- a string or another list/map stored this way
    /// would compile, but silently do the wrong thing at runtime (its
    /// refcount would never be adjusted for being held by the
    /// container, so it could be freed while the container still
    /// points at it, or never freed at all). Reject those cases at
    /// compile time instead of ever executing them.
    fn reject_unless_supported_as_container_value(&self, value: &TypedExpression, location: &Location, context: &str) -> Result<()> {
        match &value.type_ {
            Type::Int | Type::Bool | Type::Unknown => Ok(()),
            other => Err(anyhow!(
                "line {}, column {}: the native backend's lists and maps can only hold int/bool values for now, not {} -- {}",
                location.line, location.column, other, context
            )),
        }
    }

    /// Releases `val` per its Roze type, if it's an ARC type at all
    /// (String or List) -- a no-op for everything else. Centralizing
    /// this dispatch in one place, rather than repeating `if ty ==
    /// Type::String` at every release site, is what actually matters
    /// here: that exact per-site duplication is what let a real bug
    /// through during development -- `Return`, `Assign`, and bare
    /// `Expression`-statement cleanup were each independently checking
    /// only `Type::String`, so a `list` value went un-retained/
    /// un-released at every one of those sites until this was
    /// consolidated (caught by Valgrind: a list was being freed by its
    /// own function before ever reaching the caller it was returned to).
    fn emit_release_for_type(&mut self, ty: &Type, val: Value) -> Result<()> {
        match ty {
            Type::String => self.emit_string_release(val),
            Type::List => self.emit_list_release(val),
            Type::Map => self.emit_map_release(val),
            _ => Ok(()),
        }
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

    fn emit_list_new(&mut self) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_new_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_retain(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.list_retain_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_list_release(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.list_release_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_list_push(&mut self, list: Value, value: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_push_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[list, value]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_get(&mut self, list: Value, index: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_get_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[list, index]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_set(&mut self, list: Value, index: Value, value: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_set_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[list, index, value]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_remove(&mut self, list: Value, index: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_remove_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[list, index]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_length(&mut self, list: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_length_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[list]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_list_is_empty(&mut self, list: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.list_is_empty_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[list]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_new(&mut self) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_new_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_retain(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.map_retain_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_map_release(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.map_release_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_map_put(&mut self, map: Value, key: Value, value: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_put_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[map, key, value]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_get(&mut self, map: Value, key: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_get_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[map, key]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_has(&mut self, map: Value, key: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_has_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[map, key]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_remove(&mut self, map: Value, key: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_remove_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[map, key]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_size(&mut self, map: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_size_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[map]);
        Ok(self.builder.inst_results(call)[0])
    }

    fn emit_map_is_empty(&mut self, map: Value) -> Result<Value> {
        let func_ref = self.module.declare_func_in_func(self.map_is_empty_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, &[map]);
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
                require_supported_type(&value.type_, &value.location, &format!("variable '{}'", name))?;
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
                let old_val = self.builder.use_var(var);
                self.emit_release_for_type(&ty, old_val)?;
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Expression { expr, .. } => {
                let val = self.compile_expression(expr)?;
                if !matches!(expr.kind, TypedExpressionKind::Identifier(_)) {
                    self.emit_release_for_type(&expr.type_, val)?;
                }
                Ok(false)
            }
            TypedStatement::Return { value, .. } => {
                match value {
                    Some(expr) => {
                        let val = self.compile_expression(expr)?;
                        self.retain_if_aliasing(expr, val)?;
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
            TypedExpressionKind::Call { function, arguments } => self.compile_call(function, arguments, &expr.location),
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

    fn compile_call(&mut self, name: &str, arguments: &[TypedExpression], location: &Location) -> Result<Value> {
        if name == "println" {
            return self.compile_println(arguments);
        }

        match name {
            "list_new" if arguments.is_empty() => return self.emit_list_new(),
            "list_push" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "list_push")?;
                let list = self.compile_expression(&arguments[0])?;
                let value = self.compile_expression(&arguments[1])?;
                return self.emit_list_push(list, value);
            }
            "list_get" if arguments.len() == 2 => {
                let list = self.compile_expression(&arguments[0])?;
                let index = self.compile_expression(&arguments[1])?;
                return self.emit_list_get(list, index);
            }
            "list_set" if arguments.len() == 3 => {
                self.reject_unless_supported_as_container_value(&arguments[2], location, "list_set")?;
                let list = self.compile_expression(&arguments[0])?;
                let index = self.compile_expression(&arguments[1])?;
                let value = self.compile_expression(&arguments[2])?;
                return self.emit_list_set(list, index, value);
            }
            "list_remove" if arguments.len() == 2 => {
                let list = self.compile_expression(&arguments[0])?;
                let index = self.compile_expression(&arguments[1])?;
                return self.emit_list_remove(list, index);
            }
            "list_length" if arguments.len() == 1 => {
                let list = self.compile_expression(&arguments[0])?;
                return self.emit_list_length(list);
            }
            "list_is_empty" if arguments.len() == 1 => {
                let list = self.compile_expression(&arguments[0])?;
                return self.emit_list_is_empty(list);
            }
            "map_new" if arguments.is_empty() => return self.emit_map_new(),
            "map_put" if arguments.len() == 3 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_put (key)")?;
                self.reject_unless_supported_as_container_value(&arguments[2], location, "map_put (value)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                let value = self.compile_expression(&arguments[2])?;
                return self.emit_map_put(map, key, value);
            }
            "map_get" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_get (key)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                return self.emit_map_get(map, key);
            }
            "map_has" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_has (key)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                return self.emit_map_has(map, key);
            }
            "map_remove" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_remove (key)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                return self.emit_map_remove(map, key);
            }
            "map_size" if arguments.len() == 1 => {
                let map = self.compile_expression(&arguments[0])?;
                return self.emit_map_size(map);
            }
            "map_is_empty" if arguments.len() == 1 => {
                let map = self.compile_expression(&arguments[0])?;
                return self.emit_map_is_empty(map);
            }
            _ => {}
        }

        if super::jvm::is_intrinsic(name) {
            return Err(anyhow!(
                "line {}, column {}: '{}' is a Core/Collections/IO/Web/Database intrinsic, which is only available on the JVM backend today (see docs/MEMORY_MODEL_DECISION.md for why the native backend doesn't have these yet)",
                location.line, location.column, name
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
