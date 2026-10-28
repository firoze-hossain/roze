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
use crate::ir::{TypedExpression, TypedExpressionKind, TypedPattern, TypedProgram, TypedStatement};
use crate::parser::ast::{BinaryOperator, Location, UnaryOperator};
use crate::semantic::Type;
use anyhow::{anyhow, Result};
use cranelift::prelude::*;
use cranelift_module::{DataDescription, DataId, FuncId, FuncOrDataId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::process::Command;
use cranelift::codegen::ir::AtomicRmwOp;

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

#[derive(Clone)]
struct ClosureDecl {
    id: usize,
    params: Vec<(String, Type)>,
    return_type: Type,
    captures: Vec<(String, Type)>,
    body: Box<TypedStatement>,
}

fn collect_closures_and_refs_from_stmt(
    stmt: &TypedStatement,
    closures: &mut Vec<ClosureDecl>,
    func_refs: &mut HashSet<String>,
) {
    match stmt {
        TypedStatement::Block { statements, .. } => {
            for s in statements {
                collect_closures_and_refs_from_stmt(s, closures, func_refs);
            }
        }
        TypedStatement::Expression { expr, .. } => {
            collect_closures_and_refs_from_expr(expr, closures, func_refs);
        }
        TypedStatement::Let { value, .. } => {
            collect_closures_and_refs_from_expr(value, closures, func_refs);
        }
        TypedStatement::Assign { value, .. } => {
            collect_closures_and_refs_from_expr(value, closures, func_refs);
        }
        TypedStatement::FieldAssign { object, value, .. } => {
            collect_closures_and_refs_from_expr(object, closures, func_refs);
            collect_closures_and_refs_from_expr(value, closures, func_refs);
        }
        TypedStatement::If { condition, then_branch, else_branch, .. } => {
            collect_closures_and_refs_from_expr(condition, closures, func_refs);
            collect_closures_and_refs_from_stmt(then_branch, closures, func_refs);
            if let Some(other) = else_branch {
                collect_closures_and_refs_from_stmt(other, closures, func_refs);
            }
        }
        TypedStatement::While { condition, body, .. } => {
            collect_closures_and_refs_from_expr(condition, closures, func_refs);
            collect_closures_and_refs_from_stmt(body, closures, func_refs);
        }
        TypedStatement::For { init, condition, update, body, .. } => {
            collect_closures_and_refs_from_stmt(init, closures, func_refs);
            collect_closures_and_refs_from_expr(condition, closures, func_refs);
            collect_closures_and_refs_from_stmt(update, closures, func_refs);
            collect_closures_and_refs_from_stmt(body, closures, func_refs);
        }
        TypedStatement::Return { value, .. } => {
            if let Some(val) = value {
                collect_closures_and_refs_from_expr(val, closures, func_refs);
            }
        }
        TypedStatement::Match { expr, arms, .. } => {
            collect_closures_and_refs_from_expr(expr, closures, func_refs);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_closures_and_refs_from_expr(g, closures, func_refs);
                }
                collect_closures_and_refs_from_stmt(&arm.body, closures, func_refs);
            }
        }
        TypedStatement::Function { body, .. } => {
            collect_closures_and_refs_from_stmt(body, closures, func_refs);
        }
        TypedStatement::ImplDecl { methods, .. } => {
            for m in methods {
                collect_closures_and_refs_from_stmt(m, closures, func_refs);
            }
        }
        TypedStatement::UnsafeBlock { body, .. } => {
            collect_closures_and_refs_from_stmt(body, closures, func_refs);
        }
        TypedStatement::Comptime { body, .. } => {
            collect_closures_and_refs_from_stmt(body, closures, func_refs);
        }
        TypedStatement::ClassDecl { .. } | TypedStatement::EnumDecl { .. } | TypedStatement::TraitDecl { .. } | TypedStatement::ExternFunction { .. } => {}
    }
}

fn collect_closures_and_refs_from_expr(
    expr: &TypedExpression,
    closures: &mut Vec<ClosureDecl>,
    func_refs: &mut HashSet<String>,
) {
    match &expr.kind {
        TypedExpressionKind::Closure { id, params, return_type, captures, body } => {
            closures.push(ClosureDecl {
                id: *id,
                params: params.clone(),
                return_type: return_type.clone(),
                captures: captures.clone(),
                body: body.clone(),
            });
            collect_closures_and_refs_from_stmt(body, closures, func_refs);
        }
        TypedExpressionKind::FunctionRef(name) => {
            func_refs.insert(name.clone());
        }
        TypedExpressionKind::IndirectCall { callee, arguments } => {
            collect_closures_and_refs_from_expr(callee, closures, func_refs);
            for arg in arguments {
                collect_closures_and_refs_from_expr(arg, closures, func_refs);
            }
        }
        TypedExpressionKind::Call { arguments, .. } => {
            for arg in arguments {
                collect_closures_and_refs_from_expr(arg, closures, func_refs);
            }
        }
        TypedExpressionKind::Binary { left, right, .. } => {
            collect_closures_and_refs_from_expr(left, closures, func_refs);
            collect_closures_and_refs_from_expr(right, closures, func_refs);
        }
        TypedExpressionKind::Unary { operand, .. } => {
            collect_closures_and_refs_from_expr(operand, closures, func_refs);
        }
        TypedExpressionKind::FieldAccess { object, .. } => {
            collect_closures_and_refs_from_expr(object, closures, func_refs);
        }
        TypedExpressionKind::New { arguments, .. } => {
            for arg in arguments {
                collect_closures_and_refs_from_expr(arg, closures, func_refs);
            }
        }
        TypedExpressionKind::EnumConstruct { arguments, .. } => {
            for arg in arguments {
                collect_closures_and_refs_from_expr(arg, closures, func_refs);
            }
        }
        TypedExpressionKind::Try(inner) => {
            collect_closures_and_refs_from_expr(inner, closures, func_refs);
        }
        TypedExpressionKind::MethodCall { object, arguments, .. } |
        TypedExpressionKind::TraitMethodCall { object, arguments, .. } => {
            collect_closures_and_refs_from_expr(object, closures, func_refs);
            for arg in arguments {
                collect_closures_and_refs_from_expr(arg, closures, func_refs);
            }
        }
        TypedExpressionKind::String(_)
        | TypedExpressionKind::Number(_)
        | TypedExpressionKind::Identifier(_)
        | TypedExpressionKind::Boolean(_)
        | TypedExpressionKind::Null => {}
    }
}

/// Compiles `program` (the same typed IR the JVM backend consumes) to a
/// native object file, then links it into an executable using the
/// system's C compiler -- present on essentially every dev machine
/// already, and already the same kind of reliance the JVM backend has
/// on an external tool (`javac`).
pub fn compile_to_native(program: TypedProgram, input_file: &str, extra_libs: &[String]) -> Result<()> {
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
        let mut closures = Vec::new();
        let mut func_refs = HashSet::new();
        for stmt in &program.statements {
            collect_closures_and_refs_from_stmt(stmt, &mut closures, &mut func_refs);
        }

        let mut generator = NativeGenerator::new(&mut module)?;
        generator.declare_all_classes(&program)?;
        generator.declare_all_functions(&program)?;
        generator.declare_all_closures_and_trampolines(&closures, &func_refs, &program)?;
        generator.compile_trampolines(&func_refs, &program)?;
        generator.compile_closures(&closures)?;
        generator.compile_all_methods(&program)?;
        generator.compile_all_functions(&program)?;
    }

    let product = module.finish();
    let bytes = product.emit().map_err(|e| anyhow!(e.to_string()))?;
    let obj_path = format!("{}.o", output_name);
    fs::write(&obj_path, &bytes)?;
    println!("📝 Generated native object file: {}", obj_path);

    let mut all_libs = extra_libs.to_vec();
    let temp_concurrency_c = std::env::temp_dir().join("roze_concurrency.c");
    fs::write(&temp_concurrency_c, CONCURRENCY_RUNTIME_C)?;
    all_libs.push(temp_concurrency_c.to_string_lossy().to_string());
    all_libs.push("pthread".to_string());

    link_object_file(&obj_path, &output_name, &all_libs)?;

    Ok(())
}

pub const DEFAULT_FREESTANDING_LINKER_SCRIPT: &str = r#"ENTRY(_start)

SECTIONS
{
    . = 1M;

    .boot : ALIGN(4K)
    {
        *(.boot)
    }

    .text : ALIGN(4K)
    {
        *(.text*)
    }

    .rodata : ALIGN(4K)
    {
        *(.rodata*)
    }

    .data : ALIGN(4K)
    {
        *(.data*)
    }

    .bss : ALIGN(4K)
    {
        *(COMMON)
        *(.bss*)
    }
}
"#;

pub const FREESTANDING_RUNTIME_C: &str = r#"
// Roze Bare-Metal Freestanding Runtime (Zero libc, Zero glibc, Zero OS)

// 1. Multiboot 1 & Multiboot 2 Boot Trampoline (32-bit Protected Mode -> 64-bit Long Mode)
__asm__(
".section .boot, \"ax\"\n"
".align 4\n"
"    /* Multiboot 1 Header */\n"
"    .long 0x1BADB002\n"
"    .long 0x00000003\n"
"    .long -(0x1BADB002 + 0x00000003)\n"
"\n"
".align 8\n"
"mb2_start:\n"
"    /* Multiboot 2 Header */\n"
"    .long 0xE85250D6\n"
"    .long 0\n"
"    .long mb2_end - mb2_start\n"
"    .long -(0xE85250D6 + 0 + (mb2_end - mb2_start))\n"
"    .word 0\n"
"    .word 0\n"
"    .long 8\n"
"mb2_end:\n"
"\n"
".weak _start\n"
".globl _start\n"
"_start:\n"
".code32\n"
"    cli\n"
"    cld\n"
"    movl %eax, (mboot_magic)\n"
"    movl %ebx, (mboot_info)\n"
"    movl $stack_top, %esp\n"
"\n"
"    /* Setup 1GB identity paging with 2MB huge pages */\n"
"    movl $boot_pdpt, %eax\n"
"    orl $0x3, %eax\n"
"    movl %eax, (boot_pml4)\n"
"\n"
"    movl $boot_pd, %eax\n"
"    orl $0x3, %eax\n"
"    movl %eax, (boot_pdpt)\n"
"\n"
"    xorl %ecx, %ecx\n"
"1:\n"
"    movl $0x200000, %eax\n"
"    mull %ecx\n"
"    orl $0x83, %eax\n"
"    movl %eax, boot_pd(,%ecx,8)\n"
"    movl $0, boot_pd+4(,%ecx,8)\n"
"    incl %ecx\n"
"    cmpl $512, %ecx\n"
"    jne 1b\n"
"\n"
"    /* Enable PAE in CR4 */\n"
"    movl %cr4, %eax\n"
"    orl $(1 << 5), %eax\n"
"    movl %eax, %cr4\n"
"\n"
"    /* Load CR3 with PML4 */\n"
"    movl $boot_pml4, %eax\n"
"    movl %eax, %cr3\n"
"\n"
"    /* Enable Long Mode in EFER MSR */\n"
"    movl $0xC0000080, %ecx\n"
"    rdmsr\n"
"    orl $(1 << 8), %eax\n"
"    wrmsr\n"
"\n"
"    /* Enable Paging and Protected Mode in CR0 */\n"
"    movl %cr0, %eax\n"
"    orl $((1 << 31) | 1), %eax\n"
"    movl %eax, %cr0\n"
"\n"
"    /* Load 64-bit GDT */\n"
"    lgdt (gdt64_ptr)\n"
"\n"
"    /* Far jump into 64-bit Long Mode */\n"
"    ljmp $0x08, $entry64\n"
"\n"
".code64\n"
"entry64:\n"
"    xorw %ax, %ax\n"
"    movw %ax, %ds\n"
"    movw %ax, %es\n"
"    movw %ax, %fs\n"
"    movw %ax, %gs\n"
"    movw %ax, %ss\n"
"    movq $stack_top, %rsp\n"
"\n"
"    call __roze_bootstrap_main\n"
"\n"
".Lhalt:\n"
"    cli\n"
"    hlt\n"
"    jmp .Lhalt\n"
"\n"
".section .data\n"
".align 8\n"
"gdt64:\n"
"    .quad 0\n"
"    .quad 0x00209A0000000000\n"
"    .quad 0x0000920000000000\n"
"gdt64_ptr:\n"
"    .word . - gdt64 - 1\n"
"    .quad gdt64\n"
"\n"
".globl mboot_magic\n"
"mboot_magic:\n"
"    .long 0\n"
".globl mboot_info\n"
"mboot_info:\n"
"    .long 0\n"
"\n"
".section .bss\n"
".align 4096\n"
"boot_pml4:\n"
"    .skip 4096\n"
"boot_pdpt:\n"
"    .skip 4096\n"
"boot_pd:\n"
"    .skip 4096\n"
"kernel_stack:\n"
"    .skip 65536\n"
"stack_top:\n"
"\n"
".section .text\n"
);

extern unsigned int mboot_magic;
extern unsigned int mboot_info;

long multiboot2_get_magic(void) {
    return (long)(unsigned long)mboot_magic;
}

void *multiboot2_get_info_ptr(void) {
    return (void *)(unsigned long)mboot_info;
}

// 2. Hardware I/O Ports
static inline void outb_internal(unsigned short port, unsigned char val) {
    __asm__ volatile ("outb %0, %1" : : "a"(val), "Nd"(port));
}

static inline unsigned char inb_internal(unsigned short port) {
    unsigned char ret;
    __asm__ volatile ("inb %1, %0" : "=a"(ret) : "Nd"(port));
    return ret;
}

void outb(long port, long val) {
    outb_internal((unsigned short)port, (unsigned char)val);
}

long inb(long port) {
    return (long)inb_internal((unsigned short)port);
}

void qemu_exit(long code) {
    outb_internal(0xf4, (unsigned char)code);
}

// 3. Serial COM1 UART Driver (115200 8N1)
#define COM1 0x3F8

static int serial_initialized = 0;

void serial_init(void) {
    outb_internal(COM1 + 1, 0x00);
    outb_internal(COM1 + 3, 0x80);
    outb_internal(COM1 + 0, 0x01); // 115200 baud
    outb_internal(COM1 + 1, 0x00);
    outb_internal(COM1 + 3, 0x03); // 8N1
    outb_internal(COM1 + 2, 0xC7); // Enable FIFO
    outb_internal(COM1 + 4, 0x0B); // IRQs enabled, RTS/DSR set
    serial_initialized = 1;
}

void serial_putchar(long ch) {
    if (!serial_initialized) serial_init();
    if ((char)ch == '\n') {
        while ((inb_internal(COM1 + 5) & 0x20) == 0);
        outb_internal(COM1, '\r');
    }
    while ((inb_internal(COM1 + 5) & 0x20) == 0);
    outb_internal(COM1, (unsigned char)(ch & 0xFF));
}

void serial_print(const char *s) {
    if (!s) return;
    while (*s) {
        serial_putchar((long)(unsigned char)*s++);
    }
}

// 4. VGA Text Mode Buffer Driver (0xb8000)
#define VGA_BUFFER ((volatile unsigned short *)0xb8000)
#define VGA_WIDTH 80
#define VGA_HEIGHT 25

static int vga_cursor_row = 0;
static int vga_cursor_col = 0;
static unsigned char vga_active_color = 0x0F; // White on Black

void vga_clear(void) {
    for (int i = 0; i < VGA_WIDTH * VGA_HEIGHT; i++) {
        VGA_BUFFER[i] = (unsigned short)' ' | ((unsigned short)vga_active_color << 8);
    }
    vga_cursor_row = 0;
    vga_cursor_col = 0;
}

void vga_set_color(long fg, long bg) {
    vga_active_color = (unsigned char)((bg << 4) | (fg & 0x0F));
}

void vga_set_cursor(long row, long col) {
    if (row >= 0 && row < VGA_HEIGHT) vga_cursor_row = (int)row;
    if (col >= 0 && col < VGA_WIDTH) vga_cursor_col = (int)col;
}

void vga_write_char(long row, long col, long ch, long color) {
    if (row >= 0 && row < VGA_HEIGHT && col >= 0 && col < VGA_WIDTH) {
        VGA_BUFFER[row * VGA_WIDTH + col] = (unsigned short)(ch & 0xFF) | ((unsigned short)(color & 0xFF) << 8);
    }
}

void vga_write_byte(long offset, long byte_val) {
    volatile unsigned char *vga_bytes = (volatile unsigned char *)0xb8000;
    if (offset >= 0 && offset < VGA_WIDTH * VGA_HEIGHT * 2) {
        vga_bytes[offset] = (unsigned char)(byte_val & 0xFF);
    }
}

static void vga_scroll(void) {
    for (int r = 0; r < VGA_HEIGHT - 1; r++) {
        for (int c = 0; c < VGA_WIDTH; c++) {
            VGA_BUFFER[r * VGA_WIDTH + c] = VGA_BUFFER[(r + 1) * VGA_WIDTH + c];
        }
    }
    for (int c = 0; c < VGA_WIDTH; c++) {
        VGA_BUFFER[(VGA_HEIGHT - 1) * VGA_WIDTH + c] = (unsigned short)' ' | ((unsigned short)vga_active_color << 8);
    }
    vga_cursor_row = VGA_HEIGHT - 1;
}

void vga_putchar(char c) {
    serial_putchar((long)(unsigned char)c);
    if (c == '\n') {
        vga_cursor_col = 0;
        vga_cursor_row++;
    } else if (c == '\r') {
        vga_cursor_col = 0;
    } else if (c == '\t') {
        vga_cursor_col = (vga_cursor_col + 4) & ~3;
    } else {
        VGA_BUFFER[vga_cursor_row * VGA_WIDTH + vga_cursor_col] = 
            (unsigned short)(unsigned char)c | ((unsigned short)vga_active_color << 8);
        vga_cursor_col++;
    }
    if (vga_cursor_col >= VGA_WIDTH) {
        vga_cursor_col = 0;
        vga_cursor_row++;
    }
    if (vga_cursor_row >= VGA_HEIGHT) {
        vga_scroll();
    }
}

void vga_print(const char *s) {
    if (!s) return;
    while (*s) {
        vga_putchar(*s++);
    }
}

void __roze_vga_print_str(const char *s) {
    vga_print(s);
    vga_putchar('\n');
}

void __roze_vga_print_int(long val) {
    if (val == 0) {
        vga_putchar('0');
        vga_putchar('\n');
        return;
    }
    if (val < 0) {
        vga_putchar('-');
        val = -val;
    }
    char buf[32];
    int i = 0;
    while (val > 0) {
        buf[i++] = (char)('0' + (val % 10));
        val /= 10;
    }
    while (i > 0) {
        vga_putchar(buf[--i]);
    }
    vga_putchar('\n');
}

void __roze_vga_print_bool(long val) {
    if (val) {
        vga_print("true\n");
    } else {
        vga_print("false\n");
    }
}

int __roze_freestanding_printf(const char *fmt, long val) {
    (void)val;
    vga_print(fmt);
    return 0;
}

char *__roze_freestanding_gcvt(double number, int ndigit, char *buf) {
    (void)number; (void)ndigit;
    if (buf) { buf[0] = '0'; buf[1] = '\0'; }
    return buf;
}

// 4. Freestanding Memory Bump Allocator (Zero libc malloc/free)
static unsigned char __roze_freestanding_heap_pool[1048576]; // 1MB BSS Kernel Heap
static unsigned long __roze_freestanding_heap_offset = 0;

void *__roze_freestanding_alloc(long size) {
    if (size <= 0) return (void *)0;
    unsigned long aligned = (__roze_freestanding_heap_offset + 7) & ~7;
    if (aligned + size > sizeof(__roze_freestanding_heap_pool)) {
        return (void *)0;
    }
    __roze_freestanding_heap_offset = aligned + size;
    return (void *)&__roze_freestanding_heap_pool[aligned];
}

void *__roze_freestanding_alloc_zeroed(long size) {
    void *ptr = __roze_freestanding_alloc(size);
    if (ptr) {
        unsigned char *p = (unsigned char *)ptr;
        for (long i = 0; i < size; i++) p[i] = 0;
    }
    return ptr;
}

void __roze_freestanding_free(void *ptr) {
    (void)ptr;
}

void *__roze_freestanding_realloc(void *ptr, long size) {
    void *new_ptr = __roze_freestanding_alloc(size);
    if (ptr && new_ptr) {
        unsigned char *d = (unsigned char *)new_ptr;
        unsigned char *s = (unsigned char *)ptr;
        for (long i = 0; i < size; i++) d[i] = s[i];
    }
    return new_ptr;
}

void *alloc(long size) { return __roze_freestanding_alloc(size); }
void *alloc_zeroed(long size) { return __roze_freestanding_alloc_zeroed(size); }
void free(void *ptr) { __roze_freestanding_free(ptr); }
void *realloc(void *ptr, long size) { return __roze_freestanding_realloc(ptr, size); }

void *memcpy(void *dest, const void *src, unsigned long n) {
    unsigned char *d = (unsigned char *)dest;
    const unsigned char *s = (const unsigned char *)src;
    for (unsigned long i = 0; i < n; i++) d[i] = s[i];
    return dest;
}

void *memset(void *s, int c, unsigned long n) {
    unsigned char *p = (unsigned char *)s;
    for (unsigned long i = 0; i < n; i++) p[i] = (unsigned char)c;
    return s;
}

int memcmp(const void *s1, const void *s2, unsigned long n) {
    const unsigned char *p1 = (const unsigned char *)s1;
    const unsigned char *p2 = (const unsigned char *)s2;
    for (unsigned long i = 0; i < n; i++) {
        if (p1[i] != p2[i]) return (int)p1[i] - (int)p2[i];
    }
    return 0;
}

unsigned long strlen(const char *s) {
    unsigned long len = 0;
    while (s && *s++) len++;
    return len;
}

void *__roze_freestanding_memcpy(void *dest, const void *src, unsigned long n) {
    return memcpy(dest, src, n);
}

void *__roze_freestanding_memset(void *s, int c, unsigned long n) {
    return memset(s, c, n);
}

int __roze_freestanding_memcmp(const void *s1, const void *s2, unsigned long n) {
    return memcmp(s1, s2, n);
}

unsigned long __roze_freestanding_strlen(const char *s) {
    return strlen(s);
}

void __stack_chk_fail(void) {
    for (;;) {
        #if defined(__x86_64__) || defined(__i386__)
        __asm__ volatile("hlt");
        #endif
    }
}

void __roze_freestanding_exit(int code) {
    qemu_exit(code);
    for (;;) {
        #if defined(__x86_64__) || defined(__i386__)
        __asm__ volatile("hlt");
        #endif
    }
}

// Concurrency Stubs (Zero pthreads)
long __roze_thread_spawn(long f) { (void)f; return 0; }
long __roze_thread_join(long t) { (void)t; return 0; }
void __roze_thread_sleep(long ms) { (void)ms; }
long __roze_thread_current_id(void) { return 0; }
void __roze_thread_retain(long t) { (void)t; }
void __roze_thread_release(long t) { (void)t; }
long __roze_channel_new(long cap) { (void)cap; return 0; }
long __roze_channel_send(long ch, long val) { (void)ch; (void)val; return 0; }
long __roze_channel_recv(long ch) { (void)ch; return 0; }
void __roze_channel_close(long ch) { (void)ch; }
long __roze_channel_is_closed(long ch) { (void)ch; return 1; }
long __roze_channel_len(long ch) { (void)ch; return 0; }
void __roze_channel_retain(long ch) { (void)ch; }
void __roze_channel_release(long ch) { (void)ch; }

// 6. Bootstrap entry point called by entry64
__attribute__((weak))
void __roze_main(void);

void __roze_bootstrap_main(void) {
    serial_init();
    vga_clear();
    if (__roze_main) {
        __roze_main();
    }
    for (;;) {
        #if defined(__x86_64__) || defined(__i386__)
        __asm__ volatile("hlt");
        #endif
    }
}
"#;

pub fn compile_to_freestanding(
    program: TypedProgram,
    input_file: &str,
    linker_script: Option<&str>,
    _entry: Option<&str>,
) -> Result<()> {
    let output_name = crate::codegen::class_name_from_path(input_file);

    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").map_err(|e| anyhow!(e.to_string()))?;
    flag_builder.set("is_pic", "false").map_err(|e| anyhow!(e.to_string()))?;
    let isa_builder = cranelift_native::builder()
        .map_err(|e| anyhow!("unsupported host platform for the native backend: {}", e))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder))?;

    let obj_builder = ObjectBuilder::new(isa, output_name.clone(), cranelift_module::default_libcall_names())
        .map_err(|e| anyhow!(e.to_string()))?;
    let mut module = ObjectModule::new(obj_builder);

    {
        let mut closures = Vec::new();
        let mut func_refs = HashSet::new();
        for stmt in &program.statements {
            collect_closures_and_refs_from_stmt(stmt, &mut closures, &mut func_refs);
        }

        let mut generator = NativeGenerator::new_freestanding(&mut module)?;
        generator.declare_all_classes(&program)?;
        generator.declare_all_functions(&program)?;
        generator.declare_all_closures_and_trampolines(&closures, &func_refs, &program)?;
        generator.compile_trampolines(&func_refs, &program)?;
        generator.compile_closures(&closures)?;
        generator.compile_all_methods(&program)?;
        generator.compile_all_functions(&program)?;
    }

    let product = module.finish();
    let bytes = product.emit().map_err(|e| anyhow!(e.to_string()))?;
    let obj_path = format!("{}.o", output_name);
    fs::write(&obj_path, &bytes)?;
    println!("📝 Generated freestanding object file: {}", obj_path);

    let pid = std::process::id();
    let temp_freestanding_c = std::env::temp_dir().join(format!("roze_freestanding_{}_{}.c", pid, output_name));
    let temp_freestanding_o = std::env::temp_dir().join(format!("roze_freestanding_{}_{}.o", pid, output_name));
    fs::write(&temp_freestanding_c, FREESTANDING_RUNTIME_C)?;

    let status = Command::new("gcc")
        .args(["-m64", "-ffreestanding", "-nostdlib", "-fno-builtin", "-fno-stack-protector", "-c"])
        .arg(&temp_freestanding_c)
        .arg("-o")
        .arg(&temp_freestanding_o)
        .status()
        .map_err(|e| anyhow!("Failed to compile freestanding runtime via gcc: {}", e))?;

    if !status.success() {
        return Err(anyhow!("Failed to compile freestanding runtime helper"));
    }

    let ld_script_path = match linker_script {
        Some(path) => path.to_string(),
        None => {
            let default_ld = std::env::temp_dir().join(format!("roze_freestanding_{}_{}.ld", pid, output_name));
            fs::write(&default_ld, DEFAULT_FREESTANDING_LINKER_SCRIPT)?;
            default_ld.to_string_lossy().to_string()
        }
    };

    link_freestanding_binary(
        &obj_path,
        &temp_freestanding_o.to_string_lossy(),
        &output_name,
        &ld_script_path,
        _entry,
    )?;

    // Also produce 32-bit ELF version for direct QEMU -kernel loading
    let qemu_name = format!("{}.qemu", output_name);
    let objcopy_status = Command::new("objcopy")
        .args(["-I", "elf64-x86-64", "-O", "elf32-i386"])
        .arg(&output_name)
        .arg(&qemu_name)
        .status();
    if let Ok(st) = objcopy_status {
        if st.success() {
            println!("✅ Generated QEMU-compatible multiboot image: {}", qemu_name);
        }
    }

    Ok(())
}

fn link_freestanding_binary(
    roze_obj: &str,
    runtime_obj: &str,
    output_name: &str,
    linker_script: &str,
    entry: Option<&str>,
) -> Result<()> {
    let mut ld_cmd = Command::new("ld");
    ld_cmd
        .arg("-m").arg("elf_x86_64")
        .arg("-nostdlib")
        .arg("-static")
        .arg("-T").arg(linker_script);
    if let Some(entry_sym) = entry {
        ld_cmd.arg("-e").arg(entry_sym);
    }
    ld_cmd
        .arg(roze_obj)
        .arg(runtime_obj)
        .arg("-o").arg(output_name);

    if let Ok(status) = ld_cmd.status() {
        if status.success() {
            println!("✅ Linked freestanding kernel binary: {}", output_name);
            return Ok(());
        }
    }

    let mut gcc_cmd = Command::new("gcc");
    gcc_cmd
        .arg("-m64")
        .arg("-nostdlib")
        .arg("-static")
        .arg("-no-pie")
        .arg(format!("-Wl,-T,{}", linker_script));
    if let Some(entry_sym) = entry {
        gcc_cmd.arg(format!("-Wl,-e,{}", entry_sym));
    }
    gcc_cmd
        .arg(roze_obj)
        .arg(runtime_obj)
        .arg("-o").arg(output_name);

    let status = gcc_cmd.status().map_err(|e| anyhow!("Failed to link freestanding binary: {}", e))?;
    if status.success() {
        println!("✅ Linked freestanding kernel binary: {}", output_name);
        return Ok(());
    }

    Err(anyhow!("Failed to link freestanding kernel binary"))
}

pub fn run_freestanding(name: &str) -> Result<()> {
    let raw_path = if name.starts_with('.') || name.contains('/') || name.contains('\\') {
        name.to_string()
    } else {
        format!("./{}", name)
    };

    let qemu_candidate = format!("{}.qemu", raw_path);
    let boot_file = if std::path::Path::new(&qemu_candidate).exists() {
        qemu_candidate
    } else {
        raw_path.clone()
    };

    println!("🖥️ Freestanding kernel binary ready: {}", raw_path);
    if let Ok(output) = Command::new("which").arg("qemu-system-x86_64").output() {
        if output.status.success() {
            println!("Booting in QEMU (press Ctrl-C to exit)...");
            let mut cmd = Command::new("qemu-system-x86_64");
            cmd.arg("-kernel").arg(&boot_file)
               .arg("-vga").arg("std")
               .arg("-serial").arg("stdio")
               .arg("-device").arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
            let _ = cmd.status();
            return Ok(());
        }
    }

    println!("To boot this kernel on bare-metal or QEMU:");
    println!("  qemu-system-x86_64 -kernel {} -vga std -serial stdio", boot_file);
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
fn link_object_file(obj_path: &str, output_name: &str, extra_libs: &[String]) -> Result<()> {
    const CANDIDATES: [&str; 3] = ["cc", "gcc", "clang"];

    for candidate in CANDIDATES {
        let mut cmd = Command::new(candidate);
        cmd.arg(obj_path).arg("-o").arg(output_name);
        for lib in extra_libs {
            if lib.starts_with("-") || lib.ends_with(".a") || lib.ends_with(".so") || lib.ends_with(".dylib") || lib.ends_with(".lib") || lib.ends_with(".o") || lib.ends_with(".c") {
                cmd.arg(lib);
            } else {
                cmd.arg(format!("-l{}", lib));
            }
        }
        match cmd.status() {
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
/// Whether evaluating `expr` yields a *borrowed* reference (still owned
/// by some existing binding -- a plain variable, or a field read off
/// one) as opposed to a *fresh*, independently-owned one (a literal,
/// concatenation, function call, constructor call, or a field read off
/// an owned/temporary object).
///
/// This matters because a borrowed reference needs an explicit retain
/// when it's consumed at a point that creates a new independent owner
/// (a `let`, `return`, or being passed to something that stores it --
/// see `retain_if_aliasing`), while a fresh one already *is* one and
/// doesn't. Chained field access (`a.b.c`) needs this to be recursive,
/// not just "is this a bare identifier": if `a` is a plain variable,
/// `a.b` is borrowed right along with it (its lifetime is tied to
/// `a`'s), and so is `a.b.c` -- but if the *object* side of a field
/// access is itself something fresh (`some_call().field`), the field
/// value extracted from it needs to be retained before that fresh
/// object gets released (see `compile_expression`'s `FieldAccess`
/// case), at which point the *result* of that whole expression is
/// itself now fresh, not borrowed -- exactly like a normal function
/// call's result.
fn expression_is_borrowed(expr: &TypedExpression) -> bool {
    match &expr.kind {
        TypedExpressionKind::Identifier(_) => true,
        TypedExpressionKind::FieldAccess { object, .. } => expression_is_borrowed(object),
        _ => false,
    }
}

fn require_supported_type(ty: &Type, _location: &Location, _context: &str) -> Result<()> {
    match ty {
        Type::Int | Type::Float | Type::Bool | Type::Void | Type::Unknown | Type::String | Type::List | Type::Map | Type::Class(_) | Type::Option | Type::Result | Type::Enum(_) | Type::Function { .. } | Type::Generic { .. } | Type::Trait(_) | Type::U8 | Type::RawPointer(_) => Ok(()),
    }
}

/// The Cranelift register/value type a given Roze type is represented
/// in. Only `Float` differs from the uniform `VALUE_TYPE` (i64) every
/// other type shares (ints, bools, and every pointer -- strings,
/// lists, maps, class instances -- all fit naturally in a 64-bit
/// integer register; only a genuine IEEE-754 double needs its own
/// register class).
fn cranelift_type_for(ty: &Type) -> types::Type {
    if *ty == Type::Float { types::F64 } else { VALUE_TYPE }
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
    from_c_str_id: FuncId,
    #[allow(dead_code)]
    refcount_id: FuncId,
}

fn declare_string_runtime(module: &mut ObjectModule, is_freestanding: bool) -> Result<StringRuntime> {
    let malloc_name = if is_freestanding { "__roze_freestanding_alloc" } else { "malloc" };
    let free_name = if is_freestanding { "__roze_freestanding_free" } else { "free" };
    let memcpy_name = if is_freestanding { "__roze_freestanding_memcpy" } else { "memcpy" };
    let memcmp_name = if is_freestanding { "__roze_freestanding_memcmp" } else { "memcmp" };
    let strlen_name = if is_freestanding { "__roze_freestanding_strlen" } else { "strlen" };

    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let malloc_id = module.declare_function(malloc_name, Linkage::Import, &malloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(VALUE_TYPE));
    let free_id = module.declare_function(free_name, Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut memcpy_sig = module.make_signature();
    memcpy_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcpy_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcpy_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcpy_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let memcpy_id = module.declare_function(memcpy_name, Linkage::Import, &memcpy_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut memcmp_sig = module.make_signature();
    memcmp_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcmp_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcmp_sig.params.push(AbiParam::new(VALUE_TYPE));
    memcmp_sig.returns.push(AbiParam::new(types::I32));
    let memcmp_id = module.declare_function(memcmp_name, Linkage::Import, &memcmp_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut strlen_sig = module.make_signature();
    strlen_sig.params.push(AbiParam::new(VALUE_TYPE));
    strlen_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let strlen_id = module.declare_function(strlen_name, Linkage::Import, &strlen_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut ctx = module.make_context();
    let retain_id = build_string_retain(module, &mut ctx)?;
    let release_id = build_string_release(module, &mut ctx, free_id)?;
    let concat_id = build_string_concat(module, &mut ctx, malloc_id, memcpy_id)?;
    let eq_id = build_string_eq(module, &mut ctx, memcmp_id)?;
    let from_c_str_id = build_string_from_c_str(module, &mut ctx, malloc_id, memcpy_id, strlen_id)?;
    let refcount_id = build_string_refcount(module, &mut ctx)?;

    Ok(StringRuntime { retain_id, release_id, concat_id, eq_id, from_c_str_id, refcount_id })
}

/// `__roze_string_refcount(ptr: i64) -> i64`: atomically/safely returns the refcount
/// of the string, or 0 if ptr is null.
fn build_string_refcount(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_refcount", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        let valid_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(done_block, VALUE_TYPE);

        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder.ins().brif(is_null, done_block, &[zero], valid_block, &[]);

        builder.switch_to_block(valid_block);
        builder.seal_block(valid_block);
        let ref_addr = builder.ins().iadd_imm(ptr, -STRING_HEADER_SIZE);
        let count = builder.ins().load(VALUE_TYPE, MemFlags::new(), ref_addr, 0);
        builder.ins().jump(done_block, &[count]);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        let result = builder.block_params(done_block)[0];
        builder.ins().return_(&[result]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_string_retain(ptr: i64) -> void`: increments the refcount,
/// unless `ptr` is immortal (a no-op in that case).
fn build_string_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_retain", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        let non_null_block = builder.create_block();
        let done_block = builder.create_block();
        builder.ins().brif(is_null, done_block, &[], non_null_block, &[]);

        builder.switch_to_block(non_null_block);
        builder.seal_block(non_null_block);

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, -(STRING_HEADER_SIZE as i32));
        let sentinel = builder.ins().iconst(VALUE_TYPE, IMMORTAL_SENTINEL);
        let is_immortal = builder.ins().icmp(IntCC::Equal, refcount, sentinel);

        let retain_block = builder.create_block();
        builder.ins().brif(is_immortal, done_block, &[], retain_block, &[]);

        builder.switch_to_block(retain_block);
        builder.seal_block(retain_block);
        let ref_addr = builder.ins().iadd_imm(ptr, -STRING_HEADER_SIZE);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Add, ref_addr, one);
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

/// `__roze_string_release(ptr: i64) -> void`: atomically decrements the refcount
/// and frees the allocation if it reaches zero, unless `ptr` is
/// immortal (a no-op in that case).
fn build_string_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, free_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_release", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let done_block = builder.create_block();
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        let non_null_block = builder.create_block();
        builder.ins().brif(is_null, done_block, &[], non_null_block, &[]);

        builder.switch_to_block(non_null_block);
        builder.seal_block(non_null_block);

        let refcount = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, -(STRING_HEADER_SIZE as i32));
        let sentinel = builder.ins().iconst(VALUE_TYPE, IMMORTAL_SENTINEL);
        let is_immortal = builder.ins().icmp(IntCC::Equal, refcount, sentinel);

        let mutate_block = builder.create_block();
        builder.ins().brif(is_immortal, done_block, &[], mutate_block, &[]);

        builder.switch_to_block(mutate_block);
        builder.seal_block(mutate_block);
        let ref_addr = builder.ins().iadd_imm(ptr, -STRING_HEADER_SIZE);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let old_count = builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Sub, ref_addr, one);

        let should_free = builder.ins().icmp(IntCC::Equal, old_count, one);
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

/// `__roze_string_from_c_str(c_str: i64) -> i64`: converts a raw NUL-terminated
/// C string pointer into an ARC-managed Roze string pointer (with header at -16).
fn build_string_from_c_str(
    module: &mut ObjectModule,
    ctx: &mut cranelift::codegen::Context,
    malloc_id: FuncId,
    memcpy_id: FuncId,
    strlen_id: FuncId,
) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_string_from_c_str", Linkage::Local, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let c_str = builder.block_params(entry)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let zero_byte = builder.ins().iconst(types::I8, 0);
        let header_and_nul = builder.ins().iconst(VALUE_TYPE, STRING_HEADER_SIZE + 1);

        let null_block = builder.create_block();
        let non_null_block = builder.create_block();
        let merge_block = builder.create_block();
        builder.append_block_param(merge_block, VALUE_TYPE);

        let is_null = builder.ins().icmp(IntCC::Equal, c_str, zero);
        builder.ins().brif(is_null, null_block, &[], non_null_block, &[]);

        let malloc_ref = module.declare_func_in_func(malloc_id, builder.func);
        let memcpy_ref = module.declare_func_in_func(memcpy_id, builder.func);
        let strlen_ref = module.declare_func_in_func(strlen_id, builder.func);

        // NULL block: allocate empty string (16-byte header + 1 nul byte)
        builder.switch_to_block(null_block);
        builder.seal_block(null_block);
        let call_null = builder.ins().call(malloc_ref, &[header_and_nul]);
        let base_null = builder.inst_results(call_null)[0];
        builder.ins().store(MemFlags::new(), one, base_null, 0);
        builder.ins().store(MemFlags::new(), zero, base_null, 8);
        let ret_null = builder.ins().iadd_imm(base_null, STRING_HEADER_SIZE);
        builder.ins().store(MemFlags::new(), zero_byte, ret_null, 0);
        builder.ins().jump(merge_block, &[ret_null]);

        // Non-NULL block: compute strlen and copy
        builder.switch_to_block(non_null_block);
        builder.seal_block(non_null_block);
        let call_len = builder.ins().call(strlen_ref, &[c_str]);
        let len = builder.inst_results(call_len)[0];

        let total_size = builder.ins().iadd(len, header_and_nul);
        let call_alloc = builder.ins().call(malloc_ref, &[total_size]);
        let base_alloc = builder.inst_results(call_alloc)[0];
        builder.ins().store(MemFlags::new(), one, base_alloc, 0);
        builder.ins().store(MemFlags::new(), len, base_alloc, 8);
        let data_ptr = builder.ins().iadd_imm(base_alloc, STRING_HEADER_SIZE);

        let len_plus_1 = builder.ins().iadd(len, one);
        builder.ins().call(memcpy_ref, &[data_ptr, c_str, len_plus_1]);
        builder.ins().jump(merge_block, &[data_ptr]);

        // Merge block
        builder.switch_to_block(merge_block);
        builder.seal_block(merge_block);
        let res = builder.block_params(merge_block)[0];
        builder.ins().return_(&[res]);
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

fn declare_list_runtime(module: &mut ObjectModule, printf_id: FuncId, is_freestanding: bool) -> Result<ListRuntime> {
    let malloc_name = if is_freestanding { "__roze_freestanding_alloc" } else { "malloc" };
    let free_name = if is_freestanding { "__roze_freestanding_free" } else { "free" };
    let realloc_name = if is_freestanding { "__roze_freestanding_realloc" } else { "realloc" };
    let memmove_name = if is_freestanding { "__roze_freestanding_memcpy" } else { "memmove" };
    let exit_name = if is_freestanding { "__roze_freestanding_exit" } else { "exit" };

    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let malloc_id = module.declare_function(malloc_name, Linkage::Import, &malloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(VALUE_TYPE));
    let free_id = module.declare_function(free_name, Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut realloc_sig = module.make_signature();
    realloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    realloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    realloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let realloc_id = module.declare_function(realloc_name, Linkage::Import, &realloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut memmove_sig = module.make_signature();
    memmove_sig.params.push(AbiParam::new(VALUE_TYPE));
    memmove_sig.params.push(AbiParam::new(VALUE_TYPE));
    memmove_sig.params.push(AbiParam::new(VALUE_TYPE));
    memmove_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let memmove_id = module.declare_function(memmove_name, Linkage::Import, &memmove_sig).map_err(|e| anyhow!(e.to_string()))?;

    let exit_id = module.declare_function(exit_name, Linkage::Import, &{
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
    let get_id = build_list_bounds_checked_access(module, &mut ctx, printf_id, exit_id, oob_message, AccessKind::Get)?;
    let set_id = build_list_bounds_checked_access(module, &mut ctx, printf_id, exit_id, oob_message, AccessKind::Set)?;
    let remove_id = build_list_remove(module, &mut ctx, printf_id, exit_id, oob_message, memmove_id)?;
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
    exit_id: FuncId,
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

/// `__roze_list_retain(ptr: i64) -> void`: atomically increments refcount.
fn build_list_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_retain", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Add, ptr, one);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_list_release(ptr: i64) -> void`: atomically decrements refcount and frees
/// both the data buffer and the header itself once the refcount reaches zero.
fn build_list_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, free_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_list_release", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let old_count = builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Sub, ptr, one);

        let should_free = builder.ins().icmp(IntCC::Equal, old_count, one);
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
    exit_id: FuncId,
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
        emit_bounds_check(&mut builder, module, printf_id, exit_id, oob_message, index, length);

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
fn build_list_remove(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, printf_id: FuncId, exit_id: FuncId, oob_message: DataId, memmove_id: FuncId) -> Result<FuncId> {
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
        emit_bounds_check(&mut builder, module, printf_id, exit_id, oob_message, index, length);

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

fn declare_map_runtime(module: &mut ObjectModule, is_freestanding: bool) -> Result<MapRuntime> {
    let malloc_name = if is_freestanding { "__roze_freestanding_alloc" } else { "malloc" };
    let free_name = if is_freestanding { "__roze_freestanding_free" } else { "free" };
    let calloc_name = if is_freestanding { "__roze_freestanding_alloc_zeroed" } else { "calloc" };

    let mut malloc_sig = module.make_signature();
    malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let malloc_id = module.declare_function(malloc_name, Linkage::Import, &malloc_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut free_sig = module.make_signature();
    free_sig.params.push(AbiParam::new(VALUE_TYPE));
    let free_id = module.declare_function(free_name, Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

    let mut calloc_sig = module.make_signature();
    calloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    calloc_sig.params.push(AbiParam::new(VALUE_TYPE));
    calloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
    let calloc_id = module.declare_function(calloc_name, Linkage::Import, &calloc_sig).map_err(|e| anyhow!(e.to_string()))?;

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

/// `__roze_map_retain(ptr: i64) -> void`: atomically increments refcount.
fn build_map_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_retain", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Add, ptr, one);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// `__roze_map_release(ptr: i64) -> void`: atomically decrements refcount and frees
/// both the slots buffer and the map header itself once the refcount reaches zero.
fn build_map_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context, free_id: FuncId) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_map_release", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let old_count = builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Sub, ptr, one);

        let should_free = builder.ins().icmp(IntCC::Equal, old_count, one);
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

static CONCURRENCY_RUNTIME_C: &str = include_str!("concurrency.c");

struct ConcurrencyRuntime {
    thread_spawn_id: FuncId,
    thread_join_id: FuncId,
    thread_sleep_id: FuncId,
    thread_current_id: FuncId,
    thread_retain_id: FuncId,
    thread_release_id: FuncId,
    channel_new_id: FuncId,
    channel_send_id: FuncId,
    channel_recv_id: FuncId,
    channel_close_id: FuncId,
    channel_is_closed_id: FuncId,
    channel_len_id: FuncId,
    channel_retain_id: FuncId,
    channel_release_id: FuncId,
}

fn declare_concurrency_runtime(module: &mut ObjectModule) -> Result<ConcurrencyRuntime> {
    let mut i64_to_i64 = module.make_signature();
    i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let mut i64_to_void = module.make_signature();
    i64_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let mut void_to_i64 = module.make_signature();
    void_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let mut i64_i64_to_i64 = module.make_signature();
    i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let thread_spawn_id = module.declare_function("__roze_thread_spawn", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let thread_join_id = module.declare_function("__roze_thread_join", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let thread_sleep_id = module.declare_function("__roze_thread_sleep", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let thread_current_id = module.declare_function("__roze_thread_current_id", Linkage::Import, &void_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let thread_retain_id = module.declare_function("__roze_thread_retain", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let thread_release_id = module.declare_function("__roze_thread_release", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;

    let channel_new_id = module.declare_function("__roze_channel_new", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_send_id = module.declare_function("__roze_channel_send", Linkage::Import, &i64_i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_recv_id = module.declare_function("__roze_channel_recv", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_close_id = module.declare_function("__roze_channel_close", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_is_closed_id = module.declare_function("__roze_channel_is_closed", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_len_id = module.declare_function("__roze_channel_len", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_retain_id = module.declare_function("__roze_channel_retain", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let channel_release_id = module.declare_function("__roze_channel_release", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(ConcurrencyRuntime {
        thread_spawn_id,
        thread_join_id,
        thread_sleep_id,
        thread_current_id,
        thread_retain_id,
        thread_release_id,
        channel_new_id,
        channel_send_id,
        channel_recv_id,
        channel_close_id,
        channel_is_closed_id,
        channel_len_id,
        channel_retain_id,
        channel_release_id,
    })
}

struct RawMemoryRuntime {
    malloc_id: FuncId,
    calloc_id: FuncId,
    realloc_id: FuncId,
    free_id: FuncId,
    memcpy_id: FuncId,
    memset_id: FuncId,
}

fn declare_raw_memory_runtime(module: &mut ObjectModule, is_freestanding: bool) -> Result<RawMemoryRuntime> {
    let mut i64_to_i64 = module.make_signature();
    i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let mut i64_i64_to_i64 = module.make_signature();
    i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let mut i64_to_void = module.make_signature();
    i64_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let mut i64_i64_i64_to_i64 = module.make_signature();
    i64_i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_i64_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let (malloc_name, calloc_name, realloc_name, free_name, memcpy_name, memset_name) = if is_freestanding {
        ("__roze_freestanding_alloc", "__roze_freestanding_alloc_zeroed", "__roze_freestanding_realloc", "__roze_freestanding_free", "__roze_freestanding_memcpy", "__roze_freestanding_memset")
    } else {
        ("malloc", "calloc", "realloc", "free", "memcpy", "memset")
    };

    let malloc_id = module.declare_function(malloc_name, Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let calloc_id = module.declare_function(calloc_name, Linkage::Import, &i64_i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let realloc_id = module.declare_function(realloc_name, Linkage::Import, &i64_i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let free_id = module.declare_function(free_name, Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let memcpy_id = module.declare_function(memcpy_name, Linkage::Import, &i64_i64_i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let memset_id = module.declare_function(memset_name, Linkage::Import, &i64_i64_i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(RawMemoryRuntime {
        malloc_id,
        calloc_id,
        realloc_id,
        free_id,
        memcpy_id,
        memset_id,
    })
}

#[derive(Clone, Copy)]
pub struct VgaRuntime {
    pub clear_id: FuncId,
    pub print_id: FuncId,
    pub write_char_id: FuncId,
    pub write_byte_id: FuncId,
    pub set_cursor_id: FuncId,
    pub set_color_id: FuncId,
    pub print_str_id: FuncId,
    pub print_int_id: FuncId,
    pub print_bool_id: FuncId,
}

fn declare_vga_runtime(module: &mut ObjectModule) -> Result<VgaRuntime> {
    let void_to_void = module.make_signature();

    let mut i64_to_void = module.make_signature();
    i64_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let mut i64_i64_to_void = module.make_signature();
    i64_i64_to_void.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let mut i64_4_to_void = module.make_signature();
    i64_4_to_void.params.push(AbiParam::new(VALUE_TYPE));
    i64_4_to_void.params.push(AbiParam::new(VALUE_TYPE));
    i64_4_to_void.params.push(AbiParam::new(VALUE_TYPE));
    i64_4_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let clear_id = module.declare_function("vga_clear", Linkage::Import, &void_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let print_id = module.declare_function("vga_print", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let write_char_id = module.declare_function("vga_write_char", Linkage::Import, &i64_4_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let write_byte_id = module.declare_function("vga_write_byte", Linkage::Import, &i64_i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let set_cursor_id = module.declare_function("vga_set_cursor", Linkage::Import, &i64_i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let set_color_id = module.declare_function("vga_set_color", Linkage::Import, &i64_i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;

    let print_str_id = module.declare_function("__roze_vga_print_str", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let print_int_id = module.declare_function("__roze_vga_print_int", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let print_bool_id = module.declare_function("__roze_vga_print_bool", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(VgaRuntime {
        clear_id,
        print_id,
        write_char_id,
        write_byte_id,
        set_cursor_id,
        set_color_id,
        print_str_id,
        print_int_id,
        print_bool_id,
    })
}

#[derive(Clone, Copy)]
pub struct KernelRuntime {
    pub multiboot2_get_magic_id: FuncId,
    pub multiboot2_get_info_ptr_id: FuncId,
    pub outb_id: FuncId,
    pub inb_id: FuncId,
    pub qemu_exit_id: FuncId,
    pub serial_print_id: FuncId,
    pub serial_putchar_id: FuncId,
}

fn declare_kernel_runtime(module: &mut ObjectModule) -> Result<KernelRuntime> {
    let mut void_to_i64 = module.make_signature();
    void_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let mut i64_to_void = module.make_signature();
    i64_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let mut i64_to_i64 = module.make_signature();
    i64_to_i64.params.push(AbiParam::new(VALUE_TYPE));
    i64_to_i64.returns.push(AbiParam::new(VALUE_TYPE));

    let mut i64_i64_to_void = module.make_signature();
    i64_i64_to_void.params.push(AbiParam::new(VALUE_TYPE));
    i64_i64_to_void.params.push(AbiParam::new(VALUE_TYPE));

    let multiboot2_get_magic_id = module.declare_function("multiboot2_get_magic", Linkage::Import, &void_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let multiboot2_get_info_ptr_id = module.declare_function("multiboot2_get_info_ptr", Linkage::Import, &void_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let outb_id = module.declare_function("outb", Linkage::Import, &i64_i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let inb_id = module.declare_function("inb", Linkage::Import, &i64_to_i64)
        .map_err(|e| anyhow!(e.to_string()))?;
    let qemu_exit_id = module.declare_function("qemu_exit", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let serial_print_id = module.declare_function("serial_print", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;
    let serial_putchar_id = module.declare_function("serial_putchar", Linkage::Import, &i64_to_void)
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(KernelRuntime {
        multiboot2_get_magic_id,
        multiboot2_get_info_ptr_id,
        outb_id,
        inb_id,
        qemu_exit_id,
        serial_print_id,
        serial_putchar_id,
    })
}

/// A native class instance is a pointer to a heap block:
///
///   [0..8)          i64  refcount
///   [8..8+8*i)      i64  field i's value, in declaration order
///
/// No two-level indirection here (unlike `list`/`map`): a class's field
/// count never changes after construction, so there's nothing that
/// needs to grow, and the instance's own address never needs to move.
///
/// Unlike `list`/`map`'s elements, a class's field *types* are
/// statically declared, not opaque -- which is what makes proper ARC
/// support here tractable at all: `release` is generated once per
/// class definition (not a single generic implementation shared by
/// every class, the way string/list/map's retain/release are), and can
/// therefore know exactly which fields need recursively
/// retaining/releasing and which don't, the same way a typed function
/// parameter already does.
const CLASS_HEADER_SIZE: i64 = 16;
const ENUM_HEADER_SIZE: i64 = 8;

/// Retain is identical for every class regardless of field layout (just
/// bump the refcount at offset 0) -- unlike `new` (a different
/// parameter list per class) or `release` (needs to know which
/// specific fields to recursively release), so this one function is
/// shared by every class rather than generated per class.
/// `__roze_class_retain(ptr: i64) -> void`: atomically increments refcount.
fn build_class_retain(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_class_retain", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        let retain_block = builder.create_block();
        let done_block = builder.create_block();

        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder.ins().brif(is_null, done_block, &[], retain_block, &[]);

        builder.switch_to_block(retain_block);
        builder.seal_block(retain_block);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Add, ptr, one);
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

/// Dynamic release for trait objects: loads vtable from ptr + 8, loads release
/// function pointer at vtable slot 0, and indirect-calls release(ptr).
fn build_trait_release(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_trait_release", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        let release_block = builder.create_block();
        let done_block = builder.create_block();

        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder.ins().brif(is_null, done_block, &[], release_block, &[]);

        builder.switch_to_block(release_block);
        builder.seal_block(release_block);
        let vtable_ptr = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 8);
        let release_fn = builder.ins().load(VALUE_TYPE, MemFlags::new(), vtable_ptr, 0);

        let mut rel_sig = module.make_signature();
        rel_sig.params.push(AbiParam::new(VALUE_TYPE));
        let sig_ref = builder.import_signature(rel_sig);
        builder.ins().call_indirect(sig_ref, release_fn, &[ptr]);
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

/// `__roze_object_refcount(ptr: i64) -> i64`: safely returns the refcount of an
/// ARC-managed object (list, map, class) whose refcount is at offset 0, or 0 if ptr is null.
fn build_object_refcount(module: &mut ObjectModule, ctx: &mut cranelift::codegen::Context) -> Result<FuncId> {
    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(VALUE_TYPE));
    sig.returns.push(AbiParam::new(VALUE_TYPE));
    let func_id = module.declare_function("__roze_object_refcount", Linkage::Export, &sig).map_err(|e| anyhow!(e.to_string()))?;

    ctx.func.signature = sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        let valid_block = builder.create_block();
        let done_block = builder.create_block();
        builder.append_block_param(done_block, VALUE_TYPE);

        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder.ins().brif(is_null, done_block, &[zero], valid_block, &[]);

        builder.switch_to_block(valid_block);
        builder.seal_block(valid_block);
        let count = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, 0);
        builder.ins().jump(done_block, &[count]);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        let result = builder.block_params(done_block)[0];
        builder.ins().return_(&[result]);
        builder.finalize();
    }
    module.define_function(func_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);
    Ok(func_id)
}

/// Everything needed to construct and destroy instances of one class.
struct ClassInfo {
    /// Ordered exactly as declared -- this order is both the
    /// constructor's parameter order (matching `new Name(...)`'s own
    /// positional order) and how field byte offsets are computed
    /// (`CLASS_HEADER_SIZE + index * 8`).
    fields: Vec<(String, Type)>,
    new_id: FuncId,
    release_id: FuncId,
    vtable_id: Option<DataId>,
}

/// A field's declared type decides how (or whether) its value needs to
/// be retained/released -- exactly the same dispatch `emit_release_for_type`
/// does for a local variable, just needing the relevant FuncIds passed
/// in explicitly here since this runs before `FunctionCompiler` (which
/// normally owns that dispatch) exists for any given function.
fn arc_release_call_for_field_type(
    ty: &Type,
    string_release_id: FuncId,
    list_release_id: FuncId,
    map_release_id: FuncId,
    trait_release_id: FuncId,
    other_class_release_ids: &HashMap<String, FuncId>,
) -> Option<FuncId> {
    match ty {
        Type::String => Some(string_release_id),
        Type::List => Some(list_release_id),
        Type::Map => Some(map_release_id),
        Type::Trait(_) => Some(trait_release_id),
        Type::Class(name) => other_class_release_ids.get(name).copied(),
        Type::Generic { base, type_args } => {
            if base == "List" || base == "list" {
                Some(list_release_id)
            } else if base == "Map" || base == "map" {
                Some(map_release_id)
            } else {
                let spec = crate::semantic::specialized_name(base, type_args);
                other_class_release_ids.get(&spec).copied()
            }
        }
        _ => None,
    }
}

/// Builds one class's `new` and `release` functions. `new_id`/
/// `release_id` are already-declared (not yet defined) FuncIds -- see
/// the two-pass declare-then-build split in `NativeGenerator`, which
/// mirrors how ordinary Roze functions support mutual/forward
/// reference: every class's function *signatures* are declared before
/// any class's body is built, so `other_class_release_ids` can name a
/// class's own release function even for a field whose class is
/// declared later in the file (or refers back to itself).
fn build_class_runtime(
    module: &mut ObjectModule,
    ctx: &mut cranelift::codegen::Context,
    fields: &[(String, Type)],
    new_id: FuncId,
    release_id: FuncId,
    vtable_id: Option<DataId>,
    malloc_id: FuncId,
    free_id: FuncId,
    string_release_id: FuncId,
    list_release_id: FuncId,
    map_release_id: FuncId,
    trait_release_id: FuncId,
    other_class_release_ids: &HashMap<String, FuncId>,
) -> Result<()> {
    // --- new(field0, field1, ...) -> ptr ---
    let mut new_sig = module.make_signature();
    for (_, field_type) in fields {
        new_sig.params.push(AbiParam::new(cranelift_type_for(field_type)));
    }
    new_sig.returns.push(AbiParam::new(VALUE_TYPE));
    ctx.func.signature = new_sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let malloc_ref = module.declare_func_in_func(malloc_id, builder.func);
        let total_size = CLASS_HEADER_SIZE + (fields.len() as i64) * 8;
        let size_val = builder.ins().iconst(VALUE_TYPE, total_size);
        let call = builder.ins().call(malloc_ref, &[size_val]);
        let ptr = builder.inst_results(call)[0];

        let one = builder.ins().iconst(VALUE_TYPE, 1);
        builder.ins().store(MemFlags::new(), one, ptr, 0);

        if let Some(vid) = vtable_id {
            let local_data = module.declare_data_in_func(vid, builder.func);
            let vtable_ptr = builder.ins().global_value(VALUE_TYPE, local_data);
            builder.ins().store(MemFlags::new(), vtable_ptr, ptr, 8);
        } else {
            let zero = builder.ins().iconst(VALUE_TYPE, 0);
            builder.ins().store(MemFlags::new(), zero, ptr, 8);
        }

        for (i, _field) in fields.iter().enumerate() {
            let param = builder.block_params(entry)[i];
            let offset = (CLASS_HEADER_SIZE + (i as i64) * 8) as i32;
            builder.ins().store(MemFlags::new(), param, ptr, offset);
        }

        builder.ins().return_(&[ptr]);
        builder.finalize();
    }
    module.define_function(new_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);

    // --- release(ptr) -> void ---
    let mut release_sig = module.make_signature();
    release_sig.params.push(AbiParam::new(VALUE_TYPE));
    ctx.func.signature = release_sig;
    {
        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        let sub_block = builder.create_block();
        let done_block = builder.create_block();

        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);
        let ptr = builder.block_params(entry)[0];

        let zero = builder.ins().iconst(VALUE_TYPE, 0);
        let is_null = builder.ins().icmp(IntCC::Equal, ptr, zero);
        builder.ins().brif(is_null, done_block, &[], sub_block, &[]);

        builder.switch_to_block(sub_block);
        builder.seal_block(sub_block);
        let one = builder.ins().iconst(VALUE_TYPE, 1);
        let old_count = builder.ins().atomic_rmw(VALUE_TYPE, MemFlags::new(), AtomicRmwOp::Sub, ptr, one);

        let should_free = builder.ins().icmp(IntCC::Equal, old_count, one);
        let free_block = builder.create_block();
        builder.ins().brif(should_free, free_block, &[], done_block, &[]);

        builder.switch_to_block(free_block);
        builder.seal_block(free_block);
        for (i, (_, field_type)) in fields.iter().enumerate() {
            if let Some(field_release_id) = arc_release_call_for_field_type(
                field_type, string_release_id, list_release_id, map_release_id, trait_release_id, other_class_release_ids,
            ) {
                let offset = (CLASS_HEADER_SIZE + (i as i64) * 8) as i32;
                let field_val = builder.ins().load(VALUE_TYPE, MemFlags::new(), ptr, offset);
                let field_release_ref = module.declare_func_in_func(field_release_id, builder.func);
                builder.ins().call(field_release_ref, &[field_val]);
            }
        }
        let free_ref = module.declare_func_in_func(free_id, builder.func);
        builder.ins().call(free_ref, &[ptr]);
        builder.ins().jump(done_block, &[]);

        builder.switch_to_block(done_block);
        builder.seal_block(done_block);
        builder.ins().return_(&[]);
        builder.finalize();
    }
    module.define_function(release_id, ctx).map_err(|e| anyhow!(e.to_string()))?;
    module.clear_context(ctx);

    Ok(())
}

struct NativeGenerator<'a> {
    module: &'a mut ObjectModule,
    is_freestanding: bool,
    functions: HashMap<String, FuncId>,
    function_param_types: HashMap<String, Vec<Type>>,
    closure_funcs: HashMap<usize, FuncId>,
    trampoline_funcs: HashMap<String, FuncId>,
    printf_id: FuncId,
    gcvt_id: FuncId,
    malloc_id: FuncId,
    fmt_int: DataId,
    fmt_str: DataId,
    true_str: DataId,
    false_str: DataId,
    strings: StringRuntime,
    lists: ListRuntime,
    maps: MapRuntime,
    concurrency: ConcurrencyRuntime,
    raw_memory: RawMemoryRuntime,
    vga: VgaRuntime,
    kernel: KernelRuntime,
    classes: HashMap<String, ClassInfo>,
    class_retain_id: FuncId,
    trait_release_id: FuncId,
    #[allow(dead_code)]
    object_refcount_id: FuncId,
    methods: HashMap<(String, String), FuncId>,
    trait_methods: HashMap<String, usize>,
    extern_functions: HashSet<String>,
    literal_counter: usize,
}

impl<'a> NativeGenerator<'a> {
    fn new(module: &'a mut ObjectModule) -> Result<Self> {
        Self::create(module, false)
    }

    fn new_freestanding(module: &'a mut ObjectModule) -> Result<Self> {
        Self::create(module, true)
    }

    fn create(module: &'a mut ObjectModule, is_freestanding: bool) -> Result<Self> {
        let (printf_id, gcvt_id, malloc_id) = if is_freestanding {
            let mut printf_sig = module.make_signature();
            printf_sig.params.push(AbiParam::new(VALUE_TYPE));
            printf_sig.params.push(AbiParam::new(VALUE_TYPE));
            printf_sig.returns.push(AbiParam::new(types::I32));
            let p_id = module.declare_function("__roze_freestanding_printf", Linkage::Import, &printf_sig)
                .map_err(|e| anyhow!(e.to_string()))?;

            let mut gcvt_sig = module.make_signature();
            gcvt_sig.params.push(AbiParam::new(types::F64));
            gcvt_sig.params.push(AbiParam::new(types::I32));
            gcvt_sig.params.push(AbiParam::new(VALUE_TYPE));
            gcvt_sig.returns.push(AbiParam::new(VALUE_TYPE));
            let g_id = module.declare_function("__roze_freestanding_gcvt", Linkage::Import, &gcvt_sig)
                .map_err(|e| anyhow!(e.to_string()))?;

            let mut alloc_sig = module.make_signature();
            alloc_sig.params.push(AbiParam::new(VALUE_TYPE));
            alloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
            let m_id = module.declare_function("__roze_freestanding_alloc", Linkage::Import, &alloc_sig)
                .map_err(|e| anyhow!(e.to_string()))?;

            (p_id, g_id, m_id)
        } else {
            let mut printf_sig = module.make_signature();
            printf_sig.params.push(AbiParam::new(VALUE_TYPE));
            printf_sig.params.push(AbiParam::new(VALUE_TYPE));
            printf_sig.returns.push(AbiParam::new(types::I32));
            let p_id = module.declare_function("printf", Linkage::Import, &printf_sig)
                .map_err(|e| anyhow!(e.to_string()))?;

            let mut gcvt_sig = module.make_signature();
            gcvt_sig.params.push(AbiParam::new(types::F64));
            gcvt_sig.params.push(AbiParam::new(types::I32));
            gcvt_sig.params.push(AbiParam::new(VALUE_TYPE));
            gcvt_sig.returns.push(AbiParam::new(VALUE_TYPE));
            let g_id = module.declare_function("gcvt", Linkage::Import, &gcvt_sig)
                .map_err(|e| anyhow!(e.to_string()))?;

            let mut malloc_sig = module.make_signature();
            malloc_sig.params.push(AbiParam::new(VALUE_TYPE));
            malloc_sig.returns.push(AbiParam::new(VALUE_TYPE));
            let m_id = module.declare_function("malloc", Linkage::Import, &malloc_sig)
                .map_err(|e| anyhow!(e.to_string()))?;

            (p_id, g_id, m_id)
        };

        let fmt_int = Self::declare_c_string(module, "__roze_fmt_int", b"%lld\n")?;
        let fmt_str = Self::declare_c_string(module, "__roze_fmt_str", b"%s\n")?;
        let true_str = Self::declare_c_string(module, "__roze_true_str", b"true")?;
        let false_str = Self::declare_c_string(module, "__roze_false_str", b"false")?;

        let strings = declare_string_runtime(module, is_freestanding)?;
        let lists = declare_list_runtime(module, printf_id, is_freestanding)?;
        let maps = declare_map_runtime(module, is_freestanding)?;
        let concurrency = declare_concurrency_runtime(module)?;
        let raw_memory = declare_raw_memory_runtime(module, is_freestanding)?;
        let vga = declare_vga_runtime(module)?;
        let kernel = declare_kernel_runtime(module)?;

        let mut retain_ctx = module.make_context();
        let class_retain_id = build_class_retain(module, &mut retain_ctx)?;
        let mut trait_ctx = module.make_context();
        let trait_release_id = build_trait_release(module, &mut trait_ctx)?;
        let mut obj_ctx = module.make_context();
        let object_refcount_id = build_object_refcount(module, &mut obj_ctx)?;

        Ok(Self {
            module,
            is_freestanding,
            functions: HashMap::new(),
            function_param_types: HashMap::new(),
            closure_funcs: HashMap::new(),
            trampoline_funcs: HashMap::new(),
            printf_id,
            gcvt_id,
            malloc_id,
            fmt_int,
            fmt_str,
            true_str,
            false_str,
            strings,
            lists,
            maps,
            concurrency,
            raw_memory,
            vga,
            kernel,
            classes: HashMap::new(),
            class_retain_id,
            trait_release_id,
            object_refcount_id,
            methods: HashMap::new(),
            trait_methods: HashMap::new(),
            extern_functions: HashSet::new(),
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

    /// Registers every class's `new`/`release` function *signatures*
    /// first (a first pass, mirroring `declare_all_functions`'s own
    /// first pass for the same reason), then builds every class's
    /// actual function bodies in a second pass -- so a field whose type
    /// is another class (declared earlier, later, or even itself) can
    /// always find that class's release function already declared,
    /// regardless of source order.
    fn declare_all_classes(&mut self, program: &TypedProgram) -> Result<()> {
        for stmt in &program.statements {
            if let TypedStatement::TraitDecl { methods, .. } = stmt {
                for m in methods {
                    if !self.trait_methods.contains_key(&m.name) {
                        let slot = self.trait_methods.len() + 1; // 1-based (slot 0 reserved for release)
                        self.trait_methods.insert(m.name.clone(), slot);
                    }
                }
            }
        }

        for stmt in &program.statements {
            if let TypedStatement::ImplDecl { class_name, methods, .. } = stmt {
                for m in methods {
                    if let TypedStatement::Function { name, params, return_type, location, .. } = m {
                        for param in params {
                            require_supported_type(&param.type_, location, &format!("parameter '{}' of method '{}' of class '{}'", param.name, name, class_name))?;
                        }
                        require_supported_type(return_type, location, &format!("the return type of method '{}' of class '{}'", name, class_name))?;

                        let mut sig = self.module.make_signature();
                        sig.params.push(AbiParam::new(VALUE_TYPE)); // 'this'
                        for param in params {
                            sig.params.push(AbiParam::new(cranelift_type_for(&param.type_)));
                        }
                        if !matches!(return_type, Type::Void) {
                            sig.returns.push(AbiParam::new(cranelift_type_for(return_type)));
                        }
                        let fn_name = format!("__roze_method_{}_{}", class_name, name);
                        let func_id = self.module.declare_function(&fn_name, Linkage::Local, &sig)
                            .map_err(|e| anyhow!(e.to_string()))?;
                        self.methods.insert((class_name.clone(), name.clone()), func_id);
                    }
                }
            }
        }

        for stmt in &program.statements {
            if let TypedStatement::ClassDecl { name, fields, location } = stmt {
                for (field_name, field_type) in fields {
                    require_supported_type(field_type, location, &format!("field '{}' of class '{}'", field_name, name))?;
                }

                let mut new_sig = self.module.make_signature();
                for (_, field_type) in fields {
                    new_sig.params.push(AbiParam::new(cranelift_type_for(field_type)));
                }
                new_sig.returns.push(AbiParam::new(VALUE_TYPE));
                let new_id = self.module.declare_function(&format!("__roze_class_{}_new", name), Linkage::Local, &new_sig)
                    .map_err(|e| anyhow!(e.to_string()))?;

                let mut release_sig = self.module.make_signature();
                release_sig.params.push(AbiParam::new(VALUE_TYPE));
                let release_id = self.module.declare_function(&format!("__roze_class_{}_release", name), Linkage::Export, &release_sig)
                    .map_err(|e| anyhow!(e.to_string()))?;

                self.classes.insert(name.clone(), ClassInfo { fields: fields.clone(), new_id, release_id, vtable_id: None });
            }
        }

        let max_slot = self.trait_methods.values().copied().max().unwrap_or(0);
        let vtable_slot_count = (max_slot + 1).max(1);

        for (class_name, info) in self.classes.iter_mut() {
            let mut data_ctx = DataDescription::new();
            data_ctx.set_align(8);
            data_ctx.define(vec![0u8; vtable_slot_count * 8].into_boxed_slice());

            let rel_ref = self.module.declare_func_in_data(info.release_id, &mut data_ctx);
            data_ctx.write_function_addr(0, rel_ref);

            for stmt in &program.statements {
                if let TypedStatement::ImplDecl { trait_name: Some(_), class_name: c_name, methods, .. } = stmt {
                    if c_name == class_name {
                        for m in methods {
                            if let TypedStatement::Function { name: m_name, .. } = m {
                                if let Some(&slot) = self.trait_methods.get(m_name) {
                                    if let Some(&func_id) = self.methods.get(&(class_name.clone(), m_name.clone())) {
                                        let func_ref = self.module.declare_func_in_data(func_id, &mut data_ctx);
                                        data_ctx.write_function_addr((slot * 8) as u32, func_ref);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let vtable_name = format!("__roze_vtable_{}", class_name);
            let vtable_id = self.module.declare_data(&vtable_name, Linkage::Local, false, false)
                .map_err(|e| anyhow!(e.to_string()))?;
            self.module.define_data(vtable_id, &data_ctx).map_err(|e| anyhow!(e.to_string()))?;
            info.vtable_id = Some(vtable_id);
        }

        let other_class_release_ids: HashMap<String, FuncId> = self.classes.iter()
            .map(|(name, info)| (name.clone(), info.release_id))
            .collect();

        let malloc_id = self.malloc_id;

        let mut free_sig = self.module.make_signature();
        free_sig.params.push(AbiParam::new(VALUE_TYPE));
        let free_name = if self.is_freestanding { "__roze_freestanding_free" } else { "free" };
        let free_id = self.module.declare_function(free_name, Linkage::Import, &free_sig).map_err(|e| anyhow!(e.to_string()))?;

        let mut ctx = self.module.make_context();
        for info in self.classes.values() {
            build_class_runtime(
                self.module,
                &mut ctx,
                &info.fields,
                info.new_id,
                info.release_id,
                info.vtable_id,
                malloc_id,
                free_id,
                self.strings.release_id,
                self.lists.release_id,
                self.maps.release_id,
                self.trait_release_id,
                &other_class_release_ids,
            )?;
        }

        Ok(())
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
                for param in params {
                    sig.params.push(AbiParam::new(cranelift_type_for(&param.type_)));
                }
                if !matches!(return_type, Type::Void) {
                    sig.returns.push(AbiParam::new(cranelift_type_for(return_type)));
                }

                let (func_name, linkage) = if name == "_start" && self.is_freestanding {
                    ("_start", Linkage::Export)
                } else if name == "main" {
                    ("__roze_main", if self.is_freestanding { Linkage::Export } else { Linkage::Local })
                } else {
                    (name.as_str(), Linkage::Local)
                };
                let func_id = self.module.declare_function(func_name, linkage, &sig)
                    .map_err(|e| anyhow!(e.to_string()))?;
                self.functions.insert(name.clone(), func_id);
                self.function_param_types.insert(name.clone(), params.iter().map(|p| p.type_.clone()).collect());
            }
            if let TypedStatement::ExternFunction { name, params, return_type, location, .. } = stmt {
                for param in params {
                    require_supported_type(&param.type_, location, &format!("parameter '{}' of extern function '{}'", param.name, name))?;
                }
                require_supported_type(return_type, location, &format!("the return type of extern function '{}'", name))?;

                let mut sig = self.module.make_signature();
                sig.call_conv = self.module.target_config().default_call_conv;
                for param in params {
                    sig.params.push(AbiParam::new(cranelift_type_for(&param.type_)));
                }
                if !matches!(return_type, Type::Void) {
                    sig.returns.push(AbiParam::new(cranelift_type_for(return_type)));
                }

                let func_id = if let Some(FuncOrDataId::Func(existing_id)) = self.module.get_name(name) {
                    existing_id
                } else {
                    self.module.declare_function(name, Linkage::Import, &sig)
                        .map_err(|e| anyhow!(e.to_string()))?
                };
                self.functions.insert(name.clone(), func_id);
                self.function_param_types.insert(name.clone(), params.iter().map(|p| p.type_.clone()).collect());
                self.extern_functions.insert(name.clone());
            }
        }
        Ok(())
    }

    fn declare_all_closures_and_trampolines(
        &mut self,
        closures: &[ClosureDecl],
        func_refs: &HashSet<String>,
        program: &TypedProgram,
    ) -> Result<()> {
        for c in closures {
            let mut sig = self.module.make_signature();
            // Env pointer:
            sig.params.push(AbiParam::new(VALUE_TYPE));
            for (_, p_ty) in &c.params {
                sig.params.push(AbiParam::new(cranelift_type_for(p_ty)));
            }
            if !matches!(c.return_type, Type::Void) {
                sig.returns.push(AbiParam::new(cranelift_type_for(&c.return_type)));
            }
            let func_name = format!("__roze_closure_{}", c.id);
            let func_id = self.module.declare_function(&func_name, Linkage::Local, &sig)
                .map_err(|e| anyhow!(e.to_string()))?;
            self.closure_funcs.insert(c.id, func_id);
        }

        for name in func_refs {
            let func_stmt = program.statements.iter().find_map(|s| {
                if let TypedStatement::Function { name: fn_name, params, return_type, .. } = s {
                    if fn_name == name {
                        return Some((params.clone(), return_type.clone()));
                    }
                }
                None
            });
            if let Some((params, return_type)) = func_stmt {
                let mut sig = self.module.make_signature();
                // Env pointer:
                sig.params.push(AbiParam::new(VALUE_TYPE));
                for p in &params {
                    sig.params.push(AbiParam::new(cranelift_type_for(&p.type_)));
                }
                if !matches!(return_type, Type::Void) {
                    sig.returns.push(AbiParam::new(cranelift_type_for(&return_type)));
                }
                let trampoline_name = format!("__roze_trampoline_{}", name);
                let func_id = self.module.declare_function(&trampoline_name, Linkage::Local, &sig)
                    .map_err(|e| anyhow!(e.to_string()))?;
                self.trampoline_funcs.insert(name.clone(), func_id);
            }
        }
        Ok(())
    }

    fn compile_trampolines(&mut self, func_refs: &HashSet<String>, _program: &TypedProgram) -> Result<()> {
        let mut ctx = self.module.make_context();
        for name in func_refs {
            if let Some(trampoline_id) = self.trampoline_funcs.get(name).copied() {
                let top_func_id = *self.functions.get(name)
                    .expect("top-level function must be registered in functions");
                let sig = self.module.declarations().get_function_decl(trampoline_id).signature.clone();
                ctx.func.signature = sig;
                {
                    let mut func_ctx = FunctionBuilderContext::new();
                    let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
                    let entry = builder.create_block();
                    builder.append_block_params_for_function_params(entry);
                    builder.switch_to_block(entry);
                    builder.seal_block(entry);

                    let mut forward_args = Vec::new();
                    for p in &builder.block_params(entry)[1..] {
                        forward_args.push(*p);
                    }
                    let top_ref = self.module.declare_func_in_func(top_func_id, builder.func);
                    let call = builder.ins().call(top_ref, &forward_args);
                    let results = builder.inst_results(call).to_vec();
                    if results.is_empty() {
                        builder.ins().return_(&[]);
                    } else {
                        builder.ins().return_(&results);
                    }
                    builder.finalize();
                }
                self.module.define_function(trampoline_id, &mut ctx).map_err(|e| anyhow!(e.to_string()))?;
                self.module.clear_context(&mut ctx);
            }
        }
        Ok(())
    }

    fn compile_closures(&mut self, closures: &[ClosureDecl]) -> Result<()> {
        let mut ctx = self.module.make_context();
        for c in closures {
            let func_id = *self.closure_funcs.get(&c.id).unwrap();
            let sig = self.module.declarations().get_function_decl(func_id).signature.clone();
            ctx.func.signature = sig;
            {
                let mut func_ctx = FunctionBuilderContext::new();
                let builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
                let mut fc = FunctionCompiler {
                    module: self.module,
                    builder,
                    functions: &self.functions,
                    function_param_types: &self.function_param_types,
                    closure_funcs: &self.closure_funcs,
                    trampoline_funcs: &self.trampoline_funcs,
                    printf_id: self.printf_id,
                    gcvt_id: self.gcvt_id,
                    malloc_id: self.malloc_id,
                    fmt_int: self.fmt_int,
                    fmt_str: self.fmt_str,
                    true_str: self.true_str,
                    false_str: self.false_str,
                    string_retain_id: self.strings.retain_id,
                    string_release_id: self.strings.release_id,
                    string_concat_id: self.strings.concat_id,
                    string_eq_id: self.strings.eq_id,
                    string_from_c_str_id: self.strings.from_c_str_id,
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
                    concurrency: &self.concurrency,
                    raw_memory: &self.raw_memory,
                    is_freestanding: self.is_freestanding,
                    vga: &self.vga,
                    kernel: &self.kernel,
                    classes: &self.classes,
                    class_retain_id: self.class_retain_id,
                    trait_release_id: self.trait_release_id,
                    methods: &self.methods,
                    extern_functions: &self.extern_functions,
                    literal_counter: &mut self.literal_counter,
                    scopes: vec![HashMap::new()],
                    next_var_index: 0,
                    current_return_type: c.return_type.clone(),
                };

                let entry = fc.builder.create_block();
                fc.builder.append_block_params_for_function_params(entry);
                fc.builder.switch_to_block(entry);
                fc.builder.seal_block(entry);

                let env_val = fc.builder.block_params(entry)[0];

                for (i, (cap_name, cap_ty)) in c.captures.iter().enumerate() {
                    let offset = (16 + i * 8) as i32;
                    let cap_val = fc.builder.ins().load(cranelift_type_for(cap_ty), MemFlags::new(), env_val, offset);
                    let var = fc.declare_local(cap_name, cap_ty.clone());
                    fc.builder.def_var(var, cap_val);
                }

                for (i, (p_name, p_ty)) in c.params.iter().enumerate() {
                    let var = fc.declare_local(p_name, p_ty.clone());
                    let val = fc.builder.block_params(entry)[1 + i];
                    fc.builder.def_var(var, val);
                }

                let terminated = if let TypedStatement::Block { statements, .. } = c.body.as_ref() {
                    let mut terminated = false;
                    for s in statements {
                        if terminated {
                            break;
                        }
                        terminated = fc.compile_statement(s)?;
                    }
                    terminated
                } else {
                    fc.compile_statement(&c.body)?
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
        Ok(())
    }

    fn compile_all_methods(&mut self, program: &TypedProgram) -> Result<()> {
        let mut ctx = self.module.make_context();

        for stmt in &program.statements {
            if let TypedStatement::ImplDecl { class_name, methods, .. } = stmt {
                for m in methods {
                    if let TypedStatement::Function { name, params, return_type, body, .. } = m {
                        let func_id = *self.methods.get(&(class_name.clone(), name.clone()))
                            .expect("method declared in declare_all_classes");

                        ctx.func.signature = self.module.declarations().get_function_decl(func_id).signature.clone();

                        {
                            let mut func_ctx = FunctionBuilderContext::new();
                            let builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);

                            let mut fc = FunctionCompiler {
                                module: self.module,
                                builder,
                                functions: &self.functions,
                                function_param_types: &self.function_param_types,
                                closure_funcs: &self.closure_funcs,
                                trampoline_funcs: &self.trampoline_funcs,
                                methods: &self.methods,
                                extern_functions: &self.extern_functions,
                                trait_release_id: self.trait_release_id,
                                printf_id: self.printf_id,
                                gcvt_id: self.gcvt_id,
                                malloc_id: self.malloc_id,
                                fmt_int: self.fmt_int,
                                fmt_str: self.fmt_str,
                                true_str: self.true_str,
                                false_str: self.false_str,
                                string_retain_id: self.strings.retain_id,
                                string_release_id: self.strings.release_id,
                                string_concat_id: self.strings.concat_id,
                                string_eq_id: self.strings.eq_id,
                                string_from_c_str_id: self.strings.from_c_str_id,
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
                                concurrency: &self.concurrency,
                                raw_memory: &self.raw_memory,
                                is_freestanding: self.is_freestanding,
                                vga: &self.vga,
                                kernel: &self.kernel,
                                classes: &self.classes,
                                class_retain_id: self.class_retain_id,
                                literal_counter: &mut self.literal_counter,
                                scopes: vec![HashMap::new()],
                                next_var_index: 0,
                                current_return_type: return_type.clone(),
                            };

                            let entry = fc.builder.create_block();
                            fc.builder.append_block_params_for_function_params(entry);
                            fc.builder.switch_to_block(entry);
                            fc.builder.seal_block(entry);

                            let this_val = fc.builder.block_params(entry)[0];
                            let this_var = fc.declare_local("this", Type::Class(class_name.clone()));
                            fc.builder.def_var(this_var, this_val);

                            for (i, p) in params.iter().enumerate() {
                                let param_val = fc.builder.block_params(entry)[i + 1];
                                let var = fc.declare_local(&p.name, p.type_.clone());
                                fc.builder.def_var(var, param_val);
                            }

                            let terminated = fc.compile_statement(body)?;
                            if !terminated {
                                fc.release_frame(0)?;
                                if matches!(return_type, Type::Void) {
                                    fc.builder.ins().return_(&[]);
                                } else {
                                    let zero = fc.builder.ins().iconst(cranelift_type_for(return_type), 0);
                                    fc.builder.ins().return_(&[zero]);
                                }
                            }

                            fc.builder.finalize();
                        }

                        self.module.define_function(func_id, &mut ctx).map_err(|e| {
                    eprintln!("Function verification failed for {}: {:?}\nIR:\n{}", name, e, ctx.func.display());
                    anyhow!(e.to_string())
                })?;
                        self.module.clear_context(&mut ctx);
                    }
                }
            }
        }
        Ok(())
    }

    fn compile_all_functions(&mut self, program: &TypedProgram) -> Result<()> {
        let mut ctx = self.module.make_context();

        for stmt in &program.statements {
            if let TypedStatement::Function { name, params, return_type, body, .. } = stmt {
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
                        function_param_types: &self.function_param_types,
                        closure_funcs: &self.closure_funcs,
                        trampoline_funcs: &self.trampoline_funcs,
                        printf_id: self.printf_id,
                        gcvt_id: self.gcvt_id,
                        malloc_id: self.malloc_id,
                        fmt_int: self.fmt_int,
                        fmt_str: self.fmt_str,
                        true_str: self.true_str,
                        false_str: self.false_str,
                        string_retain_id: self.strings.retain_id,
                        string_release_id: self.strings.release_id,
                        string_concat_id: self.strings.concat_id,
                        string_eq_id: self.strings.eq_id,
                        string_from_c_str_id: self.strings.from_c_str_id,
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
                        concurrency: &self.concurrency,
                        raw_memory: &self.raw_memory,
                        is_freestanding: self.is_freestanding,
                        vga: &self.vga,
                        kernel: &self.kernel,
                        classes: &self.classes,
                        class_retain_id: self.class_retain_id,
                        trait_release_id: self.trait_release_id,
                        methods: &self.methods,
                        extern_functions: &self.extern_functions,
                        literal_counter: &mut self.literal_counter,
                        scopes: vec![HashMap::new()],
                        next_var_index: 0,
                        current_return_type: return_type.clone(),
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

                self.module.define_function(func_id, &mut ctx).map_err(|e| {
                    eprintln!("Function verification failed for {}: {:?}\nIR:\n{}", name, e, ctx.func.display());
                    anyhow!(e.to_string())
                })?;
                self.module.clear_context(&mut ctx);
            }
        }

        if !self.is_freestanding {
            self.emit_c_main(&mut ctx)?;
        }
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
    function_param_types: &'a HashMap<String, Vec<Type>>,
    closure_funcs: &'a HashMap<usize, FuncId>,
    trampoline_funcs: &'a HashMap<String, FuncId>,
    printf_id: FuncId,
    gcvt_id: FuncId,
    malloc_id: FuncId,
    fmt_int: DataId,
    fmt_str: DataId,
    true_str: DataId,
    false_str: DataId,
    string_retain_id: FuncId,
    string_release_id: FuncId,
    string_concat_id: FuncId,
    string_eq_id: FuncId,
    string_from_c_str_id: FuncId,
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
    concurrency: &'a ConcurrencyRuntime,
    raw_memory: &'a RawMemoryRuntime,
    is_freestanding: bool,
    vga: &'a VgaRuntime,
    kernel: &'a KernelRuntime,
    classes: &'a HashMap<String, ClassInfo>,
    class_retain_id: FuncId,
    trait_release_id: FuncId,
    methods: &'a HashMap<(String, String), FuncId>,
    extern_functions: &'a HashSet<String>,
    /// Shared across every function being compiled (not reset per
    /// function), so two string literals never collide on the same
    /// generated data symbol name.
    literal_counter: &'a mut usize,
    /// Each frame maps a local's name to (its Cranelift Variable, its
    /// Roze type) -- the type is what lets scope-exit cleanup know
    /// which locals are strings needing a release call.
    scopes: Vec<HashMap<String, (Variable, Type)>>,
    next_var_index: usize,
    /// The enclosing function's declared return type -- needed so
    /// `return` can insert an explicit int-to-float conversion when the
    /// returned expression is Int but the function is declared to
    /// return Float (the same implicit widening `types_compatible`
    /// already allows at the type-checking level; codegen has to
    /// actually perform it, not just permit it, or Cranelift's
    /// verifier rejects the mismatched i64-into-an-f64-signature return).
    current_return_type: Type,
}

impl<'a, 'b> FunctionCompiler<'a, 'b> {
    /// Performs the implicit int-to-float widening `types_compatible`
    /// already allows at the type-checking level -- Cranelift has no
    /// such thing as automatic numeric promotion, so every site that
    /// accepts "an Int where a Float is expected" (return, assignment,
    /// function-call and constructor arguments) has to actually insert
    /// this conversion, not just type-check as if it happened. A no-op
    /// (returns `val` unchanged) whenever the types already match.
    fn coerce_for_target_type(&mut self, val: Value, actual: &Type, target: &Type) -> Value {
        if *target == Type::Float && (*actual == Type::Int || *actual == Type::Unknown || self.builder.func.dfg.value_type(val) == VALUE_TYPE) {
            self.builder.ins().fcvt_from_sint(types::F64, val)
        } else {
            val
        }
    }

    fn declare_local(&mut self, name: &str, ty: Type) -> Variable {
        let var = Variable::new(self.next_var_index);
        self.next_var_index += 1;
        self.builder.declare_var(var, cranelift_type_for(&ty));
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
            .filter(|(_, ty)| matches!(ty, Type::String | Type::List | Type::Map | Type::Class(_) | Type::Generic { .. }))
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
        if expression_is_borrowed(expr) {
            self.retain_for_arc_type(&expr.type_, val)?;
        }
        Ok(())
    }

    /// Retains `val` per its Roze type, if it's an ARC type at all --
    /// shared by `retain_if_aliasing` (an identifier alias) and reading
    /// a class field (which extracts a reference the instance itself
    /// separately keeps owning), since both need exactly the same
    /// per-type dispatch.
    fn retain_for_arc_type(&mut self, ty: &Type, val: Value) -> Result<()> {
        match ty {
            Type::String => self.emit_string_retain(val),
            Type::List => self.emit_list_retain(val),
            Type::Map => self.emit_map_retain(val),
            Type::Class(_) | Type::Trait(_) => self.emit_class_retain(val),
            Type::Function { .. } => self.emit_class_retain(val),
            Type::Generic { base, .. } if base == "List" || base == "list" => self.emit_list_retain(val),
            Type::Generic { base, .. } if base == "Map" || base == "map" => self.emit_map_retain(val),
            Type::Generic { base, .. } if base == "Channel" || base == "channel" => {
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_retain_id, self.builder.func);
                self.builder.ins().call(func_ref, &[val]);
                Ok(())
            }
            Type::Generic { base, .. } if base == "Thread" || base == "thread" => {
                let func_ref = self.module.declare_func_in_func(self.concurrency.thread_retain_id, self.builder.func);
                self.builder.ins().call(func_ref, &[val]);
                Ok(())
            }
            Type::Generic { .. } => self.emit_class_retain(val),
            _ => Ok(()),
        }
    }

    /// Releases `val` (the compiled form of `expr`) if `expr` isn't a
    /// bare identifier -- i.e. if evaluating it produced a *fresh*
    /// owned reference (a literal, a concatenation, a function call's
    /// return value, a field read) that nothing else will release once
    /// this borrowing use is done. `list`/`map`'s accessor intrinsics
    /// (length, get, has, ...) all take their container argument this
    /// way: they read/mutate through it without taking ownership of
    /// the container reference itself, so whoever *does* own a fresh
    /// one needs it released right after -- the same "borrow, then
    /// release if fresh" pattern already used for binary-op operands
    /// and println's argument. This is exactly the case that leaked
    /// before this existed: `list_length(make_list())`, where
    /// `make_list()`'s fresh return value was read once and then never
    /// released.
    fn release_if_fresh(&mut self, expr: &TypedExpression, val: Value) -> Result<()> {
        if !expression_is_borrowed(expr) {
            self.emit_release_for_type(&expr.type_, val)?;
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
            Type::Trait(_) => {
                let release_ref = self.module.declare_func_in_func(self.trait_release_id, self.builder.func);
                self.builder.ins().call(release_ref, &[val]);
                Ok(())
            }
            Type::Class(name) => {
                let release_id = self.classes.get(name)
                    .map(|info| info.release_id)
                    .expect("every class referenced by a typed value was declared and registered in declare_all_classes");
                let release_ref = self.module.declare_func_in_func(release_id, self.builder.func);
                self.builder.ins().call(release_ref, &[val]);
                Ok(())
            }
            Type::Generic { base, .. } if base == "List" || base == "list" => self.emit_list_release(val),
            Type::Generic { base, .. } if base == "Map" || base == "map" => self.emit_map_release(val),
            Type::Generic { base, .. } if base == "Channel" || base == "channel" => {
                let release_ref = self.module.declare_func_in_func(self.concurrency.channel_release_id, self.builder.func);
                self.builder.ins().call(release_ref, &[val]);
                Ok(())
            }
            Type::Generic { base, .. } if base == "Thread" || base == "thread" => {
                let release_ref = self.module.declare_func_in_func(self.concurrency.thread_release_id, self.builder.func);
                self.builder.ins().call(release_ref, &[val]);
                Ok(())
            }
            Type::Generic { base, type_args } => {
                let spec = crate::semantic::specialized_name(base, type_args);
                if let Some(info) = self.classes.get(&spec) {
                    let release_ref = self.module.declare_func_in_func(info.release_id, self.builder.func);
                    self.builder.ins().call(release_ref, &[val]);
                }
                Ok(())
            }
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

    /// Shared by every class -- see `build_class_retain`'s doc comment
    /// for why retain (unlike `new`/`release`) doesn't need to be
    /// generated per class.
    fn emit_class_retain(&mut self, val: Value) -> Result<()> {
        let func_ref = self.module.declare_func_in_func(self.class_retain_id, self.builder.func);
        self.builder.ins().call(func_ref, &[val]);
        Ok(())
    }

    fn emit_class_new(&mut self, class_name: &str, arguments: &[Value]) -> Result<Value> {
        let new_id = self.classes.get(class_name)
            .map(|info| info.new_id)
            .expect("check_and_lower already verified this class exists");
        let func_ref = self.module.declare_func_in_func(new_id, self.builder.func);
        let call = self.builder.ins().call(func_ref, arguments);
        Ok(self.builder.inst_results(call)[0])
    }

    /// A field's byte offset within its class's instance layout --
    /// `CLASS_HEADER_SIZE` (the refcount) plus 8 bytes per field before
    /// it, in declaration order (see `ClassInfo`'s doc comment).
    fn class_field_offset(&self, class_name: &str, field: &str) -> i32 {
        let fields = &self.classes.get(class_name)
            .expect("check_and_lower already verified this class exists")
            .fields;
        let index = fields.iter().position(|(name, _)| name == field)
            .expect("check_and_lower already verified this field exists on this class");
        (CLASS_HEADER_SIZE + (index as i64) * 8) as i32
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
            TypedStatement::Let { name, type_, value, .. } => {
                require_supported_type(type_, &value.location, &format!("variable '{}'", name))?;
                let val = self.compile_expression(value)?;
                self.retain_if_aliasing(value, val)?;
                let var = self.declare_local(name, type_.clone());
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Assign { name, value, .. } => {
                let val = self.compile_expression(value)?;
                self.retain_if_aliasing(value, val)?;
                let (var, ty) = self.lookup_local(name);
                let val = self.coerce_for_target_type(val, &value.type_, &ty);
                let old_val = self.builder.use_var(var);
                self.emit_release_for_type(&ty, old_val)?;
                self.builder.def_var(var, val);
                Ok(false)
            }
            TypedStatement::Expression { expr, .. } => {
                let val = self.compile_expression(expr)?;
                if !expression_is_borrowed(expr) {
                    self.emit_release_for_type(&expr.type_, val)?;
                }
                Ok(false)
            }
            TypedStatement::Return { value, .. } => {
                match value {
                    Some(expr) => {
                        let val = self.compile_expression(expr)?;
                        self.retain_if_aliasing(expr, val)?;
                        let val = self.coerce_for_target_type(val, &expr.type_, &self.current_return_type.clone());
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
            TypedStatement::ClassDecl { .. } | TypedStatement::EnumDecl { .. } | TypedStatement::TraitDecl { .. } | TypedStatement::ImplDecl { .. } | TypedStatement::ExternFunction { .. } => {
                // Top-level constructs, handled elsewhere.
                Ok(false)
            }
            TypedStatement::Match { expr, arms, .. } => {
                let target_val = self.compile_expression(expr)?;
                let is_enum = matches!(expr.type_, Type::Enum(_));
                let tag_val = if is_enum {
                    Some(self.builder.ins().load(VALUE_TYPE, MemFlags::new(), target_val, ENUM_HEADER_SIZE as i32))
                } else {
                    None
                };

                let exit_block = self.builder.create_block();
                let mut all_terminated = !arms.is_empty();

                for arm in arms {
                    let next_arm_block = self.builder.create_block();
                    let arm_body_block = self.builder.create_block();

                    match &arm.pattern {
                        TypedPattern::Variant { tag, bindings, .. } => {
                            if let Some(tv) = tag_val {
                                let expected_tag = self.builder.ins().iconst(VALUE_TYPE, *tag as i64);
                                let matches_tag = self.builder.ins().icmp(IntCC::Equal, tv, expected_tag);
                                self.builder.ins().brif(matches_tag, arm_body_block, &[], next_arm_block, &[]);

                                self.builder.switch_to_block(arm_body_block);
                                self.builder.seal_block(arm_body_block);
                                self.push_scope();

                                for (i, b_pat) in bindings.iter().enumerate() {
                                    let offset = (ENUM_HEADER_SIZE + 8 + (i as i64) * 8) as i32;
                                    match b_pat {
                                        TypedPattern::Variable { name, type_: b_type, .. } => {
                                            if name != "_" {
                                                let cranelift_ty = cranelift_type_for(b_type);
                                                let val = self.builder.ins().load(cranelift_ty, MemFlags::new(), target_val, offset);
                                                let var = self.declare_local(name, b_type.clone());
                                                self.builder.def_var(var, val);
                                            }
                                        }
                                        TypedPattern::Variant { tag: sub_tag, .. } => {
                                            let sub_ptr = self.builder.ins().load(VALUE_TYPE, MemFlags::new(), target_val, offset);
                                            let sub_tag_val = self.builder.ins().load(VALUE_TYPE, MemFlags::new(), sub_ptr, ENUM_HEADER_SIZE as i32);
                                            let expected_sub_tag = self.builder.ins().iconst(VALUE_TYPE, *sub_tag as i64);
                                            let sub_matches = self.builder.ins().icmp(IntCC::Equal, sub_tag_val, expected_sub_tag);
                                            let sub_pass = self.builder.create_block();
                                            self.builder.ins().brif(sub_matches, sub_pass, &[], next_arm_block, &[]);
                                            self.builder.switch_to_block(sub_pass);
                                            self.builder.seal_block(sub_pass);
                                        }
                                        _ => {}
                                    }
                                }

                                if let Some(guard) = &arm.guard {
                                    let guard_val = self.compile_expression(guard)?;
                                    let guard_pass = self.builder.create_block();
                                    self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                    self.builder.switch_to_block(guard_pass);
                                    self.builder.seal_block(guard_pass);
                                }

                                let terminated = self.compile_statement(&arm.body)?;
                                if !terminated {
                                    self.release_frame(self.scopes.len() - 1)?;
                                    self.builder.ins().jump(exit_block, &[]);
                                    all_terminated = false;
                                } else {
                                    self.release_frame(self.scopes.len() - 1)?;
                                }
                                self.scopes.pop();

                                self.builder.switch_to_block(next_arm_block);
                                self.builder.seal_block(next_arm_block);
                            } else {
                                self.builder.ins().jump(next_arm_block, &[]);
                                self.builder.switch_to_block(next_arm_block);
                                self.builder.seal_block(next_arm_block);
                            }
                        }
                        TypedPattern::Struct { class_name, fields, .. } => {
                            let not_null = self.builder.ins().icmp_imm(IntCC::NotEqual, target_val, 0);
                            self.builder.ins().brif(not_null, arm_body_block, &[], next_arm_block, &[]);

                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            for (f_name, f_pat) in fields {
                                if let TypedPattern::Variable { name, type_: f_type, .. } = f_pat {
                                    if name != "_" {
                                        let offset = self.class_field_offset(class_name, f_name);
                                        let cranelift_ty = cranelift_type_for(f_type);
                                        let val = self.builder.ins().load(cranelift_ty, MemFlags::new(), target_val, offset);
                                        let var = self.declare_local(name, f_type.clone());
                                        self.builder.def_var(var, val);
                                    }
                                }
                            }

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                        TypedPattern::Range { start, end, inclusive, .. } => {
                            let start_val = self.compile_expression(start)?;
                            let end_val = self.compile_expression(end)?;
                            let ge = self.builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, target_val, start_val);
                            let le = self.builder.ins().icmp(if *inclusive { IntCC::SignedLessThanOrEqual } else { IntCC::SignedLessThan }, target_val, end_val);
                            let in_range = self.builder.ins().band(ge, le);
                            self.builder.ins().brif(in_range, arm_body_block, &[], next_arm_block, &[]);

                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                        TypedPattern::Literal { expr: lit, .. } => {
                            let lit_val = self.compile_expression(lit)?;
                            let eq = self.builder.ins().icmp(IntCC::Equal, target_val, lit_val);
                            self.builder.ins().brif(eq, arm_body_block, &[], next_arm_block, &[]);

                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                        TypedPattern::Wildcard { .. } => {
                            self.builder.ins().jump(arm_body_block, &[]);
                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                        TypedPattern::Variable { name, type_: ty, .. } => {
                            self.builder.ins().jump(arm_body_block, &[]);
                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            if name != "_" {
                                let var = self.declare_local(name, ty.clone());
                                self.builder.def_var(var, target_val);
                            }

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                        TypedPattern::Slice { prefix, slice_rest, suffix, .. } => {
                            let len_val = self.emit_list_length(target_val)?;
                            let min_len = (prefix.len() + suffix.len()) as i64;
                            let min_len_val = self.builder.ins().iconst(VALUE_TYPE, min_len);
                            let cond = if slice_rest.is_some() {
                                self.builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, len_val, min_len_val)
                            } else {
                                self.builder.ins().icmp(IntCC::Equal, len_val, min_len_val)
                            };
                            self.builder.ins().brif(cond, arm_body_block, &[], next_arm_block, &[]);

                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            for (i, p) in prefix.iter().enumerate() {
                                if let TypedPattern::Variable { name, type_: p_type, .. } = p {
                                    if name != "_" {
                                        let idx_val = self.builder.ins().iconst(VALUE_TYPE, i as i64);
                                        let elem_val = self.emit_list_get(target_val, idx_val)?;
                                        let var = self.declare_local(name, p_type.clone());
                                        self.builder.def_var(var, elem_val);
                                    }
                                }
                            }

                            if let Some(rest_pat) = slice_rest {
                                if let TypedPattern::Variable { name, type_: r_type, .. } = &**rest_pat {
                                    if name != "_" {
                                        let tail_list = self.emit_list_new()?;

                                        let start_idx = prefix.len() as i64;
                                        let loop_head = self.builder.create_block();
                                        let loop_body = self.builder.create_block();
                                        let loop_exit = self.builder.create_block();

                                        let idx_var = self.declare_local(&format!("__tail_idx_{}", self.scopes.len()), Type::Int);
                                        let start_val = self.builder.ins().iconst(VALUE_TYPE, start_idx);
                                        self.builder.def_var(idx_var, start_val);
                                        self.builder.ins().jump(loop_head, &[]);

                                        self.builder.switch_to_block(loop_head);
                                        let curr_idx = self.builder.use_var(idx_var);
                                        let has_more = self.builder.ins().icmp(IntCC::SignedLessThan, curr_idx, len_val);
                                        self.builder.ins().brif(has_more, loop_body, &[], loop_exit, &[]);

                                        self.builder.switch_to_block(loop_body);
                                        self.builder.seal_block(loop_body);
                                        let item_val = self.emit_list_get(target_val, curr_idx)?;
                                        self.emit_list_push(tail_list, item_val)?;
                                        let one = self.builder.ins().iconst(VALUE_TYPE, 1);
                                        let next_idx = self.builder.ins().iadd(curr_idx, one);
                                        self.builder.def_var(idx_var, next_idx);
                                        self.builder.ins().jump(loop_head, &[]);

                                        self.builder.switch_to_block(loop_exit);
                                        self.builder.seal_block(loop_head);
                                        self.builder.seal_block(loop_exit);

                                        let var = self.declare_local(name, r_type.clone());
                                        self.builder.def_var(var, tail_list);
                                    }
                                }
                            }

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                        TypedPattern::Tuple { elements, .. } => {
                            let len_val = self.emit_list_length(target_val)?;
                            let expected_len = self.builder.ins().iconst(VALUE_TYPE, elements.len() as i64);
                            let cond = self.builder.ins().icmp(IntCC::Equal, len_val, expected_len);
                            self.builder.ins().brif(cond, arm_body_block, &[], next_arm_block, &[]);

                            self.builder.switch_to_block(arm_body_block);
                            self.builder.seal_block(arm_body_block);
                            self.push_scope();

                            for (i, elem) in elements.iter().enumerate() {
                                if let TypedPattern::Variable { name, type_: e_type, .. } = elem {
                                    if name != "_" {
                                        let idx_val = self.builder.ins().iconst(VALUE_TYPE, i as i64);
                                        let elem_val = self.emit_list_get(target_val, idx_val)?;
                                        let var = self.declare_local(name, e_type.clone());
                                        self.builder.def_var(var, elem_val);
                                    }
                                }
                            }

                            if let Some(guard) = &arm.guard {
                                let guard_val = self.compile_expression(guard)?;
                                let guard_pass = self.builder.create_block();
                                self.builder.ins().brif(guard_val, guard_pass, &[], next_arm_block, &[]);
                                self.builder.switch_to_block(guard_pass);
                                self.builder.seal_block(guard_pass);
                            }

                            let terminated = self.compile_statement(&arm.body)?;
                            if !terminated {
                                self.release_frame(self.scopes.len() - 1)?;
                                self.builder.ins().jump(exit_block, &[]);
                                all_terminated = false;
                            } else {
                                self.release_frame(self.scopes.len() - 1)?;
                            }
                            self.scopes.pop();

                            self.builder.switch_to_block(next_arm_block);
                            self.builder.seal_block(next_arm_block);
                        }
                    }
                }

                self.builder.ins().trap(TrapCode::UnreachableCodeReached);

                if !all_terminated {
                    self.builder.switch_to_block(exit_block);
                    self.builder.seal_block(exit_block);
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            TypedStatement::FieldAssign { object, field, field_type, value, .. } => {
                let class_name = match &object.type_ {
                    Type::Class(name) => name.clone(),
                    other => unreachable!(
                        "check_and_lower guarantees a FieldAssign's object is always a class instance, got {:?}",
                        other
                    ),
                };
                let object_val = self.compile_expression(object)?;

                let val = self.compile_expression(value)?;
                self.retain_if_aliasing(value, val)?;
                let val = self.coerce_for_target_type(val, &value.type_, field_type);

                let offset = self.class_field_offset(&class_name, field);
                let old_val = self.builder.ins().load(cranelift_type_for(field_type), MemFlags::new(), object_val, offset);
                self.emit_release_for_type(field_type, old_val)?;
                self.builder.ins().store(MemFlags::new(), val, object_val, offset);

                // If `object` is a fresh, owned temporary (not
                // ultimately rooted in a plain variable -- see
                // expression_is_borrowed), nothing else owns it going
                // forward -- release it now, or it leaks. A borrowed
                // object (e.g. `a` in `a.b = x`, or even `a.b` in
                // `a.b.c = x` when `a` is a plain variable) must NOT be
                // released here -- its lifetime belongs to whatever
                // variable it's ultimately rooted in.
                if !expression_is_borrowed(object) {
                    self.emit_release_for_type(&object.type_, object_val)?;
                }

                Ok(false)
            }
            TypedStatement::UnsafeBlock { body, .. } => {
                self.compile_statement(body)
            }
            TypedStatement::Comptime { body, .. } => {
                self.compile_statement(body)
            }
        }
    }

    fn compile_expression(&mut self, expr: &TypedExpression) -> Result<Value> {
        match &expr.kind {
            TypedExpressionKind::Number(s) => {
                if expr.type_ == Type::Float {
                    let n: f64 = s.parse().map_err(|_| anyhow!("invalid float literal '{}'", s))?;
                    Ok(self.builder.ins().f64const(n))
                } else {
                    let n: i64 = s.parse().map_err(|_| anyhow!("invalid integer literal '{}'", s))?;
                    Ok(self.builder.ins().iconst(VALUE_TYPE, n))
                }
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
                    UnaryOperator::Negate if operand.type_ == Type::Float => Ok(self.builder.ins().fneg(val)),
                    UnaryOperator::Negate => Ok(self.builder.ins().ineg(val)),
                    UnaryOperator::Not => {
                        let zero = self.builder.ins().iconst(VALUE_TYPE, 0);
                        Ok(self.compile_cmp(IntCC::Equal, val, zero))
                    }
                }
            }
            TypedExpressionKind::Binary { left, operator, right } => self.compile_binary(left, operator, right),
            TypedExpressionKind::Call { function, arguments } => self.compile_call(function, arguments, &expr.type_, &expr.location),
            TypedExpressionKind::New { class_name, arguments } => {
                // Each argument follows the same ownership handoff as a
                // function call argument (see compile_call): compile it
                // (an owned temp), then retain-if-aliasing, since the
                // constructor stores it into a field -- taking
                // ownership of a reference distinct from wherever it
                // came from, if it was a bare identifier.
                let field_types: Vec<Type> = self.classes.get(class_name)
                    .map(|info| info.fields.iter().map(|(_, ty)| ty.clone()).collect())
                    .unwrap_or_default();
                let mut arg_values = Vec::with_capacity(arguments.len());
                for (i, arg) in arguments.iter().enumerate() {
                    let val = self.compile_expression(arg)?;
                    self.retain_if_aliasing(arg, val)?;
                    let val = match field_types.get(i) {
                        Some(field_type) => self.coerce_for_target_type(val, &arg.type_, field_type),
                        None => val,
                    };
                    arg_values.push(val);
                }
                self.emit_class_new(class_name, &arg_values)
            }
            TypedExpressionKind::FieldAccess { object, field } => {
                let class_name = match &object.type_ {
                    Type::Class(name) => name.clone(),
                    other => unreachable!(
                        "check_and_lower guarantees a FieldAccess's object is always a class instance, got {:?}",
                        other
                    ),
                };
                let object_val = self.compile_expression(object)?;
                let offset = self.class_field_offset(&class_name, field);
                let field_val = self.builder.ins().load(cranelift_type_for(&expr.type_), MemFlags::new(), object_val, offset);

                if !expression_is_borrowed(object) {
                    // `object`'s value is a fresh, owned temporary
                    // (e.g. `some_call().field`) with nothing else
                    // referencing it. Protect the field we're
                    // extracting from `object`'s own release (which
                    // would otherwise recursively release this same
                    // field, since it doesn't know we still need it)
                    // by retaining it *first*, then release `object`
                    // itself, or it leaks. The result is now a fresh,
                    // independently-owned value -- exactly like a
                    // normal function call's result, needing no further
                    // retention wherever it's consumed next.
                    self.retain_for_arc_type(&expr.type_, field_val)?;
                    self.emit_release_for_type(&object.type_, object_val)?;
                }
                // Else: `object` is borrowed (ultimately rooted in a
                // plain variable that stays alive on its own), so the
                // field value is borrowed right along with it -- no
                // retain/release here. Whatever consumes this
                // FieldAccess result treats it exactly like reading a
                // plain variable (see retain_if_aliasing).

                Ok(field_val)
            }
            TypedExpressionKind::Try(inner) => {
                let inner_val = self.compile_expression(inner)?;
                let status_flag = self.builder.ins().load(types::I64, MemFlags::new(), inner_val, 8);
                let ok_block = self.builder.create_block();
                let err_block = self.builder.create_block();
                self.builder.ins().brif(status_flag, ok_block, &[], err_block, &[]);

                self.builder.switch_to_block(err_block);
                self.retain_if_aliasing(inner, inner_val)?;
                self.builder.ins().return_(&[inner_val]);

                self.builder.switch_to_block(ok_block);
                let unwrapped = self.builder.ins().load(types::I64, MemFlags::new(), inner_val, 16);
                Ok(unwrapped)
            }
            TypedExpressionKind::EnumConstruct { tag, arguments, .. } => {
                let malloc_ref = self.module.declare_func_in_func(self.malloc_id, self.builder.func);
                let total_size = ENUM_HEADER_SIZE + 8 + (arguments.len() as i64) * 8;
                let size_val = self.builder.ins().iconst(VALUE_TYPE, total_size);
                let call = self.builder.ins().call(malloc_ref, &[size_val]);
                let ptr = self.builder.inst_results(call)[0];

                let one = self.builder.ins().iconst(VALUE_TYPE, 1);
                self.builder.ins().store(MemFlags::new(), one, ptr, 0);

                let tag_val = self.builder.ins().iconst(VALUE_TYPE, *tag as i64);
                self.builder.ins().store(MemFlags::new(), tag_val, ptr, ENUM_HEADER_SIZE as i32);

                for (i, arg) in arguments.iter().enumerate() {
                    let arg_val = self.compile_expression(arg)?;
                    let offset = (ENUM_HEADER_SIZE + 8 + (i as i64) * 8) as i32;
                    self.builder.ins().store(MemFlags::new(), arg_val, ptr, offset);
                }

                Ok(ptr)
            }
            TypedExpressionKind::Closure { id, captures, .. } => {
                let total_size = 16 + captures.len() * 8;
                let size_val = self.builder.ins().iconst(VALUE_TYPE, total_size as i64);
                let malloc_ref = self.module.declare_func_in_func(self.malloc_id, self.builder.func);
                let call = self.builder.ins().call(malloc_ref, &[size_val]);
                let closure_ptr = self.builder.inst_results(call)[0];

                let one = self.builder.ins().iconst(VALUE_TYPE, 1);
                self.builder.ins().store(MemFlags::new(), one, closure_ptr, 0);

                let closure_func_id = *self.closure_funcs.get(id)
                    .ok_or_else(|| anyhow!("closure {} not declared", id))?;
                let func_ref = self.module.declare_func_in_func(closure_func_id, self.builder.func);
                let fn_ptr = self.builder.ins().func_addr(VALUE_TYPE, func_ref);
                self.builder.ins().store(MemFlags::new(), fn_ptr, closure_ptr, 8);

                for (i, (cap_name, cap_ty)) in captures.iter().enumerate() {
                    let (var, _ty) = self.lookup_local(cap_name);
                    let val = self.builder.use_var(var);
                    self.retain_for_arc_type(cap_ty, val)?;
                    let offset = (16 + i * 8) as i32;
                    self.builder.ins().store(MemFlags::new(), val, closure_ptr, offset);
                }

                Ok(closure_ptr)
            }
            TypedExpressionKind::FunctionRef(name) => {
                let total_size = 16;
                let size_val = self.builder.ins().iconst(VALUE_TYPE, total_size as i64);
                let malloc_ref = self.module.declare_func_in_func(self.malloc_id, self.builder.func);
                let call = self.builder.ins().call(malloc_ref, &[size_val]);
                let closure_ptr = self.builder.inst_results(call)[0];

                let one = self.builder.ins().iconst(VALUE_TYPE, 1);
                self.builder.ins().store(MemFlags::new(), one, closure_ptr, 0);

                let trampoline_id = *self.trampoline_funcs.get(name)
                    .ok_or_else(|| anyhow!("trampoline for {} not declared", name))?;
                let func_ref = self.module.declare_func_in_func(trampoline_id, self.builder.func);
                let fn_ptr = self.builder.ins().func_addr(VALUE_TYPE, func_ref);
                self.builder.ins().store(MemFlags::new(), fn_ptr, closure_ptr, 8);

                Ok(closure_ptr)
            }
            TypedExpressionKind::IndirectCall { callee, arguments } => {
                let closure_ptr = self.compile_expression(callee)?;
                let fn_ptr = self.builder.ins().load(VALUE_TYPE, MemFlags::new(), closure_ptr, 8);

                let mut sig = self.module.make_signature();
                sig.params.push(AbiParam::new(VALUE_TYPE));
                for arg in arguments {
                    sig.params.push(AbiParam::new(cranelift_type_for(&arg.type_)));
                }
                if !matches!(expr.type_, Type::Void) {
                    sig.returns.push(AbiParam::new(cranelift_type_for(&expr.type_)));
                }

                let sig_ref = self.builder.import_signature(sig);

                let mut call_args = Vec::with_capacity(arguments.len() + 1);
                call_args.push(closure_ptr);
                for arg in arguments {
                    let arg_val = self.compile_expression(arg)?;
                    self.retain_if_aliasing(arg, arg_val)?;
                    call_args.push(arg_val);
                }

                let call = self.builder.ins().call_indirect(sig_ref, fn_ptr, &call_args);
                let results = self.builder.inst_results(call);
                if results.is_empty() {
                    Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
                } else {
                    Ok(results[0])
                }
            }
            TypedExpressionKind::MethodCall { object, class_name, method, arguments } => {
                let obj_val = self.compile_expression(object)?;
                self.retain_if_aliasing(object, obj_val)?;
                let mut call_args = Vec::with_capacity(arguments.len() + 1);
                call_args.push(obj_val);
                for arg in arguments {
                    let arg_val = self.compile_expression(arg)?;
                    self.retain_if_aliasing(arg, arg_val)?;
                    call_args.push(arg_val);
                }

                let func_id = *self.methods.get(&(class_name.clone(), method.clone()))
                    .expect("method registered in declare_all_classes");
                let func_ref = self.module.declare_func_in_func(func_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &call_args);
                let results = self.builder.inst_results(call);
                if results.is_empty() {
                    Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
                } else {
                    Ok(results[0])
                }
            }
            TypedExpressionKind::TraitMethodCall { object, slot, arguments, .. } => {
                let obj_val = self.compile_expression(object)?;
                self.retain_if_aliasing(object, obj_val)?;
                let vtable_ptr = self.builder.ins().load(VALUE_TYPE, MemFlags::new(), obj_val, 8);
                let fn_ptr = self.builder.ins().load(VALUE_TYPE, MemFlags::new(), vtable_ptr, (*slot * 8) as i32);

                let mut sig = self.module.make_signature();
                sig.params.push(AbiParam::new(VALUE_TYPE)); // 'this'
                for arg in arguments {
                    sig.params.push(AbiParam::new(cranelift_type_for(&arg.type_)));
                }
                if !matches!(expr.type_, Type::Void) {
                    sig.returns.push(AbiParam::new(cranelift_type_for(&expr.type_)));
                }
                let sig_ref = self.builder.import_signature(sig);

                let mut call_args = Vec::with_capacity(arguments.len() + 1);
                call_args.push(obj_val);
                for arg in arguments {
                    let arg_val = self.compile_expression(arg)?;
                    self.retain_if_aliasing(arg, arg_val)?;
                    call_args.push(arg_val);
                }

                let call = self.builder.ins().call_indirect(sig_ref, fn_ptr, &call_args);
                let results = self.builder.inst_results(call);
                if results.is_empty() {
                    Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
                } else {
                    Ok(results[0])
                }
            }
        }
    }

    fn compile_cmp(&mut self, cc: IntCC, l: Value, r: Value) -> Value {
        let cmp = self.builder.ins().icmp(cc, l, r);
        self.builder.ins().uextend(VALUE_TYPE, cmp)
    }

    fn compile_fcmp(&mut self, cc: FloatCC, l: Value, r: Value) -> Value {
        let cmp = self.builder.ins().fcmp(cc, l, r);
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
        // A numeric op is "float" if either side resolved to Float --
        // matches the exact promotion direction the type checker
        // already decided in numeric_binary_result_type (mixed int/
        // float promotes to float), so codegen and type-checking never
        // disagree about which representation a given expression uses.
        let is_float_op = !is_string_op && (left.type_ == Type::Float || right.type_ == Type::Float);

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
            if !expression_is_borrowed(left) {
                self.emit_string_release(l)?;
            }
            if !expression_is_borrowed(right) {
                self.emit_string_release(r)?;
            }
            return Ok(result);
        }

        if is_float_op {
            // A mixed int/float pair needs the int side explicitly
            // converted -- Cranelift has no implicit numeric promotion
            // the way the C/Java/Roze source-level semantics do, so
            // this has to happen here, once per operand, rather than
            // anywhere the value was originally produced (an Int-typed
            // value stays a plain i64 everywhere else it's used).
            let lf = if left.type_ == Type::Float { l } else { self.builder.ins().fcvt_from_sint(types::F64, l) };
            let rf = if right.type_ == Type::Float { r } else { self.builder.ins().fcvt_from_sint(types::F64, r) };
            return Ok(match operator {
                BinaryOperator::Add => self.builder.ins().fadd(lf, rf),
                BinaryOperator::Subtract => self.builder.ins().fsub(lf, rf),
                BinaryOperator::Multiply => self.builder.ins().fmul(lf, rf),
                BinaryOperator::Divide => self.builder.ins().fdiv(lf, rf),
                BinaryOperator::Modulo => {
                    return Err(anyhow!("Modulo on floating-point values is not currently supported in native backend"));
                }
                BinaryOperator::Equal => self.compile_fcmp(FloatCC::Equal, lf, rf),
                BinaryOperator::NotEqual => self.compile_fcmp(FloatCC::NotEqual, lf, rf),
                BinaryOperator::LessThan => self.compile_fcmp(FloatCC::LessThan, lf, rf),
                BinaryOperator::GreaterThan => self.compile_fcmp(FloatCC::GreaterThan, lf, rf),
                BinaryOperator::LessEqual => self.compile_fcmp(FloatCC::LessThanOrEqual, lf, rf),
                BinaryOperator::GreaterEqual => self.compile_fcmp(FloatCC::GreaterThanOrEqual, lf, rf),
                BinaryOperator::And | BinaryOperator::Or => unreachable!("handled above"),
            });
        }

        Ok(match operator {
            BinaryOperator::Add => self.builder.ins().iadd(l, r),
            BinaryOperator::Subtract => self.builder.ins().isub(l, r),
            BinaryOperator::Multiply => self.builder.ins().imul(l, r),
            BinaryOperator::Divide => self.builder.ins().sdiv(l, r),
            BinaryOperator::Modulo => self.builder.ins().srem(l, r),
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

    fn compile_call(&mut self, name: &str, arguments: &[TypedExpression], return_type: &Type, location: &Location) -> Result<Value> {
        if name == "println" {
            return self.compile_println(arguments);
        }

        match name {
            "list_new" if arguments.is_empty() => return self.emit_list_new(),
            "list_push" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "list_push")?;
                let list = self.compile_expression(&arguments[0])?;
                let value = self.compile_expression(&arguments[1])?;
                let result = self.emit_list_push(list, value)?;
                self.release_if_fresh(&arguments[0], list)?;
                return Ok(result);
            }
            "list_get" if arguments.len() == 2 => {
                let list = self.compile_expression(&arguments[0])?;
                let index = self.compile_expression(&arguments[1])?;
                let result = self.emit_list_get(list, index)?;
                self.release_if_fresh(&arguments[0], list)?;
                return Ok(result);
            }
            "list_set" if arguments.len() == 3 => {
                self.reject_unless_supported_as_container_value(&arguments[2], location, "list_set")?;
                let list = self.compile_expression(&arguments[0])?;
                let index = self.compile_expression(&arguments[1])?;
                let value = self.compile_expression(&arguments[2])?;
                let result = self.emit_list_set(list, index, value)?;
                self.release_if_fresh(&arguments[0], list)?;
                return Ok(result);
            }
            "list_remove" if arguments.len() == 2 => {
                let list = self.compile_expression(&arguments[0])?;
                let index = self.compile_expression(&arguments[1])?;
                let result = self.emit_list_remove(list, index)?;
                self.release_if_fresh(&arguments[0], list)?;
                return Ok(result);
            }
            "list_length" if arguments.len() == 1 => {
                let list = self.compile_expression(&arguments[0])?;
                let result = self.emit_list_length(list)?;
                self.release_if_fresh(&arguments[0], list)?;
                return Ok(result);
            }
            "list_is_empty" if arguments.len() == 1 => {
                let list = self.compile_expression(&arguments[0])?;
                let result = self.emit_list_is_empty(list)?;
                self.release_if_fresh(&arguments[0], list)?;
                return Ok(result);
            }
            "map_new" if arguments.is_empty() => return self.emit_map_new(),
            "map_put" if arguments.len() == 3 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_put (key)")?;
                self.reject_unless_supported_as_container_value(&arguments[2], location, "map_put (value)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                let value = self.compile_expression(&arguments[2])?;
                let result = self.emit_map_put(map, key, value)?;
                self.release_if_fresh(&arguments[0], map)?;
                return Ok(result);
            }
            "map_get" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_get (key)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                let result = self.emit_map_get(map, key)?;
                self.release_if_fresh(&arguments[0], map)?;
                return Ok(result);
            }
            "map_has" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_has (key)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                let result = self.emit_map_has(map, key)?;
                self.release_if_fresh(&arguments[0], map)?;
                return Ok(result);
            }
            "map_remove" if arguments.len() == 2 => {
                self.reject_unless_supported_as_container_value(&arguments[1], location, "map_remove (key)")?;
                let map = self.compile_expression(&arguments[0])?;
                let key = self.compile_expression(&arguments[1])?;
                let result = self.emit_map_remove(map, key)?;
                self.release_if_fresh(&arguments[0], map)?;
                return Ok(result);
            }
            "map_size" if arguments.len() == 1 => {
                let map = self.compile_expression(&arguments[0])?;
                let result = self.emit_map_size(map)?;
                self.release_if_fresh(&arguments[0], map)?;
                return Ok(result);
            }
            "map_is_empty" if arguments.len() == 1 => {
                let map = self.compile_expression(&arguments[0])?;
                let result = self.emit_map_is_empty(map)?;
                self.release_if_fresh(&arguments[0], map)?;
                return Ok(result);
            }

            // ---- Concurrency: Threads & Channels (MED-03) ----
            "thread_spawn" if arguments.len() == 1 => {
                let closure_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.thread_spawn_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[closure_val]);
                let result = self.builder.inst_results(call)[0];
                return Ok(result);
            }
            "thread_join" if arguments.len() == 1 => {
                let handle_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.thread_join_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[handle_val]);
                let result = self.builder.inst_results(call)[0];
                self.release_if_fresh(&arguments[0], handle_val)?;
                return Ok(result);
            }
            "thread_sleep" if arguments.len() == 1 => {
                let ms_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.thread_sleep_id, self.builder.func);
                self.builder.ins().call(func_ref, &[ms_val]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "thread_current_id" if arguments.is_empty() => {
                let func_ref = self.module.declare_func_in_func(self.concurrency.thread_current_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[]);
                return Ok(self.builder.inst_results(call)[0]);
            }
            "channel_new" => {
                let cap_val = if arguments.len() == 1 {
                    self.compile_expression(&arguments[0])?
                } else {
                    self.builder.ins().iconst(VALUE_TYPE, 64)
                };
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_new_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[cap_val]);
                return Ok(self.builder.inst_results(call)[0]);
            }
            "channel_send" | "send" if arguments.len() == 2 => {
                let chan_val = self.compile_expression(&arguments[0])?;
                let item_val = self.compile_expression(&arguments[1])?;
                self.retain_for_arc_type(&arguments[1].type_, item_val)?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_send_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[chan_val, item_val]);
                let result = self.builder.inst_results(call)[0];
                self.release_if_fresh(&arguments[0], chan_val)?;
                self.release_if_fresh(&arguments[1], item_val)?;
                return Ok(result);
            }
            "channel_recv" | "recv" if arguments.len() == 1 => {
                let chan_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_recv_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[chan_val]);
                let result = self.builder.inst_results(call)[0];
                self.release_if_fresh(&arguments[0], chan_val)?;
                return Ok(result);
            }
            "channel_close" if arguments.len() == 1 => {
                let chan_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_close_id, self.builder.func);
                self.builder.ins().call(func_ref, &[chan_val]);
                self.release_if_fresh(&arguments[0], chan_val)?;
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "channel_is_closed" if arguments.len() == 1 => {
                let chan_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_is_closed_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[chan_val]);
                let result = self.builder.inst_results(call)[0];
                self.release_if_fresh(&arguments[0], chan_val)?;
                return Ok(result);
            }
            "channel_len" if arguments.len() == 1 => {
                let chan_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.concurrency.channel_len_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[chan_val]);
                let result = self.builder.inst_results(call)[0];
                self.release_if_fresh(&arguments[0], chan_val)?;
                return Ok(result);
            }

            // Direct memory intrinsics (MED-05)
            "alloc" if arguments.len() == 1 => {
                let size_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.raw_memory.malloc_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[size_val]);
                return Ok(self.builder.inst_results(call)[0]);
            }
            "alloc_zeroed" if arguments.len() == 1 => {
                let size_val = self.compile_expression(&arguments[0])?;
                let one_val = self.builder.ins().iconst(VALUE_TYPE, 1);
                let func_ref = self.module.declare_func_in_func(self.raw_memory.calloc_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[one_val, size_val]);
                return Ok(self.builder.inst_results(call)[0]);
            }
            "realloc" if arguments.len() == 2 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let size_val = self.compile_expression(&arguments[1])?;
                let func_ref = self.module.declare_func_in_func(self.raw_memory.realloc_id, self.builder.func);
                let call = self.builder.ins().call(func_ref, &[ptr_val, size_val]);
                return Ok(self.builder.inst_results(call)[0]);
            }
            "free" if arguments.len() == 1 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.raw_memory.free_id, self.builder.func);
                self.builder.ins().call(func_ref, &[ptr_val]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "ptr_read_u8" if arguments.len() == 1 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let byte_val = self.builder.ins().load(types::I8, MemFlags::trusted(), ptr_val, 0);
                return Ok(self.builder.ins().uextend(VALUE_TYPE, byte_val));
            }
            "ptr_read" | "ptr_read_i64" if arguments.len() == 1 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                return Ok(self.builder.ins().load(types::I64, MemFlags::trusted(), ptr_val, 0));
            }
            "ptr_read_at" if arguments.len() == 2 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let offset_val = self.compile_expression(&arguments[1])?;
                let addr = self.builder.ins().iadd(ptr_val, offset_val);
                return Ok(self.builder.ins().load(types::I64, MemFlags::trusted(), addr, 0));
            }
            "ptr_read_u8_at" if arguments.len() == 2 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let offset_val = self.compile_expression(&arguments[1])?;
                let addr = self.builder.ins().iadd(ptr_val, offset_val);
                let byte_val = self.builder.ins().load(types::I8, MemFlags::trusted(), addr, 0);
                return Ok(self.builder.ins().uextend(VALUE_TYPE, byte_val));
            }
            "ptr_write_u8" if arguments.len() == 2 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let val = self.compile_expression(&arguments[1])?;
                let byte_val = self.builder.ins().ireduce(types::I8, val);
                self.builder.ins().store(MemFlags::trusted(), byte_val, ptr_val, 0);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "ptr_write" | "ptr_write_i64" if arguments.len() == 2 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let val = self.compile_expression(&arguments[1])?;
                self.builder.ins().store(MemFlags::trusted(), val, ptr_val, 0);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "ptr_write_at" if arguments.len() == 3 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let offset_val = self.compile_expression(&arguments[1])?;
                let val = self.compile_expression(&arguments[2])?;
                let addr = self.builder.ins().iadd(ptr_val, offset_val);
                self.builder.ins().store(MemFlags::trusted(), val, addr, 0);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "ptr_write_u8_at" if arguments.len() == 3 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let offset_val = self.compile_expression(&arguments[1])?;
                let val = self.compile_expression(&arguments[2])?;
                let addr = self.builder.ins().iadd(ptr_val, offset_val);
                let byte_val = self.builder.ins().ireduce(types::I8, val);
                self.builder.ins().store(MemFlags::trusted(), byte_val, addr, 0);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "ptr_add" | "ptr_offset" if arguments.len() == 2 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let offset_val = self.compile_expression(&arguments[1])?;
                return Ok(self.builder.ins().iadd(ptr_val, offset_val));
            }
            "ptr_diff" if arguments.len() == 2 => {
                let p1_val = self.compile_expression(&arguments[0])?;
                let p2_val = self.compile_expression(&arguments[1])?;
                return Ok(self.builder.ins().isub(p1_val, p2_val));
            }
            "ptr_to_int" if arguments.len() == 1 => {
                return self.compile_expression(&arguments[0]);
            }
            "int_to_ptr" if arguments.len() == 1 => {
                return self.compile_expression(&arguments[0]);
            }
            "ptr_is_null" if arguments.len() == 1 => {
                let ptr_val = self.compile_expression(&arguments[0])?;
                let zero = self.builder.ins().iconst(VALUE_TYPE, 0);
                let cmp = self.builder.ins().icmp(IntCC::Equal, ptr_val, zero);
                return Ok(self.builder.ins().uextend(VALUE_TYPE, cmp));
            }
            "mem_copy" if arguments.len() == 3 => {
                let dest_val = self.compile_expression(&arguments[0])?;
                let src_val = self.compile_expression(&arguments[1])?;
                let count_val = self.compile_expression(&arguments[2])?;
                let func_ref = self.module.declare_func_in_func(self.raw_memory.memcpy_id, self.builder.func);
                self.builder.ins().call(func_ref, &[dest_val, src_val, count_val]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "mem_set" if arguments.len() == 3 => {
                let dest_val = self.compile_expression(&arguments[0])?;
                let val = self.compile_expression(&arguments[1])?;
                let count_val = self.compile_expression(&arguments[2])?;
                let func_ref = self.module.declare_func_in_func(self.raw_memory.memset_id, self.builder.func);
                self.builder.ins().call(func_ref, &[dest_val, val, count_val]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "vga_clear" if arguments.is_empty() => {
                let func_ref = self.module.declare_func_in_func(self.vga.clear_id, self.builder.func);
                self.builder.ins().call(func_ref, &[]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "vga_print" if arguments.len() == 1 => {
                let s_val = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.vga.print_id, self.builder.func);
                self.builder.ins().call(func_ref, &[s_val]);
                if !expression_is_borrowed(&arguments[0]) {
                    self.emit_string_release(s_val)?;
                }
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "vga_write_char" if arguments.len() == 4 => {
                let row = self.compile_expression(&arguments[0])?;
                let col = self.compile_expression(&arguments[1])?;
                let ch = self.compile_expression(&arguments[2])?;
                let color = self.compile_expression(&arguments[3])?;
                let func_ref = self.module.declare_func_in_func(self.vga.write_char_id, self.builder.func);
                self.builder.ins().call(func_ref, &[row, col, ch, color]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "vga_write_byte" if arguments.len() == 2 => {
                let offset = self.compile_expression(&arguments[0])?;
                let byte_val = self.compile_expression(&arguments[1])?;
                let func_ref = self.module.declare_func_in_func(self.vga.write_byte_id, self.builder.func);
                self.builder.ins().call(func_ref, &[offset, byte_val]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "vga_set_cursor" if arguments.len() == 2 => {
                let row = self.compile_expression(&arguments[0])?;
                let col = self.compile_expression(&arguments[1])?;
                let func_ref = self.module.declare_func_in_func(self.vga.set_cursor_id, self.builder.func);
                self.builder.ins().call(func_ref, &[row, col]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "vga_set_color" if arguments.len() == 2 => {
                let fg = self.compile_expression(&arguments[0])?;
                let bg = self.compile_expression(&arguments[1])?;
                let func_ref = self.module.declare_func_in_func(self.vga.set_color_id, self.builder.func);
                self.builder.ins().call(func_ref, &[fg, bg]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }

            // Freestanding Kernel & Multiboot2 (ADV-02)
            "multiboot2_get_magic" if arguments.is_empty() => {
                let func_ref = self.module.declare_func_in_func(self.kernel.multiboot2_get_magic_id, self.builder.func);
                let call_inst = self.builder.ins().call(func_ref, &[]);
                let results = self.builder.inst_results(call_inst);
                return Ok(results[0]);
            }
            "multiboot2_get_info_ptr" if arguments.is_empty() => {
                let func_ref = self.module.declare_func_in_func(self.kernel.multiboot2_get_info_ptr_id, self.builder.func);
                let call_inst = self.builder.ins().call(func_ref, &[]);
                let results = self.builder.inst_results(call_inst);
                return Ok(results[0]);
            }
            "outb" if arguments.len() == 2 => {
                let port = self.compile_expression(&arguments[0])?;
                let val = self.compile_expression(&arguments[1])?;
                let func_ref = self.module.declare_func_in_func(self.kernel.outb_id, self.builder.func);
                self.builder.ins().call(func_ref, &[port, val]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "inb" if arguments.len() == 1 => {
                let port = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.kernel.inb_id, self.builder.func);
                let call_inst = self.builder.ins().call(func_ref, &[port]);
                let results = self.builder.inst_results(call_inst);
                return Ok(results[0]);
            }
            "qemu_exit" if arguments.len() == 1 => {
                let code = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.kernel.qemu_exit_id, self.builder.func);
                self.builder.ins().call(func_ref, &[code]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "serial_print" if arguments.len() == 1 => {
                let arg = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.kernel.serial_print_id, self.builder.func);
                self.builder.ins().call(func_ref, &[arg]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            "serial_putchar" if arguments.len() == 1 => {
                let arg = self.compile_expression(&arguments[0])?;
                let func_ref = self.module.declare_func_in_func(self.kernel.serial_putchar_id, self.builder.func);
                self.builder.ins().call(func_ref, &[arg]);
                return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
            }
            _ => {}
        }

        if super::jvm::is_intrinsic(name) && !self.extern_functions.contains(name) {
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

        let is_extern = self.extern_functions.contains(name);
        let mut arg_values = Vec::with_capacity(arguments.len());
        let mut temporaries_to_release = Vec::new();
        let param_types = self.function_param_types.get(name).cloned().unwrap_or_default();
        for (i, arg) in arguments.iter().enumerate() {
            let val = self.compile_expression(arg)?;
            if is_extern {
                if !expression_is_borrowed(arg) {
                    temporaries_to_release.push((arg.type_.clone(), val));
                }
            } else {
                // The callee will release this reference at its own scope
                // exit (parameters are tracked exactly like locals -- see
                // `compile_all_functions`), so it needs to receive a
                // reference it's safe to consume that way.
                self.retain_if_aliasing(arg, val)?;
            }
            let val = match param_types.get(i) {
                Some(param_type) => self.coerce_for_target_type(val, &arg.type_, param_type),
                None => val,
            };
            arg_values.push(val);
        }

        let call = self.builder.ins().call(func_ref, &arg_values);
        for (ty, val) in temporaries_to_release {
            self.emit_release_for_type(&ty, val)?;
        }
        let results = self.builder.inst_results(call);
        if results.is_empty() {
            // A void-returning function used where a value is
            // syntactically expected (only reachable as a bare
            // Expression-statement in practice, since the type checker
            // wouldn't allow using a void value otherwise) -- return an
            // unused placeholder so callers don't need special-casing.
            Ok(self.builder.ins().iconst(VALUE_TYPE, 0))
        } else {
            let raw_res = results[0];
            if is_extern && *return_type == Type::String {
                let str_from_c_ref = self.module.declare_func_in_func(self.string_from_c_str_id, self.builder.func);
                let call = self.builder.ins().call(str_from_c_ref, &[raw_res]);
                Ok(self.builder.inst_results(call)[0])
            } else {
                Ok(raw_res)
            }
        }
    }

    fn compile_println(&mut self, arguments: &[TypedExpression]) -> Result<Value> {
        if arguments.len() != 1 {
            return Err(anyhow!("the native backend's println only supports exactly one argument for now"));
        }
        let arg = &arguments[0];

        if self.is_freestanding {
            match &arg.type_ {
                Type::String => {
                    let val = self.compile_expression(arg)?;
                    let func_ref = self.module.declare_func_in_func(self.vga.print_str_id, self.builder.func);
                    self.builder.ins().call(func_ref, &[val]);
                    if !expression_is_borrowed(arg) {
                        self.emit_string_release(val)?;
                    }
                    return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
                }
                Type::Bool => {
                    let val = self.compile_expression(arg)?;
                    let val_i64 = self.builder.ins().uextend(VALUE_TYPE, val);
                    let func_ref = self.module.declare_func_in_func(self.vga.print_bool_id, self.builder.func);
                    self.builder.ins().call(func_ref, &[val_i64]);
                    return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
                }
                Type::Int | Type::U8 | Type::RawPointer(_) | Type::Unknown => {
                    let val = self.compile_expression(arg)?;
                    let func_ref = self.module.declare_func_in_func(self.vga.print_int_id, self.builder.func);
                    self.builder.ins().call(func_ref, &[val]);
                    return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
                }
                Type::Float => {
                    let val = self.compile_expression(arg)?;
                    let int_val = self.builder.ins().fcvt_to_sint(VALUE_TYPE, val);
                    let func_ref = self.module.declare_func_in_func(self.vga.print_int_id, self.builder.func);
                    self.builder.ins().call(func_ref, &[int_val]);
                    return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
                }
                Type::Function { .. } => {
                    let _val = self.compile_expression(arg)?;
                    let fn_str = self.compile_string_literal("<function>")?;
                    let func_ref = self.module.declare_func_in_func(self.vga.print_str_id, self.builder.func);
                    self.builder.ins().call(func_ref, &[fn_str]);
                    self.emit_string_release(fn_str)?;
                    return Ok(self.builder.ins().iconst(VALUE_TYPE, 0));
                }
                other => return Err(anyhow!("println of a {} isn't supported on freestanding yet", other)),
            }
        }

        match &arg.type_ {
            Type::String => {
                let val = self.compile_expression(arg)?;
                let result = self.emit_printf(self.fmt_str, val)?;
                if !expression_is_borrowed(arg) {
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
            Type::Int | Type::U8 | Type::RawPointer(_) | Type::Unknown => {
                let val = self.compile_expression(arg)?;
                self.emit_printf(self.fmt_int, val)
            }
            Type::Float => {
                let val = self.compile_expression(arg)?;
                self.emit_printf_float(val)
            }
            Type::Function { .. } => {
                let _val = self.compile_expression(arg)?;
                let fn_str = self.compile_string_literal("<function>")?;
                let result = self.emit_printf(self.fmt_str, fn_str)?;
                self.emit_string_release(fn_str)?;
                Ok(result)
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

    /// Prints a float value. Renders it into a small stack buffer via
    /// gcvt first (see `gcvt_id`'s declaration for why this, and not a
    /// direct variadic printf call, is used), then prints that buffer
    /// as an ordinary string through the existing int/pointer-shaped
    /// printf path.
    fn emit_printf_float(&mut self, arg: Value) -> Result<Value> {
        let buffer_size: i64 = 64; // ample for gcvt's rendering at 15 significant digits
        let slot = self.builder.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, buffer_size as u32));
        let buffer_ptr = self.builder.ins().stack_addr(VALUE_TYPE, slot, 0);

        let ndigit = self.builder.ins().iconst(types::I32, 15);
        let gcvt_ref = self.module.declare_func_in_func(self.gcvt_id, self.builder.func);
        let call = self.builder.ins().call(gcvt_ref, &[arg, ndigit, buffer_ptr]);
        let _ = self.builder.inst_results(call);

        self.emit_printf(self.fmt_str, buffer_ptr)
    }
}
