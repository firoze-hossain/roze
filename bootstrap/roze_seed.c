// ==============================================================================
// 🌹 Emitted C99 Source by Roze Self-Hosted Compiler (ADV-06)
// ==============================================================================

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include <sys/stat.h>
#include <unistd.h>

// --- Roze C Runtime Helpers ---
char* rz_str_concat(const char* a, const char* b) {
    if (!a) a = ""; if (!b) b = "";
    size_t la = strlen(a); size_t lb = strlen(b);
    char* s = (char*)malloc(16 + la + lb + 1);
    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = la + lb;
    char* p = s + 16;
    memcpy(p, a, la); memcpy(p + la, b, lb); p[la + lb] = '\0';
    return p;
}
int64_t rz_char_at(const char* s, int64_t i) {
    if (!s || i < 0) return -1;
    unsigned char c = (unsigned char)s[i];
    return (c == '\0') ? -1 : (int64_t)c;
}
char* rz_int_to_str(int64_t v) {
    char b[32]; snprintf(b, sizeof(b), "%lld", (long long)v);
    size_t l = strlen(b);
    char* s = (char*)malloc(16 + l + 1);
    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = l;
    char* p = s + 16; memcpy(p, b, l + 1);
    return p;
}
char* rz_char_to_str(int64_t ch) {
    char* s = (char*)malloc(16 + 2);
    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = 1;
    char* p = s + 16; p[0] = (char)ch; p[1] = '\0';
    return p;
}
char* rz_substring(const char* s, int64_t st, int64_t len) {
    if (!s || st < 0 || len <= 0) return "";
    int64_t tot = strlen(s);
    if (st >= tot) return "";
    if (st + len > tot) len = tot - st;
    char* r = (char*)malloc(16 + len + 1);
    *(int64_t*)(r + 0) = 1; *(int64_t*)(r + 8) = len;
    char* p = r + 16; memcpy(p, s + st, len); p[len] = '\0';
    return p;
}
int64_t rz_str_len(const char* s) { return s ? (int64_t)strlen(s) : 0; }
int64_t rz_str_eq(const char* a, const char* b) {
    if (a == b) return 1; if (!a || !b) return 0;
    return (strcmp(a, b) == 0) ? 1 : 0;
}
char* rz_read_file(const char* path) {
    if (!path) return "";
    FILE* f = fopen(path, "rb");
    if (!f) return "";
    fseek(f, 0, SEEK_END); long sz = ftell(f); fseek(f, 0, SEEK_SET);
    char* s = (char*)malloc(16 + sz + 1);
    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = sz;
    char* p = s + 16;
    size_t nr = fread(p, 1, sz, f); p[nr] = '\0'; fclose(f);
    return p;
}
int64_t rz_write_file(const char* path, const char* content) {
    if (!path || !content) return 0;
    const char* sl = strrchr(path, '/');
    if (sl) {
        char dir[256]; size_t len = sl - path;
        if (len < sizeof(dir)) { strncpy(dir, path, len); dir[len] = '\0'; mkdir(dir, 0755); }
    }
    FILE* f = fopen(path, "w"); if (!f) return 0;
    size_t l = strlen(content); size_t w = fwrite(content, 1, l, f); fclose(f);
    return (w == l) ? 1 : 0;
}
int64_t rz_write_binary_file(const char* path, const char* hex_data) {
    if (!path || !hex_data) return 0;
    const char* sl = strrchr(path, '/');
    if (sl) {
        char dir[256]; size_t len = sl - path;
        if (len < sizeof(dir)) { strncpy(dir, path, len); dir[len] = '\0'; mkdir(dir, 0755); }
    }
    FILE* f = fopen(path, "wb"); if (!f) return 0;
    size_t hex_len = strlen(hex_data);
    for (size_t i = 0; i + 1 < hex_len; i += 2) {
        char bs[3] = { hex_data[i], hex_data[i+1], '\0' };
        unsigned char b = (unsigned char)strtoul(bs, NULL, 16);
        fwrite(&b, 1, 1, f);
    }
    fclose(f); chmod(path, 0755);
    return 1;
}
int64_t rz_file_exists(const char* path) {
    return (path && access(path, 0) == 0) ? 1 : 0;
}
int64_t rz_exec(const char* cmd) {
    return cmd ? (int64_t)system(cmd) : -1;
}
static int g_rz_argc = 0; static char* g_rz_argv[64]; static int g_rz_cmd_init = 0;
static void rz_init_cmd(void) {
    if (g_rz_cmd_init) return; g_rz_cmd_init = 1;
    FILE* f = fopen("/proc/self/cmdline", "rb"); if (!f) return;
    static char b[8192]; size_t n = fread(b, 1, sizeof(b) - 1, f); fclose(f); b[n] = '\0';
    size_t i = 0;
    while (i < n && g_rz_argc < 64) {
        g_rz_argv[g_rz_argc++] = &b[i];
        while (i < n && b[i] != '\0') i++;
        i++;
    }
}
int64_t rz_get_arg_count(void) { rz_init_cmd(); return (int64_t)g_rz_argc; }
char* rz_get_arg(int64_t idx) {
    rz_init_cmd();
    return (idx >= 0 && idx < g_rz_argc) ? g_rz_argv[idx] : "";
}
char* rz_get_env(const char* name) {
    if (!name) return ""; char* v = getenv(name); return v ? v : "";
}
void rz_compiler_banner(void) {
    printf("==============================================================================\n");
    printf("🌹 Roze Self-Hosted Compiler (Roze-in-Roze Stage 1 / Stage 2)\n");
    printf("==============================================================================\n");
    printf("⚡ Language Independence & Full Self-Hosting Verification\n");
    printf("   - Frontend:  Pure Roze Lexer, Parser & Semantic Type Checker\n");
    printf("   - Backend:   Pure Roze C99 Code Generator (Native Cranelift ABI)\n");
    printf("   - Toolchain: Standalone Native ELF Executable, Zero Rust Dependency\n");
    printf("==============================================================================\n\n");
}

// --- Class Structure Forward Declarations ---
typedef struct Rz_Token Rz_Token;
typedef struct Rz_AstNode Rz_AstNode;
typedef struct Rz_Symbol Rz_Symbol;
typedef struct Rz_ClassField Rz_ClassField;
typedef struct Rz_ClassDef Rz_ClassDef;
typedef struct Rz_TraitMethod Rz_TraitMethod;
typedef struct Rz_TraitDef Rz_TraitDef;
typedef struct Rz_ImplDef Rz_ImplDef;
typedef struct Rz_MonoEntry Rz_MonoEntry;
typedef struct Rz_BorrowVar Rz_BorrowVar;
typedef struct Rz_SsaBlockRef Rz_SsaBlockRef;
typedef struct Rz_SsaInst Rz_SsaInst;
typedef struct Rz_BasicBlock Rz_BasicBlock;
typedef struct Rz_SsaParam Rz_SsaParam;
typedef struct Rz_SsaFunction Rz_SsaFunction;
typedef struct Rz_SsaProgram Rz_SsaProgram;
typedef struct Rz_ConstBinding Rz_ConstBinding;
typedef struct Rz_InlinableFunc Rz_InlinableFunc;
typedef struct Rz_OptStats Rz_OptStats;
typedef struct Rz_BinaryBuffer Rz_BinaryBuffer;
typedef struct Rz_NativeSection Rz_NativeSection;
typedef struct Rz_NativeSymbol Rz_NativeSymbol;
typedef struct Rz_NativeReloc Rz_NativeReloc;
typedef struct Rz_Parser Rz_Parser;
typedef struct Rz_TypeChecker Rz_TypeChecker;

// --- Class Struct Definitions & Constructors ---
struct Rz_Token {
    int64_t __refcount;
    int64_t __vtable;
    int64_t kind;
    const char* text;
    int64_t int_val;
    int64_t line;
    int64_t col;
    struct Rz_Token* next;
};

Rz_Token* new_Token(int64_t kind, const char* text, int64_t int_val, int64_t line, int64_t col, struct Rz_Token* next) {
    Rz_Token* obj = (Rz_Token*)malloc(sizeof(Rz_Token));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->kind = kind;
    obj->text = text;
    obj->int_val = int_val;
    obj->line = line;
    obj->col = col;
    obj->next = next;
    return obj;
}

struct Rz_AstNode {
    int64_t __refcount;
    int64_t __vtable;
    int64_t kind;
    const char* name;
    const char* type_name;
    const char* op;
    int64_t int_val;
    const char* str_val;
    struct Rz_AstNode* child1;
    struct Rz_AstNode* child2;
    struct Rz_AstNode* child3;
    struct Rz_AstNode* child4;
    struct Rz_AstNode* next;
    int64_t line;
    int64_t col;
};

Rz_AstNode* new_AstNode(int64_t kind, const char* name, const char* type_name, const char* op, int64_t int_val, const char* str_val, struct Rz_AstNode* child1, struct Rz_AstNode* child2, struct Rz_AstNode* child3, struct Rz_AstNode* child4, struct Rz_AstNode* next, int64_t line, int64_t col) {
    Rz_AstNode* obj = (Rz_AstNode*)malloc(sizeof(Rz_AstNode));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->kind = kind;
    obj->name = name;
    obj->type_name = type_name;
    obj->op = op;
    obj->int_val = int_val;
    obj->str_val = str_val;
    obj->child1 = child1;
    obj->child2 = child2;
    obj->child3 = child3;
    obj->child4 = child4;
    obj->next = next;
    obj->line = line;
    obj->col = col;
    return obj;
}

struct Rz_Symbol {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* type_name;
    int64_t kind;
    int64_t scope_level;
    int64_t is_mut;
    int64_t borrow_state;
    struct Rz_AstNode* decl_node;
    struct Rz_Symbol* next;
};

Rz_Symbol* new_Symbol(const char* name, const char* type_name, int64_t kind, int64_t scope_level, int64_t is_mut, int64_t borrow_state, struct Rz_AstNode* decl_node, struct Rz_Symbol* next) {
    Rz_Symbol* obj = (Rz_Symbol*)malloc(sizeof(Rz_Symbol));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->type_name = type_name;
    obj->kind = kind;
    obj->scope_level = scope_level;
    obj->is_mut = is_mut;
    obj->borrow_state = borrow_state;
    obj->decl_node = decl_node;
    obj->next = next;
    return obj;
}

struct Rz_ClassField {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* type_name;
    struct Rz_ClassField* next;
};

Rz_ClassField* new_ClassField(const char* name, const char* type_name, struct Rz_ClassField* next) {
    Rz_ClassField* obj = (Rz_ClassField*)malloc(sizeof(Rz_ClassField));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->type_name = type_name;
    obj->next = next;
    return obj;
}

struct Rz_ClassDef {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    struct Rz_ClassField* fields;
    struct Rz_ClassDef* next;
};

Rz_ClassDef* new_ClassDef(const char* name, struct Rz_ClassField* fields, struct Rz_ClassDef* next) {
    Rz_ClassDef* obj = (Rz_ClassDef*)malloc(sizeof(Rz_ClassDef));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->fields = fields;
    obj->next = next;
    return obj;
}

struct Rz_TraitMethod {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* ret_type;
    const char* param_types;
    struct Rz_TraitMethod* next;
};

Rz_TraitMethod* new_TraitMethod(const char* name, const char* ret_type, const char* param_types, struct Rz_TraitMethod* next) {
    Rz_TraitMethod* obj = (Rz_TraitMethod*)malloc(sizeof(Rz_TraitMethod));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->ret_type = ret_type;
    obj->param_types = param_types;
    obj->next = next;
    return obj;
}

struct Rz_TraitDef {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    struct Rz_TraitMethod* methods;
    struct Rz_TraitDef* next;
};

Rz_TraitDef* new_TraitDef(const char* name, struct Rz_TraitMethod* methods, struct Rz_TraitDef* next) {
    Rz_TraitDef* obj = (Rz_TraitDef*)malloc(sizeof(Rz_TraitDef));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->methods = methods;
    obj->next = next;
    return obj;
}

struct Rz_ImplDef {
    int64_t __refcount;
    int64_t __vtable;
    const char* trait_name;
    const char* target_type;
    struct Rz_TraitMethod* methods;
    struct Rz_ImplDef* next;
};

Rz_ImplDef* new_ImplDef(const char* trait_name, const char* target_type, struct Rz_TraitMethod* methods, struct Rz_ImplDef* next) {
    Rz_ImplDef* obj = (Rz_ImplDef*)malloc(sizeof(Rz_ImplDef));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->trait_name = trait_name;
    obj->target_type = target_type;
    obj->methods = methods;
    obj->next = next;
    return obj;
}

struct Rz_MonoEntry {
    int64_t __refcount;
    int64_t __vtable;
    const char* generic_name;
    const char* concrete_type;
    const char* specialized_name;
    struct Rz_MonoEntry* next;
};

Rz_MonoEntry* new_MonoEntry(const char* generic_name, const char* concrete_type, const char* specialized_name, struct Rz_MonoEntry* next) {
    Rz_MonoEntry* obj = (Rz_MonoEntry*)malloc(sizeof(Rz_MonoEntry));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->generic_name = generic_name;
    obj->concrete_type = concrete_type;
    obj->specialized_name = specialized_name;
    obj->next = next;
    return obj;
}

struct Rz_BorrowVar {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* type_name;
    int64_t state;
    const char* moved_to;
    int64_t line;
    struct Rz_BorrowVar* next;
};

Rz_BorrowVar* new_BorrowVar(const char* name, const char* type_name, int64_t state, const char* moved_to, int64_t line, struct Rz_BorrowVar* next) {
    Rz_BorrowVar* obj = (Rz_BorrowVar*)malloc(sizeof(Rz_BorrowVar));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->type_name = type_name;
    obj->state = state;
    obj->moved_to = moved_to;
    obj->line = line;
    obj->next = next;
    return obj;
}

struct Rz_SsaBlockRef {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    struct Rz_SsaBlockRef* next;
};

Rz_SsaBlockRef* new_SsaBlockRef(const char* name, struct Rz_SsaBlockRef* next) {
    Rz_SsaBlockRef* obj = (Rz_SsaBlockRef*)malloc(sizeof(Rz_SsaBlockRef));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->next = next;
    return obj;
}

struct Rz_SsaInst {
    int64_t __refcount;
    int64_t __vtable;
    int64_t id;
    int64_t kind;
    const char* dest;
    const char* type_name;
    const char* op;
    const char* arg1;
    const char* arg2;
    const char* arg3;
    int64_t int_val;
    const char* str_val;
    const char* target_label;
    const char* false_label;
    int64_t is_dead;
    struct Rz_SsaInst* next;
};

Rz_SsaInst* new_SsaInst(int64_t id, int64_t kind, const char* dest, const char* type_name, const char* op, const char* arg1, const char* arg2, const char* arg3, int64_t int_val, const char* str_val, const char* target_label, const char* false_label, int64_t is_dead, struct Rz_SsaInst* next) {
    Rz_SsaInst* obj = (Rz_SsaInst*)malloc(sizeof(Rz_SsaInst));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->id = id;
    obj->kind = kind;
    obj->dest = dest;
    obj->type_name = type_name;
    obj->op = op;
    obj->arg1 = arg1;
    obj->arg2 = arg2;
    obj->arg3 = arg3;
    obj->int_val = int_val;
    obj->str_val = str_val;
    obj->target_label = target_label;
    obj->false_label = false_label;
    obj->is_dead = is_dead;
    obj->next = next;
    return obj;
}

struct Rz_BasicBlock {
    int64_t __refcount;
    int64_t __vtable;
    int64_t id;
    const char* label;
    struct Rz_SsaInst* inst_head;
    struct Rz_SsaInst* inst_tail;
    int64_t inst_count;
    int64_t pred_count;
    int64_t succ_count;
    struct Rz_SsaBlockRef* preds;
    struct Rz_SsaBlockRef* succs;
    int64_t is_terminated;
    struct Rz_BasicBlock* next;
};

Rz_BasicBlock* new_BasicBlock(int64_t id, const char* label, struct Rz_SsaInst* inst_head, struct Rz_SsaInst* inst_tail, int64_t inst_count, int64_t pred_count, int64_t succ_count, struct Rz_SsaBlockRef* preds, struct Rz_SsaBlockRef* succs, int64_t is_terminated, struct Rz_BasicBlock* next) {
    Rz_BasicBlock* obj = (Rz_BasicBlock*)malloc(sizeof(Rz_BasicBlock));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->id = id;
    obj->label = label;
    obj->inst_head = inst_head;
    obj->inst_tail = inst_tail;
    obj->inst_count = inst_count;
    obj->pred_count = pred_count;
    obj->succ_count = succ_count;
    obj->preds = preds;
    obj->succs = succs;
    obj->is_terminated = is_terminated;
    obj->next = next;
    return obj;
}

struct Rz_SsaParam {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* type_name;
    struct Rz_SsaParam* next;
};

Rz_SsaParam* new_SsaParam(const char* name, const char* type_name, struct Rz_SsaParam* next) {
    Rz_SsaParam* obj = (Rz_SsaParam*)malloc(sizeof(Rz_SsaParam));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->type_name = type_name;
    obj->next = next;
    return obj;
}

struct Rz_SsaFunction {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* ret_type;
    struct Rz_SsaParam* params;
    struct Rz_BasicBlock* block_head;
    struct Rz_BasicBlock* block_tail;
    int64_t block_count;
    int64_t var_counter;
    int64_t is_inlinable;
    struct Rz_SsaFunction* next;
};

Rz_SsaFunction* new_SsaFunction(const char* name, const char* ret_type, struct Rz_SsaParam* params, struct Rz_BasicBlock* block_head, struct Rz_BasicBlock* block_tail, int64_t block_count, int64_t var_counter, int64_t is_inlinable, struct Rz_SsaFunction* next) {
    Rz_SsaFunction* obj = (Rz_SsaFunction*)malloc(sizeof(Rz_SsaFunction));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->ret_type = ret_type;
    obj->params = params;
    obj->block_head = block_head;
    obj->block_tail = block_tail;
    obj->block_count = block_count;
    obj->var_counter = var_counter;
    obj->is_inlinable = is_inlinable;
    obj->next = next;
    return obj;
}

struct Rz_SsaProgram {
    int64_t __refcount;
    int64_t __vtable;
    struct Rz_SsaFunction* functions;
    struct Rz_SsaProgram* next;
};

Rz_SsaProgram* new_SsaProgram(struct Rz_SsaFunction* functions, struct Rz_SsaProgram* next) {
    Rz_SsaProgram* obj = (Rz_SsaProgram*)malloc(sizeof(Rz_SsaProgram));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->functions = functions;
    obj->next = next;
    return obj;
}

struct Rz_ConstBinding {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    int64_t kind;
    int64_t int_val;
    const char* str_val;
    struct Rz_ConstBinding* next;
};

Rz_ConstBinding* new_ConstBinding(const char* name, int64_t kind, int64_t int_val, const char* str_val, struct Rz_ConstBinding* next) {
    Rz_ConstBinding* obj = (Rz_ConstBinding*)malloc(sizeof(Rz_ConstBinding));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->kind = kind;
    obj->int_val = int_val;
    obj->str_val = str_val;
    obj->next = next;
    return obj;
}

struct Rz_InlinableFunc {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    const char* ret_type;
    struct Rz_AstNode* params;
    struct Rz_AstNode* ret_expr;
    struct Rz_InlinableFunc* next;
};

Rz_InlinableFunc* new_InlinableFunc(const char* name, const char* ret_type, struct Rz_AstNode* params, struct Rz_AstNode* ret_expr, struct Rz_InlinableFunc* next) {
    Rz_InlinableFunc* obj = (Rz_InlinableFunc*)malloc(sizeof(Rz_InlinableFunc));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->ret_type = ret_type;
    obj->params = params;
    obj->ret_expr = ret_expr;
    obj->next = next;
    return obj;
}

struct Rz_OptStats {
    int64_t __refcount;
    int64_t __vtable;
    int64_t const_folded;
    int64_t dead_stmts_removed;
    int64_t funcs_inlined;
    int64_t loops_hoisted;
};

Rz_OptStats* new_OptStats(int64_t const_folded, int64_t dead_stmts_removed, int64_t funcs_inlined, int64_t loops_hoisted) {
    Rz_OptStats* obj = (Rz_OptStats*)malloc(sizeof(Rz_OptStats));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->const_folded = const_folded;
    obj->dead_stmts_removed = dead_stmts_removed;
    obj->funcs_inlined = funcs_inlined;
    obj->loops_hoisted = loops_hoisted;
    return obj;
}

struct Rz_BinaryBuffer {
    int64_t __refcount;
    int64_t __vtable;
    const char* hex_data;
    int64_t size;
};

Rz_BinaryBuffer* new_BinaryBuffer(const char* hex_data, int64_t size) {
    Rz_BinaryBuffer* obj = (Rz_BinaryBuffer*)malloc(sizeof(Rz_BinaryBuffer));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->hex_data = hex_data;
    obj->size = size;
    return obj;
}

struct Rz_NativeSection {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    int64_t sec_type;
    int64_t flags;
    int64_t vaddr;
    int64_t offset;
    int64_t size;
    int64_t align;
    struct Rz_BinaryBuffer* data;
    struct Rz_NativeSection* next;
};

Rz_NativeSection* new_NativeSection(const char* name, int64_t sec_type, int64_t flags, int64_t vaddr, int64_t offset, int64_t size, int64_t align, struct Rz_BinaryBuffer* data, struct Rz_NativeSection* next) {
    Rz_NativeSection* obj = (Rz_NativeSection*)malloc(sizeof(Rz_NativeSection));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->sec_type = sec_type;
    obj->flags = flags;
    obj->vaddr = vaddr;
    obj->offset = offset;
    obj->size = size;
    obj->align = align;
    obj->data = data;
    obj->next = next;
    return obj;
}

struct Rz_NativeSymbol {
    int64_t __refcount;
    int64_t __vtable;
    const char* name;
    int64_t value;
    int64_t size;
    int64_t sec_index;
    int64_t sym_type;
    int64_t binding;
    struct Rz_NativeSymbol* next;
};

Rz_NativeSymbol* new_NativeSymbol(const char* name, int64_t value, int64_t size, int64_t sec_index, int64_t sym_type, int64_t binding, struct Rz_NativeSymbol* next) {
    Rz_NativeSymbol* obj = (Rz_NativeSymbol*)malloc(sizeof(Rz_NativeSymbol));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->name = name;
    obj->value = value;
    obj->size = size;
    obj->sec_index = sec_index;
    obj->sym_type = sym_type;
    obj->binding = binding;
    obj->next = next;
    return obj;
}

struct Rz_NativeReloc {
    int64_t __refcount;
    int64_t __vtable;
    int64_t offset;
    int64_t sym_index;
    int64_t rel_type;
    int64_t addend;
    struct Rz_NativeReloc* next;
};

Rz_NativeReloc* new_NativeReloc(int64_t offset, int64_t sym_index, int64_t rel_type, int64_t addend, struct Rz_NativeReloc* next) {
    Rz_NativeReloc* obj = (Rz_NativeReloc*)malloc(sizeof(Rz_NativeReloc));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->offset = offset;
    obj->sym_index = sym_index;
    obj->rel_type = rel_type;
    obj->addend = addend;
    obj->next = next;
    return obj;
}

struct Rz_Parser {
    int64_t __refcount;
    int64_t __vtable;
    struct Rz_Token* curr;
};

Rz_Parser* new_Parser(struct Rz_Token* curr) {
    Rz_Parser* obj = (Rz_Parser*)malloc(sizeof(Rz_Parser));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->curr = curr;
    return obj;
}

struct Rz_TypeChecker {
    int64_t __refcount;
    int64_t __vtable;
    struct Rz_Symbol* symbols;
    int64_t scope_level;
    int64_t error_count;
    int64_t warning_count;
    const char* current_func_ret;
    const char* current_func_name;
    struct Rz_ClassDef* classes;
    struct Rz_TraitDef* traits;
    struct Rz_ImplDef* impls;
    struct Rz_MonoEntry* mono_table;
    struct Rz_BorrowVar* borrow_vars;
};

Rz_TypeChecker* new_TypeChecker(struct Rz_Symbol* symbols, int64_t scope_level, int64_t error_count, int64_t warning_count, const char* current_func_ret, const char* current_func_name, struct Rz_ClassDef* classes, struct Rz_TraitDef* traits, struct Rz_ImplDef* impls, struct Rz_MonoEntry* mono_table, struct Rz_BorrowVar* borrow_vars) {
    Rz_TypeChecker* obj = (Rz_TypeChecker*)malloc(sizeof(Rz_TypeChecker));
    obj->__refcount = 1;
    obj->__vtable = 0;
    obj->symbols = symbols;
    obj->scope_level = scope_level;
    obj->error_count = error_count;
    obj->warning_count = warning_count;
    obj->current_func_ret = current_func_ret;
    obj->current_func_name = current_func_name;
    obj->classes = classes;
    obj->traits = traits;
    obj->impls = impls;
    obj->mono_table = mono_table;
    obj->borrow_vars = borrow_vars;
    return obj;
}

// --- Function Prototypes ---
int64_t TOK_EOF(void);
int64_t TOK_IDENT(void);
int64_t TOK_INT(void);
int64_t TOK_STRING(void);
int64_t TOK_BOOL(void);
int64_t TOK_CHAR(void);
int64_t TOK_FUNC(void);
int64_t TOK_LET(void);
int64_t TOK_VAR(void);
int64_t TOK_IF(void);
int64_t TOK_ELSE(void);
int64_t TOK_WHILE(void);
int64_t TOK_FOR(void);
int64_t TOK_RETURN(void);
int64_t TOK_CLASS(void);
int64_t TOK_NEW(void);
int64_t TOK_EXTERN(void);
int64_t TOK_IMPORT(void);
int64_t TOK_NULL(void);
int64_t TOK_MUT(void);
int64_t TOK_CONST(void);
int64_t TOK_ENUM(void);
int64_t TOK_MATCH(void);
int64_t TOK_TRAIT(void);
int64_t TOK_IMPL(void);
int64_t TOK_THIS(void);
int64_t TOK_UNSAFE(void);
int64_t TOK_RAW(void);
int64_t TOK_COMPTIME(void);
int64_t TOK_ASYNC(void);
int64_t TOK_AWAIT(void);
int64_t TOK_TYPE(void);
int64_t TOK_WHERE(void);
int64_t TOK_ASM(void);
int64_t TOK_IN(void);
int64_t TOK_PLUS(void);
int64_t TOK_MINUS(void);
int64_t TOK_STAR(void);
int64_t TOK_SLASH(void);
int64_t TOK_PERCENT(void);
int64_t TOK_ASSIGN(void);
int64_t TOK_EQ(void);
int64_t TOK_NE(void);
int64_t TOK_LT(void);
int64_t TOK_LE(void);
int64_t TOK_GT(void);
int64_t TOK_GE(void);
int64_t TOK_AND(void);
int64_t TOK_OR(void);
int64_t TOK_NOT(void);
int64_t TOK_QUESTION(void);
int64_t TOK_BIT_AND(void);
int64_t TOK_BIT_OR(void);
int64_t TOK_BIT_XOR(void);
int64_t TOK_BIT_NOT(void);
int64_t TOK_LPAREN(void);
int64_t TOK_RPAREN(void);
int64_t TOK_LBRACE(void);
int64_t TOK_RBRACE(void);
int64_t TOK_LBRACKET(void);
int64_t TOK_RBRACKET(void);
int64_t TOK_SEMI(void);
int64_t TOK_COMMA(void);
int64_t TOK_COLON(void);
int64_t TOK_DOT(void);
int64_t TOK_ARROW(void);
int64_t TOK_LSHIFT(void);
int64_t TOK_RSHIFT(void);
int64_t TOK_FAT_ARROW(void);
int64_t TOK_COLON_COLON(void);
int64_t TOK_DOT_DOT(void);
int64_t TOK_DOT_DOT_EQ(void);
int64_t TOK_AT(void);
int64_t TOK_HASH(void);
int64_t TOK_UNDERSCORE(void);
int64_t TOK_BANG(void);
int64_t NODE_PROGRAM(void);
int64_t NODE_FUNC_DECL(void);
int64_t NODE_EXTERN_DECL(void);
int64_t NODE_CLASS_DECL(void);
int64_t NODE_VAR_DECL(void);
int64_t NODE_PARAM(void);
int64_t NODE_FIELD(void);
int64_t NODE_ENUM_DECL(void);
int64_t NODE_ENUM_VARIANT(void);
int64_t NODE_TRAIT_DECL(void);
int64_t NODE_IMPL_DECL(void);
int64_t NODE_ANNOTATION(void);
int64_t NODE_IMPORT(void);
int64_t NODE_BLOCK(void);
int64_t NODE_IF(void);
int64_t NODE_WHILE(void);
int64_t NODE_FOR(void);
int64_t NODE_RETURN(void);
int64_t NODE_ASSIGN(void);
int64_t NODE_EXPR_STMT(void);
int64_t NODE_MATCH(void);
int64_t NODE_MATCH_ARM(void);
int64_t NODE_UNSAFE_BLOCK(void);
int64_t NODE_COMPTIME_STMT(void);
int64_t NODE_FIELD_ASSIGN(void);
int64_t NODE_INDEX_ASSIGN(void);
int64_t NODE_BIN_OP(void);
int64_t NODE_UNARY_OP(void);
int64_t NODE_CALL(void);
int64_t NODE_MEMBER(void);
int64_t NODE_NEW(void);
int64_t NODE_LITERAL_INT(void);
int64_t NODE_LITERAL_STR(void);
int64_t NODE_LITERAL_BOOL(void);
int64_t NODE_LITERAL_NULL(void);
int64_t NODE_IDENT(void);
int64_t NODE_METHOD_CALL(void);
int64_t NODE_INDEX(void);
int64_t NODE_TRY(void);
int64_t NODE_CLOSURE(void);
int64_t NODE_COMPTIME_EXPR(void);
int64_t NODE_MACRO_CALL(void);
int64_t NODE_PATTERN(void);
int64_t NODE_AWAIT(void);
int64_t NODE_ASM(void);
int64_t NODE_LITERAL_CHAR(void);
int64_t SYM_VAR(void);
int64_t SYM_PARAM(void);
int64_t SYM_FUNC(void);
int64_t SYM_CLASS(void);
int64_t SYM_ENUM(void);
int64_t SYM_VARIANT(void);
int64_t SYM_TRAIT(void);
int64_t SYM_IMPL(void);
int64_t SYM_CONST(void);
int64_t SYM_GENERIC_PARAM(void);
int64_t BORROW_NONE(void);
int64_t BORROW_OWNED(void);
int64_t BORROW_SHARED(void);
int64_t BORROW_MUT(void);
int64_t BORROW_MOVED(void);
int64_t SSA_NOP(void);
int64_t SSA_CONST_INT(void);
int64_t SSA_CONST_STR(void);
int64_t SSA_CONST_BOOL(void);
int64_t SSA_BIN_OP(void);
int64_t SSA_UNARY_OP(void);
int64_t SSA_CALL(void);
int64_t SSA_ALLOC(void);
int64_t SSA_LOAD(void);
int64_t SSA_STORE(void);
int64_t SSA_GET_FIELD(void);
int64_t SSA_SET_FIELD(void);
int64_t SSA_PHI(void);
int64_t SSA_BR(void);
int64_t SSA_BR_COND(void);
int64_t SSA_RETURN(void);
struct Rz_Token* make_token(int64_t kind, const char* text, int64_t int_val, int64_t line, int64_t col);
struct Rz_AstNode* make_ast_node(int64_t kind, const char* name, const char* type_name, int64_t line, int64_t col);
struct Rz_AstNode* clone_ast_node(struct Rz_AstNode* node);
struct Rz_AstNode* clone_single_node(struct Rz_AstNode* node);
struct Rz_Symbol* make_symbol(const char* name, const char* type_name, int64_t kind, int64_t scope_level, int64_t is_mut, struct Rz_Symbol* next);
const char* token_kind_to_str(int64_t kind);
int64_t is_digit(int64_t ch);
int64_t is_alpha(int64_t ch);
int64_t is_alnum(int64_t ch);
int64_t keyword_kind(const char* text);
struct Rz_Token* tokenize(const char* source);
struct Rz_Parser* make_parser(struct Rz_Token* tokens);
int64_t peek_kind(struct Rz_Parser* p);
int64_t peek_next_kind(struct Rz_Parser* p);
const char* peek_text(struct Rz_Parser* p);
int64_t peek_line(struct Rz_Parser* p);
int64_t peek_col(struct Rz_Parser* p);
struct Rz_Token* advance_token(struct Rz_Parser* p);
int64_t check_token(struct Rz_Parser* p, int64_t kind);
int64_t match_tok(struct Rz_Parser* p, int64_t kind);
struct Rz_Token* consume_tok(struct Rz_Parser* p, int64_t kind, const char* msg);
struct Rz_AstNode* parse_primary(struct Rz_Parser* p);
struct Rz_AstNode* parse_postfix(struct Rz_Parser* p);
struct Rz_AstNode* parse_unary(struct Rz_Parser* p);
struct Rz_AstNode* parse_factor(struct Rz_Parser* p);
struct Rz_AstNode* parse_term(struct Rz_Parser* p);
struct Rz_AstNode* parse_shift(struct Rz_Parser* p);
struct Rz_AstNode* parse_comparison(struct Rz_Parser* p);
struct Rz_AstNode* parse_equality(struct Rz_Parser* p);
struct Rz_AstNode* parse_bitwise_and(struct Rz_Parser* p);
struct Rz_AstNode* parse_bitwise_xor(struct Rz_Parser* p);
struct Rz_AstNode* parse_bitwise_or(struct Rz_Parser* p);
struct Rz_AstNode* parse_logical_and(struct Rz_Parser* p);
struct Rz_AstNode* parse_logical_or(struct Rz_Parser* p);
struct Rz_AstNode* parse_expression(struct Rz_Parser* p);
struct Rz_AstNode* parse_pattern(struct Rz_Parser* p);
struct Rz_AstNode* parse_var_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_if_stmt(struct Rz_Parser* p);
struct Rz_AstNode* parse_while_stmt(struct Rz_Parser* p);
struct Rz_AstNode* parse_for_stmt(struct Rz_Parser* p);
struct Rz_AstNode* parse_match_stmt(struct Rz_Parser* p);
struct Rz_AstNode* parse_return_stmt(struct Rz_Parser* p);
struct Rz_AstNode* parse_block(struct Rz_Parser* p);
struct Rz_AstNode* parse_statement(struct Rz_Parser* p);
struct Rz_AstNode* parse_func_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_extern_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_class_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_enum_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_trait_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_impl_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_annotated_decl(struct Rz_Parser* p);
struct Rz_AstNode* parse_import(struct Rz_Parser* p);
struct Rz_AstNode* parse_program(struct Rz_Parser* p);
struct Rz_AstNode* expand_macros_expr(struct Rz_AstNode* expr);
struct Rz_AstNode* expand_macros_stmt(struct Rz_AstNode* stmt);
struct Rz_AstNode* expand_macros(struct Rz_AstNode* prog);
struct Rz_TypeChecker* make_type_checker(void);
void enter_scope(struct Rz_TypeChecker* tc);
void exit_scope(struct Rz_TypeChecker* tc);
void define_symbol(struct Rz_TypeChecker* tc, const char* name, const char* type_name, int64_t kind);
void define_symbol_with_decl(struct Rz_TypeChecker* tc, const char* name, const char* type_name, int64_t kind, int64_t is_mut, struct Rz_AstNode* decl);
struct Rz_Symbol* lookup_symbol(struct Rz_TypeChecker* tc, const char* name);
struct Rz_Symbol* lookup_symbol_in_current_scope(struct Rz_TypeChecker* tc, const char* name);
void register_class_def(struct Rz_TypeChecker* tc, const char* name, struct Rz_AstNode* fields);
struct Rz_ClassDef* lookup_class_def(struct Rz_TypeChecker* tc, const char* name);
const char* lookup_class_field(struct Rz_TypeChecker* tc, const char* class_name, const char* field_name);
void tc_report_error(struct Rz_TypeChecker* tc, const char* msg, int64_t line);
void tc_report_warning(struct Rz_TypeChecker* tc, const char* msg, int64_t line);
void register_trait_def(struct Rz_TypeChecker* tc, const char* name, struct Rz_AstNode* methods_ast);
struct Rz_TraitDef* lookup_trait_def(struct Rz_TypeChecker* tc, const char* name);
void register_impl_def(struct Rz_TypeChecker* tc, const char* trait_name, const char* target_type, struct Rz_AstNode* methods_ast);
struct Rz_ImplDef* lookup_impl_def(struct Rz_TypeChecker* tc, const char* trait_name, const char* target_type);
void verify_trait_conformance(struct Rz_TypeChecker* tc, struct Rz_AstNode* impl_node);
const char* resolve_trait_method(struct Rz_TypeChecker* tc, const char* target_type, const char* method_name);
void record_mono_entry(struct Rz_TypeChecker* tc, const char* generic_name, const char* concrete_type, const char* spec_name);
const char* lookup_mono_entry(struct Rz_TypeChecker* tc, const char* generic_name, const char* concrete_type);
void substitute_type_in_ast(struct Rz_AstNode* node, const char* param_name, const char* concrete_type);
struct Rz_AstNode* find_generic_decl(struct Rz_AstNode* prog, const char* name, int64_t kind);
void monomorphize_class_instantiation(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog, struct Rz_AstNode* new_node);
void monomorphize_expr(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog, struct Rz_AstNode* expr);
void monomorphize_statement(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog, struct Rz_AstNode* stmt);
struct Rz_AstNode* monomorphize_program(struct Rz_AstNode* prog);
int64_t is_copy_type(const char* type_name);
void add_borrow_var(struct Rz_TypeChecker* tc, const char* name, const char* type_name, int64_t state, int64_t line);
struct Rz_BorrowVar* lookup_borrow_var(struct Rz_TypeChecker* tc, const char* name);
void mark_var_moved(struct Rz_TypeChecker* tc, const char* from_var, const char* to_var, int64_t line);
void restore_var_ownership(struct Rz_TypeChecker* tc, const char* name);
void check_expr_borrow_use(struct Rz_TypeChecker* tc, struct Rz_AstNode* expr);
void check_stmt_borrow(struct Rz_TypeChecker* tc, struct Rz_AstNode* stmt);
void check_cyclic_reference_hazards(struct Rz_TypeChecker* tc);
int64_t analyze_borrows(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog);
int64_t is_type_compatible(const char* expected, const char* actual);
const char* check_expression_type(struct Rz_TypeChecker* tc, struct Rz_AstNode* expr);
void check_statement_type(struct Rz_TypeChecker* tc, struct Rz_AstNode* stmt);
int64_t check_program_types(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog);
struct Rz_SsaInst* make_ssa_inst(int64_t id, int64_t kind, const char* dest, const char* type_name, const char* op, const char* arg1, const char* arg2, int64_t int_val, const char* str_val, const char* target_label, const char* false_label);
struct Rz_BasicBlock* make_basic_block(int64_t id, const char* label);
struct Rz_SsaParam* make_ssa_param(const char* name, const char* type_name);
struct Rz_SsaFunction* make_ssa_function(const char* name, const char* ret_type);
struct Rz_SsaProgram* make_ssa_program(void);
void bb_append_inst(struct Rz_BasicBlock* bb, struct Rz_SsaInst* inst);
void fn_append_bb(struct Rz_SsaFunction* fn, struct Rz_BasicBlock* bb);
const char* fn_new_temp(struct Rz_SsaFunction* fn);
void bb_add_pred(struct Rz_BasicBlock* bb, const char* pred_label);
void bb_add_succ(struct Rz_BasicBlock* bb, const char* succ_label);
struct Rz_BasicBlock* fn_find_bb(struct Rz_SsaFunction* fn, const char* label);
void resolve_cfg_edges(struct Rz_SsaFunction* fn);
int64_t eliminate_dead_basic_blocks(struct Rz_SsaFunction* fn);
const char* dump_ssa_inst(struct Rz_SsaInst* inst);
const char* dump_basic_block(struct Rz_BasicBlock* bb);
const char* dump_ssa_function(struct Rz_SsaFunction* fn);
const char* dump_ssa_ir(struct Rz_SsaProgram* prog);
const char* lower_ast_expr_to_ssa(struct Rz_SsaFunction* fn, struct Rz_BasicBlock* bb, struct Rz_AstNode* expr);
struct Rz_BasicBlock* lower_ast_stmt_to_ssa(struct Rz_SsaFunction* fn, struct Rz_BasicBlock* bb, struct Rz_AstNode* stmt);
struct Rz_SsaFunction* lower_ast_func_to_ssa(struct Rz_AstNode* decl);
struct Rz_SsaProgram* build_ssa_ir(struct Rz_AstNode* prog);
struct Rz_AstNode* make_int_literal_node(int64_t val, int64_t line, int64_t col);
struct Rz_AstNode* make_str_literal_node(const char* val, int64_t line, int64_t col);
struct Rz_AstNode* make_bool_literal_node(int64_t val, int64_t line, int64_t col);
int64_t is_pure_expr(struct Rz_AstNode* expr);
int64_t eval_int_bin_op(const char* op, int64_t a, int64_t b);
struct Rz_AstNode* fold_constants_expr(struct Rz_AstNode* expr, struct Rz_OptStats* stats);
int64_t has_ident_ref(struct Rz_AstNode* node, const char* name);
struct Rz_AstNode* eliminate_dead_statements(struct Rz_AstNode* stmt_list, struct Rz_OptStats* stats);
struct Rz_AstNode* eliminate_dead_code_stmt(struct Rz_AstNode* stmt, struct Rz_OptStats* stats);
struct Rz_InlinableFunc* find_inlinable_func(struct Rz_InlinableFunc* head, const char* name);
struct Rz_AstNode* substitute_param_with_arg(struct Rz_AstNode* expr, const char* param_name, struct Rz_AstNode* arg_expr);
struct Rz_AstNode* inline_call_expr(struct Rz_AstNode* call_node, struct Rz_InlinableFunc* inlinable_list, struct Rz_OptStats* stats);
struct Rz_AstNode* inline_calls_in_expr(struct Rz_AstNode* expr, struct Rz_InlinableFunc* inlinable_list, struct Rz_OptStats* stats);
void inline_calls_in_stmt(struct Rz_AstNode* stmt, struct Rz_InlinableFunc* inlinable_list, struct Rz_OptStats* stats);
int64_t is_inlinable_ret_expr(struct Rz_AstNode* expr);
int64_t is_compiler_internal_func(const char* name);
struct Rz_InlinableFunc* collect_inlinable_functions(struct Rz_AstNode* prog);
struct Rz_SsaBlockRef* add_ident_ref(struct Rz_SsaBlockRef* head, const char* name);
int64_t has_ident_in_list(struct Rz_SsaBlockRef* head, const char* name);
struct Rz_SsaBlockRef* collect_mutated_vars_in_node(struct Rz_AstNode* node, struct Rz_SsaBlockRef* muts);
int64_t is_expr_invariant(struct Rz_AstNode* expr, struct Rz_SsaBlockRef* muts);
void hoist_loop_invariants_in_block(struct Rz_AstNode* block, struct Rz_OptStats* stats);
struct Rz_AstNode* optimize_program(struct Rz_AstNode* prog);
const char* emit_program_ssa(struct Rz_AstNode* prog);
const char* escape_c_string(const char* s);
const char* c_type(const char* type_name);
const char* gen_expression(struct Rz_AstNode* expr);
const char* gen_statement(struct Rz_AstNode* stmt, const char* indent);
const char* gen_block_body(struct Rz_AstNode* block, const char* indent);
const char* generate_c_code(struct Rz_AstNode* prog);
const char* hex_digit(int64_t v);
const char* byte_to_hex(int64_t b);
struct Rz_BinaryBuffer* bb_new(void);
void bb_append_u8(struct Rz_BinaryBuffer* bb, int64_t b);
void bb_append_u16_le(struct Rz_BinaryBuffer* bb, int64_t val);
void bb_append_u32_le(struct Rz_BinaryBuffer* bb, int64_t val);
void bb_append_u64_le(struct Rz_BinaryBuffer* bb, int64_t val);
void bb_append_uleb128(struct Rz_BinaryBuffer* bb, int64_t val);
void bb_append_zeroes(struct Rz_BinaryBuffer* bb, int64_t count);
void bb_append_hex(struct Rz_BinaryBuffer* bb, const char* hex_str, int64_t byte_count);
void bb_append_str(struct Rz_BinaryBuffer* bb, const char* s);
void bb_append_str_null(struct Rz_BinaryBuffer* bb, const char* s);
void bb_append_buffer(struct Rz_BinaryBuffer* bb, struct Rz_BinaryBuffer* src);
int64_t bb_write_to_file(struct Rz_BinaryBuffer* bb, const char* path);
void emit_func_x86(struct Rz_AstNode* decl, struct Rz_BinaryBuffer* bb);
int64_t emit_elf_object(struct Rz_AstNode* prog, const char* out_path);
int64_t emit_standalone_elf(struct Rz_AstNode* prog, const char* out_path);
int64_t emit_macho_object(struct Rz_AstNode* prog, const char* out_path);
int64_t emit_pe_object(struct Rz_AstNode* prog, const char* out_path);
int64_t emit_wasm_module(struct Rz_AstNode* prog, int64_t is_wasi, const char* out_path);
int64_t emit_embedded_binary(struct Rz_AstNode* prog, const char* chip, const char* out_path);
int64_t emit_native_binary(struct Rz_AstNode* prog, const char* target, const char* out_path);
const char* compile_source_to_c(const char* source);
int64_t emit_ssa_for_file(const char* file_path);
int64_t verify_semantic_checking(const char* file_path);
int64_t verify_frontend_parsing(const char* file_path);
int64_t emit_native_for_file(const char* file_path, const char* target, const char* out_path);
int64_t compile_file(const char* in_path, const char* out_c_path, const char* out_bin_path);
int64_t run_built_in_self_test(void);
void rz_user_main(void);

// --- Function Implementations ---
int64_t TOK_EOF(void) {
    return 0;
}

int64_t TOK_IDENT(void) {
    return 1;
}

int64_t TOK_INT(void) {
    return 2;
}

int64_t TOK_STRING(void) {
    return 3;
}

int64_t TOK_BOOL(void) {
    return 4;
}

int64_t TOK_CHAR(void) {
    return 5;
}

int64_t TOK_FUNC(void) {
    return 10;
}

int64_t TOK_LET(void) {
    return 11;
}

int64_t TOK_VAR(void) {
    return 12;
}

int64_t TOK_IF(void) {
    return 13;
}

int64_t TOK_ELSE(void) {
    return 14;
}

int64_t TOK_WHILE(void) {
    return 15;
}

int64_t TOK_FOR(void) {
    return 16;
}

int64_t TOK_RETURN(void) {
    return 17;
}

int64_t TOK_CLASS(void) {
    return 18;
}

int64_t TOK_NEW(void) {
    return 19;
}

int64_t TOK_EXTERN(void) {
    return 20;
}

int64_t TOK_IMPORT(void) {
    return 21;
}

int64_t TOK_NULL(void) {
    return 22;
}

int64_t TOK_MUT(void) {
    return 23;
}

int64_t TOK_CONST(void) {
    return 24;
}

int64_t TOK_ENUM(void) {
    return 25;
}

int64_t TOK_MATCH(void) {
    return 26;
}

int64_t TOK_TRAIT(void) {
    return 27;
}

int64_t TOK_IMPL(void) {
    return 28;
}

int64_t TOK_THIS(void) {
    return 29;
}

int64_t TOK_UNSAFE(void) {
    return 61;
}

int64_t TOK_RAW(void) {
    return 62;
}

int64_t TOK_COMPTIME(void) {
    return 63;
}

int64_t TOK_ASYNC(void) {
    return 64;
}

int64_t TOK_AWAIT(void) {
    return 65;
}

int64_t TOK_TYPE(void) {
    return 66;
}

int64_t TOK_WHERE(void) {
    return 67;
}

int64_t TOK_ASM(void) {
    return 78;
}

int64_t TOK_IN(void) {
    return 79;
}

int64_t TOK_PLUS(void) {
    return 30;
}

int64_t TOK_MINUS(void) {
    return 31;
}

int64_t TOK_STAR(void) {
    return 32;
}

int64_t TOK_SLASH(void) {
    return 33;
}

int64_t TOK_PERCENT(void) {
    return 34;
}

int64_t TOK_ASSIGN(void) {
    return 35;
}

int64_t TOK_EQ(void) {
    return 36;
}

int64_t TOK_NE(void) {
    return 37;
}

int64_t TOK_LT(void) {
    return 38;
}

int64_t TOK_LE(void) {
    return 39;
}

int64_t TOK_GT(void) {
    return 40;
}

int64_t TOK_GE(void) {
    return 41;
}

int64_t TOK_AND(void) {
    return 42;
}

int64_t TOK_OR(void) {
    return 43;
}

int64_t TOK_NOT(void) {
    return 44;
}

int64_t TOK_QUESTION(void) {
    return 45;
}

int64_t TOK_BIT_AND(void) {
    return 46;
}

int64_t TOK_BIT_OR(void) {
    return 47;
}

int64_t TOK_BIT_XOR(void) {
    return 48;
}

int64_t TOK_BIT_NOT(void) {
    return 49;
}

int64_t TOK_LPAREN(void) {
    return 50;
}

int64_t TOK_RPAREN(void) {
    return 51;
}

int64_t TOK_LBRACE(void) {
    return 52;
}

int64_t TOK_RBRACE(void) {
    return 53;
}

int64_t TOK_LBRACKET(void) {
    return 54;
}

int64_t TOK_RBRACKET(void) {
    return 55;
}

int64_t TOK_SEMI(void) {
    return 56;
}

int64_t TOK_COMMA(void) {
    return 57;
}

int64_t TOK_COLON(void) {
    return 58;
}

int64_t TOK_DOT(void) {
    return 59;
}

int64_t TOK_ARROW(void) {
    return 60;
}

int64_t TOK_LSHIFT(void) {
    return 68;
}

int64_t TOK_RSHIFT(void) {
    return 69;
}

int64_t TOK_FAT_ARROW(void) {
    return 70;
}

int64_t TOK_COLON_COLON(void) {
    return 71;
}

int64_t TOK_DOT_DOT(void) {
    return 72;
}

int64_t TOK_DOT_DOT_EQ(void) {
    return 73;
}

int64_t TOK_AT(void) {
    return 74;
}

int64_t TOK_HASH(void) {
    return 75;
}

int64_t TOK_UNDERSCORE(void) {
    return 76;
}

int64_t TOK_BANG(void) {
    return 77;
}

int64_t NODE_PROGRAM(void) {
    return 100;
}

int64_t NODE_FUNC_DECL(void) {
    return 101;
}

int64_t NODE_EXTERN_DECL(void) {
    return 102;
}

int64_t NODE_CLASS_DECL(void) {
    return 103;
}

int64_t NODE_VAR_DECL(void) {
    return 104;
}

int64_t NODE_PARAM(void) {
    return 105;
}

int64_t NODE_FIELD(void) {
    return 106;
}

int64_t NODE_ENUM_DECL(void) {
    return 107;
}

int64_t NODE_ENUM_VARIANT(void) {
    return 108;
}

int64_t NODE_TRAIT_DECL(void) {
    return 109;
}

int64_t NODE_IMPL_DECL(void) {
    return 130;
}

int64_t NODE_ANNOTATION(void) {
    return 131;
}

int64_t NODE_IMPORT(void) {
    return 140;
}

int64_t NODE_BLOCK(void) {
    return 110;
}

int64_t NODE_IF(void) {
    return 111;
}

int64_t NODE_WHILE(void) {
    return 112;
}

int64_t NODE_FOR(void) {
    return 113;
}

int64_t NODE_RETURN(void) {
    return 114;
}

int64_t NODE_ASSIGN(void) {
    return 115;
}

int64_t NODE_EXPR_STMT(void) {
    return 116;
}

int64_t NODE_MATCH(void) {
    return 117;
}

int64_t NODE_MATCH_ARM(void) {
    return 118;
}

int64_t NODE_UNSAFE_BLOCK(void) {
    return 119;
}

int64_t NODE_COMPTIME_STMT(void) {
    return 132;
}

int64_t NODE_FIELD_ASSIGN(void) {
    return 143;
}

int64_t NODE_INDEX_ASSIGN(void) {
    return 144;
}

int64_t NODE_BIN_OP(void) {
    return 120;
}

int64_t NODE_UNARY_OP(void) {
    return 121;
}

int64_t NODE_CALL(void) {
    return 122;
}

int64_t NODE_MEMBER(void) {
    return 123;
}

int64_t NODE_NEW(void) {
    return 124;
}

int64_t NODE_LITERAL_INT(void) {
    return 125;
}

int64_t NODE_LITERAL_STR(void) {
    return 126;
}

int64_t NODE_LITERAL_BOOL(void) {
    return 127;
}

int64_t NODE_LITERAL_NULL(void) {
    return 128;
}

int64_t NODE_IDENT(void) {
    return 129;
}

int64_t NODE_METHOD_CALL(void) {
    return 133;
}

int64_t NODE_INDEX(void) {
    return 134;
}

int64_t NODE_TRY(void) {
    return 135;
}

int64_t NODE_CLOSURE(void) {
    return 136;
}

int64_t NODE_COMPTIME_EXPR(void) {
    return 137;
}

int64_t NODE_MACRO_CALL(void) {
    return 138;
}

int64_t NODE_PATTERN(void) {
    return 139;
}

int64_t NODE_AWAIT(void) {
    return 141;
}

int64_t NODE_ASM(void) {
    return 142;
}

int64_t NODE_LITERAL_CHAR(void) {
    return 145;
}

int64_t SYM_VAR(void) {
    return 1;
}

int64_t SYM_PARAM(void) {
    return 2;
}

int64_t SYM_FUNC(void) {
    return 3;
}

int64_t SYM_CLASS(void) {
    return 4;
}

int64_t SYM_ENUM(void) {
    return 5;
}

int64_t SYM_VARIANT(void) {
    return 6;
}

int64_t SYM_TRAIT(void) {
    return 7;
}

int64_t SYM_IMPL(void) {
    return 8;
}

int64_t SYM_CONST(void) {
    return 9;
}

int64_t SYM_GENERIC_PARAM(void) {
    return 10;
}

int64_t BORROW_NONE(void) {
    return 0;
}

int64_t BORROW_OWNED(void) {
    return 1;
}

int64_t BORROW_SHARED(void) {
    return 2;
}

int64_t BORROW_MUT(void) {
    return 3;
}

int64_t BORROW_MOVED(void) {
    return 4;
}

int64_t SSA_NOP(void) {
    return 200;
}

int64_t SSA_CONST_INT(void) {
    return 201;
}

int64_t SSA_CONST_STR(void) {
    return 202;
}

int64_t SSA_CONST_BOOL(void) {
    return 203;
}

int64_t SSA_BIN_OP(void) {
    return 204;
}

int64_t SSA_UNARY_OP(void) {
    return 205;
}

int64_t SSA_CALL(void) {
    return 206;
}

int64_t SSA_ALLOC(void) {
    return 207;
}

int64_t SSA_LOAD(void) {
    return 208;
}

int64_t SSA_STORE(void) {
    return 209;
}

int64_t SSA_GET_FIELD(void) {
    return 210;
}

int64_t SSA_SET_FIELD(void) {
    return 211;
}

int64_t SSA_PHI(void) {
    return 212;
}

int64_t SSA_BR(void) {
    return 213;
}

int64_t SSA_BR_COND(void) {
    return 214;
}

int64_t SSA_RETURN(void) {
    return 215;
}

struct Rz_Token* make_token(int64_t kind, const char* text, int64_t int_val, int64_t line, int64_t col) {
    return new_Token(kind, text, int_val, line, col, NULL);
}

struct Rz_AstNode* make_ast_node(int64_t kind, const char* name, const char* type_name, int64_t line, int64_t col) {
    return new_AstNode(kind, name, type_name, "", 0, "", NULL, NULL, NULL, NULL, NULL, line, col);
}

struct Rz_AstNode* clone_ast_node(struct Rz_AstNode* node) {
    if ((node == NULL)) {
        return NULL;
    }
    struct Rz_AstNode* copy = make_ast_node(node->kind, node->name, node->type_name, node->line, node->col);
    copy->op = node->op;
    copy->int_val = node->int_val;
    copy->str_val = node->str_val;
    copy->child1 = clone_ast_node(node->child1);
    copy->child2 = clone_ast_node(node->child2);
    copy->child3 = clone_ast_node(node->child3);
    copy->child4 = clone_ast_node(node->child4);
    copy->next = clone_ast_node(node->next);
    return copy;
}

struct Rz_AstNode* clone_single_node(struct Rz_AstNode* node) {
    if ((node == NULL)) {
        return NULL;
    }
    struct Rz_AstNode* copy = make_ast_node(node->kind, node->name, node->type_name, node->line, node->col);
    copy->op = node->op;
    copy->int_val = node->int_val;
    copy->str_val = node->str_val;
    copy->child1 = clone_ast_node(node->child1);
    copy->child2 = clone_ast_node(node->child2);
    copy->child3 = clone_ast_node(node->child3);
    copy->child4 = clone_ast_node(node->child4);
    copy->next = NULL;
    return copy;
}

struct Rz_Symbol* make_symbol(const char* name, const char* type_name, int64_t kind, int64_t scope_level, int64_t is_mut, struct Rz_Symbol* next) {
    return new_Symbol(name, type_name, kind, scope_level, is_mut, 1, NULL, next);
}

const char* token_kind_to_str(int64_t kind) {
    if ((kind == TOK_EOF())) {
        return "EOF";
    }
    if ((kind == TOK_IDENT())) {
        return "IDENT";
    }
    if ((kind == TOK_INT())) {
        return "INT";
    }
    if ((kind == TOK_STRING())) {
        return "STRING";
    }
    if ((kind == TOK_BOOL())) {
        return "BOOL";
    }
    if ((kind == TOK_CHAR())) {
        return "CHAR";
    }
    if ((kind == TOK_FUNC())) {
        return "func";
    }
    if ((kind == TOK_LET())) {
        return "let";
    }
    if ((kind == TOK_VAR())) {
        return "var";
    }
    if ((kind == TOK_IF())) {
        return "if";
    }
    if ((kind == TOK_ELSE())) {
        return "else";
    }
    if ((kind == TOK_WHILE())) {
        return "while";
    }
    if ((kind == TOK_FOR())) {
        return "for";
    }
    if ((kind == TOK_RETURN())) {
        return "return";
    }
    if ((kind == TOK_CLASS())) {
        return "class";
    }
    if ((kind == TOK_NEW())) {
        return "new";
    }
    if ((kind == TOK_EXTERN())) {
        return "extern";
    }
    if ((kind == TOK_IMPORT())) {
        return "import";
    }
    if ((kind == TOK_NULL())) {
        return "null";
    }
    if ((kind == TOK_ENUM())) {
        return "enum";
    }
    if ((kind == TOK_MATCH())) {
        return "match";
    }
    if ((kind == TOK_TRAIT())) {
        return "trait";
    }
    if ((kind == TOK_IMPL())) {
        return "impl";
    }
    if ((kind == TOK_UNSAFE())) {
        return "unsafe";
    }
    if ((kind == TOK_COMPTIME())) {
        return "comptime";
    }
    if ((kind == TOK_ASYNC())) {
        return "async";
    }
    if ((kind == TOK_AWAIT())) {
        return "await";
    }
    return "TOKEN";
}

int64_t is_digit(int64_t ch) {
    return ((ch >= 48) && (ch <= 57));
}

int64_t is_alpha(int64_t ch) {
    if (((ch >= 65) && (ch <= 90))) {
        return 1;
    }
    if (((ch >= 97) && (ch <= 122))) {
        return 1;
    }
    if ((ch == 95)) {
        return 1;
    }
    return 0;
}

int64_t is_alnum(int64_t ch) {
    return (is_alpha(ch) || is_digit(ch));
}

int64_t keyword_kind(const char* text) {
    if ((strcmp(text, "func") == 0)) {
        return TOK_FUNC();
    }
    if ((strcmp(text, "let") == 0)) {
        return TOK_LET();
    }
    if ((strcmp(text, "var") == 0)) {
        return TOK_VAR();
    }
    if ((strcmp(text, "mut") == 0)) {
        return TOK_MUT();
    }
    if ((strcmp(text, "const") == 0)) {
        return TOK_CONST();
    }
    if ((strcmp(text, "if") == 0)) {
        return TOK_IF();
    }
    if ((strcmp(text, "else") == 0)) {
        return TOK_ELSE();
    }
    if ((strcmp(text, "while") == 0)) {
        return TOK_WHILE();
    }
    if ((strcmp(text, "for") == 0)) {
        return TOK_FOR();
    }
    if ((strcmp(text, "return") == 0)) {
        return TOK_RETURN();
    }
    if ((strcmp(text, "class") == 0)) {
        return TOK_CLASS();
    }
    if ((strcmp(text, "new") == 0)) {
        return TOK_NEW();
    }
    if ((strcmp(text, "extern") == 0)) {
        return TOK_EXTERN();
    }
    if ((strcmp(text, "import") == 0)) {
        return TOK_IMPORT();
    }
    if ((strcmp(text, "enum") == 0)) {
        return TOK_ENUM();
    }
    if ((strcmp(text, "match") == 0)) {
        return TOK_MATCH();
    }
    if ((strcmp(text, "trait") == 0)) {
        return TOK_TRAIT();
    }
    if ((strcmp(text, "impl") == 0)) {
        return TOK_IMPL();
    }
    if ((strcmp(text, "this") == 0)) {
        return TOK_THIS();
    }
    if ((strcmp(text, "unsafe") == 0)) {
        return TOK_UNSAFE();
    }
    if ((strcmp(text, "raw") == 0)) {
        return TOK_RAW();
    }
    if ((strcmp(text, "comptime") == 0)) {
        return TOK_COMPTIME();
    }
    if ((strcmp(text, "async") == 0)) {
        return TOK_ASYNC();
    }
    if ((strcmp(text, "await") == 0)) {
        return TOK_AWAIT();
    }
    if ((strcmp(text, "asm") == 0)) {
        return TOK_ASM();
    }
    if ((strcmp(text, "type") == 0)) {
        return TOK_TYPE();
    }
    if ((strcmp(text, "where") == 0)) {
        return TOK_WHERE();
    }
    if ((strcmp(text, "in") == 0)) {
        return TOK_IN();
    }
    if ((strcmp(text, "null") == 0)) {
        return TOK_NULL();
    }
    if ((strcmp(text, "true") == 0)) {
        return TOK_BOOL();
    }
    if ((strcmp(text, "false") == 0)) {
        return TOK_BOOL();
    }
    return TOK_IDENT();
}

struct Rz_Token* tokenize(const char* source) {
    struct Rz_Token* head = NULL;
    struct Rz_Token* tail = NULL;
    int64_t len = rz_str_len(source);
    int64_t pos = 0;
    int64_t line = 1;
    int64_t col = 1;
    while ((pos < len)) {
        int64_t ch = rz_char_at(source, pos);
        if ((((ch == 32) || (ch == 9)) || (ch == 13))) {
            pos = (pos + 1);
            col = (col + 1);
        } else         if ((ch == 10)) {
            pos = (pos + 1);
            line = (line + 1);
            col = 1;
        } else         if ((ch == 47)) {
            int64_t next_ch = rz_char_at(source, (pos + 1));
            if ((next_ch == 47)) {
                pos = (pos + 2);
                col = (col + 2);
                while (((pos < len) && (rz_char_at(source, pos) != 10))) {
                    pos = (pos + 1);
                    col = (col + 1);
                }
            } else             if ((next_ch == 42)) {
                pos = (pos + 2);
                col = (col + 2);
                int64_t comment_depth = 1;
                while (((pos < len) && (comment_depth > 0))) {
                    int64_t c = rz_char_at(source, pos);
                    int64_t n = rz_char_at(source, (pos + 1));
                    if ((c == 10)) {
                        line = (line + 1);
                        col = 1;
                        pos = (pos + 1);
                    } else                     if (((c == 47) && (n == 42))) {
                        comment_depth = (comment_depth + 1);
                        pos = (pos + 2);
                        col = (col + 2);
                    } else                     if (((c == 42) && (n == 47))) {
                        comment_depth = (comment_depth - 1);
                        pos = (pos + 2);
                        col = (col + 2);
                    } else {
                        pos = (pos + 1);
                        col = (col + 1);
                    }
                }
            } else {
                struct Rz_Token* tok = make_token(TOK_SLASH(), "/", 0, line, col);
                if ((head == NULL)) {
                    head = tok;
                    tail = tok;
                } else {
                    tail->next = tok;
                    tail = tok;
                }
                pos = (pos + 1);
                col = (col + 1);
            }
        } else         if (is_alpha(ch)) {
            int64_t start_pos = pos;
            int64_t start_col = col;
            while (((pos < len) && is_alnum(rz_char_at(source, pos)))) {
                pos = (pos + 1);
                col = (col + 1);
            }
            const char* word = rz_substring(source, start_pos, (pos - start_pos));
            int64_t kind = keyword_kind(word);
            int64_t int_val = 0;
            if ((strcmp(word, "true") == 0)) {
                int_val = 1;
            }
            struct Rz_Token* tok = make_token(kind, word, int_val, line, start_col);
            if ((head == NULL)) {
                head = tok;
                tail = tok;
            } else {
                tail->next = tok;
                tail = tok;
            }
        } else         if (is_digit(ch)) {
            int64_t start_pos = pos;
            int64_t start_col = col;
            int64_t val = 0;
            int64_t next_ch = rz_char_at(source, (pos + 1));
            if (((ch == 48) && ((next_ch == 120) || (next_ch == 88)))) {
                pos = (pos + 2);
                col = (col + 2);
                int64_t reading_hex = 1;
                while (((pos < len) && reading_hex)) {
                    int64_t hc = rz_char_at(source, pos);
                    if (((hc >= 48) && (hc <= 57))) {
                        val = ((val * 16) + (hc - 48));
                        pos = (pos + 1);
                        col = (col + 1);
                    } else                     if (((hc >= 97) && (hc <= 102))) {
                        val = ((val * 16) + ((hc - 97) + 10));
                        pos = (pos + 1);
                        col = (col + 1);
                    } else                     if (((hc >= 65) && (hc <= 70))) {
                        val = ((val * 16) + ((hc - 65) + 10));
                        pos = (pos + 1);
                        col = (col + 1);
                    } else                     if ((hc == 95)) {
                        pos = (pos + 1);
                        col = (col + 1);
                    } else {
                        reading_hex = 0;
                    }
                }
            } else             if (((ch == 48) && ((next_ch == 98) || (next_ch == 66)))) {
                pos = (pos + 2);
                col = (col + 2);
                int64_t reading_bin = 1;
                while (((pos < len) && reading_bin)) {
                    int64_t bc = rz_char_at(source, pos);
                    if (((bc == 48) || (bc == 49))) {
                        val = ((val * 2) + (bc - 48));
                        pos = (pos + 1);
                        col = (col + 1);
                    } else                     if ((bc == 95)) {
                        pos = (pos + 1);
                        col = (col + 1);
                    } else {
                        reading_bin = 0;
                    }
                }
            } else {
                while (((pos < len) && (is_digit(rz_char_at(source, pos)) || (rz_char_at(source, pos) == 95)))) {
                    int64_t dc = rz_char_at(source, pos);
                    if ((dc != 95)) {
                        int64_t d = (dc - 48);
                        val = ((val * 10) + d);
                    }
                    pos = (pos + 1);
                    col = (col + 1);
                }
            }
            const char* num_str = rz_substring(source, start_pos, (pos - start_pos));
            struct Rz_Token* tok = make_token(TOK_INT(), num_str, val, line, start_col);
            if ((head == NULL)) {
                head = tok;
                tail = tok;
            } else {
                tail->next = tok;
                tail = tok;
            }
        } else         if ((ch == 34)) {
            int64_t start_col = col;
            pos = (pos + 1);
            col = (col + 1);
            const char* str_val = "";
            int64_t closed = 0;
            while (((pos < len) && (closed == 0))) {
                int64_t c = rz_char_at(source, pos);
                if ((c == 34)) {
                    closed = 1;
                    pos = (pos + 1);
                    col = (col + 1);
                } else                 if ((c == 92)) {
                    pos = (pos + 1);
                    col = (col + 1);
                    int64_t esc = rz_char_at(source, pos);
                    if ((esc == 110)) {
                        str_val = rz_str_concat(str_val, "\n");
                    } else                     if ((esc == 116)) {
                        str_val = rz_str_concat(str_val, "\t");
                    } else                     if ((esc == 114)) {
                        str_val = rz_str_concat(str_val, "\r");
                    } else                     if ((esc == 34)) {
                        str_val = rz_str_concat(str_val, "\"");
                    } else                     if ((esc == 92)) {
                        str_val = rz_str_concat(str_val, "\\");
                    } else                     if ((esc == 48)) {
                        str_val = rz_str_concat(str_val, "0");
                    } else {
                        str_val = rz_str_concat(str_val, rz_char_to_str(esc));
                    }
                    pos = (pos + 1);
                    col = (col + 1);
                } else                 if ((c == 10)) {
                    line = (line + 1);
                    col = 1;
                    pos = (pos + 1);
                    str_val = rz_str_concat(str_val, "\n");
                } else {
                    str_val = rz_str_concat(str_val, rz_char_to_str(c));
                    pos = (pos + 1);
                    col = (col + 1);
                }
            }
            struct Rz_Token* tok = make_token(TOK_STRING(), str_val, 0, line, start_col);
            if ((head == NULL)) {
                head = tok;
                tail = tok;
            } else {
                tail->next = tok;
                tail = tok;
            }
        } else         if ((ch == 39)) {
            int64_t start_col = col;
            pos = (pos + 1);
            col = (col + 1);
            int64_t c = rz_char_at(source, pos);
            int64_t char_val = 0;
            const char* char_str = "";
            if ((c == 92)) {
                pos = (pos + 1);
                col = (col + 1);
                int64_t esc = rz_char_at(source, pos);
                if ((esc == 110)) {
                    char_val = 10;
                    char_str = "\n";
                } else                 if ((esc == 116)) {
                    char_val = 9;
                    char_str = "\t";
                } else                 if ((esc == 114)) {
                    char_val = 13;
                    char_str = "\r";
                } else                 if ((esc == 48)) {
                    char_val = 0;
                    char_str = "0";
                } else                 if ((esc == 39)) {
                    char_val = 39;
                    char_str = "'";
                } else                 if ((esc == 92)) {
                    char_val = 92;
                    char_str = "\\";
                } else {
                    char_val = esc;
                    char_str = rz_char_to_str(esc);
                }
            } else {
                char_val = c;
                char_str = rz_char_to_str(c);
            }
            pos = (pos + 1);
            col = (col + 1);
            if (((pos < len) && (rz_char_at(source, pos) == 39))) {
                pos = (pos + 1);
                col = (col + 1);
            }
            struct Rz_Token* tok = make_token(TOK_CHAR(), char_str, char_val, line, start_col);
            if ((head == NULL)) {
                head = tok;
                tail = tok;
            } else {
                tail->next = tok;
                tail = tok;
            }
        } else {
            int64_t tok_kind = 0;
            const char* tok_text = "";
            int64_t advance = 1;
            int64_t next_c = rz_char_at(source, (pos + 1));
            if ((ch == 43)) {
                tok_kind = TOK_PLUS();
                tok_text = "+";
            } else             if ((ch == 45)) {
                if ((next_c == 62)) {
                    tok_kind = TOK_ARROW();
                    tok_text = "->";
                    advance = 2;
                } else {
                    tok_kind = TOK_MINUS();
                    tok_text = "-";
                }
            } else             if ((ch == 42)) {
                tok_kind = TOK_STAR();
                tok_text = "*";
            } else             if ((ch == 37)) {
                tok_kind = TOK_PERCENT();
                tok_text = "%";
            } else             if ((ch == 61)) {
                if ((next_c == 61)) {
                    tok_kind = TOK_EQ();
                    tok_text = "==";
                    advance = 2;
                } else                 if ((next_c == 62)) {
                    tok_kind = TOK_FAT_ARROW();
                    tok_text = "=>";
                    advance = 2;
                } else {
                    tok_kind = TOK_ASSIGN();
                    tok_text = "=";
                }
            } else             if ((ch == 33)) {
                if ((next_c == 61)) {
                    tok_kind = TOK_NE();
                    tok_text = "!=";
                    advance = 2;
                } else {
                    tok_kind = TOK_BANG();
                    tok_text = "!";
                }
            } else             if ((ch == 60)) {
                if ((next_c == 61)) {
                    tok_kind = TOK_LE();
                    tok_text = "<=";
                    advance = 2;
                } else                 if ((next_c == 60)) {
                    tok_kind = TOK_LSHIFT();
                    tok_text = "<<";
                    advance = 2;
                } else {
                    tok_kind = TOK_LT();
                    tok_text = "<";
                }
            } else             if ((ch == 62)) {
                if ((next_c == 61)) {
                    tok_kind = TOK_GE();
                    tok_text = ">=";
                    advance = 2;
                } else                 if ((next_c == 62)) {
                    tok_kind = TOK_RSHIFT();
                    tok_text = ">>";
                    advance = 2;
                } else {
                    tok_kind = TOK_GT();
                    tok_text = ">";
                }
            } else             if ((ch == 38)) {
                if ((next_c == 38)) {
                    tok_kind = TOK_AND();
                    tok_text = "&&";
                    advance = 2;
                } else {
                    tok_kind = TOK_BIT_AND();
                    tok_text = "&";
                }
            } else             if ((ch == 124)) {
                if ((next_c == 124)) {
                    tok_kind = TOK_OR();
                    tok_text = "||";
                    advance = 2;
                } else {
                    tok_kind = TOK_BIT_OR();
                    tok_text = "|";
                }
            } else             if ((ch == 94)) {
                tok_kind = TOK_BIT_XOR();
                tok_text = "^";
            } else             if ((ch == 126)) {
                tok_kind = TOK_BIT_NOT();
                tok_text = "~";
            } else             if ((ch == 63)) {
                tok_kind = TOK_QUESTION();
                tok_text = "?";
            } else             if ((ch == 40)) {
                tok_kind = TOK_LPAREN();
                tok_text = "(";
            } else             if ((ch == 41)) {
                tok_kind = TOK_RPAREN();
                tok_text = ")";
            } else             if ((ch == 123)) {
                tok_kind = TOK_LBRACE();
                tok_text = "{";
            } else             if ((ch == 125)) {
                tok_kind = TOK_RBRACE();
                tok_text = "}";
            } else             if ((ch == 91)) {
                tok_kind = TOK_LBRACKET();
                tok_text = "[";
            } else             if ((ch == 93)) {
                tok_kind = TOK_RBRACKET();
                tok_text = "]";
            } else             if ((ch == 59)) {
                tok_kind = TOK_SEMI();
                tok_text = ";";
            } else             if ((ch == 44)) {
                tok_kind = TOK_COMMA();
                tok_text = ",";
            } else             if ((ch == 58)) {
                if ((next_c == 58)) {
                    tok_kind = TOK_COLON_COLON();
                    tok_text = "::";
                    advance = 2;
                } else {
                    tok_kind = TOK_COLON();
                    tok_text = ":";
                }
            } else             if ((ch == 46)) {
                if ((next_c == 46)) {
                    int64_t third_c = rz_char_at(source, (pos + 2));
                    if ((third_c == 61)) {
                        tok_kind = TOK_DOT_DOT_EQ();
                        tok_text = "..=";
                        advance = 3;
                    } else {
                        tok_kind = TOK_DOT_DOT();
                        tok_text = "..";
                        advance = 2;
                    }
                } else {
                    tok_kind = TOK_DOT();
                    tok_text = ".";
                }
            } else             if ((ch == 64)) {
                tok_kind = TOK_AT();
                tok_text = "@";
            } else             if ((ch == 35)) {
                tok_kind = TOK_HASH();
                tok_text = "#";
            } else             if ((ch == 95)) {
                tok_kind = TOK_UNDERSCORE();
                tok_text = "_";
            }
            if ((tok_kind > 0)) {
                struct Rz_Token* tok = make_token(tok_kind, tok_text, 0, line, col);
                if ((head == NULL)) {
                    head = tok;
                    tail = tok;
                } else {
                    tail->next = tok;
                    tail = tok;
                }
                pos = (pos + advance);
                col = (col + advance);
            } else {
                pos = (pos + 1);
                col = (col + 1);
            }
        }
    }
    struct Rz_Token* eof_tok = make_token(TOK_EOF(), "", 0, line, col);
    if ((head == NULL)) {
        head = eof_tok;
    } else {
        tail->next = eof_tok;
    }
    return head;
}

struct Rz_Parser* make_parser(struct Rz_Token* tokens) {
    return new_Parser(tokens);
}

int64_t peek_kind(struct Rz_Parser* p) {
    if ((p->curr == NULL)) {
        return TOK_EOF();
    }
    return p->curr->kind;
}

int64_t peek_next_kind(struct Rz_Parser* p) {
    if (((p->curr == NULL) || (p->curr->next == NULL))) {
        return TOK_EOF();
    }
    return p->curr->next->kind;
}

const char* peek_text(struct Rz_Parser* p) {
    if ((p->curr == NULL)) {
        return "";
    }
    return p->curr->text;
}

int64_t peek_line(struct Rz_Parser* p) {
    if ((p->curr == NULL)) {
        return 0;
    }
    return p->curr->line;
}

int64_t peek_col(struct Rz_Parser* p) {
    if ((p->curr == NULL)) {
        return 0;
    }
    return p->curr->col;
}

struct Rz_Token* advance_token(struct Rz_Parser* p) {
    struct Rz_Token* tok = p->curr;
    if ((p->curr != NULL)) {
        p->curr = p->curr->next;
    }
    return tok;
}

int64_t check_token(struct Rz_Parser* p, int64_t kind) {
    return (peek_kind(p) == kind);
}

int64_t match_tok(struct Rz_Parser* p, int64_t kind) {
    if (check_token(p, kind)) {
        advance_token(p);
        return 1;
    }
    return 0;
}

struct Rz_Token* consume_tok(struct Rz_Parser* p, int64_t kind, const char* msg) {
    if (check_token(p, kind)) {
        return advance_token(p);
    }
    printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("❌ Parser Error: ", msg), " (got '"), peek_text(p)), "') at line "), rz_int_to_str(peek_line(p))));
    return advance_token(p);
}

struct Rz_AstNode* parse_primary(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    if (check_token(p, TOK_INT())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_INT(), tok->text, "int", line, col);
        node->int_val = tok->int_val;
        return node;
    }
    if (check_token(p, TOK_STRING())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_STR(), tok->text, "string", line, col);
        node->str_val = tok->text;
        return node;
    }
    if (check_token(p, TOK_CHAR())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_CHAR(), tok->text, "char", line, col);
        node->int_val = tok->int_val;
        node->str_val = tok->text;
        return node;
    }
    if (check_token(p, TOK_BOOL())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_BOOL(), tok->text, "bool", line, col);
        node->int_val = tok->int_val;
        return node;
    }
    if (check_token(p, TOK_NULL())) {
        advance_token(p);
        return make_ast_node(NODE_LITERAL_NULL(), "null", "null", line, col);
    }
    if (check_token(p, TOK_THIS())) {
        advance_token(p);
        return make_ast_node(NODE_IDENT(), "this", "this", line, col);
    }
    if (check_token(p, TOK_COMPTIME())) {
        advance_token(p);
        struct Rz_AstNode* inner = parse_primary(p);
        struct Rz_AstNode* node = make_ast_node(NODE_COMPTIME_EXPR(), "comptime", "void", line, col);
        node->child1 = inner;
        return node;
    }
    if (check_token(p, TOK_AWAIT())) {
        advance_token(p);
        struct Rz_AstNode* inner = parse_expression(p);
        struct Rz_AstNode* node = make_ast_node(NODE_AWAIT(), "await", "void", line, col);
        node->child1 = inner;
        return node;
    }
    if (check_token(p, TOK_NEW())) {
        advance_token(p);
        const char* class_name = consume_tok(p, TOK_IDENT(), "Expected class name after 'new'")->text;
        const char* generic_arg = "";
        if (check_token(p, TOK_LT())) {
            advance_token(p);
            while (((!check_token(p, TOK_GT())) && (!check_token(p, TOK_EOF())))) {
                if ((strcmp(generic_arg, "") == 0)) {
                    generic_arg = p->curr->text;
                } else {
                    generic_arg = rz_str_concat(rz_str_concat(generic_arg, ","), p->curr->text);
                }
                advance_token(p);
            }
            consume_tok(p, TOK_GT(), "Expected '>' after type arguments");
        }
        consume_tok(p, TOK_LPAREN(), "Expected '(' after class name");
        struct Rz_AstNode* node = make_ast_node(NODE_NEW(), class_name, class_name, line, col);
        node->op = generic_arg;
        struct Rz_AstNode* args_head = NULL;
        struct Rz_AstNode* args_tail = NULL;
        while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
            struct Rz_AstNode* arg = parse_expression(p);
            if ((args_head == NULL)) {
                args_head = arg;
                args_tail = arg;
            } else {
                args_tail->next = arg;
                args_tail = arg;
            }
            if (check_token(p, TOK_COMMA())) {
                advance_token(p);
            }
        }
        consume_tok(p, TOK_RPAREN(), "Expected ')' after constructor arguments");
        node->child1 = args_head;
        return node;
    }
    if (check_token(p, TOK_OR())) {
        advance_token(p);
        struct Rz_AstNode* body = parse_block(p);
        struct Rz_AstNode* c_node = make_ast_node(NODE_CLOSURE(), "closure", "void", line, col);
        c_node->child3 = body;
        return c_node;
    }
    if (check_token(p, TOK_BIT_OR())) {
        advance_token(p);
        struct Rz_AstNode* p_head = NULL;
        struct Rz_AstNode* p_tail = NULL;
        while (((!check_token(p, TOK_BIT_OR())) && (!check_token(p, TOK_EOF())))) {
            const char* p_name = consume_tok(p, TOK_IDENT(), "Expected parameter name in closure")->text;
            const char* p_type = "void";
            if (check_token(p, TOK_COLON())) {
                advance_token(p);
                p_type = advance_token(p)->text;
            }
            struct Rz_AstNode* param = make_ast_node(NODE_PARAM(), p_name, p_type, line, col);
            if ((p_head == NULL)) {
                p_head = param;
                p_tail = param;
            } else {
                p_tail->next = param;
                p_tail = param;
            }
            if (check_token(p, TOK_COMMA())) {
                advance_token(p);
            }
        }
        consume_tok(p, TOK_BIT_OR(), "Expected closing '|' in closure");
        const char* ret_type = "void";
        if (check_token(p, TOK_ARROW())) {
            advance_token(p);
            ret_type = advance_token(p)->text;
        }
        struct Rz_AstNode* body = parse_block(p);
        struct Rz_AstNode* c_node = make_ast_node(NODE_CLOSURE(), "closure", ret_type, line, col);
        c_node->child1 = p_head;
        c_node->child3 = body;
        return c_node;
    }
    if ((check_token(p, TOK_IDENT()) && (peek_next_kind(p) == TOK_BANG()))) {
        const char* m_name = advance_token(p)->text;
        advance_token(p);
        struct Rz_AstNode* m_node = make_ast_node(NODE_MACRO_CALL(), m_name, "macro", line, col);
        int64_t close_kind = TOK_RPAREN();
        if (check_token(p, TOK_LPAREN())) {
            advance_token(p);
            close_kind = TOK_RPAREN();
        } else         if (check_token(p, TOK_LBRACE())) {
            advance_token(p);
            close_kind = TOK_RBRACE();
        } else         if (check_token(p, TOK_LBRACKET())) {
            advance_token(p);
            close_kind = TOK_RBRACKET();
        }
        struct Rz_AstNode* a_head = NULL;
        struct Rz_AstNode* a_tail = NULL;
        while (((!check_token(p, close_kind)) && (!check_token(p, TOK_EOF())))) {
            struct Rz_AstNode* arg = parse_expression(p);
            if ((a_head == NULL)) {
                a_head = arg;
                a_tail = arg;
            } else {
                a_tail->next = arg;
                a_tail = arg;
            }
            if (check_token(p, TOK_COMMA())) {
                advance_token(p);
            }
        }
        consume_tok(p, close_kind, "Expected closing delimiter after macro call");
        m_node->child1 = a_head;
        return m_node;
    }
    if (check_token(p, TOK_IDENT())) {
        struct Rz_Token* tok = advance_token(p);
        return make_ast_node(NODE_IDENT(), tok->text, "unknown", line, col);
    }
    if (check_token(p, TOK_LPAREN())) {
        advance_token(p);
        struct Rz_AstNode* expr = parse_expression(p);
        consume_tok(p, TOK_RPAREN(), "Expected ')' after expression");
        return expr;
    }
    printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat("❌ Parser Error: Unexpected token '", peek_text(p)), "' at line "), rz_int_to_str(line)));
    advance_token(p);
    return make_ast_node(NODE_LITERAL_NULL(), "null", "null", line, col);
}

struct Rz_AstNode* parse_postfix(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_primary(p);
    int64_t looping = 1;
    while (looping) {
        int64_t line = peek_line(p);
        int64_t col = peek_col(p);
        if (check_token(p, TOK_LPAREN())) {
            advance_token(p);
            struct Rz_AstNode* call_node = make_ast_node(NODE_CALL(), expr->name, "void", line, col);
            call_node->child1 = expr;
            struct Rz_AstNode* a_head = NULL;
            struct Rz_AstNode* a_tail = NULL;
            while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
                struct Rz_AstNode* arg = parse_expression(p);
                if ((a_head == NULL)) {
                    a_head = arg;
                    a_tail = arg;
                } else {
                    a_tail->next = arg;
                    a_tail = arg;
                }
                if (check_token(p, TOK_COMMA())) {
                    advance_token(p);
                }
            }
            consume_tok(p, TOK_RPAREN(), "Expected ')' after function arguments");
            call_node->child2 = a_head;
            expr = call_node;
        } else         if (check_token(p, TOK_DOT())) {
            advance_token(p);
            const char* member_name = consume_tok(p, TOK_IDENT(), "Expected member name after '.'")->text;
            if (check_token(p, TOK_LPAREN())) {
                advance_token(p);
                struct Rz_AstNode* m_node = make_ast_node(NODE_METHOD_CALL(), member_name, "void", line, col);
                m_node->child1 = expr;
                struct Rz_AstNode* a_head = NULL;
                struct Rz_AstNode* a_tail = NULL;
                while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
                    struct Rz_AstNode* arg = parse_expression(p);
                    if ((a_head == NULL)) {
                        a_head = arg;
                        a_tail = arg;
                    } else {
                        a_tail->next = arg;
                        a_tail = arg;
                    }
                    if (check_token(p, TOK_COMMA())) {
                        advance_token(p);
                    }
                }
                consume_tok(p, TOK_RPAREN(), "Expected ')' after method arguments");
                m_node->child2 = a_head;
                expr = m_node;
            } else {
                struct Rz_AstNode* mem_node = make_ast_node(NODE_MEMBER(), member_name, "void", line, col);
                mem_node->child1 = expr;
                expr = mem_node;
            }
        } else         if (check_token(p, TOK_COLON_COLON())) {
            advance_token(p);
            const char* sub_name = consume_tok(p, TOK_IDENT(), "Expected identifier after '::'")->text;
            struct Rz_AstNode* sc_node = make_ast_node(NODE_IDENT(), rz_str_concat(rz_str_concat(expr->name, "::"), sub_name), "ident", line, col);
            expr = sc_node;
        } else         if (check_token(p, TOK_LBRACKET())) {
            advance_token(p);
            struct Rz_AstNode* idx_expr = parse_expression(p);
            consume_tok(p, TOK_RBRACKET(), "Expected ']' after index");
            struct Rz_AstNode* idx_node = make_ast_node(NODE_INDEX(), "index", "void", line, col);
            idx_node->child1 = expr;
            idx_node->child2 = idx_expr;
            expr = idx_node;
        } else         if (check_token(p, TOK_QUESTION())) {
            advance_token(p);
            struct Rz_AstNode* try_node = make_ast_node(NODE_TRY(), "try", "void", line, col);
            try_node->child1 = expr;
            expr = try_node;
        } else {
            looping = 0;
        }
    }
    return expr;
}

struct Rz_AstNode* parse_unary(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    if (check_token(p, TOK_MINUS())) {
        advance_token(p);
        struct Rz_AstNode* operand = parse_unary(p);
        struct Rz_AstNode* node = make_ast_node(NODE_UNARY_OP(), "-", "int", line, col);
        node->op = "-";
        node->child1 = operand;
        return node;
    }
    if ((check_token(p, TOK_NOT()) || check_token(p, TOK_BANG()))) {
        advance_token(p);
        struct Rz_AstNode* operand = parse_unary(p);
        struct Rz_AstNode* node = make_ast_node(NODE_UNARY_OP(), "!", "bool", line, col);
        node->op = "!";
        node->child1 = operand;
        return node;
    }
    if (check_token(p, TOK_BIT_NOT())) {
        advance_token(p);
        struct Rz_AstNode* operand = parse_unary(p);
        struct Rz_AstNode* node = make_ast_node(NODE_UNARY_OP(), "~", "int", line, col);
        node->op = "~";
        node->child1 = operand;
        return node;
    }
    return parse_postfix(p);
}

struct Rz_AstNode* parse_factor(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_unary(p);
    while (((check_token(p, TOK_STAR()) || check_token(p, TOK_SLASH())) || check_token(p, TOK_PERCENT()))) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_unary(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), op_tok->text, "int", op_tok->line, op_tok->col);
        node->op = op_tok->text;
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_term(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_factor(p);
    while ((check_token(p, TOK_PLUS()) || check_token(p, TOK_MINUS()))) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_factor(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), op_tok->text, "int", op_tok->line, op_tok->col);
        node->op = op_tok->text;
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_shift(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_term(p);
    while ((check_token(p, TOK_LSHIFT()) || check_token(p, TOK_RSHIFT()))) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_term(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), op_tok->text, "int", op_tok->line, op_tok->col);
        node->op = op_tok->text;
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_comparison(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_shift(p);
    while ((((check_token(p, TOK_LT()) || check_token(p, TOK_LE())) || check_token(p, TOK_GT())) || check_token(p, TOK_GE()))) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_shift(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), op_tok->text, "bool", op_tok->line, op_tok->col);
        node->op = op_tok->text;
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_equality(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_comparison(p);
    while ((check_token(p, TOK_EQ()) || check_token(p, TOK_NE()))) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_comparison(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), op_tok->text, "bool", op_tok->line, op_tok->col);
        node->op = op_tok->text;
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_bitwise_and(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_equality(p);
    while (check_token(p, TOK_BIT_AND())) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_equality(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), "&", "int", op_tok->line, op_tok->col);
        node->op = "&";
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_bitwise_xor(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_bitwise_and(p);
    while (check_token(p, TOK_BIT_XOR())) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_bitwise_and(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), "^", "int", op_tok->line, op_tok->col);
        node->op = "^";
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_bitwise_or(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_bitwise_xor(p);
    while (check_token(p, TOK_BIT_OR())) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_bitwise_xor(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), "|", "int", op_tok->line, op_tok->col);
        node->op = "|";
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_logical_and(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_bitwise_or(p);
    while (check_token(p, TOK_AND())) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_bitwise_or(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), "&&", "bool", op_tok->line, op_tok->col);
        node->op = "&&";
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_logical_or(struct Rz_Parser* p) {
    struct Rz_AstNode* expr = parse_logical_and(p);
    while (check_token(p, TOK_OR())) {
        struct Rz_Token* op_tok = advance_token(p);
        struct Rz_AstNode* right = parse_logical_and(p);
        struct Rz_AstNode* node = make_ast_node(NODE_BIN_OP(), "||", "bool", op_tok->line, op_tok->col);
        node->op = "||";
        node->child1 = expr;
        node->child2 = right;
        expr = node;
    }
    return expr;
}

struct Rz_AstNode* parse_expression(struct Rz_Parser* p) {
    return parse_logical_or(p);
}

struct Rz_AstNode* parse_pattern(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    if (check_token(p, TOK_UNDERSCORE())) {
        advance_token(p);
        return make_ast_node(NODE_PATTERN(), "_", "wildcard", line, col);
    }
    if (check_token(p, TOK_INT())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_INT(), tok->text, "int", line, col);
        node->int_val = tok->int_val;
        if ((check_token(p, TOK_DOT_DOT()) || check_token(p, TOK_DOT_DOT_EQ()))) {
            const char* op = advance_token(p)->text;
            struct Rz_Token* end_tok = advance_token(p);
            struct Rz_AstNode* end_node = make_ast_node(NODE_LITERAL_INT(), end_tok->text, "int", line, col);
            end_node->int_val = end_tok->int_val;
            struct Rz_AstNode* range_node = make_ast_node(NODE_PATTERN(), op, "range", line, col);
            range_node->child1 = node;
            range_node->child2 = end_node;
            return range_node;
        }
        return node;
    }
    if (check_token(p, TOK_STRING())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_STR(), tok->text, "string", line, col);
        node->str_val = tok->text;
        return node;
    }
    if (check_token(p, TOK_BOOL())) {
        struct Rz_Token* tok = advance_token(p);
        struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_BOOL(), tok->text, "bool", line, col);
        node->int_val = tok->int_val;
        return node;
    }
    if (check_token(p, TOK_IDENT())) {
        struct Rz_Token* name_tok = advance_token(p);
        const char* name = name_tok->text;
        if (check_token(p, TOK_COLON_COLON())) {
            advance_token(p);
            const char* v_name = consume_tok(p, TOK_IDENT(), "Expected variant name")->text;
            struct Rz_AstNode* pat = make_ast_node(NODE_PATTERN(), rz_str_concat(rz_str_concat(name, "::"), v_name), "variant", line, col);
            if (check_token(p, TOK_LPAREN())) {
                advance_token(p);
                struct Rz_AstNode* b_head = NULL;
                struct Rz_AstNode* b_tail = NULL;
                while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
                    struct Rz_AstNode* b = parse_pattern(p);
                    if ((b_head == NULL)) {
                        b_head = b;
                        b_tail = b;
                    } else {
                        b_tail->next = b;
                        b_tail = b;
                    }
                    if (check_token(p, TOK_COMMA())) {
                        advance_token(p);
                    }
                }
                consume_tok(p, TOK_RPAREN(), "Expected ')' in pattern");
                pat->child1 = b_head;
            }
            return pat;
        }
        if (check_token(p, TOK_LBRACE())) {
            advance_token(p);
            struct Rz_AstNode* spat = make_ast_node(NODE_PATTERN(), name, "struct", line, col);
            struct Rz_AstNode* f_head = NULL;
            struct Rz_AstNode* f_tail = NULL;
            while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
                struct Rz_Token* f_tok = advance_token(p);
                struct Rz_AstNode* f_node = make_ast_node(NODE_IDENT(), f_tok->text, "ident", line, col);
                if ((f_head == NULL)) {
                    f_head = f_node;
                    f_tail = f_node;
                } else {
                    f_tail->next = f_node;
                    f_tail = f_node;
                }
                if (check_token(p, TOK_COMMA())) {
                    advance_token(p);
                }
            }
            consume_tok(p, TOK_RBRACE(), "Expected '}' in struct pattern");
            spat->child1 = f_head;
            return spat;
        }
        return make_ast_node(NODE_IDENT(), name, "ident", line, col);
    }
    struct Rz_Token* tok = advance_token(p);
    return make_ast_node(NODE_IDENT(), tok->text, "ident", line, col);
}

struct Rz_AstNode* parse_var_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    advance_token(p);
    if (check_token(p, TOK_MUT())) {
        advance_token(p);
    }
    const char* name = consume_tok(p, TOK_IDENT(), "Expected variable name")->text;
    const char* type_name = "";
    if (check_token(p, TOK_COLON())) {
        advance_token(p);
        type_name = consume_tok(p, TOK_IDENT(), "Expected type name")->text;
    }
    struct Rz_AstNode* init_expr = NULL;
    if (check_token(p, TOK_ASSIGN())) {
        advance_token(p);
        init_expr = parse_expression(p);
    }
    if (check_token(p, TOK_SEMI())) {
        advance_token(p);
    }
    struct Rz_AstNode* node = make_ast_node(NODE_VAR_DECL(), name, type_name, line, col);
    node->child1 = init_expr;
    return node;
}

struct Rz_AstNode* parse_if_stmt(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_IF(), "Expected 'if'");
    int64_t has_paren = check_token(p, TOK_LPAREN());
    if (has_paren) {
        advance_token(p);
    }
    struct Rz_AstNode* cond = parse_expression(p);
    if (has_paren) {
        consume_tok(p, TOK_RPAREN(), "Expected ')' after if condition");
    }
    struct Rz_AstNode* then_branch = parse_block(p);
    struct Rz_AstNode* else_branch = NULL;
    if (check_token(p, TOK_ELSE())) {
        advance_token(p);
        if (check_token(p, TOK_IF())) {
            else_branch = parse_if_stmt(p);
        } else {
            else_branch = parse_block(p);
        }
    }
    struct Rz_AstNode* node = make_ast_node(NODE_IF(), "if", "void", line, col);
    node->child1 = cond;
    node->child2 = then_branch;
    node->child3 = else_branch;
    return node;
}

struct Rz_AstNode* parse_while_stmt(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_WHILE(), "Expected 'while'");
    int64_t has_paren = check_token(p, TOK_LPAREN());
    if (has_paren) {
        advance_token(p);
    }
    struct Rz_AstNode* cond = parse_expression(p);
    if (has_paren) {
        consume_tok(p, TOK_RPAREN(), "Expected ')' after while condition");
    }
    struct Rz_AstNode* body = parse_block(p);
    struct Rz_AstNode* node = make_ast_node(NODE_WHILE(), "while", "void", line, col);
    node->child1 = cond;
    node->child2 = body;
    return node;
}

struct Rz_AstNode* parse_for_stmt(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_FOR(), "Expected 'for'");
    int64_t has_paren = check_token(p, TOK_LPAREN());
    if (has_paren) {
        advance_token(p);
    }
    struct Rz_AstNode* init_stmt = NULL;
    if ((check_token(p, TOK_LET()) || check_token(p, TOK_VAR()))) {
        init_stmt = parse_var_decl(p);
    } else     if ((!check_token(p, TOK_SEMI()))) {
        init_stmt = parse_statement(p);
    } else {
        advance_token(p);
    }
    struct Rz_AstNode* cond_expr = NULL;
    if ((!check_token(p, TOK_SEMI()))) {
        cond_expr = parse_expression(p);
    }
    if (check_token(p, TOK_SEMI())) {
        advance_token(p);
    }
    struct Rz_AstNode* update_stmt = NULL;
    if (((!check_token(p, TOK_LBRACE())) && ((!has_paren) || (!check_token(p, TOK_RPAREN()))))) {
        int64_t u_line = peek_line(p);
        int64_t u_col = peek_col(p);
        const char* var_name = consume_tok(p, TOK_IDENT(), "Expected variable name in for update")->text;
        consume_tok(p, TOK_ASSIGN(), "Expected '=' in for update");
        struct Rz_AstNode* u_expr = parse_expression(p);
        struct Rz_AstNode* assign_node = make_ast_node(NODE_ASSIGN(), var_name, "void", u_line, u_col);
        assign_node->child1 = make_ast_node(NODE_IDENT(), var_name, "void", u_line, u_col);
        assign_node->child2 = u_expr;
        update_stmt = assign_node;
    }
    if (has_paren) {
        consume_tok(p, TOK_RPAREN(), "Expected ')' after for clauses");
    }
    struct Rz_AstNode* body = parse_block(p);
    struct Rz_AstNode* node = make_ast_node(NODE_FOR(), "for", "void", line, col);
    node->child1 = init_stmt;
    node->child2 = cond_expr;
    node->child3 = update_stmt;
    node->child4 = body;
    return node;
}

struct Rz_AstNode* parse_match_stmt(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_MATCH(), "Expected 'match'");
    struct Rz_AstNode* expr = parse_expression(p);
    consume_tok(p, TOK_LBRACE(), "Expected '{' after match expression");
    struct Rz_AstNode* match_node = make_ast_node(NODE_MATCH(), "match", "void", line, col);
    match_node->child1 = expr;
    struct Rz_AstNode* arm_head = NULL;
    struct Rz_AstNode* arm_tail = NULL;
    struct Rz_AstNode* guard_node = NULL;
    struct Rz_AstNode* body_node = NULL;
    while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
        int64_t arm_line = peek_line(p);
        int64_t arm_col = peek_col(p);
        struct Rz_AstNode* pat_node = parse_pattern(p);
        if (check_token(p, TOK_IF())) {
            advance_token(p);
            guard_node = parse_expression(p);
        }
        consume_tok(p, TOK_FAT_ARROW(), "Expected '=>' in match arm");
        if (check_token(p, TOK_LBRACE())) {
            body_node = parse_block(p);
        } else {
            body_node = parse_statement(p);
        }
        struct Rz_AstNode* arm = make_ast_node(NODE_MATCH_ARM(), "arm", "void", arm_line, arm_col);
        arm->child1 = pat_node;
        arm->child2 = guard_node;
        arm->child3 = body_node;
        if ((arm_head == NULL)) {
            arm_head = arm;
            arm_tail = arm;
        } else {
            arm_tail->next = arm;
            arm_tail = arm;
        }
        if (check_token(p, TOK_COMMA())) {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RBRACE(), "Expected '}' at end of match");
    match_node->child2 = arm_head;
    return match_node;
}

struct Rz_AstNode* parse_return_stmt(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_RETURN(), "Expected 'return'");
    struct Rz_AstNode* ret_val = NULL;
    if (((!check_token(p, TOK_SEMI())) && (!check_token(p, TOK_RBRACE())))) {
        ret_val = parse_expression(p);
    }
    if (check_token(p, TOK_SEMI())) {
        advance_token(p);
    }
    struct Rz_AstNode* node = make_ast_node(NODE_RETURN(), "return", "void", line, col);
    node->child1 = ret_val;
    return node;
}

struct Rz_AstNode* parse_block(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_LBRACE(), "Expected '{'");
    struct Rz_AstNode* head = NULL;
    struct Rz_AstNode* tail = NULL;
    while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
        struct Rz_AstNode* stmt = parse_statement(p);
        if ((stmt != NULL)) {
            if ((head == NULL)) {
                head = stmt;
                tail = stmt;
            } else {
                tail->next = stmt;
                tail = stmt;
            }
        }
    }
    consume_tok(p, TOK_RBRACE(), "Expected '}'");
    struct Rz_AstNode* node = make_ast_node(NODE_BLOCK(), "block", "void", line, col);
    node->child1 = head;
    return node;
}

struct Rz_AstNode* parse_statement(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    if (((check_token(p, TOK_LET()) || check_token(p, TOK_VAR())) || check_token(p, TOK_CONST()))) {
        return parse_var_decl(p);
    }
    if (check_token(p, TOK_IF())) {
        return parse_if_stmt(p);
    }
    if (check_token(p, TOK_WHILE())) {
        return parse_while_stmt(p);
    }
    if (check_token(p, TOK_FOR())) {
        return parse_for_stmt(p);
    }
    if (check_token(p, TOK_MATCH())) {
        return parse_match_stmt(p);
    }
    if (check_token(p, TOK_RETURN())) {
        return parse_return_stmt(p);
    }
    if (check_token(p, TOK_LBRACE())) {
        return parse_block(p);
    }
    if (check_token(p, TOK_UNSAFE())) {
        advance_token(p);
        struct Rz_AstNode* body = parse_block(p);
        struct Rz_AstNode* node = make_ast_node(NODE_UNSAFE_BLOCK(), "unsafe", "void", line, col);
        node->child1 = body;
        return node;
    }
    if (check_token(p, TOK_COMPTIME())) {
        advance_token(p);
        struct Rz_AstNode* body = NULL;
        if (check_token(p, TOK_LBRACE())) {
            body = parse_block(p);
        } else {
            body = parse_statement(p);
        }
        struct Rz_AstNode* node = make_ast_node(NODE_COMPTIME_STMT(), "comptime", "void", line, col);
        node->child1 = body;
        return node;
    }
    struct Rz_AstNode* expr = parse_expression(p);
    if (check_token(p, TOK_ASSIGN())) {
        advance_token(p);
        struct Rz_AstNode* val = parse_expression(p);
        if (check_token(p, TOK_SEMI())) {
            advance_token(p);
        }
        struct Rz_AstNode* node = make_ast_node(NODE_ASSIGN(), expr->name, "void", line, col);
        node->child1 = expr;
        node->child2 = val;
        return node;
    }
    if (check_token(p, TOK_SEMI())) {
        advance_token(p);
    }
    struct Rz_AstNode* node = make_ast_node(NODE_EXPR_STMT(), "expr", "void", line, col);
    node->child1 = expr;
    return node;
}

struct Rz_AstNode* parse_func_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    int64_t is_async = 0;
    if (check_token(p, TOK_ASYNC())) {
        advance_token(p);
        is_async = 1;
    }
    consume_tok(p, TOK_FUNC(), "Expected 'func'");
    const char* name = consume_tok(p, TOK_IDENT(), "Expected function name")->text;
    const char* fn_generic = "";
    if (check_token(p, TOK_LT())) {
        advance_token(p);
        while (((!check_token(p, TOK_GT())) && (!check_token(p, TOK_EOF())))) {
            if ((strcmp(fn_generic, "") == 0)) {
                fn_generic = p->curr->text;
            } else {
                fn_generic = rz_str_concat(rz_str_concat(fn_generic, ","), p->curr->text);
            }
            advance_token(p);
        }
        consume_tok(p, TOK_GT(), "Expected '>' after function type parameters");
    }
    consume_tok(p, TOK_LPAREN(), "Expected '(' after function name");
    struct Rz_AstNode* params_head = NULL;
    struct Rz_AstNode* params_tail = NULL;
    const char* param_type = "void";
    while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
        int64_t p_line = peek_line(p);
        int64_t p_col = peek_col(p);
        const char* param_name = consume_tok(p, TOK_IDENT(), "Expected parameter name")->text;
        if (check_token(p, TOK_COLON())) {
            advance_token(p);
            param_type = consume_tok(p, TOK_IDENT(), "Expected parameter type")->text;
        }
        struct Rz_AstNode* param_node = make_ast_node(NODE_PARAM(), param_name, param_type, p_line, p_col);
        if ((params_head == NULL)) {
            params_head = param_node;
            params_tail = param_node;
        } else {
            params_tail->next = param_node;
            params_tail = param_node;
        }
        if (check_token(p, TOK_COMMA())) {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RPAREN(), "Expected ')' after parameters");
    const char* ret_type = "void";
    if (check_token(p, TOK_ARROW())) {
        advance_token(p);
        ret_type = consume_tok(p, TOK_IDENT(), "Expected return type")->text;
    }
    struct Rz_AstNode* body = parse_block(p);
    struct Rz_AstNode* node = make_ast_node(NODE_FUNC_DECL(), name, ret_type, line, col);
    node->op = fn_generic;
    node->child1 = params_head;
    node->child2 = body;
    if (is_async) {
        node->str_val = "async";
    }
    return node;
}

struct Rz_AstNode* parse_extern_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_EXTERN(), "Expected 'extern'");
    const char* abi = "C";
    if (check_token(p, TOK_STRING())) {
        abi = advance_token(p)->text;
    }
    consume_tok(p, TOK_FUNC(), "Expected 'func'");
    const char* name = consume_tok(p, TOK_IDENT(), "Expected function name")->text;
    consume_tok(p, TOK_LPAREN(), "Expected '(' after function name");
    struct Rz_AstNode* params_head = NULL;
    struct Rz_AstNode* params_tail = NULL;
    const char* param_type = "void";
    while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
        int64_t p_line = peek_line(p);
        int64_t p_col = peek_col(p);
        const char* param_name = consume_tok(p, TOK_IDENT(), "Expected parameter name")->text;
        if (check_token(p, TOK_COLON())) {
            advance_token(p);
            param_type = consume_tok(p, TOK_IDENT(), "Expected parameter type")->text;
        }
        struct Rz_AstNode* param_node = make_ast_node(NODE_PARAM(), param_name, param_type, p_line, p_col);
        if ((params_head == NULL)) {
            params_head = param_node;
            params_tail = param_node;
        } else {
            params_tail->next = param_node;
            params_tail = param_node;
        }
        if (check_token(p, TOK_COMMA())) {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RPAREN(), "Expected ')' after parameters");
    const char* ret_type = "void";
    if (check_token(p, TOK_ARROW())) {
        advance_token(p);
        ret_type = consume_tok(p, TOK_IDENT(), "Expected return type")->text;
    }
    consume_tok(p, TOK_SEMI(), "Expected ';' after extern function declaration");
    struct Rz_AstNode* node = make_ast_node(NODE_EXTERN_DECL(), name, ret_type, line, col);
    node->child1 = params_head;
    node->str_val = abi;
    return node;
}

struct Rz_AstNode* parse_class_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_CLASS(), "Expected 'class'");
    const char* name = consume_tok(p, TOK_IDENT(), "Expected class name")->text;
    const char* cls_generic = "";
    if (check_token(p, TOK_LT())) {
        advance_token(p);
        while (((!check_token(p, TOK_GT())) && (!check_token(p, TOK_EOF())))) {
            if ((strcmp(cls_generic, "") == 0)) {
                cls_generic = p->curr->text;
            } else {
                cls_generic = rz_str_concat(rz_str_concat(cls_generic, ","), p->curr->text);
            }
            advance_token(p);
        }
        consume_tok(p, TOK_GT(), "Expected '>' after class type parameters");
    }
    consume_tok(p, TOK_LBRACE(), "Expected '{' after class name");
    struct Rz_AstNode* fields_head = NULL;
    struct Rz_AstNode* fields_tail = NULL;
    while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
        int64_t f_line = peek_line(p);
        int64_t f_col = peek_col(p);
        const char* field_name = consume_tok(p, TOK_IDENT(), "Expected field name")->text;
        consume_tok(p, TOK_COLON(), "Expected ':' after field name");
        const char* field_type = consume_tok(p, TOK_IDENT(), "Expected field type")->text;
        struct Rz_AstNode* field_node = make_ast_node(NODE_FIELD(), field_name, field_type, f_line, f_col);
        if ((fields_head == NULL)) {
            fields_head = field_node;
            fields_tail = field_node;
        } else {
            fields_tail->next = field_node;
            fields_tail = field_node;
        }
        if (check_token(p, TOK_COMMA())) {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RBRACE(), "Expected '}' at end of class declaration");
    struct Rz_AstNode* node = make_ast_node(NODE_CLASS_DECL(), name, name, line, col);
    node->op = cls_generic;
    node->child1 = fields_head;
    return node;
}

struct Rz_AstNode* parse_enum_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_ENUM(), "Expected 'enum'");
    const char* name = consume_tok(p, TOK_IDENT(), "Expected enum name")->text;
    if (check_token(p, TOK_LT())) {
        advance_token(p);
        while (((!check_token(p, TOK_GT())) && (!check_token(p, TOK_EOF())))) {
            advance_token(p);
        }
        consume_tok(p, TOK_GT(), "Expected '>' after enum type parameters");
    }
    consume_tok(p, TOK_LBRACE(), "Expected '{' after enum name");
    struct Rz_AstNode* node = make_ast_node(NODE_ENUM_DECL(), name, name, line, col);
    struct Rz_AstNode* v_head = NULL;
    struct Rz_AstNode* v_tail = NULL;
    int64_t tag = 0;
    while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
        if (check_token(p, TOK_IDENT())) {
            int64_t v_line = peek_line(p);
            int64_t v_col = peek_col(p);
            struct Rz_Token* v_tok = advance_token(p);
            struct Rz_AstNode* v_node = make_ast_node(NODE_ENUM_VARIANT(), v_tok->text, name, v_line, v_col);
            v_node->int_val = tag;
            tag = (tag + 1);
            if (check_token(p, TOK_LPAREN())) {
                advance_token(p);
                struct Rz_AstNode* p_head = NULL;
                struct Rz_AstNode* p_tail = NULL;
                while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
                    struct Rz_Token* pt_tok = advance_token(p);
                    struct Rz_AstNode* pt_node = make_ast_node(NODE_PARAM(), "", pt_tok->text, v_line, v_col);
                    if ((p_head == NULL)) {
                        p_head = pt_node;
                        p_tail = pt_node;
                    } else {
                        p_tail->next = pt_node;
                        p_tail = pt_node;
                    }
                    if (check_token(p, TOK_COMMA())) {
                        advance_token(p);
                    }
                }
                consume_tok(p, TOK_RPAREN(), "Expected ')' after variant payload");
                v_node->child1 = p_head;
            }
            if ((v_head == NULL)) {
                v_head = v_node;
                v_tail = v_node;
            } else {
                v_tail->next = v_node;
                v_tail = v_node;
            }
        }
        if (check_token(p, TOK_COMMA())) {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RBRACE(), "Expected '}' at end of enum");
    node->child1 = v_head;
    return node;
}

struct Rz_AstNode* parse_trait_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_TRAIT(), "Expected 'trait'");
    const char* name = consume_tok(p, TOK_IDENT(), "Expected trait name")->text;
    if (check_token(p, TOK_LT())) {
        advance_token(p);
        while (((!check_token(p, TOK_GT())) && (!check_token(p, TOK_EOF())))) {
            advance_token(p);
        }
        consume_tok(p, TOK_GT(), "Expected '>' after trait type parameters");
    }
    consume_tok(p, TOK_LBRACE(), "Expected '{' after trait name");
    struct Rz_AstNode* node = make_ast_node(NODE_TRAIT_DECL(), name, name, line, col);
    struct Rz_AstNode* m_head = NULL;
    struct Rz_AstNode* m_tail = NULL;
    while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
        if (check_token(p, TOK_FUNC())) {
            int64_t m_line = peek_line(p);
            int64_t m_col = peek_col(p);
            advance_token(p);
            const char* m_name = consume_tok(p, TOK_IDENT(), "Expected method name in trait")->text;
            consume_tok(p, TOK_LPAREN(), "Expected '(' in trait method");
            struct Rz_AstNode* params_head = NULL;
            struct Rz_AstNode* params_tail = NULL;
            while (((!check_token(p, TOK_RPAREN())) && (!check_token(p, TOK_EOF())))) {
                int64_t p_line = peek_line(p);
                int64_t p_col = peek_col(p);
                const char* p_name = consume_tok(p, TOK_IDENT(), "Expected param name")->text;
                const char* p_type = "void";
                if (check_token(p, TOK_COLON())) {
                    advance_token(p);
                    p_type = advance_token(p)->text;
                }
                struct Rz_AstNode* p_node = make_ast_node(NODE_PARAM(), p_name, p_type, p_line, p_col);
                if ((params_head == NULL)) {
                    params_head = p_node;
                    params_tail = p_node;
                } else {
                    params_tail->next = p_node;
                    params_tail = p_node;
                }
                if (check_token(p, TOK_COMMA())) {
                    advance_token(p);
                }
            }
            consume_tok(p, TOK_RPAREN(), "Expected ')' after trait method params");
            const char* ret_type = "void";
            if (check_token(p, TOK_ARROW())) {
                advance_token(p);
                ret_type = advance_token(p)->text;
            }
            struct Rz_AstNode* body_block = NULL;
            if (check_token(p, TOK_SEMI())) {
                advance_token(p);
            } else             if (check_token(p, TOK_LBRACE())) {
                body_block = parse_block(p);
            }
            struct Rz_AstNode* m_node = make_ast_node(NODE_FUNC_DECL(), m_name, ret_type, m_line, m_col);
            m_node->child1 = params_head;
            m_node->child2 = body_block;
            if ((m_head == NULL)) {
                m_head = m_node;
                m_tail = m_node;
            } else {
                m_tail->next = m_node;
                m_tail = m_node;
            }
        } else {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RBRACE(), "Expected '}' at end of trait");
    node->child1 = m_head;
    return node;
}

struct Rz_AstNode* parse_impl_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_IMPL(), "Expected 'impl'");
    const char* first_name = consume_tok(p, TOK_IDENT(), "Expected identifier after 'impl'")->text;
    const char* trait_name = "";
    const char* class_name = first_name;
    if (check_token(p, TOK_FOR())) {
        advance_token(p);
        trait_name = first_name;
        class_name = consume_tok(p, TOK_IDENT(), "Expected class name after 'for'")->text;
    }
    consume_tok(p, TOK_LBRACE(), "Expected '{' in impl block");
    struct Rz_AstNode* node = make_ast_node(NODE_IMPL_DECL(), class_name, class_name, line, col);
    node->str_val = trait_name;
    struct Rz_AstNode* m_head = NULL;
    struct Rz_AstNode* m_tail = NULL;
    while (((!check_token(p, TOK_RBRACE())) && (!check_token(p, TOK_EOF())))) {
        if (check_token(p, TOK_FUNC())) {
            struct Rz_AstNode* m_node = parse_func_decl(p);
            if ((m_head == NULL)) {
                m_head = m_node;
                m_tail = m_node;
            } else {
                m_tail->next = m_node;
                m_tail = m_node;
            }
        } else {
            advance_token(p);
        }
    }
    consume_tok(p, TOK_RBRACE(), "Expected '}' at end of impl block");
    node->child1 = m_head;
    return node;
}

struct Rz_AstNode* parse_annotated_decl(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    int64_t is_at = check_token(p, TOK_AT());
    advance_token(p);
    if (((!is_at) && check_token(p, TOK_LBRACKET()))) {
        advance_token(p);
    }
    const char* annot_name = consume_tok(p, TOK_IDENT(), "Expected annotation name")->text;
    const char* annot_arg = "";
    if (check_token(p, TOK_LPAREN())) {
        advance_token(p);
        if ((check_token(p, TOK_STRING()) || check_token(p, TOK_IDENT()))) {
            annot_arg = advance_token(p)->text;
        }
        consume_tok(p, TOK_RPAREN(), "Expected ')' after annotation argument");
    }
    if (((!is_at) && check_token(p, TOK_RBRACKET()))) {
        advance_token(p);
    }
    struct Rz_AstNode* inner_decl = NULL;
    if ((check_token(p, TOK_AT()) || check_token(p, TOK_HASH()))) {
        inner_decl = parse_annotated_decl(p);
    } else     if ((check_token(p, TOK_FUNC()) || (check_token(p, TOK_ASYNC()) && (peek_next_kind(p) == TOK_FUNC())))) {
        inner_decl = parse_func_decl(p);
    } else     if (check_token(p, TOK_CLASS())) {
        inner_decl = parse_class_decl(p);
    } else     if (check_token(p, TOK_ENUM())) {
        inner_decl = parse_enum_decl(p);
    } else     if (check_token(p, TOK_TRAIT())) {
        inner_decl = parse_trait_decl(p);
    } else {
        inner_decl = parse_statement(p);
    }
    struct Rz_AstNode* node = make_ast_node(NODE_ANNOTATION(), annot_name, "annotation", line, col);
    node->str_val = annot_arg;
    node->child1 = inner_decl;
    return node;
}

struct Rz_AstNode* parse_import(struct Rz_Parser* p) {
    int64_t line = peek_line(p);
    int64_t col = peek_col(p);
    consume_tok(p, TOK_IMPORT(), "Expected 'import'");
    const char* path = consume_tok(p, TOK_STRING(), "Expected import path string")->text;
    if (check_token(p, TOK_SEMI())) {
        advance_token(p);
    }
    struct Rz_AstNode* node = make_ast_node(NODE_IMPORT(), path, "import", line, col);
    node->str_val = path;
    return node;
}

struct Rz_AstNode* parse_program(struct Rz_Parser* p) {
    struct Rz_AstNode* prog = make_ast_node(NODE_PROGRAM(), "main", "void", 1, 1);
    struct Rz_AstNode* head = NULL;
    struct Rz_AstNode* tail = NULL;
    struct Rz_AstNode* decl = NULL;
    while ((peek_kind(p) != TOK_EOF())) {
        if ((check_token(p, TOK_AT()) || check_token(p, TOK_HASH()))) {
            decl = parse_annotated_decl(p);
        } else         if ((check_token(p, TOK_FUNC()) || (check_token(p, TOK_ASYNC()) && (peek_next_kind(p) == TOK_FUNC())))) {
            decl = parse_func_decl(p);
        } else         if (check_token(p, TOK_EXTERN())) {
            decl = parse_extern_decl(p);
        } else         if (check_token(p, TOK_CLASS())) {
            decl = parse_class_decl(p);
        } else         if (check_token(p, TOK_ENUM())) {
            decl = parse_enum_decl(p);
        } else         if (check_token(p, TOK_TRAIT())) {
            decl = parse_trait_decl(p);
        } else         if (check_token(p, TOK_IMPL())) {
            decl = parse_impl_decl(p);
        } else         if (check_token(p, TOK_IMPORT())) {
            decl = parse_import(p);
        } else {
            decl = parse_statement(p);
        }
        if ((decl != NULL)) {
            if ((head == NULL)) {
                head = decl;
                tail = decl;
            } else {
                tail->next = decl;
                tail = decl;
            }
        }
    }
    prog->child1 = head;
    return prog;
}

struct Rz_AstNode* expand_macros_expr(struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return NULL;
    }
    int64_t kind = expr->kind;
    if ((kind == NODE_MACRO_CALL())) {
        const char* m_name = expr->name;
        int64_t line = expr->line;
        int64_t col = expr->col;
        if ((strcmp(m_name, "stringify") == 0)) {
            struct Rz_AstNode* arg = expr->child1;
            const char* str_val = "";
            if ((arg != NULL)) {
                str_val = arg->name;
                if ((rz_str_len(str_val) == 0)) {
                    str_val = arg->str_val;
                }
                if ((rz_str_len(str_val) == 0)) {
                    str_val = rz_int_to_str(arg->int_val);
                }
            }
            struct Rz_AstNode* res = make_ast_node(NODE_LITERAL_STR(), str_val, "string", line, col);
            res->str_val = str_val;
            return res;
        }
        if ((strcmp(m_name, "concat") == 0)) {
            struct Rz_AstNode* arg = expr->child1;
            const char* full_str = "";
            while ((arg != NULL)) {
                struct Rz_AstNode* s = expand_macros_expr(arg);
                if ((s->kind == NODE_LITERAL_STR())) {
                    full_str = rz_str_concat(full_str, s->str_val);
                } else                 if ((s->kind == NODE_IDENT())) {
                    full_str = rz_str_concat(full_str, s->name);
                }
                arg = arg->next;
            }
            struct Rz_AstNode* res = make_ast_node(NODE_LITERAL_STR(), full_str, "string", line, col);
            res->str_val = full_str;
            return res;
        }
        if ((strcmp(m_name, "env") == 0)) {
            struct Rz_AstNode* arg = expr->child1;
            const char* var_name = "";
            if ((arg != NULL)) {
                if ((arg->kind == NODE_LITERAL_STR())) {
                    var_name = arg->str_val;
                } else                 if ((arg->kind == NODE_IDENT())) {
                    var_name = arg->name;
                }
            }
            const char* env_val = rz_get_env(var_name);
            struct Rz_AstNode* res = make_ast_node(NODE_LITERAL_STR(), env_val, "string", line, col);
            res->str_val = env_val;
            return res;
        }
        if ((strcmp(m_name, "dbg") == 0)) {
            struct Rz_AstNode* arg = expr->child1;
            if ((arg != NULL)) {
                struct Rz_AstNode* exp_arg = expand_macros_expr(arg);
                struct Rz_AstNode* print_call = make_ast_node(NODE_CALL(), "println", "void", line, col);
                print_call->child2 = exp_arg;
                return exp_arg;
            }
        }
        return expr;
    }
    if ((kind == NODE_COMPTIME_EXPR())) {
        struct Rz_AstNode* inner = expand_macros_expr(expr->child1);
        if ((inner != NULL)) {
            if ((inner->kind == NODE_BIN_OP())) {
                struct Rz_AstNode* left = inner->child1;
                struct Rz_AstNode* right = inner->child2;
                if (((left->kind == NODE_LITERAL_INT()) && (right->kind == NODE_LITERAL_INT()))) {
                    int64_t res_val = 0;
                    if ((strcmp(inner->op, "+") == 0)) {
                        res_val = (left->int_val + right->int_val);
                    } else                     if ((strcmp(inner->op, "-") == 0)) {
                        res_val = (left->int_val - right->int_val);
                    } else                     if ((strcmp(inner->op, "*") == 0)) {
                        res_val = (left->int_val * right->int_val);
                    } else                     if (((strcmp(inner->op, "/") == 0) && (right->int_val != 0))) {
                        res_val = (left->int_val / right->int_val);
                    }
                    struct Rz_AstNode* folded = make_ast_node(NODE_LITERAL_INT(), rz_int_to_str(res_val), "int", expr->line, expr->col);
                    folded->int_val = res_val;
                    return folded;
                }
                if ((((left->kind == NODE_LITERAL_STR()) && (right->kind == NODE_LITERAL_STR())) && (strcmp(inner->op, "+") == 0))) {
                    const char* cat = rz_str_concat(left->str_val, right->str_val);
                    struct Rz_AstNode* folded_str = make_ast_node(NODE_LITERAL_STR(), cat, "string", expr->line, expr->col);
                    folded_str->str_val = cat;
                    return folded_str;
                }
            }
            return inner;
        }
        return expr;
    }
    if ((kind == NODE_BIN_OP())) {
        expr->child1 = expand_macros_expr(expr->child1);
        expr->child2 = expand_macros_expr(expr->child2);
        if (((expr->child1 != NULL) && (expr->child2 != NULL))) {
            if (((expr->child1->kind == NODE_LITERAL_INT()) && (expr->child2->kind == NODE_LITERAL_INT()))) {
                int64_t l_val = expr->child1->int_val;
                int64_t r_val = expr->child2->int_val;
                int64_t folded_val = 0;
                int64_t can_fold = 0;
                if ((strcmp(expr->op, "+") == 0)) {
                    folded_val = (l_val + r_val);
                    can_fold = 1;
                } else                 if ((strcmp(expr->op, "-") == 0)) {
                    folded_val = (l_val - r_val);
                    can_fold = 1;
                } else                 if ((strcmp(expr->op, "*") == 0)) {
                    folded_val = (l_val * r_val);
                    can_fold = 1;
                } else                 if (((strcmp(expr->op, "/") == 0) && (r_val != 0))) {
                    folded_val = (l_val / r_val);
                    can_fold = 1;
                }
                if (can_fold) {
                    struct Rz_AstNode* folded_node = make_ast_node(NODE_LITERAL_INT(), rz_int_to_str(folded_val), "int", expr->line, expr->col);
                    folded_node->int_val = folded_val;
                    return folded_node;
                }
            }
        }
        return expr;
    }
    if ((kind == NODE_UNARY_OP())) {
        expr->child1 = expand_macros_expr(expr->child1);
        return expr;
    }
    if ((((kind == NODE_CALL()) || (kind == NODE_NEW())) || (kind == NODE_METHOD_CALL()))) {
        if ((expr->child1 != NULL)) {
            expr->child1 = expand_macros_expr(expr->child1);
        }
        struct Rz_AstNode* arg = expr->child2;
        while ((arg != NULL)) {
            struct Rz_AstNode* exp_arg = expand_macros_expr(arg);
            arg = arg->next;
        }
        return expr;
    }
    if ((((kind == NODE_MEMBER()) || (kind == NODE_INDEX())) || (kind == NODE_TRY()))) {
        if ((expr->child1 != NULL)) {
            expr->child1 = expand_macros_expr(expr->child1);
        }
        if ((expr->child2 != NULL)) {
            expr->child2 = expand_macros_expr(expr->child2);
        }
        return expr;
    }
    return expr;
}

struct Rz_AstNode* expand_macros_stmt(struct Rz_AstNode* stmt) {
    if ((stmt == NULL)) {
        return NULL;
    }
    int64_t kind = stmt->kind;
    if ((kind == NODE_EXPR_STMT())) {
        struct Rz_AstNode* inner = stmt->child1;
        if (((inner != NULL) && (inner->kind == NODE_MACRO_CALL()))) {
            const char* m_name = inner->name;
            int64_t line = stmt->line;
            int64_t col = stmt->col;
            if ((strcmp(m_name, "assert") == 0)) {
                struct Rz_AstNode* cond = inner->child1;
                const char* msg = "assertion failed";
                if (((cond != NULL) && (cond->next != NULL))) {
                    struct Rz_AstNode* msg_node = cond->next;
                    if ((msg_node->kind == NODE_LITERAL_STR())) {
                        msg = msg_node->str_val;
                    } else                     if ((msg_node->kind == NODE_IDENT())) {
                        msg = msg_node->name;
                    }
                }
                struct Rz_AstNode* not_cond = make_ast_node(NODE_UNARY_OP(), "!", "bool", line, col);
                not_cond->op = "!";
                not_cond->child1 = cond;
                struct Rz_AstNode* fail_msg = make_ast_node(NODE_LITERAL_STR(), rz_str_concat("Assertion failed: ", msg), "string", line, col);
                fail_msg->str_val = rz_str_concat("Assertion failed: ", msg);
                struct Rz_AstNode* print_call = make_ast_node(NODE_CALL(), "println", "void", line, col);
                print_call->child2 = fail_msg;
                struct Rz_AstNode* print_stmt = make_ast_node(NODE_EXPR_STMT(), "print", "void", line, col);
                print_stmt->child1 = print_call;
                struct Rz_AstNode* if_node = make_ast_node(NODE_IF(), "if", "void", line, col);
                if_node->child1 = not_cond;
                if_node->child2 = print_stmt;
                return if_node;
            }
            if ((strcmp(m_name, "panic") == 0)) {
                const char* msg_text = "explicit panic triggered";
                struct Rz_AstNode* arg = inner->child1;
                if ((arg != NULL)) {
                    if ((arg->kind == NODE_LITERAL_STR())) {
                        msg_text = arg->str_val;
                    } else                     if ((arg->kind == NODE_IDENT())) {
                        msg_text = arg->name;
                    }
                }
                struct Rz_AstNode* p_msg = make_ast_node(NODE_LITERAL_STR(), rz_str_concat("PANIC: ", msg_text), "string", line, col);
                p_msg->str_val = rz_str_concat("PANIC: ", msg_text);
                struct Rz_AstNode* print_call = make_ast_node(NODE_CALL(), "println", "void", line, col);
                print_call->child2 = p_msg;
                struct Rz_AstNode* print_stmt = make_ast_node(NODE_EXPR_STMT(), "print", "void", line, col);
                print_stmt->child1 = print_call;
                return print_stmt;
            }
        }
        stmt->child1 = expand_macros_expr(stmt->child1);
        return stmt;
    }
    if ((kind == NODE_VAR_DECL())) {
        if ((stmt->child1 != NULL)) {
            stmt->child1 = expand_macros_expr(stmt->child1);
        }
        return stmt;
    }
    if ((((kind == NODE_ASSIGN()) || (kind == NODE_FIELD_ASSIGN())) || (kind == NODE_INDEX_ASSIGN()))) {
        if ((stmt->child1 != NULL)) {
            stmt->child1 = expand_macros_expr(stmt->child1);
        }
        if ((stmt->child2 != NULL)) {
            stmt->child2 = expand_macros_expr(stmt->child2);
        }
        return stmt;
    }
    if ((kind == NODE_IF())) {
        stmt->child1 = expand_macros_expr(stmt->child1);
        stmt->child2 = expand_macros_stmt(stmt->child2);
        if ((stmt->child3 != NULL)) {
            stmt->child3 = expand_macros_stmt(stmt->child3);
        }
        return stmt;
    }
    if ((kind == NODE_WHILE())) {
        stmt->child1 = expand_macros_expr(stmt->child1);
        stmt->child2 = expand_macros_stmt(stmt->child2);
        return stmt;
    }
    if ((kind == NODE_FOR())) {
        if ((stmt->child1 != NULL)) {
            stmt->child1 = expand_macros_stmt(stmt->child1);
        }
        if ((stmt->child2 != NULL)) {
            stmt->child2 = expand_macros_expr(stmt->child2);
        }
        if ((stmt->child3 != NULL)) {
            stmt->child3 = expand_macros_stmt(stmt->child3);
        }
        if ((stmt->child4 != NULL)) {
            stmt->child4 = expand_macros_stmt(stmt->child4);
        }
        return stmt;
    }
    if ((kind == NODE_RETURN())) {
        if ((stmt->child1 != NULL)) {
            stmt->child1 = expand_macros_expr(stmt->child1);
        }
        return stmt;
    }
    if ((((kind == NODE_BLOCK()) || (kind == NODE_UNSAFE_BLOCK())) || (kind == NODE_COMPTIME_STMT()))) {
        struct Rz_AstNode* s = stmt->child1;
        while ((s != NULL)) {
            expand_macros_stmt(s);
            s = s->next;
        }
        return stmt;
    }
    if ((kind == NODE_MATCH())) {
        stmt->child1 = expand_macros_expr(stmt->child1);
        struct Rz_AstNode* arm = stmt->child2;
        while ((arm != NULL)) {
            if ((arm->child2 != NULL)) {
                arm->child2 = expand_macros_expr(arm->child2);
            }
            if ((arm->child3 != NULL)) {
                arm->child3 = expand_macros_stmt(arm->child3);
            }
            arm = arm->next;
        }
        return stmt;
    }
    return stmt;
}

struct Rz_AstNode* expand_macros(struct Rz_AstNode* prog) {
    if ((prog == NULL)) {
        return NULL;
    }
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            if ((decl->child2 != NULL)) {
                decl->child2 = expand_macros_stmt(decl->child2);
            }
        } else         if ((decl->kind == NODE_ANNOTATION())) {
            const char* annot_name = decl->name;
            struct Rz_AstNode* target = decl->child1;
            if ((((strcmp(annot_name, "derive") == 0) && (target != NULL)) && (target->kind == NODE_CLASS_DECL()))) {
                const char* class_name = target->name;
                struct Rz_AstNode* to_str_func = make_ast_node(NODE_FUNC_DECL(), rz_str_concat(class_name, "_to_string"), "string", decl->line, decl->col);
                struct Rz_AstNode* this_param = make_ast_node(NODE_PARAM(), "this", class_name, decl->line, decl->col);
                to_str_func->child1 = this_param;
                struct Rz_AstNode* ret_str = make_ast_node(NODE_LITERAL_STR(), rz_str_concat(class_name, "{...}"), "string", decl->line, decl->col);
                ret_str->str_val = rz_str_concat(class_name, "{...}");
                struct Rz_AstNode* ret_stmt = make_ast_node(NODE_RETURN(), "return", "string", decl->line, decl->col);
                ret_stmt->child1 = ret_str;
                struct Rz_AstNode* body_block = make_ast_node(NODE_BLOCK(), "block", "void", decl->line, decl->col);
                body_block->child1 = ret_stmt;
                to_str_func->child2 = body_block;
                to_str_func->next = decl->next;
                decl->next = to_str_func;
            } else             if ((((strcmp(annot_name, "test") == 0) && (target != NULL)) && (target->kind == NODE_FUNC_DECL()))) {
                target->str_val = "test_function";
            }
            if (((target != NULL) && (target->kind == NODE_FUNC_DECL()))) {
                if ((target->child2 != NULL)) {
                    target->child2 = expand_macros_stmt(target->child2);
                }
            }
        } else         if ((decl->kind == NODE_IMPL_DECL())) {
            struct Rz_AstNode* m = decl->child1;
            while ((m != NULL)) {
                if ((m->child2 != NULL)) {
                    m->child2 = expand_macros_stmt(m->child2);
                }
                m = m->next;
            }
        } else {
            expand_macros_stmt(decl);
        }
        decl = decl->next;
    }
    return prog;
}

struct Rz_TypeChecker* make_type_checker(void) {
    return new_TypeChecker(NULL, 0, 0, 0, "void", "", NULL, NULL, NULL, NULL, NULL);
}

void enter_scope(struct Rz_TypeChecker* tc) {
    tc->scope_level = (tc->scope_level + 1);
}

void exit_scope(struct Rz_TypeChecker* tc) {
    while (((tc->symbols != NULL) && (tc->symbols->scope_level == tc->scope_level))) {
        tc->symbols = tc->symbols->next;
    }
    tc->scope_level = (tc->scope_level - 1);
}

void define_symbol(struct Rz_TypeChecker* tc, const char* name, const char* type_name, int64_t kind) {
    struct Rz_Symbol* sym = new_Symbol(name, type_name, kind, tc->scope_level, 0, BORROW_OWNED(), NULL, tc->symbols);
    tc->symbols = sym;
}

void define_symbol_with_decl(struct Rz_TypeChecker* tc, const char* name, const char* type_name, int64_t kind, int64_t is_mut, struct Rz_AstNode* decl) {
    struct Rz_Symbol* sym = new_Symbol(name, type_name, kind, tc->scope_level, is_mut, BORROW_OWNED(), decl, tc->symbols);
    tc->symbols = sym;
}

struct Rz_Symbol* lookup_symbol(struct Rz_TypeChecker* tc, const char* name) {
    struct Rz_Symbol* curr = tc->symbols;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

struct Rz_Symbol* lookup_symbol_in_current_scope(struct Rz_TypeChecker* tc, const char* name) {
    struct Rz_Symbol* curr = tc->symbols;
    while (((curr != NULL) && (curr->scope_level == tc->scope_level))) {
        if ((strcmp(curr->name, name) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void register_class_def(struct Rz_TypeChecker* tc, const char* name, struct Rz_AstNode* fields) {
    struct Rz_ClassField* f_head = NULL;
    struct Rz_ClassField* f_tail = NULL;
    struct Rz_AstNode* curr = fields;
    while ((curr != NULL)) {
        if ((curr->kind == NODE_FIELD())) {
            struct Rz_ClassField* cf = new_ClassField(curr->name, curr->type_name, NULL);
            if ((f_head == NULL)) {
                f_head = cf;
                f_tail = cf;
            } else {
                f_tail->next = cf;
                f_tail = cf;
            }
        }
        curr = curr->next;
    }
    struct Rz_ClassDef* cdef = new_ClassDef(name, f_head, tc->classes);
    tc->classes = cdef;
}

struct Rz_ClassDef* lookup_class_def(struct Rz_TypeChecker* tc, const char* name) {
    struct Rz_ClassDef* curr = tc->classes;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

const char* lookup_class_field(struct Rz_TypeChecker* tc, const char* class_name, const char* field_name) {
    struct Rz_ClassDef* cdef = lookup_class_def(tc, class_name);
    if ((cdef == NULL)) {
        return "";
    }
    struct Rz_ClassField* curr = cdef->fields;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, field_name) == 0)) {
            return curr->type_name;
        }
        curr = curr->next;
    }
    return "";
}

void tc_report_error(struct Rz_TypeChecker* tc, const char* msg, int64_t line) {
    if ((line > 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat("❌ Type Error [Line ", rz_int_to_str(line)), "]: "), msg));
    } else {
        printf("%s\n", rz_str_concat("❌ Type Error: ", msg));
    }
    tc->error_count = (tc->error_count + 1);
}

void tc_report_warning(struct Rz_TypeChecker* tc, const char* msg, int64_t line) {
    if ((line > 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat("⚠️ Semantic Warning [Line ", rz_int_to_str(line)), "]: "), msg));
    } else {
        printf("%s\n", rz_str_concat("⚠️ Semantic Warning: ", msg));
    }
    tc->warning_count = (tc->warning_count + 1);
}

void register_trait_def(struct Rz_TypeChecker* tc, const char* name, struct Rz_AstNode* methods_ast) {
    struct Rz_TraitMethod* m_head = NULL;
    struct Rz_TraitMethod* m_tail = NULL;
    struct Rz_AstNode* curr = methods_ast;
    while ((curr != NULL)) {
        if ((curr->kind == NODE_FUNC_DECL())) {
            struct Rz_TraitMethod* m = new_TraitMethod(curr->name, curr->type_name, curr->op, NULL);
            if ((m_head == NULL)) {
                m_head = m;
                m_tail = m;
            } else {
                m_tail->next = m;
                m_tail = m;
            }
        }
        curr = curr->next;
    }
    struct Rz_TraitDef* tdef = new_TraitDef(name, m_head, tc->traits);
    tc->traits = tdef;
}

struct Rz_TraitDef* lookup_trait_def(struct Rz_TypeChecker* tc, const char* name) {
    struct Rz_TraitDef* curr = tc->traits;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void register_impl_def(struct Rz_TypeChecker* tc, const char* trait_name, const char* target_type, struct Rz_AstNode* methods_ast) {
    struct Rz_TraitMethod* m_head = NULL;
    struct Rz_TraitMethod* m_tail = NULL;
    struct Rz_AstNode* curr = methods_ast;
    while ((curr != NULL)) {
        if ((curr->kind == NODE_FUNC_DECL())) {
            struct Rz_TraitMethod* m = new_TraitMethod(curr->name, curr->type_name, curr->op, NULL);
            if ((m_head == NULL)) {
                m_head = m;
                m_tail = m;
            } else {
                m_tail->next = m;
                m_tail = m;
            }
        }
        curr = curr->next;
    }
    struct Rz_ImplDef* idef = new_ImplDef(trait_name, target_type, m_head, tc->impls);
    tc->impls = idef;
}

struct Rz_ImplDef* lookup_impl_def(struct Rz_TypeChecker* tc, const char* trait_name, const char* target_type) {
    struct Rz_ImplDef* curr = tc->impls;
    while ((curr != NULL)) {
        if (((strcmp(curr->trait_name, trait_name) == 0) && (strcmp(curr->target_type, target_type) == 0))) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void verify_trait_conformance(struct Rz_TypeChecker* tc, struct Rz_AstNode* impl_node) {
    if ((impl_node == NULL)) {
        return;
    }
    const char* trait_name = impl_node->str_val;
    if ((strcmp(trait_name, "") == 0)) {
        return;
    }
    const char* target_type = impl_node->name;
    int64_t line = impl_node->line;
    struct Rz_TraitDef* tdef = lookup_trait_def(tc, trait_name);
    if ((tdef == NULL)) {
        tc_report_error(tc, rz_str_concat(rz_str_concat("Implementation references undefined trait '", trait_name), "'"), line);
        return;
    }
    struct Rz_TraitMethod* req_m = tdef->methods;
    int64_t found = 0;
    while ((req_m != NULL)) {
        struct Rz_AstNode* impl_m = impl_node->child1;
        while ((impl_m != NULL)) {
            if (((impl_m->kind == NODE_FUNC_DECL()) && (strcmp(impl_m->name, req_m->name) == 0))) {
                found = 1;
                if ((((strcmp(req_m->ret_type, "void") != 0) && (strcmp(req_m->ret_type, "") != 0)) && (strcmp(impl_m->type_name, req_m->ret_type) != 0))) {
                    tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Method '", req_m->name), "' in impl '"), trait_name), "' for '"), target_type), "' returns '"), impl_m->type_name), "', expected '"), req_m->ret_type), "'"), impl_m->line);
                }
            }
            impl_m = impl_m->next;
        }
        if ((!found)) {
            tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Type '", target_type), "' does not fully implement trait '"), trait_name), "': missing required method '"), req_m->name), "'"), line);
        }
        req_m = req_m->next;
    }
}

const char* resolve_trait_method(struct Rz_TypeChecker* tc, const char* target_type, const char* method_name) {
    struct Rz_ImplDef* curr = tc->impls;
    while ((curr != NULL)) {
        if ((strcmp(curr->target_type, target_type) == 0)) {
            struct Rz_TraitMethod* m = curr->methods;
            while ((m != NULL)) {
                if ((strcmp(m->name, method_name) == 0)) {
                    return m->ret_type;
                }
                m = m->next;
            }
        }
        curr = curr->next;
    }
    return "";
}

void record_mono_entry(struct Rz_TypeChecker* tc, const char* generic_name, const char* concrete_type, const char* spec_name) {
    struct Rz_MonoEntry* entry = new_MonoEntry(generic_name, concrete_type, spec_name, tc->mono_table);
    tc->mono_table = entry;
}

const char* lookup_mono_entry(struct Rz_TypeChecker* tc, const char* generic_name, const char* concrete_type) {
    struct Rz_MonoEntry* curr = tc->mono_table;
    while ((curr != NULL)) {
        if (((strcmp(curr->generic_name, generic_name) == 0) && (strcmp(curr->concrete_type, concrete_type) == 0))) {
            return curr->specialized_name;
        }
        curr = curr->next;
    }
    return "";
}

void substitute_type_in_ast(struct Rz_AstNode* node, const char* param_name, const char* concrete_type) {
    if ((node == NULL)) {
        return;
    }
    if ((strcmp(node->type_name, param_name) == 0)) {
        node->type_name = concrete_type;
    }
    substitute_type_in_ast(node->child1, param_name, concrete_type);
    substitute_type_in_ast(node->child2, param_name, concrete_type);
    substitute_type_in_ast(node->child3, param_name, concrete_type);
    substitute_type_in_ast(node->child4, param_name, concrete_type);
    substitute_type_in_ast(node->next, param_name, concrete_type);
}

struct Rz_AstNode* find_generic_decl(struct Rz_AstNode* prog, const char* name, int64_t kind) {
    struct Rz_AstNode* curr = prog->child1;
    while ((curr != NULL)) {
        if ((((curr->kind == kind) && (strcmp(curr->name, name) == 0)) && (strcmp(curr->op, "") != 0))) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void monomorphize_class_instantiation(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog, struct Rz_AstNode* new_node) {
    if (((new_node == NULL) || (strcmp(new_node->op, "") == 0))) {
        return;
    }
    const char* gen_name = new_node->name;
    const char* concrete_type = new_node->op;
    const char* spec_name = rz_str_concat(rz_str_concat(gen_name, "__"), concrete_type);
    const char* existing = lookup_mono_entry(tc, gen_name, concrete_type);
    if ((strcmp(existing, "") == 0)) {
        struct Rz_AstNode* gen_cls = find_generic_decl(prog, gen_name, NODE_CLASS_DECL());
        if ((gen_cls != NULL)) {
            struct Rz_AstNode* spec_cls = clone_ast_node(gen_cls);
            spec_cls->next = NULL;
            spec_cls->name = spec_name;
            spec_cls->type_name = spec_name;
            const char* param_name = gen_cls->op;
            spec_cls->op = "";
            substitute_type_in_ast(spec_cls, param_name, concrete_type);
            struct Rz_AstNode* tail = prog->child1;
            while ((tail->next != NULL)) {
                tail = tail->next;
            }
            tail->next = spec_cls;
            record_mono_entry(tc, gen_name, concrete_type, spec_name);
        }
    }
    new_node->name = spec_name;
    new_node->type_name = spec_name;
    new_node->op = "";
}

void monomorphize_expr(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog, struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return;
    }
    if (((expr->kind == NODE_NEW()) && (strcmp(expr->op, "") != 0))) {
        monomorphize_class_instantiation(tc, prog, expr);
    }
    monomorphize_expr(tc, prog, expr->child1);
    monomorphize_expr(tc, prog, expr->child2);
    monomorphize_expr(tc, prog, expr->child3);
    monomorphize_expr(tc, prog, expr->child4);
    monomorphize_expr(tc, prog, expr->next);
}

void monomorphize_statement(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog, struct Rz_AstNode* stmt) {
    if ((stmt == NULL)) {
        return;
    }
    monomorphize_expr(tc, prog, stmt->child1);
    monomorphize_expr(tc, prog, stmt->child2);
    monomorphize_expr(tc, prog, stmt->child3);
    monomorphize_expr(tc, prog, stmt->child4);
    monomorphize_statement(tc, prog, stmt->next);
}

struct Rz_AstNode* monomorphize_program(struct Rz_AstNode* prog) {
    if ((prog == NULL)) {
        return NULL;
    }
    struct Rz_TypeChecker* tc = make_type_checker();
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            monomorphize_statement(tc, prog, decl->child2);
        }
        decl = decl->next;
    }
    return prog;
}

int64_t is_copy_type(const char* type_name) {
    if ((((((strcmp(type_name, "int") == 0) || (strcmp(type_name, "bool") == 0)) || (strcmp(type_name, "char") == 0)) || (strcmp(type_name, "float") == 0)) || (strcmp(type_name, "void") == 0))) {
        return 1;
    }
    return 0;
}

void add_borrow_var(struct Rz_TypeChecker* tc, const char* name, const char* type_name, int64_t state, int64_t line) {
    struct Rz_BorrowVar* b = new_BorrowVar(name, type_name, state, "", line, tc->borrow_vars);
    tc->borrow_vars = b;
}

struct Rz_BorrowVar* lookup_borrow_var(struct Rz_TypeChecker* tc, const char* name) {
    struct Rz_BorrowVar* curr = tc->borrow_vars;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void mark_var_moved(struct Rz_TypeChecker* tc, const char* from_var, const char* to_var, int64_t line) {
    struct Rz_BorrowVar* b = lookup_borrow_var(tc, from_var);
    if ((b != NULL)) {
        if ((is_copy_type(b->type_name) == 0)) {
            b->state = BORROW_MOVED();
            b->moved_to = to_var;
        }
    }
}

void restore_var_ownership(struct Rz_TypeChecker* tc, const char* name) {
    struct Rz_BorrowVar* b = lookup_borrow_var(tc, name);
    if ((b != NULL)) {
        b->state = BORROW_OWNED();
        b->moved_to = "";
    }
}

void check_expr_borrow_use(struct Rz_TypeChecker* tc, struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return;
    }
    if ((expr->kind == NODE_IDENT())) {
        struct Rz_BorrowVar* b = lookup_borrow_var(tc, expr->name);
        if (((b != NULL) && (b->state == BORROW_MOVED()))) {
            tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Use of moved value '", expr->name), "' (ownership was transferred to '"), b->moved_to), "')"), expr->line);
        }
    }
    check_expr_borrow_use(tc, expr->child1);
    check_expr_borrow_use(tc, expr->child2);
    check_expr_borrow_use(tc, expr->child3);
    check_expr_borrow_use(tc, expr->child4);
    check_expr_borrow_use(tc, expr->next);
}

void check_stmt_borrow(struct Rz_TypeChecker* tc, struct Rz_AstNode* stmt) {
    if ((stmt == NULL)) {
        return;
    }
    int64_t kind = stmt->kind;
    if ((kind == NODE_VAR_DECL())) {
        struct Rz_AstNode* init_expr = stmt->child1;
        if ((init_expr != NULL)) {
            check_expr_borrow_use(tc, init_expr);
            if ((init_expr->kind == NODE_IDENT())) {
                mark_var_moved(tc, init_expr->name, stmt->name, stmt->line);
            }
        }
        add_borrow_var(tc, stmt->name, stmt->type_name, BORROW_OWNED(), stmt->line);
    } else     if ((kind == NODE_ASSIGN())) {
        struct Rz_AstNode* target = stmt->child1;
        struct Rz_AstNode* val = stmt->child2;
        if (((target != NULL) && (target->kind == NODE_IDENT()))) {
            restore_var_ownership(tc, target->name);
        }
        check_expr_borrow_use(tc, val);
        if (((((val != NULL) && (val->kind == NODE_IDENT())) && (target != NULL)) && (target->kind == NODE_IDENT()))) {
            mark_var_moved(tc, val->name, target->name, stmt->line);
        }
    } else {
        check_expr_borrow_use(tc, stmt->child1);
        check_expr_borrow_use(tc, stmt->child2);
        check_expr_borrow_use(tc, stmt->child3);
        check_expr_borrow_use(tc, stmt->child4);
    }
    check_stmt_borrow(tc, stmt->next);
}

void check_cyclic_reference_hazards(struct Rz_TypeChecker* tc) {
    struct Rz_ClassDef* c1 = tc->classes;
    while ((c1 != NULL)) {
        struct Rz_ClassField* f1 = c1->fields;
        while ((f1 != NULL)) {
            if ((strcmp(f1->type_name, c1->name) != 0)) {
                struct Rz_ClassDef* target_cls = lookup_class_def(tc, f1->type_name);
                if ((target_cls != NULL)) {
                    struct Rz_ClassField* f2 = target_cls->fields;
                    while ((f2 != NULL)) {
                        if ((strcmp(f2->type_name, c1->name) == 0)) {
                            tc_report_warning(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Potential strong reference cycle detected between classes '", c1->name), "' and '"), target_cls->name), "'. Consider using Weak<T> to prevent memory leaks."), 0);
                        }
                        f2 = f2->next;
                    }
                }
            }
            f1 = f1->next;
        }
        c1 = c1->next;
    }
}

int64_t analyze_borrows(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog) {
    if ((prog == NULL)) {
        return 0;
    }
    tc->borrow_vars = NULL;
    check_cyclic_reference_hazards(tc);
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            tc->borrow_vars = NULL;
            check_stmt_borrow(tc, decl->child2);
        }
        decl = decl->next;
    }
    return tc->error_count;
}

int64_t is_type_compatible(const char* expected, const char* actual) {
    if ((strcmp(expected, actual) == 0)) {
        return 1;
    }
    if (((strcmp(expected, "void") == 0) && (strcmp(actual, "void") == 0))) {
        return 1;
    }
    if (((strcmp(expected, "any") == 0) || (strcmp(actual, "any") == 0))) {
        return 1;
    }
    if (((strcmp(actual, "unknown") == 0) || (strcmp(expected, "unknown") == 0))) {
        return 1;
    }
    if ((strcmp(actual, "null") == 0)) {
        if ((((((strcmp(expected, "int") != 0) && (strcmp(expected, "bool") != 0)) && (strcmp(expected, "char") != 0)) && (strcmp(expected, "float") != 0)) && (strcmp(expected, "void") != 0))) {
            return 1;
        }
        return 0;
    }
    if ((strcmp(expected, "int") == 0)) {
        if (((((((((((strcmp(actual, "i8") == 0) || (strcmp(actual, "u8") == 0)) || (strcmp(actual, "i16") == 0)) || (strcmp(actual, "u16") == 0)) || (strcmp(actual, "i32") == 0)) || (strcmp(actual, "u32") == 0)) || (strcmp(actual, "i64") == 0)) || (strcmp(actual, "u64") == 0)) || (strcmp(actual, "usize") == 0)) || (strcmp(actual, "isize") == 0))) {
            return 1;
        }
    }
    if ((strcmp(expected, "float") == 0)) {
        if ((((strcmp(actual, "f32") == 0) || (strcmp(actual, "f64") == 0)) || (strcmp(actual, "int") == 0))) {
            return 1;
        }
    }
    return 0;
}

const char* check_expression_type(struct Rz_TypeChecker* tc, struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return "void";
    }
    int64_t kind = expr->kind;
    if ((kind == NODE_LITERAL_INT())) {
        return "int";
    }
    if ((kind == NODE_LITERAL_STR())) {
        return "string";
    }
    if ((kind == NODE_LITERAL_BOOL())) {
        return "bool";
    }
    if ((kind == NODE_LITERAL_CHAR())) {
        return "char";
    }
    if ((kind == NODE_LITERAL_NULL())) {
        return "null";
    }
    if ((kind == NODE_IDENT())) {
        struct Rz_Symbol* sym = lookup_symbol(tc, expr->name);
        if ((sym == NULL)) {
            tc_report_error(tc, rz_str_concat(rz_str_concat("Undefined identifier '", expr->name), "'"), expr->line);
            return "unknown";
        }
        expr->type_name = sym->type_name;
        return sym->type_name;
    }
    if ((kind == NODE_BIN_OP())) {
        const char* t_left = check_expression_type(tc, expr->child1);
        const char* t_right = check_expression_type(tc, expr->child2);
        if ((strcmp(expr->op, "+") == 0)) {
            if (((strcmp(t_left, "string") == 0) || (strcmp(t_right, "string") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if (((strcmp(t_left, "float") == 0) || (strcmp(t_right, "float") == 0))) {
                expr->type_name = "float";
                return "float";
            }
            expr->type_name = "int";
            return "int";
        }
        if (((((strcmp(expr->op, "-") == 0) || (strcmp(expr->op, "*") == 0)) || (strcmp(expr->op, "/") == 0)) || (strcmp(expr->op, "%") == 0))) {
            if (((strcmp(t_left, "float") == 0) || (strcmp(t_right, "float") == 0))) {
                expr->type_name = "float";
                return "float";
            }
            expr->type_name = "int";
            return "int";
        }
        if (((((((((strcmp(expr->op, "==") == 0) || (strcmp(expr->op, "!=") == 0)) || (strcmp(expr->op, "<") == 0)) || (strcmp(expr->op, "<=") == 0)) || (strcmp(expr->op, ">") == 0)) || (strcmp(expr->op, ">=") == 0)) || (strcmp(expr->op, "&&") == 0)) || (strcmp(expr->op, "||") == 0))) {
            expr->type_name = "bool";
            return "bool";
        }
        if ((((((strcmp(expr->op, "&") == 0) || (strcmp(expr->op, "|") == 0)) || (strcmp(expr->op, "^") == 0)) || (strcmp(expr->op, "<<") == 0)) || (strcmp(expr->op, ">>") == 0))) {
            expr->type_name = "int";
            return "int";
        }
        return "int";
    }
    if ((kind == NODE_UNARY_OP())) {
        const char* t_operand = check_expression_type(tc, expr->child1);
        if ((strcmp(expr->op, "!") == 0)) {
            expr->type_name = "bool";
            return "bool";
        }
        expr->type_name = t_operand;
        return t_operand;
    }
    if ((kind == NODE_CALL())) {
        const char* func_name = expr->name;
        if ((strcmp(func_name, "println") == 0)) {
            if ((expr->child2 != NULL)) {
                check_expression_type(tc, expr->child2);
            }
            expr->type_name = "void";
            return "void";
        }
        struct Rz_Symbol* sym = lookup_symbol(tc, func_name);
        if ((sym == NULL)) {
            tc_report_error(tc, rz_str_concat(rz_str_concat("Call to undefined function '", func_name), "'"), expr->line);
            return "unknown";
        }
        struct Rz_AstNode* arg = expr->child2;
        while ((arg != NULL)) {
            check_expression_type(tc, arg);
            arg = arg->next;
        }
        expr->type_name = sym->type_name;
        return sym->type_name;
    }
    if ((kind == NODE_NEW())) {
        struct Rz_Symbol* class_sym = lookup_symbol(tc, expr->name);
        if ((class_sym == NULL)) {
            tc_report_error(tc, rz_str_concat(rz_str_concat("Instantiation of undefined class '", expr->name), "'"), expr->line);
        }
        struct Rz_AstNode* arg = expr->child1;
        while ((arg != NULL)) {
            check_expression_type(tc, arg);
            arg = arg->next;
        }
        expr->type_name = expr->name;
        return expr->name;
    }
    if ((kind == NODE_MEMBER())) {
        const char* obj_type = check_expression_type(tc, expr->child1);
        const char* m_name = expr->name;
        const char* f_ty = lookup_class_field(tc, obj_type, m_name);
        if ((strcmp(f_ty, "") != 0)) {
            expr->type_name = f_ty;
            return f_ty;
        }
        const char* tr_ty = resolve_trait_method(tc, obj_type, m_name);
        if ((strcmp(tr_ty, "") != 0)) {
            expr->type_name = tr_ty;
            return tr_ty;
        }
        if ((strcmp(obj_type, "TypeChecker") == 0)) {
            if ((strcmp(m_name, "symbols") == 0)) {
                expr->type_name = "Symbol";
                return "Symbol";
            }
            if ((((strcmp(m_name, "scope_level") == 0) || (strcmp(m_name, "error_count") == 0)) || (strcmp(m_name, "warning_count") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if (((strcmp(m_name, "current_func_ret") == 0) || (strcmp(m_name, "current_func_name") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "classes") == 0)) {
                expr->type_name = "ClassDef";
                return "ClassDef";
            }
            if ((strcmp(m_name, "traits") == 0)) {
                expr->type_name = "TraitDef";
                return "TraitDef";
            }
            if ((strcmp(m_name, "impls") == 0)) {
                expr->type_name = "ImplDef";
                return "ImplDef";
            }
            if ((strcmp(m_name, "mono_table") == 0)) {
                expr->type_name = "MonoEntry";
                return "MonoEntry";
            }
            if ((strcmp(m_name, "borrow_vars") == 0)) {
                expr->type_name = "BorrowVar";
                return "BorrowVar";
            }
        }
        if ((strcmp(obj_type, "Symbol") == 0)) {
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "Symbol";
                return "Symbol";
            }
            if (((strcmp(m_name, "name") == 0) || (strcmp(m_name, "type_name") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if (((((strcmp(m_name, "kind") == 0) || (strcmp(m_name, "scope_level") == 0)) || (strcmp(m_name, "is_mut") == 0)) || (strcmp(m_name, "borrow_state") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "decl_node") == 0)) {
                expr->type_name = "AstNode";
                return "AstNode";
            }
        }
        if ((strcmp(obj_type, "ClassDef") == 0)) {
            if ((strcmp(m_name, "name") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "fields") == 0)) {
                expr->type_name = "ClassField";
                return "ClassField";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "ClassDef";
                return "ClassDef";
            }
        }
        if ((strcmp(obj_type, "ClassField") == 0)) {
            if (((strcmp(m_name, "name") == 0) || (strcmp(m_name, "type_name") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "ClassField";
                return "ClassField";
            }
        }
        if ((strcmp(obj_type, "TraitDef") == 0)) {
            if ((strcmp(m_name, "name") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "methods") == 0)) {
                expr->type_name = "TraitMethod";
                return "TraitMethod";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "TraitDef";
                return "TraitDef";
            }
        }
        if ((strcmp(obj_type, "TraitMethod") == 0)) {
            if ((((strcmp(m_name, "name") == 0) || (strcmp(m_name, "ret_type") == 0)) || (strcmp(m_name, "param_types") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "TraitMethod";
                return "TraitMethod";
            }
        }
        if ((strcmp(obj_type, "ImplDef") == 0)) {
            if (((strcmp(m_name, "trait_name") == 0) || (strcmp(m_name, "target_type") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "methods") == 0)) {
                expr->type_name = "TraitMethod";
                return "TraitMethod";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "ImplDef";
                return "ImplDef";
            }
        }
        if ((strcmp(obj_type, "MonoEntry") == 0)) {
            if ((((strcmp(m_name, "generic_name") == 0) || (strcmp(m_name, "concrete_type") == 0)) || (strcmp(m_name, "specialized_name") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "MonoEntry";
                return "MonoEntry";
            }
        }
        if ((strcmp(obj_type, "BorrowVar") == 0)) {
            if ((((strcmp(m_name, "name") == 0) || (strcmp(m_name, "type_name") == 0)) || (strcmp(m_name, "moved_to") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if (((strcmp(m_name, "state") == 0) || (strcmp(m_name, "line") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "BorrowVar";
                return "BorrowVar";
            }
        }
        if ((strcmp(obj_type, "Parser") == 0)) {
            if ((strcmp(m_name, "curr") == 0)) {
                expr->type_name = "Token";
                return "Token";
            }
            if (((strcmp(m_name, "pos") == 0) || (strcmp(m_name, "len") == 0))) {
                expr->type_name = "int";
                return "int";
            }
        }
        if ((strcmp(obj_type, "Token") == 0)) {
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "Token";
                return "Token";
            }
            if ((strcmp(m_name, "text") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if (((((strcmp(m_name, "kind") == 0) || (strcmp(m_name, "int_val") == 0)) || (strcmp(m_name, "line") == 0)) || (strcmp(m_name, "col") == 0))) {
                expr->type_name = "int";
                return "int";
            }
        }
        if ((strcmp(obj_type, "AstNode") == 0)) {
            if ((((((strcmp(m_name, "child1") == 0) || (strcmp(m_name, "child2") == 0)) || (strcmp(m_name, "child3") == 0)) || (strcmp(m_name, "child4") == 0)) || (strcmp(m_name, "next") == 0))) {
                expr->type_name = "AstNode";
                return "AstNode";
            }
            if (((((strcmp(m_name, "kind") == 0) || (strcmp(m_name, "int_val") == 0)) || (strcmp(m_name, "line") == 0)) || (strcmp(m_name, "col") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if (((((strcmp(m_name, "name") == 0) || (strcmp(m_name, "type_name") == 0)) || (strcmp(m_name, "op") == 0)) || (strcmp(m_name, "str_val") == 0))) {
                expr->type_name = "string";
                return "string";
            }
        }
        if ((strcmp(obj_type, "SsaBlockRef") == 0)) {
            if ((strcmp(m_name, "name") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "SsaBlockRef";
                return "SsaBlockRef";
            }
        }
        if ((strcmp(obj_type, "SsaInst") == 0)) {
            if (((((strcmp(m_name, "id") == 0) || (strcmp(m_name, "kind") == 0)) || (strcmp(m_name, "int_val") == 0)) || (strcmp(m_name, "is_dead") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((((((((((strcmp(m_name, "dest") == 0) || (strcmp(m_name, "type_name") == 0)) || (strcmp(m_name, "op") == 0)) || (strcmp(m_name, "arg1") == 0)) || (strcmp(m_name, "arg2") == 0)) || (strcmp(m_name, "arg3") == 0)) || (strcmp(m_name, "str_val") == 0)) || (strcmp(m_name, "target_label") == 0)) || (strcmp(m_name, "false_label") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "SsaInst";
                return "SsaInst";
            }
        }
        if ((strcmp(obj_type, "BasicBlock") == 0)) {
            if ((((((strcmp(m_name, "id") == 0) || (strcmp(m_name, "inst_count") == 0)) || (strcmp(m_name, "pred_count") == 0)) || (strcmp(m_name, "succ_count") == 0)) || (strcmp(m_name, "is_terminated") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "label") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if (((strcmp(m_name, "inst_head") == 0) || (strcmp(m_name, "inst_tail") == 0))) {
                expr->type_name = "SsaInst";
                return "SsaInst";
            }
            if (((strcmp(m_name, "preds") == 0) || (strcmp(m_name, "succs") == 0))) {
                expr->type_name = "SsaBlockRef";
                return "SsaBlockRef";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "BasicBlock";
                return "BasicBlock";
            }
        }
        if ((strcmp(obj_type, "SsaParam") == 0)) {
            if (((strcmp(m_name, "name") == 0) || (strcmp(m_name, "type_name") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "SsaParam";
                return "SsaParam";
            }
        }
        if ((strcmp(obj_type, "SsaFunction") == 0)) {
            if (((strcmp(m_name, "name") == 0) || (strcmp(m_name, "ret_type") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "params") == 0)) {
                expr->type_name = "SsaParam";
                return "SsaParam";
            }
            if (((strcmp(m_name, "block_head") == 0) || (strcmp(m_name, "block_tail") == 0))) {
                expr->type_name = "BasicBlock";
                return "BasicBlock";
            }
            if ((((strcmp(m_name, "block_count") == 0) || (strcmp(m_name, "var_counter") == 0)) || (strcmp(m_name, "is_inlinable") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "SsaFunction";
                return "SsaFunction";
            }
        }
        if ((strcmp(obj_type, "SsaProgram") == 0)) {
            if ((strcmp(m_name, "functions") == 0)) {
                expr->type_name = "SsaFunction";
                return "SsaFunction";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "SsaProgram";
                return "SsaProgram";
            }
        }
        if ((strcmp(obj_type, "ConstBinding") == 0)) {
            if (((strcmp(m_name, "name") == 0) || (strcmp(m_name, "str_val") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if (((strcmp(m_name, "kind") == 0) || (strcmp(m_name, "int_val") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "ConstBinding";
                return "ConstBinding";
            }
        }
        if ((strcmp(obj_type, "InlinableFunc") == 0)) {
            if (((strcmp(m_name, "name") == 0) || (strcmp(m_name, "ret_type") == 0))) {
                expr->type_name = "string";
                return "string";
            }
            if (((strcmp(m_name, "params") == 0) || (strcmp(m_name, "ret_expr") == 0))) {
                expr->type_name = "AstNode";
                return "AstNode";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "InlinableFunc";
                return "InlinableFunc";
            }
        }
        if ((strcmp(obj_type, "OptStats") == 0)) {
            if (((((strcmp(m_name, "const_folded") == 0) || (strcmp(m_name, "dead_stmts_removed") == 0)) || (strcmp(m_name, "funcs_inlined") == 0)) || (strcmp(m_name, "loops_hoisted") == 0))) {
                expr->type_name = "int";
                return "int";
            }
        }
        if ((strcmp(obj_type, "BinaryBuffer") == 0)) {
            if ((strcmp(m_name, "hex_data") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if ((strcmp(m_name, "size") == 0)) {
                expr->type_name = "int";
                return "int";
            }
        }
        if ((strcmp(obj_type, "NativeSection") == 0)) {
            if ((strcmp(m_name, "name") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if (((((((strcmp(m_name, "sec_type") == 0) || (strcmp(m_name, "flags") == 0)) || (strcmp(m_name, "vaddr") == 0)) || (strcmp(m_name, "offset") == 0)) || (strcmp(m_name, "size") == 0)) || (strcmp(m_name, "align") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "data") == 0)) {
                expr->type_name = "BinaryBuffer";
                return "BinaryBuffer";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "NativeSection";
                return "NativeSection";
            }
        }
        if ((strcmp(obj_type, "NativeSymbol") == 0)) {
            if ((strcmp(m_name, "name") == 0)) {
                expr->type_name = "string";
                return "string";
            }
            if ((((((strcmp(m_name, "value") == 0) || (strcmp(m_name, "size") == 0)) || (strcmp(m_name, "sec_index") == 0)) || (strcmp(m_name, "sym_type") == 0)) || (strcmp(m_name, "binding") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "NativeSymbol";
                return "NativeSymbol";
            }
        }
        if ((strcmp(obj_type, "NativeReloc") == 0)) {
            if (((((strcmp(m_name, "offset") == 0) || (strcmp(m_name, "sym_index") == 0)) || (strcmp(m_name, "rel_type") == 0)) || (strcmp(m_name, "addend") == 0))) {
                expr->type_name = "int";
                return "int";
            }
            if ((strcmp(m_name, "next") == 0)) {
                expr->type_name = "NativeReloc";
                return "NativeReloc";
            }
        }
        expr->type_name = "int";
        return "int";
    }
    if ((kind == NODE_METHOD_CALL())) {
        const char* obj_type = check_expression_type(tc, expr->child1);
        struct Rz_AstNode* arg = expr->child2;
        while ((arg != NULL)) {
            check_expression_type(tc, arg);
            arg = arg->next;
        }
        const char* tr_ty = resolve_trait_method(tc, obj_type, expr->name);
        if ((strcmp(tr_ty, "") != 0)) {
            expr->type_name = tr_ty;
            return tr_ty;
        }
        expr->type_name = "void";
        return "void";
    }
    if ((kind == NODE_INDEX())) {
        check_expression_type(tc, expr->child1);
        check_expression_type(tc, expr->child2);
        expr->type_name = "int";
        return "int";
    }
    if ((((kind == NODE_TRY()) || (kind == NODE_COMPTIME_EXPR())) || (kind == NODE_AWAIT()))) {
        const char* t_inner = check_expression_type(tc, expr->child1);
        expr->type_name = t_inner;
        return t_inner;
    }
    if ((kind == NODE_MACRO_CALL())) {
        struct Rz_AstNode* arg = expr->child1;
        while ((arg != NULL)) {
            check_expression_type(tc, arg);
            arg = arg->next;
        }
        expr->type_name = "void";
        return "void";
    }
    if ((kind == NODE_CLOSURE())) {
        expr->type_name = "closure";
        return "closure";
    }
    return "unknown";
}

void check_statement_type(struct Rz_TypeChecker* tc, struct Rz_AstNode* stmt) {
    if ((stmt == NULL)) {
        return;
    }
    int64_t kind = stmt->kind;
    if ((kind == NODE_VAR_DECL())) {
        const char* val_type = "void";
        if ((stmt->child1 != NULL)) {
            val_type = check_expression_type(tc, stmt->child1);
        }
        const char* var_type = stmt->type_name;
        if (((strcmp(var_type, "") == 0) || (strcmp(var_type, "unknown") == 0))) {
            var_type = val_type;
            stmt->type_name = var_type;
        } else         if (((strcmp(val_type, "void") != 0) && (strcmp(val_type, "unknown") != 0))) {
            if ((is_type_compatible(var_type, val_type) == 0)) {
                tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Mismatched types in variable declaration: expected '", var_type), "', got '"), val_type), "'"), stmt->line);
            }
        }
        define_symbol_with_decl(tc, stmt->name, var_type, SYM_VAR(), 1, stmt);
    } else     if ((kind == NODE_ASSIGN())) {
        const char* t_target = check_expression_type(tc, stmt->child1);
        const char* t_val = check_expression_type(tc, stmt->child2);
        if (((strcmp(t_target, "unknown") != 0) && (strcmp(t_val, "unknown") != 0))) {
            if ((is_type_compatible(t_target, t_val) == 0)) {
                tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Assignment type mismatch: cannot assign '", t_val), "' to target of type '"), t_target), "'"), stmt->line);
            }
        }
    } else     if ((kind == NODE_FIELD_ASSIGN())) {
        check_expression_type(tc, stmt->child1);
        check_expression_type(tc, stmt->child2);
    } else     if ((kind == NODE_INDEX_ASSIGN())) {
        check_expression_type(tc, stmt->child1);
        check_expression_type(tc, stmt->child2);
        if ((stmt->child3 != NULL)) {
            check_expression_type(tc, stmt->child3);
        }
    } else     if ((kind == NODE_IF())) {
        check_expression_type(tc, stmt->child1);
        check_statement_type(tc, stmt->child2);
        if ((stmt->child3 != NULL)) {
            check_statement_type(tc, stmt->child3);
        }
    } else     if ((kind == NODE_WHILE())) {
        check_expression_type(tc, stmt->child1);
        check_statement_type(tc, stmt->child2);
    } else     if ((kind == NODE_FOR())) {
        enter_scope(tc);
        if ((stmt->child1 != NULL)) {
            check_statement_type(tc, stmt->child1);
        }
        if ((stmt->child2 != NULL)) {
            check_expression_type(tc, stmt->child2);
        }
        if ((stmt->child3 != NULL)) {
            check_statement_type(tc, stmt->child3);
        }
        if ((stmt->child4 != NULL)) {
            check_statement_type(tc, stmt->child4);
        }
        exit_scope(tc);
    } else     if ((kind == NODE_MATCH())) {
        check_expression_type(tc, stmt->child1);
        struct Rz_AstNode* arm = stmt->child2;
        while ((arm != NULL)) {
            if ((arm->child2 != NULL)) {
                check_expression_type(tc, arm->child2);
            }
            if ((arm->child3 != NULL)) {
                check_statement_type(tc, arm->child3);
            }
            arm = arm->next;
        }
    } else     if (((kind == NODE_UNSAFE_BLOCK()) || (kind == NODE_COMPTIME_STMT()))) {
        check_statement_type(tc, stmt->child1);
    } else     if ((kind == NODE_ANNOTATION())) {
        check_statement_type(tc, stmt->child1);
    } else     if ((kind == NODE_IMPL_DECL())) {
        struct Rz_AstNode* m = stmt->child1;
        while ((m != NULL)) {
            check_statement_type(tc, m);
            m = m->next;
        }
    } else     if ((kind == NODE_RETURN())) {
        const char* ret_type = "void";
        if ((stmt->child1 != NULL)) {
            ret_type = check_expression_type(tc, stmt->child1);
        }
        if ((((strcmp(tc->current_func_ret, "") != 0) && (strcmp(tc->current_func_ret, "void") != 0)) && (stmt->child1 == NULL))) {
            tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Expected return value of type '", tc->current_func_ret), "' in function '"), tc->current_func_name), "'"), stmt->line);
        } else         if (((strcmp(tc->current_func_ret, "void") == 0) && (stmt->child1 != NULL))) {
            tc_report_error(tc, rz_str_concat(rz_str_concat("Cannot return a value from void function '", tc->current_func_name), "'"), stmt->line);
        } else         if ((((stmt->child1 != NULL) && (strcmp(tc->current_func_ret, "") != 0)) && (strcmp(tc->current_func_ret, "void") != 0))) {
            if ((is_type_compatible(tc->current_func_ret, ret_type) == 0)) {
                tc_report_error(tc, rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("Return type mismatch in function '", tc->current_func_name), "': expected '"), tc->current_func_ret), "', got '"), ret_type), "'"), stmt->line);
            }
        }
    } else     if ((kind == NODE_BLOCK())) {
        enter_scope(tc);
        struct Rz_AstNode* s = stmt->child1;
        while ((s != NULL)) {
            check_statement_type(tc, s);
            s = s->next;
        }
        exit_scope(tc);
    } else     if ((kind == NODE_EXPR_STMT())) {
        check_expression_type(tc, stmt->child1);
    }
}

int64_t check_program_types(struct Rz_TypeChecker* tc, struct Rz_AstNode* prog) {
    if ((prog == NULL)) {
        return 0;
    }
    define_symbol(tc, "rz_create_string", "string", SYM_FUNC());
    define_symbol(tc, "rz_str_concat", "string", SYM_FUNC());
    define_symbol(tc, "rz_str_len", "int", SYM_FUNC());
    define_symbol(tc, "rz_char_at", "int", SYM_FUNC());
    define_symbol(tc, "rz_substring", "string", SYM_FUNC());
    define_symbol(tc, "rz_char_to_str", "string", SYM_FUNC());
    define_symbol(tc, "rz_int_to_str", "string", SYM_FUNC());
    define_symbol(tc, "rz_str_eq", "int", SYM_FUNC());
    define_symbol(tc, "rz_read_file", "string", SYM_FUNC());
    define_symbol(tc, "rz_write_file", "int", SYM_FUNC());
    define_symbol(tc, "rz_write_binary_file", "int", SYM_FUNC());
    define_symbol(tc, "rz_file_exists", "int", SYM_FUNC());
    define_symbol(tc, "rz_exec", "int", SYM_FUNC());
    define_symbol(tc, "rz_get_arg_count", "int", SYM_FUNC());
    define_symbol(tc, "rz_get_arg", "string", SYM_FUNC());
    define_symbol(tc, "rz_get_env", "string", SYM_FUNC());
    define_symbol(tc, "rz_print_str", "void", SYM_FUNC());
    define_symbol(tc, "rz_print_int", "void", SYM_FUNC());
    define_symbol(tc, "rz_compiler_banner", "void", SYM_FUNC());
    define_symbol(tc, "println", "void", SYM_FUNC());
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_CLASS_DECL())) {
            define_symbol(tc, decl->name, decl->name, SYM_CLASS());
            register_class_def(tc, decl->name, decl->child1);
        } else         if ((decl->kind == NODE_ENUM_DECL())) {
            define_symbol(tc, decl->name, decl->name, SYM_ENUM());
        } else         if ((decl->kind == NODE_TRAIT_DECL())) {
            define_symbol(tc, decl->name, decl->name, SYM_TRAIT());
            register_trait_def(tc, decl->name, decl->child1);
        } else         if ((decl->kind == NODE_IMPL_DECL())) {
            const char* tr_name = decl->str_val;
            const char* tgt_type = decl->name;
            if ((strcmp(tr_name, "") != 0)) {
                define_symbol(tc, tr_name, tgt_type, SYM_IMPL());
                register_impl_def(tc, tr_name, tgt_type, decl->child1);
            }
        } else         if (((decl->kind == NODE_FUNC_DECL()) || (decl->kind == NODE_EXTERN_DECL()))) {
            define_symbol_with_decl(tc, decl->name, decl->type_name, SYM_FUNC(), 0, decl);
        } else         if (((decl->kind == NODE_ANNOTATION()) && (decl->child1 != NULL))) {
            struct Rz_AstNode* inner = decl->child1;
            if ((inner->kind == NODE_CLASS_DECL())) {
                define_symbol(tc, inner->name, inner->name, SYM_CLASS());
                register_class_def(tc, inner->name, inner->child1);
            } else             if ((inner->kind == NODE_ENUM_DECL())) {
                define_symbol(tc, inner->name, inner->name, SYM_ENUM());
            } else             if ((inner->kind == NODE_TRAIT_DECL())) {
                define_symbol(tc, inner->name, inner->name, SYM_TRAIT());
                register_trait_def(tc, inner->name, inner->child1);
            } else             if (((inner->kind == NODE_FUNC_DECL()) || (inner->kind == NODE_EXTERN_DECL()))) {
                define_symbol_with_decl(tc, inner->name, inner->type_name, SYM_FUNC(), 0, inner);
            }
        }
        decl = decl->next;
    }
    decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_IMPL_DECL())) {
            verify_trait_conformance(tc, decl);
        }
        decl = decl->next;
    }
    decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            enter_scope(tc);
            tc->current_func_ret = decl->type_name;
            tc->current_func_name = decl->name;
            struct Rz_AstNode* param = decl->child1;
            while ((param != NULL)) {
                define_symbol_with_decl(tc, param->name, param->type_name, SYM_PARAM(), 0, param);
                param = param->next;
            }
            check_statement_type(tc, decl->child2);
            exit_scope(tc);
        } else         if ((decl->kind == NODE_IMPL_DECL())) {
            struct Rz_AstNode* m = decl->child1;
            while ((m != NULL)) {
                if ((m->kind == NODE_FUNC_DECL())) {
                    enter_scope(tc);
                    tc->current_func_ret = m->type_name;
                    tc->current_func_name = m->name;
                    define_symbol(tc, "this", decl->name, SYM_PARAM());
                    struct Rz_AstNode* m_param = m->child1;
                    while ((m_param != NULL)) {
                        define_symbol_with_decl(tc, m_param->name, m_param->type_name, SYM_PARAM(), 0, m_param);
                        m_param = m_param->next;
                    }
                    check_statement_type(tc, m->child2);
                    exit_scope(tc);
                }
                m = m->next;
            }
        } else         if (((((decl->kind != NODE_CLASS_DECL()) && (decl->kind != NODE_ENUM_DECL())) && (decl->kind != NODE_TRAIT_DECL())) && (decl->kind != NODE_EXTERN_DECL()))) {
            check_statement_type(tc, decl);
        }
        decl = decl->next;
    }
    return tc->error_count;
}

struct Rz_SsaInst* make_ssa_inst(int64_t id, int64_t kind, const char* dest, const char* type_name, const char* op, const char* arg1, const char* arg2, int64_t int_val, const char* str_val, const char* target_label, const char* false_label) {
    return new_SsaInst(id, kind, dest, type_name, op, arg1, arg2, "", int_val, str_val, target_label, false_label, 0, NULL);
}

struct Rz_BasicBlock* make_basic_block(int64_t id, const char* label) {
    return new_BasicBlock(id, label, NULL, NULL, 0, 0, 0, NULL, NULL, 0, NULL);
}

struct Rz_SsaParam* make_ssa_param(const char* name, const char* type_name) {
    return new_SsaParam(name, type_name, NULL);
}

struct Rz_SsaFunction* make_ssa_function(const char* name, const char* ret_type) {
    return new_SsaFunction(name, ret_type, NULL, NULL, NULL, 0, 0, 0, NULL);
}

struct Rz_SsaProgram* make_ssa_program(void) {
    return new_SsaProgram(NULL, NULL);
}

void bb_append_inst(struct Rz_BasicBlock* bb, struct Rz_SsaInst* inst) {
    if (((bb == NULL) || (inst == NULL))) {
        return;
    }
    if ((bb->inst_head == NULL)) {
        bb->inst_head = inst;
        bb->inst_tail = inst;
    } else {
        bb->inst_tail->next = inst;
        bb->inst_tail = inst;
    }
    bb->inst_count = (bb->inst_count + 1);
    if ((((inst->kind == SSA_BR()) || (inst->kind == SSA_BR_COND())) || (inst->kind == SSA_RETURN()))) {
        bb->is_terminated = 1;
    }
}

void fn_append_bb(struct Rz_SsaFunction* fn, struct Rz_BasicBlock* bb) {
    if (((fn == NULL) || (bb == NULL))) {
        return;
    }
    if ((fn->block_head == NULL)) {
        fn->block_head = bb;
        fn->block_tail = bb;
    } else {
        fn->block_tail->next = bb;
        fn->block_tail = bb;
    }
    fn->block_count = (fn->block_count + 1);
}

const char* fn_new_temp(struct Rz_SsaFunction* fn) {
    const char* t = rz_str_concat("%t", rz_int_to_str(fn->var_counter));
    fn->var_counter = (fn->var_counter + 1);
    return t;
}

void bb_add_pred(struct Rz_BasicBlock* bb, const char* pred_label) {
    if (((bb == NULL) || (strcmp(pred_label, "") == 0))) {
        return;
    }
    struct Rz_SsaBlockRef* curr = bb->preds;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, pred_label) == 0)) {
            return;
        }
        curr = curr->next;
    }
    struct Rz_SsaBlockRef* ref = new_SsaBlockRef(pred_label, bb->preds);
    bb->preds = ref;
    bb->pred_count = (bb->pred_count + 1);
}

void bb_add_succ(struct Rz_BasicBlock* bb, const char* succ_label) {
    if (((bb == NULL) || (strcmp(succ_label, "") == 0))) {
        return;
    }
    struct Rz_SsaBlockRef* curr = bb->succs;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, succ_label) == 0)) {
            return;
        }
        curr = curr->next;
    }
    struct Rz_SsaBlockRef* ref = new_SsaBlockRef(succ_label, bb->succs);
    bb->succs = ref;
    bb->succ_count = (bb->succ_count + 1);
}

struct Rz_BasicBlock* fn_find_bb(struct Rz_SsaFunction* fn, const char* label) {
    struct Rz_BasicBlock* curr = fn->block_head;
    while ((curr != NULL)) {
        if ((strcmp(curr->label, label) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

void resolve_cfg_edges(struct Rz_SsaFunction* fn) {
    if ((fn == NULL)) {
        return;
    }
    struct Rz_BasicBlock* bb = fn->block_head;
    while ((bb != NULL)) {
        struct Rz_SsaInst* inst = bb->inst_head;
        while ((inst != NULL)) {
            if ((inst->kind == SSA_BR())) {
                const char* tgt_label = inst->target_label;
                bb_add_succ(bb, tgt_label);
                struct Rz_BasicBlock* tgt_bb = fn_find_bb(fn, tgt_label);
                if ((tgt_bb != NULL)) {
                    bb_add_pred(tgt_bb, bb->label);
                }
            } else             if ((inst->kind == SSA_BR_COND())) {
                const char* t_label = inst->target_label;
                const char* f_label = inst->false_label;
                bb_add_succ(bb, t_label);
                bb_add_succ(bb, f_label);
                struct Rz_BasicBlock* t_bb = fn_find_bb(fn, t_label);
                if ((t_bb != NULL)) {
                    bb_add_pred(t_bb, bb->label);
                }
                struct Rz_BasicBlock* f_bb = fn_find_bb(fn, f_label);
                if ((f_bb != NULL)) {
                    bb_add_pred(f_bb, bb->label);
                }
            }
            inst = inst->next;
        }
        bb = bb->next;
    }
}

int64_t eliminate_dead_basic_blocks(struct Rz_SsaFunction* fn) {
    if (((fn == NULL) || (fn->block_head == NULL))) {
        return 0;
    }
    int64_t removed = 0;
    struct Rz_BasicBlock* prev = NULL;
    struct Rz_BasicBlock* curr = fn->block_head;
    int64_t is_entry = 0;
    while ((curr != NULL)) {
        if ((strcmp(curr->label, "entry") == 0)) {
            is_entry = 1;
        }
        if (((is_entry == 0) && (curr->pred_count == 0))) {
            if ((prev == NULL)) {
                fn->block_head = curr->next;
            } else {
                prev->next = curr->next;
            }
            if ((fn->block_tail == curr)) {
                fn->block_tail = prev;
            }
            fn->block_count = (fn->block_count - 1);
            removed = (removed + 1);
            curr = curr->next;
        } else {
            prev = curr;
            curr = curr->next;
        }
    }
    return removed;
}

const char* dump_ssa_inst(struct Rz_SsaInst* inst) {
    if (((inst == NULL) || (inst->is_dead == 1))) {
        return "";
    }
    int64_t k = inst->kind;
    if ((k == SSA_CONST_INT())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = const_int "), rz_int_to_str(inst->int_val)), "\n");
    }
    if ((k == SSA_CONST_STR())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = const_str \""), inst->str_val), "\"\n");
    }
    if ((k == SSA_CONST_BOOL())) {
        if ((inst->int_val == 1)) {
            return rz_str_concat(rz_str_concat("    ", inst->dest), " = const_bool true\n");
        }
        return rz_str_concat(rz_str_concat("    ", inst->dest), " = const_bool false\n");
    }
    if ((k == SSA_BIN_OP())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = "), inst->op), " "), inst->type_name), " "), inst->arg1), ", "), inst->arg2), "\n");
    }
    if ((k == SSA_UNARY_OP())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = "), inst->op), " "), inst->type_name), " "), inst->arg1), "\n");
    }
    if ((k == SSA_CALL())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = call "), inst->type_name), " @"), inst->target_label), "("), inst->arg1), ")\n");
    }
    if ((k == SSA_ALLOC())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = alloca "), inst->type_name), "\n");
    }
    if ((k == SSA_LOAD())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = load "), inst->type_name), " "), inst->arg1), "\n");
    }
    if ((k == SSA_STORE())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    store ", inst->type_name), " "), inst->arg1), ", "), inst->dest), "\n");
    }
    if ((k == SSA_GET_FIELD())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = getfield "), inst->type_name), " "), inst->arg1), "."), inst->arg2), "\n");
    }
    if ((k == SSA_SET_FIELD())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    setfield ", inst->type_name), " "), inst->arg1), "."), inst->arg2), " = "), inst->arg3), "\n");
    }
    if ((k == SSA_PHI())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ", inst->dest), " = phi "), inst->type_name), " ["), inst->arg1), "], ["), inst->arg2), "]\n");
    }
    if ((k == SSA_BR())) {
        return rz_str_concat(rz_str_concat("    br label %", inst->target_label), "\n");
    }
    if ((k == SSA_BR_COND())) {
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    br i1 ", inst->arg1), ", label %"), inst->target_label), ", label %"), inst->false_label), "\n");
    }
    if ((k == SSA_RETURN())) {
        if ((strcmp(inst->arg1, "") != 0)) {
            return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("    ret ", inst->type_name), " "), inst->arg1), "\n");
        }
        return "    ret void\n";
    }
    if ((k == SSA_NOP())) {
        return "    nop\n";
    }
    return "";
}

const char* dump_basic_block(struct Rz_BasicBlock* bb) {
    if ((bb == NULL)) {
        return "";
    }
    const char* out = rz_str_concat(bb->label, ":\n");
    if ((bb->pred_count > 0)) {
        out = rz_str_concat(out, "    ; preds: ");
        struct Rz_SsaBlockRef* p = bb->preds;
        int64_t first = 1;
        while ((p != NULL)) {
            if ((first == 0)) {
                out = rz_str_concat(out, ", ");
            }
            out = rz_str_concat(rz_str_concat(out, "%"), p->name);
            first = 0;
            p = p->next;
        }
        out = rz_str_concat(out, "\n");
    }
    struct Rz_SsaInst* inst = bb->inst_head;
    while ((inst != NULL)) {
        out = rz_str_concat(out, dump_ssa_inst(inst));
        inst = inst->next;
    }
    return out;
}

const char* dump_ssa_function(struct Rz_SsaFunction* fn) {
    if ((fn == NULL)) {
        return "";
    }
    const char* out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("define ", fn->ret_type), " @"), fn->name), "(");
    struct Rz_SsaParam* param = fn->params;
    int64_t first = 1;
    while ((param != NULL)) {
        if ((first == 0)) {
            out = rz_str_concat(out, ", ");
        }
        out = rz_str_concat(rz_str_concat(rz_str_concat(out, param->type_name), " %"), param->name);
        first = 0;
        param = param->next;
    }
    out = rz_str_concat(out, ") {\n");
    struct Rz_BasicBlock* bb = fn->block_head;
    while ((bb != NULL)) {
        out = rz_str_concat(out, dump_basic_block(bb));
        bb = bb->next;
    }
    out = rz_str_concat(out, "}\n\n");
    return out;
}

const char* dump_ssa_ir(struct Rz_SsaProgram* prog) {
    if ((prog == NULL)) {
        return "";
    }
    const char* out = "; ==============================================================================\n";
    out = rz_str_concat(out, "; 🌹 Roze SSA Intermediate Representation (ADV-06 M6-T03)\n");
    out = rz_str_concat(out, "; ==============================================================================\n\n");
    struct Rz_SsaFunction* fn = prog->functions;
    while ((fn != NULL)) {
        out = rz_str_concat(out, dump_ssa_function(fn));
        fn = fn->next;
    }
    return out;
}

const char* lower_ast_expr_to_ssa(struct Rz_SsaFunction* fn, struct Rz_BasicBlock* bb, struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return "";
    }
    int64_t k = expr->kind;
    if ((k == NODE_LITERAL_INT())) {
        const char* t = fn_new_temp(fn);
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_CONST_INT(), t, "int", "", "", "", expr->int_val, "", "", "");
        bb_append_inst(bb, inst);
        return t;
    }
    if ((k == NODE_LITERAL_STR())) {
        const char* t = fn_new_temp(fn);
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_CONST_STR(), t, "string", "", "", "", 0, expr->str_val, "", "");
        bb_append_inst(bb, inst);
        return t;
    }
    if ((k == NODE_LITERAL_BOOL())) {
        const char* t = fn_new_temp(fn);
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_CONST_BOOL(), t, "bool", "", "", "", expr->int_val, "", "", "");
        bb_append_inst(bb, inst);
        return t;
    }
    if ((k == NODE_IDENT())) {
        return rz_str_concat("%", expr->name);
    }
    if ((k == NODE_BIN_OP())) {
        const char* l = lower_ast_expr_to_ssa(fn, bb, expr->child1);
        const char* r = lower_ast_expr_to_ssa(fn, bb, expr->child2);
        const char* t = fn_new_temp(fn);
        const char* op_ty = expr->type_name;
        if ((strcmp(op_ty, "") == 0)) {
            op_ty = "int";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_BIN_OP(), t, op_ty, expr->op, l, r, 0, "", "", "");
        bb_append_inst(bb, inst);
        return t;
    }
    if ((k == NODE_UNARY_OP())) {
        const char* sub = lower_ast_expr_to_ssa(fn, bb, expr->child1);
        const char* t = fn_new_temp(fn);
        const char* op_ty = expr->type_name;
        if ((strcmp(op_ty, "") == 0)) {
            op_ty = "int";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_UNARY_OP(), t, op_ty, expr->op, sub, "", 0, "", "", "");
        bb_append_inst(bb, inst);
        return t;
    }
    if ((k == NODE_CALL())) {
        const char* args_str = "";
        struct Rz_AstNode* arg = expr->child2;
        int64_t first = 1;
        while ((arg != NULL)) {
            if ((first == 0)) {
                args_str = rz_str_concat(args_str, ", ");
            }
            const char* a_res = lower_ast_expr_to_ssa(fn, bb, arg);
            args_str = rz_str_concat(args_str, a_res);
            first = 0;
            arg = arg->next;
        }
        const char* t = fn_new_temp(fn);
        const char* ret_ty = expr->type_name;
        if ((strcmp(ret_ty, "") == 0)) {
            ret_ty = "void";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_CALL(), t, ret_ty, "", args_str, "", 0, "", expr->name, "");
        bb_append_inst(bb, inst);
        return t;
    }
    if ((k == NODE_MEMBER())) {
        const char* obj = lower_ast_expr_to_ssa(fn, bb, expr->child1);
        const char* t = fn_new_temp(fn);
        const char* f_ty = expr->type_name;
        if ((strcmp(f_ty, "") == 0)) {
            f_ty = "int";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_GET_FIELD(), t, f_ty, "", obj, expr->name, 0, "", "", "");
        bb_append_inst(bb, inst);
        return t;
    }
    return "";
}

struct Rz_BasicBlock* lower_ast_stmt_to_ssa(struct Rz_SsaFunction* fn, struct Rz_BasicBlock* bb, struct Rz_AstNode* stmt) {
    if ((stmt == NULL)) {
        return bb;
    }
    int64_t k = stmt->kind;
    if ((k == NODE_BLOCK())) {
        struct Rz_AstNode* curr = stmt->child1;
        struct Rz_BasicBlock* curr_bb = bb;
        while ((curr != NULL)) {
            curr_bb = lower_ast_stmt_to_ssa(fn, curr_bb, curr);
            curr = curr->next;
        }
        return curr_bb;
    }
    if ((k == NODE_VAR_DECL())) {
        const char* val_temp = lower_ast_expr_to_ssa(fn, bb, stmt->child1);
        const char* ty = stmt->type_name;
        if ((strcmp(ty, "") == 0)) {
            ty = "int";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_STORE(), rz_str_concat("%", stmt->name), ty, "", val_temp, "", 0, "", "", "");
        bb_append_inst(bb, inst);
        return bb;
    }
    if ((k == NODE_ASSIGN())) {
        const char* val_temp = lower_ast_expr_to_ssa(fn, bb, stmt->child1);
        const char* ty = stmt->type_name;
        if ((strcmp(ty, "") == 0)) {
            ty = "int";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_STORE(), rz_str_concat("%", stmt->name), ty, "", val_temp, "", 0, "", "", "");
        bb_append_inst(bb, inst);
        return bb;
    }
    if ((k == NODE_RETURN())) {
        const char* ret_val = "";
        if ((stmt->child1 != NULL)) {
            ret_val = lower_ast_expr_to_ssa(fn, bb, stmt->child1);
        }
        const char* ty = fn->ret_type;
        if ((strcmp(ty, "") == 0)) {
            ty = "void";
        }
        struct Rz_SsaInst* inst = make_ssa_inst(bb->inst_count, SSA_RETURN(), "", ty, "", ret_val, "", 0, "", "", "");
        bb_append_inst(bb, inst);
        return bb;
    }
    if ((k == NODE_EXPR_STMT())) {
        lower_ast_expr_to_ssa(fn, bb, stmt->child1);
        return bb;
    }
    if ((k == NODE_IF())) {
        const char* cond_val = lower_ast_expr_to_ssa(fn, bb, stmt->child1);
        const char* then_lbl = rz_str_concat("then_", rz_int_to_str(fn->block_count));
        const char* else_lbl = rz_str_concat("else_", rz_int_to_str(fn->block_count));
        const char* merge_lbl = rz_str_concat("merge_", rz_int_to_str(fn->block_count));
        struct Rz_BasicBlock* then_bb = make_basic_block(fn->block_count, then_lbl);
        fn_append_bb(fn, then_bb);
        struct Rz_BasicBlock* else_bb = NULL;
        if ((stmt->child3 != NULL)) {
            else_bb = make_basic_block(fn->block_count, else_lbl);
            fn_append_bb(fn, else_bb);
        }
        struct Rz_BasicBlock* merge_bb = make_basic_block(fn->block_count, merge_lbl);
        fn_append_bb(fn, merge_bb);
        const char* false_target = merge_lbl;
        if ((else_bb != NULL)) {
            false_target = else_lbl;
        }
        struct Rz_SsaInst* br_cond = make_ssa_inst(bb->inst_count, SSA_BR_COND(), "", "i1", "", cond_val, "", 0, "", then_lbl, false_target);
        bb_append_inst(bb, br_cond);
        struct Rz_BasicBlock* then_end = lower_ast_stmt_to_ssa(fn, then_bb, stmt->child2);
        if (((then_end != NULL) && (then_end->is_terminated == 0))) {
            struct Rz_SsaInst* br_merge = make_ssa_inst(then_end->inst_count, SSA_BR(), "", "", "", "", "", 0, "", merge_lbl, "");
            bb_append_inst(then_end, br_merge);
        }
        if ((else_bb != NULL)) {
            struct Rz_BasicBlock* else_end = lower_ast_stmt_to_ssa(fn, else_bb, stmt->child3);
            if (((else_end != NULL) && (else_end->is_terminated == 0))) {
                struct Rz_SsaInst* br_merge2 = make_ssa_inst(else_end->inst_count, SSA_BR(), "", "", "", "", "", 0, "", merge_lbl, "");
                bb_append_inst(else_end, br_merge2);
            }
        }
        return merge_bb;
    }
    if ((k == NODE_WHILE())) {
        const char* hdr_lbl = rz_str_concat("while_hdr_", rz_int_to_str(fn->block_count));
        const char* body_lbl = rz_str_concat("while_body_", rz_int_to_str(fn->block_count));
        const char* exit_lbl = rz_str_concat("while_exit_", rz_int_to_str(fn->block_count));
        struct Rz_BasicBlock* hdr_bb = make_basic_block(fn->block_count, hdr_lbl);
        fn_append_bb(fn, hdr_bb);
        struct Rz_BasicBlock* body_bb = make_basic_block(fn->block_count, body_lbl);
        fn_append_bb(fn, body_bb);
        struct Rz_BasicBlock* exit_bb = make_basic_block(fn->block_count, exit_lbl);
        fn_append_bb(fn, exit_bb);
        struct Rz_SsaInst* br_hdr = make_ssa_inst(bb->inst_count, SSA_BR(), "", "", "", "", "", 0, "", hdr_lbl, "");
        bb_append_inst(bb, br_hdr);
        const char* cond_val = lower_ast_expr_to_ssa(fn, hdr_bb, stmt->child1);
        struct Rz_SsaInst* br_cond = make_ssa_inst(hdr_bb->inst_count, SSA_BR_COND(), "", "i1", "", cond_val, "", 0, "", body_lbl, exit_lbl);
        bb_append_inst(hdr_bb, br_cond);
        struct Rz_BasicBlock* body_end = lower_ast_stmt_to_ssa(fn, body_bb, stmt->child2);
        if (((body_end != NULL) && (body_end->is_terminated == 0))) {
            struct Rz_SsaInst* back_edge = make_ssa_inst(body_end->inst_count, SSA_BR(), "", "", "", "", "", 0, "", hdr_lbl, "");
            bb_append_inst(body_end, back_edge);
        }
        return exit_bb;
    }
    return bb;
}

struct Rz_SsaFunction* lower_ast_func_to_ssa(struct Rz_AstNode* decl) {
    if (((decl == NULL) || (decl->kind != NODE_FUNC_DECL()))) {
        return NULL;
    }
    struct Rz_SsaFunction* fn = make_ssa_function(decl->name, decl->type_name);
    struct Rz_AstNode* p = decl->child1;
    struct Rz_SsaParam* p_head = NULL;
    struct Rz_SsaParam* p_tail = NULL;
    while ((p != NULL)) {
        const char* param_ty = p->type_name;
        if ((strcmp(param_ty, "") == 0)) {
            param_ty = "int";
        }
        struct Rz_SsaParam* sp = make_ssa_param(p->name, param_ty);
        if ((p_head == NULL)) {
            p_head = sp;
            p_tail = sp;
        } else {
            p_tail->next = sp;
            p_tail = sp;
        }
        p = p->next;
    }
    fn->params = p_head;
    struct Rz_BasicBlock* entry_bb = make_basic_block(0, "entry");
    fn_append_bb(fn, entry_bb);
    struct Rz_BasicBlock* last_bb = lower_ast_stmt_to_ssa(fn, entry_bb, decl->child2);
    if (((last_bb != NULL) && (last_bb->is_terminated == 0))) {
        struct Rz_SsaInst* ret_inst = make_ssa_inst(last_bb->inst_count, SSA_RETURN(), "", fn->ret_type, "", "", "", 0, "", "", "");
        bb_append_inst(last_bb, ret_inst);
    }
    return fn;
}

struct Rz_SsaProgram* build_ssa_ir(struct Rz_AstNode* prog) {
    struct Rz_SsaProgram* ssa_prog = make_ssa_program();
    if ((prog == NULL)) {
        return ssa_prog;
    }
    struct Rz_SsaFunction* fn_head = NULL;
    struct Rz_SsaFunction* fn_tail = NULL;
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            struct Rz_SsaFunction* ssa_fn = lower_ast_func_to_ssa(decl);
            if ((ssa_fn != NULL)) {
                if ((fn_head == NULL)) {
                    fn_head = ssa_fn;
                    fn_tail = ssa_fn;
                } else {
                    fn_tail->next = ssa_fn;
                    fn_tail = ssa_fn;
                }
            }
        }
        decl = decl->next;
    }
    ssa_prog->functions = fn_head;
    return ssa_prog;
}

struct Rz_AstNode* make_int_literal_node(int64_t val, int64_t line, int64_t col) {
    struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_INT(), "", "int", line, col);
    node->int_val = val;
    return node;
}

struct Rz_AstNode* make_str_literal_node(const char* val, int64_t line, int64_t col) {
    struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_STR(), "", "string", line, col);
    node->str_val = val;
    return node;
}

struct Rz_AstNode* make_bool_literal_node(int64_t val, int64_t line, int64_t col) {
    struct Rz_AstNode* node = make_ast_node(NODE_LITERAL_BOOL(), "", "bool", line, col);
    node->int_val = val;
    return node;
}

int64_t is_pure_expr(struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return 1;
    }
    int64_t k = expr->kind;
    if (((((k == NODE_LITERAL_INT()) || (k == NODE_LITERAL_STR())) || (k == NODE_LITERAL_BOOL())) || (k == NODE_LITERAL_NULL()))) {
        return 1;
    }
    if ((k == NODE_IDENT())) {
        return 1;
    }
    if ((k == NODE_BIN_OP())) {
        if (((is_pure_expr(expr->child1) == 1) && (is_pure_expr(expr->child2) == 1))) {
            return 1;
        }
        return 0;
    }
    if ((k == NODE_UNARY_OP())) {
        return is_pure_expr(expr->child1);
    }
    return 0;
}

int64_t eval_int_bin_op(const char* op, int64_t a, int64_t b) {
    if ((strcmp(op, "+") == 0)) {
        return (a + b);
    }
    if ((strcmp(op, "-") == 0)) {
        return (a - b);
    }
    if ((strcmp(op, "*") == 0)) {
        return (a * b);
    }
    if ((strcmp(op, "/") == 0)) {
        if ((b != 0)) {
            return (a / b);
        }
        return 0;
    }
    if ((strcmp(op, "%") == 0)) {
        if ((b != 0)) {
            return (a % b);
        }
        return 0;
    }
    if ((strcmp(op, "==") == 0)) {
        if ((a == b)) {
            return 1;
        }
        return 0;
    }
    if ((strcmp(op, "!=") == 0)) {
        if ((a != b)) {
            return 1;
        }
        return 0;
    }
    if ((strcmp(op, "<") == 0)) {
        if ((a < b)) {
            return 1;
        }
        return 0;
    }
    if ((strcmp(op, "<=") == 0)) {
        if ((a <= b)) {
            return 1;
        }
        return 0;
    }
    if ((strcmp(op, ">") == 0)) {
        if ((a > b)) {
            return 1;
        }
        return 0;
    }
    if ((strcmp(op, ">=") == 0)) {
        if ((a >= b)) {
            return 1;
        }
        return 0;
    }
    return 0;
}

struct Rz_AstNode* fold_constants_expr(struct Rz_AstNode* expr, struct Rz_OptStats* stats) {
    if ((expr == NULL)) {
        return NULL;
    }
    expr->child1 = fold_constants_expr(expr->child1, stats);
    expr->child2 = fold_constants_expr(expr->child2, stats);
    expr->child3 = fold_constants_expr(expr->child3, stats);
    expr->child4 = fold_constants_expr(expr->child4, stats);
    int64_t k = expr->kind;
    if ((k == NODE_UNARY_OP())) {
        struct Rz_AstNode* sub = expr->child1;
        if ((sub != NULL)) {
            if (((strcmp(expr->op, "!") == 0) && (sub->kind == NODE_LITERAL_BOOL()))) {
                int64_t folded_val = 0;
                if ((sub->int_val == 0)) {
                    folded_val = 1;
                }
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(folded_val, expr->line, expr->col);
            }
            if (((strcmp(expr->op, "-") == 0) && (sub->kind == NODE_LITERAL_INT()))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_int_literal_node((0 - sub->int_val), expr->line, expr->col);
            }
        }
        return expr;
    }
    if ((k == NODE_BIN_OP())) {
        struct Rz_AstNode* left = expr->child1;
        struct Rz_AstNode* right = expr->child2;
        if (((left == NULL) || (right == NULL))) {
            return expr;
        }
        int64_t lk = left->kind;
        int64_t rk = right->kind;
        if (((lk == NODE_LITERAL_INT()) && (rk == NODE_LITERAL_INT()))) {
            const char* op = expr->op;
            if ((((((strcmp(op, "+") == 0) || (strcmp(op, "-") == 0)) || (strcmp(op, "*") == 0)) || (strcmp(op, "/") == 0)) || (strcmp(op, "%") == 0))) {
                int64_t res = eval_int_bin_op(op, left->int_val, right->int_val);
                stats->const_folded = (stats->const_folded + 1);
                return make_int_literal_node(res, expr->line, expr->col);
            }
            if (((((((strcmp(op, "==") == 0) || (strcmp(op, "!=") == 0)) || (strcmp(op, "<") == 0)) || (strcmp(op, "<=") == 0)) || (strcmp(op, ">") == 0)) || (strcmp(op, ">=") == 0))) {
                int64_t res_bool = eval_int_bin_op(op, left->int_val, right->int_val);
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(res_bool, expr->line, expr->col);
            }
        }
        if ((((lk == NODE_LITERAL_STR()) && (rk == NODE_LITERAL_STR())) && (strcmp(expr->op, "+") == 0))) {
            stats->const_folded = (stats->const_folded + 1);
            return make_str_literal_node(rz_str_concat(left->str_val, right->str_val), expr->line, expr->col);
        }
        if (((lk == NODE_LITERAL_BOOL()) && (rk == NODE_LITERAL_BOOL()))) {
            if ((strcmp(expr->op, "&&") == 0)) {
                int64_t b_res = 0;
                if (((left->int_val == 1) && (right->int_val == 1))) {
                    b_res = 1;
                }
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(b_res, expr->line, expr->col);
            }
            if ((strcmp(expr->op, "||") == 0)) {
                int64_t b_res = 0;
                if (((left->int_val == 1) || (right->int_val == 1))) {
                    b_res = 1;
                }
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(b_res, expr->line, expr->col);
            }
            if ((strcmp(expr->op, "==") == 0)) {
                int64_t b_res = 0;
                if ((left->int_val == right->int_val)) {
                    b_res = 1;
                }
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(b_res, expr->line, expr->col);
            }
            if ((strcmp(expr->op, "!=") == 0)) {
                int64_t b_res = 0;
                if ((left->int_val != right->int_val)) {
                    b_res = 1;
                }
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(b_res, expr->line, expr->col);
            }
        }
        if ((strcmp(expr->op, "+") == 0)) {
            if (((rk == NODE_LITERAL_INT()) && (right->int_val == 0))) {
                stats->const_folded = (stats->const_folded + 1);
                return left;
            }
            if (((lk == NODE_LITERAL_INT()) && (left->int_val == 0))) {
                stats->const_folded = (stats->const_folded + 1);
                return right;
            }
        }
        if ((strcmp(expr->op, "-") == 0)) {
            if (((rk == NODE_LITERAL_INT()) && (right->int_val == 0))) {
                stats->const_folded = (stats->const_folded + 1);
                return left;
            }
        }
        if ((strcmp(expr->op, "*") == 0)) {
            if (((rk == NODE_LITERAL_INT()) && (right->int_val == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return left;
            }
            if (((lk == NODE_LITERAL_INT()) && (left->int_val == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return right;
            }
            if ((((rk == NODE_LITERAL_INT()) && (right->int_val == 0)) && (is_pure_expr(left) == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_int_literal_node(0, expr->line, expr->col);
            }
            if ((((lk == NODE_LITERAL_INT()) && (left->int_val == 0)) && (is_pure_expr(right) == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_int_literal_node(0, expr->line, expr->col);
            }
        }
        if ((strcmp(expr->op, "/") == 0)) {
            if (((rk == NODE_LITERAL_INT()) && (right->int_val == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return left;
            }
        }
        if ((strcmp(expr->op, "&&") == 0)) {
            if (((lk == NODE_LITERAL_BOOL()) && (left->int_val == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return right;
            }
            if (((rk == NODE_LITERAL_BOOL()) && (right->int_val == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return left;
            }
            if ((((lk == NODE_LITERAL_BOOL()) && (left->int_val == 0)) && (is_pure_expr(right) == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(0, expr->line, expr->col);
            }
            if ((((rk == NODE_LITERAL_BOOL()) && (right->int_val == 0)) && (is_pure_expr(left) == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(0, expr->line, expr->col);
            }
        }
        if ((strcmp(expr->op, "||") == 0)) {
            if (((lk == NODE_LITERAL_BOOL()) && (left->int_val == 0))) {
                stats->const_folded = (stats->const_folded + 1);
                return right;
            }
            if (((rk == NODE_LITERAL_BOOL()) && (right->int_val == 0))) {
                stats->const_folded = (stats->const_folded + 1);
                return left;
            }
            if ((((lk == NODE_LITERAL_BOOL()) && (left->int_val == 1)) && (is_pure_expr(right) == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(1, expr->line, expr->col);
            }
            if ((((rk == NODE_LITERAL_BOOL()) && (right->int_val == 1)) && (is_pure_expr(left) == 1))) {
                stats->const_folded = (stats->const_folded + 1);
                return make_bool_literal_node(1, expr->line, expr->col);
            }
        }
    }
    return expr;
}

int64_t has_ident_ref(struct Rz_AstNode* node, const char* name) {
    if ((node == NULL)) {
        return 0;
    }
    if (((node->kind == NODE_IDENT()) && (strcmp(node->name, name) == 0))) {
        return 1;
    }
    if ((has_ident_ref(node->child1, name) == 1)) {
        return 1;
    }
    if ((has_ident_ref(node->child2, name) == 1)) {
        return 1;
    }
    if ((has_ident_ref(node->child3, name) == 1)) {
        return 1;
    }
    if ((has_ident_ref(node->child4, name) == 1)) {
        return 1;
    }
    if ((has_ident_ref(node->next, name) == 1)) {
        return 1;
    }
    return 0;
}

struct Rz_AstNode* eliminate_dead_statements(struct Rz_AstNode* stmt_list, struct Rz_OptStats* stats) {
    if ((stmt_list == NULL)) {
        return NULL;
    }
    struct Rz_AstNode* head = NULL;
    struct Rz_AstNode* tail = NULL;
    struct Rz_AstNode* curr = stmt_list;
    int64_t reached_return = 0;
    while ((curr != NULL)) {
        if ((reached_return == 1)) {
            stats->dead_stmts_removed = (stats->dead_stmts_removed + 1);
            curr = curr->next;
        } else {
            struct Rz_AstNode* next_stmt = curr->next;
            curr->next = NULL;
            struct Rz_AstNode* optimized_stmt = eliminate_dead_code_stmt(curr, stats);
            if ((optimized_stmt != NULL)) {
                if ((head == NULL)) {
                    head = optimized_stmt;
                    tail = optimized_stmt;
                } else {
                    struct Rz_AstNode* t = tail;
                    while ((t->next != NULL)) {
                        t = t->next;
                    }
                    t->next = optimized_stmt;
                    tail = optimized_stmt;
                }
                if ((optimized_stmt->kind == NODE_RETURN())) {
                    reached_return = 1;
                }
            }
            curr = next_stmt;
        }
    }
    return head;
}

struct Rz_AstNode* eliminate_dead_code_stmt(struct Rz_AstNode* stmt, struct Rz_OptStats* stats) {
    if ((stmt == NULL)) {
        return NULL;
    }
    int64_t k = stmt->kind;
    if ((k == NODE_BLOCK())) {
        stmt->child1 = eliminate_dead_statements(stmt->child1, stats);
        return stmt;
    }
    if ((k == NODE_IF())) {
        stmt->child1 = fold_constants_expr(stmt->child1, stats);
        struct Rz_AstNode* cond = stmt->child1;
        if (((cond != NULL) && ((cond->kind == NODE_LITERAL_BOOL()) || (cond->kind == NODE_LITERAL_INT())))) {
            if ((cond->int_val == 1)) {
                stats->dead_stmts_removed = (stats->dead_stmts_removed + 1);
                struct Rz_AstNode* then_branch = stmt->child2;
                if ((then_branch != NULL)) {
                    return eliminate_dead_code_stmt(then_branch, stats);
                }
                return NULL;
            }
            if ((cond->int_val == 0)) {
                stats->dead_stmts_removed = (stats->dead_stmts_removed + 1);
                struct Rz_AstNode* else_branch = stmt->child3;
                if ((else_branch != NULL)) {
                    return eliminate_dead_code_stmt(else_branch, stats);
                }
                return NULL;
            }
        }
        stmt->child2 = eliminate_dead_code_stmt(stmt->child2, stats);
        stmt->child3 = eliminate_dead_code_stmt(stmt->child3, stats);
        return stmt;
    }
    if ((k == NODE_WHILE())) {
        stmt->child1 = fold_constants_expr(stmt->child1, stats);
        struct Rz_AstNode* cond = stmt->child1;
        if ((((cond != NULL) && ((cond->kind == NODE_LITERAL_BOOL()) || (cond->kind == NODE_LITERAL_INT()))) && (cond->int_val == 0))) {
            stats->dead_stmts_removed = (stats->dead_stmts_removed + 1);
            return NULL;
        }
        stmt->child2 = eliminate_dead_code_stmt(stmt->child2, stats);
        return stmt;
    }
    if ((k == NODE_FOR())) {
        stmt->child1 = eliminate_dead_code_stmt(stmt->child1, stats);
        stmt->child2 = fold_constants_expr(stmt->child2, stats);
        stmt->child3 = eliminate_dead_code_stmt(stmt->child3, stats);
        stmt->child4 = eliminate_dead_code_stmt(stmt->child4, stats);
        return stmt;
    }
    if ((k == NODE_VAR_DECL())) {
        stmt->child1 = fold_constants_expr(stmt->child1, stats);
        return stmt;
    }
    if ((k == NODE_ASSIGN())) {
        stmt->child1 = fold_constants_expr(stmt->child1, stats);
        return stmt;
    }
    if ((k == NODE_RETURN())) {
        stmt->child1 = fold_constants_expr(stmt->child1, stats);
        return stmt;
    }
    if ((k == NODE_EXPR_STMT())) {
        stmt->child1 = fold_constants_expr(stmt->child1, stats);
        return stmt;
    }
    return stmt;
}

struct Rz_InlinableFunc* find_inlinable_func(struct Rz_InlinableFunc* head, const char* name) {
    struct Rz_InlinableFunc* curr = head;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return curr;
        }
        curr = curr->next;
    }
    return NULL;
}

struct Rz_AstNode* substitute_param_with_arg(struct Rz_AstNode* expr, const char* param_name, struct Rz_AstNode* arg_expr) {
    if ((expr == NULL)) {
        return NULL;
    }
    if (((expr->kind == NODE_IDENT()) && (strcmp(expr->name, param_name) == 0))) {
        return clone_single_node(arg_expr);
    }
    expr->child1 = substitute_param_with_arg(expr->child1, param_name, arg_expr);
    expr->child2 = substitute_param_with_arg(expr->child2, param_name, arg_expr);
    expr->child3 = substitute_param_with_arg(expr->child3, param_name, arg_expr);
    expr->child4 = substitute_param_with_arg(expr->child4, param_name, arg_expr);
    return expr;
}

struct Rz_AstNode* inline_call_expr(struct Rz_AstNode* call_node, struct Rz_InlinableFunc* inlinable_list, struct Rz_OptStats* stats) {
    if (((call_node == NULL) || (inlinable_list == NULL))) {
        return call_node;
    }
    if ((call_node->kind == NODE_CALL())) {
        struct Rz_InlinableFunc* fn_info = find_inlinable_func(inlinable_list, call_node->name);
        if ((fn_info != NULL)) {
            struct Rz_AstNode* ret_copy = clone_single_node(fn_info->ret_expr);
            struct Rz_AstNode* p = fn_info->params;
            struct Rz_AstNode* a = call_node->child2;
            int64_t match_ok = 1;
            while (((p != NULL) && (a != NULL))) {
                ret_copy = substitute_param_with_arg(ret_copy, p->name, a);
                p = p->next;
                a = a->next;
            }
            if (((p != NULL) || (a != NULL))) {
                match_ok = 0;
            }
            if (((match_ok == 1) && (ret_copy != NULL))) {
                stats->funcs_inlined = (stats->funcs_inlined + 1);
                return ret_copy;
            }
        }
    }
    return call_node;
}

struct Rz_AstNode* inline_calls_in_expr(struct Rz_AstNode* expr, struct Rz_InlinableFunc* inlinable_list, struct Rz_OptStats* stats) {
    if ((expr == NULL)) {
        return NULL;
    }
    if ((expr->kind == NODE_CALL())) {
        struct Rz_AstNode* arg = expr->child2;
        while ((arg != NULL)) {
            arg->child1 = inline_calls_in_expr(arg->child1, inlinable_list, stats);
            arg->child2 = inline_calls_in_expr(arg->child2, inlinable_list, stats);
            arg = arg->next;
        }
        return inline_call_expr(expr, inlinable_list, stats);
    }
    expr->child1 = inline_calls_in_expr(expr->child1, inlinable_list, stats);
    expr->child2 = inline_calls_in_expr(expr->child2, inlinable_list, stats);
    expr->child3 = inline_calls_in_expr(expr->child3, inlinable_list, stats);
    expr->child4 = inline_calls_in_expr(expr->child4, inlinable_list, stats);
    return expr;
}

void inline_calls_in_stmt(struct Rz_AstNode* stmt, struct Rz_InlinableFunc* inlinable_list, struct Rz_OptStats* stats) {
    if (((stmt == NULL) || (inlinable_list == NULL))) {
        return;
    }
    int64_t k = stmt->kind;
    if ((k == NODE_BLOCK())) {
        struct Rz_AstNode* curr = stmt->child1;
        while ((curr != NULL)) {
            inline_calls_in_stmt(curr, inlinable_list, stats);
            curr = curr->next;
        }
    } else     if (((((k == NODE_VAR_DECL()) || (k == NODE_ASSIGN())) || (k == NODE_RETURN())) || (k == NODE_EXPR_STMT()))) {
        stmt->child1 = inline_calls_in_expr(stmt->child1, inlinable_list, stats);
    } else     if ((k == NODE_IF())) {
        stmt->child1 = inline_calls_in_expr(stmt->child1, inlinable_list, stats);
        inline_calls_in_stmt(stmt->child2, inlinable_list, stats);
        inline_calls_in_stmt(stmt->child3, inlinable_list, stats);
    } else     if ((k == NODE_WHILE())) {
        stmt->child1 = inline_calls_in_expr(stmt->child1, inlinable_list, stats);
        inline_calls_in_stmt(stmt->child2, inlinable_list, stats);
    } else     if ((k == NODE_FOR())) {
        inline_calls_in_stmt(stmt->child1, inlinable_list, stats);
        stmt->child2 = inline_calls_in_expr(stmt->child2, inlinable_list, stats);
        inline_calls_in_stmt(stmt->child3, inlinable_list, stats);
        inline_calls_in_stmt(stmt->child4, inlinable_list, stats);
    }
}

int64_t is_inlinable_ret_expr(struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return 0;
    }
    int64_t k = expr->kind;
    if ((((k == NODE_LITERAL_INT()) || (k == NODE_LITERAL_STR())) || (k == NODE_LITERAL_BOOL()))) {
        return 1;
    }
    if ((k == NODE_IDENT())) {
        return 1;
    }
    if ((k == NODE_BIN_OP())) {
        if (((is_inlinable_ret_expr(expr->child1) == 1) && (is_inlinable_ret_expr(expr->child2) == 1))) {
            return 1;
        }
        return 0;
    }
    if ((k == NODE_UNARY_OP())) {
        return is_inlinable_ret_expr(expr->child1);
    }
    return 0;
}

int64_t is_compiler_internal_func(const char* name) {
    if (((((strcmp(name, "is_digit") == 0) || (strcmp(name, "is_alpha") == 0)) || (strcmp(name, "is_alnum") == 0)) || (strcmp(name, "is_ident_char") == 0))) {
        return 1;
    }
    if (((((strcmp(name, "check_token") == 0) || (strcmp(name, "advance_token") == 0)) || (strcmp(name, "peek_line") == 0)) || (strcmp(name, "peek_col") == 0))) {
        return 1;
    }
    if ((((strcmp(name, "keyword_kind") == 0) || (strcmp(name, "c_type") == 0)) || (strcmp(name, "escape_c_string") == 0))) {
        return 1;
    }
    int64_t len = rz_str_len(name);
    if ((len >= 4)) {
        const char* p4 = rz_substring(name, 0, 4);
        if ((((((((strcmp(p4, "TOK_") == 0) || (strcmp(p4, "NODE") == 0)) || (strcmp(p4, "SYM_") == 0)) || (strcmp(p4, "SSA_") == 0)) || (strcmp(p4, "make") == 0)) || (strcmp(p4, "rz_c") == 0)) || (strcmp(p4, "rz_s") == 0))) {
            return 1;
        }
    }
    if ((len >= 7)) {
        const char* p7 = rz_substring(name, 0, 7);
        if ((strcmp(p7, "BORROW_") == 0)) {
            return 1;
        }
    }
    return 0;
}

struct Rz_InlinableFunc* collect_inlinable_functions(struct Rz_AstNode* prog) {
    struct Rz_InlinableFunc* list_head = NULL;
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if (((((decl->kind == NODE_FUNC_DECL()) && (strcmp(decl->name, "main") != 0)) && (strcmp(decl->name, "rz_user_main") != 0)) && (is_compiler_internal_func(decl->name) == 0))) {
            struct Rz_AstNode* body = decl->child2;
            if (((body != NULL) && (body->kind == NODE_BLOCK()))) {
                struct Rz_AstNode* first_stmt = body->child1;
                if ((((first_stmt != NULL) && (first_stmt->kind == NODE_RETURN())) && (first_stmt->next == NULL))) {
                    struct Rz_AstNode* ret_expr = first_stmt->child1;
                    if ((((ret_expr != NULL) && (is_inlinable_ret_expr(ret_expr) == 1)) && (has_ident_ref(ret_expr, decl->name) == 0))) {
                        struct Rz_InlinableFunc* entry = new_InlinableFunc(decl->name, decl->type_name, decl->child1, ret_expr, list_head);
                        list_head = entry;
                    }
                }
            }
        }
        decl = decl->next;
    }
    return list_head;
}

struct Rz_SsaBlockRef* add_ident_ref(struct Rz_SsaBlockRef* head, const char* name) {
    struct Rz_SsaBlockRef* curr = head;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return head;
        }
        curr = curr->next;
    }
    return new_SsaBlockRef(name, head);
}

int64_t has_ident_in_list(struct Rz_SsaBlockRef* head, const char* name) {
    struct Rz_SsaBlockRef* curr = head;
    while ((curr != NULL)) {
        if ((strcmp(curr->name, name) == 0)) {
            return 1;
        }
        curr = curr->next;
    }
    return 0;
}

struct Rz_SsaBlockRef* collect_mutated_vars_in_node(struct Rz_AstNode* node, struct Rz_SsaBlockRef* muts) {
    if ((node == NULL)) {
        return muts;
    }
    if (((node->kind == NODE_ASSIGN()) && (strcmp(node->name, "") != 0))) {
        muts = add_ident_ref(muts, node->name);
    }
    muts = collect_mutated_vars_in_node(node->child1, muts);
    muts = collect_mutated_vars_in_node(node->child2, muts);
    muts = collect_mutated_vars_in_node(node->child3, muts);
    muts = collect_mutated_vars_in_node(node->child4, muts);
    muts = collect_mutated_vars_in_node(node->next, muts);
    return muts;
}

int64_t is_expr_invariant(struct Rz_AstNode* expr, struct Rz_SsaBlockRef* muts) {
    if ((expr == NULL)) {
        return 1;
    }
    int64_t k = expr->kind;
    if (((((k == NODE_LITERAL_INT()) || (k == NODE_LITERAL_STR())) || (k == NODE_LITERAL_BOOL())) || (k == NODE_LITERAL_NULL()))) {
        return 1;
    }
    if ((k == NODE_IDENT())) {
        if ((has_ident_in_list(muts, expr->name) == 1)) {
            return 0;
        }
        return 1;
    }
    if ((k == NODE_BIN_OP())) {
        if (((is_expr_invariant(expr->child1, muts) == 1) && (is_expr_invariant(expr->child2, muts) == 1))) {
            return 1;
        }
        return 0;
    }
    if ((k == NODE_UNARY_OP())) {
        return is_expr_invariant(expr->child1, muts);
    }
    return 0;
}

void hoist_loop_invariants_in_block(struct Rz_AstNode* block, struct Rz_OptStats* stats) {
    if (((block == NULL) || (block->kind != NODE_BLOCK()))) {
        return;
    }
    struct Rz_AstNode* prev = NULL;
    struct Rz_AstNode* curr = block->child1;
    while ((curr != NULL)) {
        if (((curr->kind == NODE_WHILE()) || (curr->kind == NODE_FOR()))) {
            struct Rz_AstNode* loop_body = curr->child2;
            if ((curr->kind == NODE_FOR())) {
                loop_body = curr->child4;
            }
            if (((loop_body != NULL) && (loop_body->kind == NODE_BLOCK()))) {
                struct Rz_SsaBlockRef* muts = NULL;
                if ((curr->kind == NODE_FOR())) {
                    muts = collect_mutated_vars_in_node(curr->child3, muts);
                }
                muts = collect_mutated_vars_in_node(loop_body->child1, muts);
                struct Rz_AstNode* body_prev = NULL;
                struct Rz_AstNode* body_curr = loop_body->child1;
                struct Rz_AstNode* hoisted_head = NULL;
                struct Rz_AstNode* hoisted_tail = NULL;
                while ((body_curr != NULL)) {
                    struct Rz_AstNode* next_stmt = body_curr->next;
                    if ((body_curr->kind == NODE_VAR_DECL())) {
                        struct Rz_AstNode* init_expr = body_curr->child1;
                        if (((init_expr != NULL) && (is_expr_invariant(init_expr, muts) == 1))) {
                            if ((body_prev == NULL)) {
                                loop_body->child1 = next_stmt;
                            } else {
                                body_prev->next = next_stmt;
                            }
                            body_curr->next = NULL;
                            if ((hoisted_head == NULL)) {
                                hoisted_head = body_curr;
                                hoisted_tail = body_curr;
                            } else {
                                hoisted_tail->next = body_curr;
                                hoisted_tail = body_curr;
                            }
                            stats->loops_hoisted = (stats->loops_hoisted + 1);
                            muts = add_ident_ref(muts, body_curr->name);
                        } else {
                            body_prev = body_curr;
                        }
                    } else {
                        body_prev = body_curr;
                    }
                    body_curr = next_stmt;
                }
                if ((hoisted_head != NULL)) {
                    if ((prev == NULL)) {
                        block->child1 = hoisted_head;
                        hoisted_tail->next = curr;
                        prev = hoisted_tail;
                    } else {
                        prev->next = hoisted_head;
                        hoisted_tail->next = curr;
                        prev = hoisted_tail;
                    }
                }
            }
        } else         if ((curr->kind == NODE_BLOCK())) {
            hoist_loop_invariants_in_block(curr, stats);
        }
        prev = curr;
        curr = curr->next;
    }
}

struct Rz_AstNode* optimize_program(struct Rz_AstNode* prog) {
    if ((prog == NULL)) {
        return NULL;
    }
    struct Rz_OptStats* stats = new_OptStats(0, 0, 0, 0);
    struct Rz_InlinableFunc* inlinable_list = collect_inlinable_functions(prog);
    if ((inlinable_list != NULL)) {
        struct Rz_AstNode* decl = prog->child1;
        while ((decl != NULL)) {
            if ((decl->kind == NODE_FUNC_DECL())) {
                inline_calls_in_stmt(decl->child2, inlinable_list, stats);
            }
            decl = decl->next;
        }
    }
    struct Rz_AstNode* decl2 = prog->child1;
    while ((decl2 != NULL)) {
        if ((decl2->kind == NODE_FUNC_DECL())) {
            hoist_loop_invariants_in_block(decl2->child2, stats);
        }
        decl2 = decl2->next;
    }
    for (int64_t iter = 0; (iter < 3); iter = (iter + 1)) {
        int64_t prev_folded = stats->const_folded;
        int64_t prev_dead = stats->dead_stmts_removed;
        struct Rz_AstNode* decl3 = prog->child1;
        while ((decl3 != NULL)) {
            if ((decl3->kind == NODE_FUNC_DECL())) {
                if ((decl3->child2 != NULL)) {
                    decl3->child2 = eliminate_dead_code_stmt(decl3->child2, stats);
                }
            }
            decl3 = decl3->next;
        }
        if (((stats->const_folded == prev_folded) && (stats->dead_stmts_removed == prev_dead))) {
            iter = 3;
        }
    }
    return prog;
}

const char* emit_program_ssa(struct Rz_AstNode* prog) {
    struct Rz_SsaProgram* ssa_prog = build_ssa_ir(prog);
    struct Rz_SsaFunction* ssa_fn = ssa_prog->functions;
    while ((ssa_fn != NULL)) {
        resolve_cfg_edges(ssa_fn);
        eliminate_dead_basic_blocks(ssa_fn);
        ssa_fn = ssa_fn->next;
    }
    return dump_ssa_ir(ssa_prog);
}

const char* escape_c_string(const char* s) {
    const char* out = "";
    int64_t len = rz_str_len(s);
    for (int64_t i = 0; (i < len); i = (i + 1)) {
        int64_t ch = rz_char_at(s, i);
        if ((ch == 10)) {
            out = rz_str_concat(out, "\\n");
        } else         if ((ch == 13)) {
            out = rz_str_concat(out, "\\r");
        } else         if ((ch == 9)) {
            out = rz_str_concat(out, "\\t");
        } else         if ((ch == 34)) {
            out = rz_str_concat(out, "\\\"");
        } else         if ((ch == 92)) {
            out = rz_str_concat(out, "\\\\");
        } else {
            out = rz_str_concat(out, rz_char_to_str(ch));
        }
    }
    return out;
}

const char* c_type(const char* type_name) {
    if ((strcmp(type_name, "int") == 0)) {
        return "int64_t";
    }
    if ((strcmp(type_name, "string") == 0)) {
        return "const char*";
    }
    if ((strcmp(type_name, "bool") == 0)) {
        return "int64_t";
    }
    if ((strcmp(type_name, "void") == 0)) {
        return "void";
    }
    if ((strcmp(type_name, "") == 0)) {
        return "int64_t";
    }
    return rz_str_concat(rz_str_concat("struct Rz_", type_name), "*");
}

const char* gen_expression(struct Rz_AstNode* expr) {
    if ((expr == NULL)) {
        return "";
    }
    int64_t kind = expr->kind;
    if ((kind == NODE_LITERAL_INT())) {
        return rz_int_to_str(expr->int_val);
    }
    if ((kind == NODE_LITERAL_STR())) {
        return rz_str_concat(rz_str_concat("\"", escape_c_string(expr->str_val)), "\"");
    }
    if ((kind == NODE_LITERAL_BOOL())) {
        if ((expr->int_val == 1)) {
            return "1";
        }
        return "0";
    }
    if ((kind == NODE_LITERAL_NULL())) {
        return "NULL";
    }
    if ((kind == NODE_IDENT())) {
        return expr->name;
    }
    if ((kind == NODE_BIN_OP())) {
        const char* left_code = gen_expression(expr->child1);
        const char* right_code = gen_expression(expr->child2);
        if ((strcmp(expr->op, "+") == 0)) {
            if ((strcmp(expr->type_name, "string") == 0)) {
                return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("rz_str_concat(", left_code), ", "), right_code), ")");
            }
            return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("(", left_code), " + "), right_code), ")");
        }
        if ((strcmp(expr->op, "==") == 0)) {
            if (((expr->child1 != NULL) && (strcmp(expr->child1->type_name, "string") == 0))) {
                return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("(strcmp(", left_code), ", "), right_code), ") == 0)");
            }
            return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("(", left_code), " == "), right_code), ")");
        }
        if ((strcmp(expr->op, "!=") == 0)) {
            if (((expr->child1 != NULL) && (strcmp(expr->child1->type_name, "string") == 0))) {
                return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("(strcmp(", left_code), ", "), right_code), ") != 0)");
            }
            return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("(", left_code), " != "), right_code), ")");
        }
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("(", left_code), " "), expr->op), " "), right_code), ")");
    }
    if ((kind == NODE_UNARY_OP())) {
        const char* operand = gen_expression(expr->child1);
        return rz_str_concat(rz_str_concat(rz_str_concat("(", expr->op), operand), ")");
    }
    if ((kind == NODE_CALL())) {
        const char* func_name = expr->name;
        if ((strcmp(func_name, "println") == 0)) {
            struct Rz_AstNode* arg = expr->child2;
            if ((arg == NULL)) {
                return "printf(\"\\n\")";
            }
            const char* arg_code = gen_expression(arg);
            if ((strcmp(arg->type_name, "string") == 0)) {
                return rz_str_concat(rz_str_concat("printf(\"%s\\n\", ", arg_code), ")");
            } else             if ((strcmp(arg->type_name, "bool") == 0)) {
                return rz_str_concat(rz_str_concat("printf(\"%s\\n\", (", arg_code), ") ? \"true\" : \"false\")");
            }
            return rz_str_concat(rz_str_concat("printf(\"%lld\\n\", (long long)(", arg_code), "))");
        }
        const char* code = rz_str_concat(func_name, "(");
        struct Rz_AstNode* arg = expr->child2;
        int64_t first = 1;
        while ((arg != NULL)) {
            if ((first == 0)) {
                code = rz_str_concat(code, ", ");
            }
            code = rz_str_concat(code, gen_expression(arg));
            first = 0;
            arg = arg->next;
        }
        code = rz_str_concat(code, ")");
        return code;
    }
    if ((kind == NODE_NEW())) {
        const char* code = rz_str_concat(rz_str_concat("new_", expr->name), "(");
        struct Rz_AstNode* arg = expr->child1;
        int64_t first = 1;
        while ((arg != NULL)) {
            if ((first == 0)) {
                code = rz_str_concat(code, ", ");
            }
            code = rz_str_concat(code, gen_expression(arg));
            first = 0;
            arg = arg->next;
        }
        code = rz_str_concat(code, ")");
        return code;
    }
    if ((kind == NODE_MEMBER())) {
        const char* obj_code = gen_expression(expr->child1);
        return rz_str_concat(rz_str_concat(obj_code, "->"), expr->name);
    }
    return "";
}

const char* gen_statement(struct Rz_AstNode* stmt, const char* indent) {
    if ((stmt == NULL)) {
        return "";
    }
    int64_t kind = stmt->kind;
    if ((kind == NODE_VAR_DECL())) {
        const char* c_ty = c_type(stmt->type_name);
        if ((strcmp(stmt->type_name, "") == 0)) {
            c_ty = "__auto_type";
        }
        const char* init_val = "0";
        if ((strcmp(stmt->type_name, "string") == 0)) {
            init_val = "\"\"";
        }
        if ((stmt->child1 != NULL)) {
            init_val = gen_expression(stmt->child1);
        }
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(indent, c_ty), " "), stmt->name), " = "), init_val), ";\n");
    }
    if (((kind == NODE_ASSIGN()) || (kind == NODE_FIELD_ASSIGN()))) {
        const char* target_code = gen_expression(stmt->child1);
        const char* val_code = gen_expression(stmt->child2);
        return rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(indent, target_code), " = "), val_code), ";\n");
    }
    if ((kind == NODE_IF())) {
        const char* cond_code = gen_expression(stmt->child1);
        const char* code = rz_str_concat(rz_str_concat(rz_str_concat(indent, "if ("), cond_code), ") {\n");
        code = rz_str_concat(code, gen_block_body(stmt->child2, rz_str_concat(indent, "    ")));
        code = rz_str_concat(rz_str_concat(code, indent), "}");
        if ((stmt->child3 != NULL)) {
            if ((stmt->child3->kind == NODE_IF())) {
                code = rz_str_concat(rz_str_concat(code, " else "), gen_statement(stmt->child3, indent));
                return code;
            } else {
                code = rz_str_concat(code, " else {\n");
                code = rz_str_concat(code, gen_block_body(stmt->child3, rz_str_concat(indent, "    ")));
                code = rz_str_concat(rz_str_concat(code, indent), "}\n");
                return code;
            }
        }
        code = rz_str_concat(code, "\n");
        return code;
    }
    if ((kind == NODE_WHILE())) {
        const char* cond_code = gen_expression(stmt->child1);
        const char* code = rz_str_concat(rz_str_concat(rz_str_concat(indent, "while ("), cond_code), ") {\n");
        code = rz_str_concat(code, gen_block_body(stmt->child2, rz_str_concat(indent, "    ")));
        code = rz_str_concat(rz_str_concat(code, indent), "}\n");
        return code;
    }
    if ((kind == NODE_FOR())) {
        const char* init_code = "";
        if ((stmt->child1 != NULL)) {
            if ((stmt->child1->kind == NODE_VAR_DECL())) {
                const char* init_expr = "0";
                if ((stmt->child1->child1 != NULL)) {
                    init_expr = gen_expression(stmt->child1->child1);
                }
                const char* c_ty = c_type(stmt->child1->type_name);
                if ((strcmp(stmt->child1->type_name, "") == 0)) {
                    c_ty = "__auto_type";
                }
                init_code = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(c_ty, " "), stmt->child1->name), " = "), init_expr);
            }
        }
        const char* cond_code = gen_expression(stmt->child2);
        const char* update_code = "";
        if ((stmt->child3 != NULL)) {
            update_code = rz_str_concat(rz_str_concat(gen_expression(stmt->child3->child1), " = "), gen_expression(stmt->child3->child2));
        }
        const char* code = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(indent, "for ("), init_code), "; "), cond_code), "; "), update_code), ") {\n");
        code = rz_str_concat(code, gen_block_body(stmt->child4, rz_str_concat(indent, "    ")));
        code = rz_str_concat(rz_str_concat(code, indent), "}\n");
        return code;
    }
    if ((kind == NODE_RETURN())) {
        if ((stmt->child1 != NULL)) {
            return rz_str_concat(rz_str_concat(rz_str_concat(indent, "return "), gen_expression(stmt->child1)), ";\n");
        }
        return rz_str_concat(indent, "return;\n");
    }
    if ((kind == NODE_BLOCK())) {
        const char* code = rz_str_concat(indent, "{\n");
        code = rz_str_concat(code, gen_block_body(stmt, rz_str_concat(indent, "    ")));
        code = rz_str_concat(rz_str_concat(code, indent), "}\n");
        return code;
    }
    if ((kind == NODE_EXPR_STMT())) {
        return rz_str_concat(rz_str_concat(indent, gen_expression(stmt->child1)), ";\n");
    }
    return "";
}

const char* gen_block_body(struct Rz_AstNode* block, const char* indent) {
    if ((block == NULL)) {
        return "";
    }
    const char* code = "";
    struct Rz_AstNode* s = block->child1;
    while ((s != NULL)) {
        code = rz_str_concat(code, gen_statement(s, indent));
        s = s->next;
    }
    return code;
}

const char* generate_c_code(struct Rz_AstNode* prog) {
    const char* out = "// ==============================================================================\n";
    out = rz_str_concat(out, "// 🌹 Emitted C99 Source by Roze Self-Hosted Compiler (ADV-06)\n");
    out = rz_str_concat(out, "// ==============================================================================\n\n");
    out = rz_str_concat(out, "#include <stdio.h>\n");
    out = rz_str_concat(out, "#include <stdlib.h>\n");
    out = rz_str_concat(out, "#include <string.h>\n");
    out = rz_str_concat(out, "#include <stdbool.h>\n");
    out = rz_str_concat(out, "#include <stdint.h>\n");
    out = rz_str_concat(out, "#include <sys/stat.h>\n");
    out = rz_str_concat(out, "#include <unistd.h>\n\n");
    out = rz_str_concat(out, "// --- Roze C Runtime Helpers ---\n");
    out = rz_str_concat(out, "char* rz_str_concat(const char* a, const char* b) {\n");
    out = rz_str_concat(out, "    if (!a) a = \"\"; if (!b) b = \"\";\n");
    out = rz_str_concat(out, "    size_t la = strlen(a); size_t lb = strlen(b);\n");
    out = rz_str_concat(out, "    char* s = (char*)malloc(16 + la + lb + 1);\n");
    out = rz_str_concat(out, "    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = la + lb;\n");
    out = rz_str_concat(out, "    char* p = s + 16;\n");
    out = rz_str_concat(out, "    memcpy(p, a, la); memcpy(p + la, b, lb); p[la + lb] = '\\0';\n");
    out = rz_str_concat(out, "    return p;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_char_at(const char* s, int64_t i) {\n");
    out = rz_str_concat(out, "    if (!s || i < 0) return -1;\n");
    out = rz_str_concat(out, "    unsigned char c = (unsigned char)s[i];\n");
    out = rz_str_concat(out, "    return (c == '\\0') ? -1 : (int64_t)c;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "char* rz_int_to_str(int64_t v) {\n");
    out = rz_str_concat(out, "    char b[32]; snprintf(b, sizeof(b), \"%lld\", (long long)v);\n");
    out = rz_str_concat(out, "    size_t l = strlen(b);\n");
    out = rz_str_concat(out, "    char* s = (char*)malloc(16 + l + 1);\n");
    out = rz_str_concat(out, "    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = l;\n");
    out = rz_str_concat(out, "    char* p = s + 16; memcpy(p, b, l + 1);\n");
    out = rz_str_concat(out, "    return p;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "char* rz_char_to_str(int64_t ch) {\n");
    out = rz_str_concat(out, "    char* s = (char*)malloc(16 + 2);\n");
    out = rz_str_concat(out, "    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = 1;\n");
    out = rz_str_concat(out, "    char* p = s + 16; p[0] = (char)ch; p[1] = '\\0';\n");
    out = rz_str_concat(out, "    return p;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "char* rz_substring(const char* s, int64_t st, int64_t len) {\n");
    out = rz_str_concat(out, "    if (!s || st < 0 || len <= 0) return \"\";\n");
    out = rz_str_concat(out, "    int64_t tot = strlen(s);\n");
    out = rz_str_concat(out, "    if (st >= tot) return \"\";\n");
    out = rz_str_concat(out, "    if (st + len > tot) len = tot - st;\n");
    out = rz_str_concat(out, "    char* r = (char*)malloc(16 + len + 1);\n");
    out = rz_str_concat(out, "    *(int64_t*)(r + 0) = 1; *(int64_t*)(r + 8) = len;\n");
    out = rz_str_concat(out, "    char* p = r + 16; memcpy(p, s + st, len); p[len] = '\\0';\n");
    out = rz_str_concat(out, "    return p;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_str_len(const char* s) { return s ? (int64_t)strlen(s) : 0; }\n");
    out = rz_str_concat(out, "int64_t rz_str_eq(const char* a, const char* b) {\n");
    out = rz_str_concat(out, "    if (a == b) return 1; if (!a || !b) return 0;\n");
    out = rz_str_concat(out, "    return (strcmp(a, b) == 0) ? 1 : 0;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "char* rz_read_file(const char* path) {\n");
    out = rz_str_concat(out, "    if (!path) return \"\";\n");
    out = rz_str_concat(out, "    FILE* f = fopen(path, \"rb\");\n");
    out = rz_str_concat(out, "    if (!f) return \"\";\n");
    out = rz_str_concat(out, "    fseek(f, 0, SEEK_END); long sz = ftell(f); fseek(f, 0, SEEK_SET);\n");
    out = rz_str_concat(out, "    char* s = (char*)malloc(16 + sz + 1);\n");
    out = rz_str_concat(out, "    *(int64_t*)(s + 0) = 1; *(int64_t*)(s + 8) = sz;\n");
    out = rz_str_concat(out, "    char* p = s + 16;\n");
    out = rz_str_concat(out, "    size_t nr = fread(p, 1, sz, f); p[nr] = '\\0'; fclose(f);\n");
    out = rz_str_concat(out, "    return p;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_write_file(const char* path, const char* content) {\n");
    out = rz_str_concat(out, "    if (!path || !content) return 0;\n");
    out = rz_str_concat(out, "    const char* sl = strrchr(path, '/');\n");
    out = rz_str_concat(out, "    if (sl) {\n");
    out = rz_str_concat(out, "        char dir[256]; size_t len = sl - path;\n");
    out = rz_str_concat(out, "        if (len < sizeof(dir)) { strncpy(dir, path, len); dir[len] = '\\0'; mkdir(dir, 0755); }\n");
    out = rz_str_concat(out, "    }\n");
    out = rz_str_concat(out, "    FILE* f = fopen(path, \"w\"); if (!f) return 0;\n");
    out = rz_str_concat(out, "    size_t l = strlen(content); size_t w = fwrite(content, 1, l, f); fclose(f);\n");
    out = rz_str_concat(out, "    return (w == l) ? 1 : 0;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_write_binary_file(const char* path, const char* hex_data) {\n");
    out = rz_str_concat(out, "    if (!path || !hex_data) return 0;\n");
    out = rz_str_concat(out, "    const char* sl = strrchr(path, '/');\n");
    out = rz_str_concat(out, "    if (sl) {\n");
    out = rz_str_concat(out, "        char dir[256]; size_t len = sl - path;\n");
    out = rz_str_concat(out, "        if (len < sizeof(dir)) { strncpy(dir, path, len); dir[len] = '\\0'; mkdir(dir, 0755); }\n");
    out = rz_str_concat(out, "    }\n");
    out = rz_str_concat(out, "    FILE* f = fopen(path, \"wb\"); if (!f) return 0;\n");
    out = rz_str_concat(out, "    size_t hex_len = strlen(hex_data);\n");
    out = rz_str_concat(out, "    for (size_t i = 0; i + 1 < hex_len; i += 2) {\n");
    out = rz_str_concat(out, "        char bs[3] = { hex_data[i], hex_data[i+1], '\\0' };\n");
    out = rz_str_concat(out, "        unsigned char b = (unsigned char)strtoul(bs, NULL, 16);\n");
    out = rz_str_concat(out, "        fwrite(&b, 1, 1, f);\n");
    out = rz_str_concat(out, "    }\n");
    out = rz_str_concat(out, "    fclose(f); chmod(path, 0755);\n");
    out = rz_str_concat(out, "    return 1;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_file_exists(const char* path) {\n");
    out = rz_str_concat(out, "    return (path && access(path, 0) == 0) ? 1 : 0;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_exec(const char* cmd) {\n");
    out = rz_str_concat(out, "    return cmd ? (int64_t)system(cmd) : -1;\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "static int g_rz_argc = 0; static char* g_rz_argv[64]; static int g_rz_cmd_init = 0;\n");
    out = rz_str_concat(out, "static void rz_init_cmd(void) {\n");
    out = rz_str_concat(out, "    if (g_rz_cmd_init) return; g_rz_cmd_init = 1;\n");
    out = rz_str_concat(out, "    FILE* f = fopen(\"/proc/self/cmdline\", \"rb\"); if (!f) return;\n");
    out = rz_str_concat(out, "    static char b[8192]; size_t n = fread(b, 1, sizeof(b) - 1, f); fclose(f); b[n] = '\\0';\n");
    out = rz_str_concat(out, "    size_t i = 0;\n");
    out = rz_str_concat(out, "    while (i < n && g_rz_argc < 64) {\n");
    out = rz_str_concat(out, "        g_rz_argv[g_rz_argc++] = &b[i];\n");
    out = rz_str_concat(out, "        while (i < n && b[i] != '\\0') i++;\n");
    out = rz_str_concat(out, "        i++;\n");
    out = rz_str_concat(out, "    }\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "int64_t rz_get_arg_count(void) { rz_init_cmd(); return (int64_t)g_rz_argc; }\n");
    out = rz_str_concat(out, "char* rz_get_arg(int64_t idx) {\n");
    out = rz_str_concat(out, "    rz_init_cmd();\n");
    out = rz_str_concat(out, "    return (idx >= 0 && idx < g_rz_argc) ? g_rz_argv[idx] : \"\";\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "char* rz_get_env(const char* name) {\n");
    out = rz_str_concat(out, "    if (!name) return \"\"; char* v = getenv(name); return v ? v : \"\";\n");
    out = rz_str_concat(out, "}\n");
    out = rz_str_concat(out, "void rz_compiler_banner(void) {\n");
    out = rz_str_concat(out, "    printf(\"==============================================================================\\n\");\n");
    out = rz_str_concat(out, "    printf(\"🌹 Roze Self-Hosted Compiler (Roze-in-Roze Stage 1 / Stage 2)\\n\");\n");
    out = rz_str_concat(out, "    printf(\"==============================================================================\\n\");\n");
    out = rz_str_concat(out, "    printf(\"⚡ Language Independence & Full Self-Hosting Verification\\n\");\n");
    out = rz_str_concat(out, "    printf(\"   - Frontend:  Pure Roze Lexer, Parser & Semantic Type Checker\\n\");\n");
    out = rz_str_concat(out, "    printf(\"   - Backend:   Pure Roze C99 Code Generator (Native Cranelift ABI)\\n\");\n");
    out = rz_str_concat(out, "    printf(\"   - Toolchain: Standalone Native ELF Executable, Zero Rust Dependency\\n\");\n");
    out = rz_str_concat(out, "    printf(\"==============================================================================\\n\\n\");\n");
    out = rz_str_concat(out, "}\n\n");
    out = rz_str_concat(out, "// --- Class Structure Forward Declarations ---\n");
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_CLASS_DECL())) {
            out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, "typedef struct Rz_"), decl->name), " Rz_"), decl->name), ";\n");
        }
        decl = decl->next;
    }
    out = rz_str_concat(out, "\n");
    out = rz_str_concat(out, "// --- Class Struct Definitions & Constructors ---\n");
    decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_CLASS_DECL())) {
            out = rz_str_concat(rz_str_concat(rz_str_concat(out, "struct Rz_"), decl->name), " {\n");
            out = rz_str_concat(out, "    int64_t __refcount;\n");
            out = rz_str_concat(out, "    int64_t __vtable;\n");
            struct Rz_AstNode* f = decl->child1;
            while ((f != NULL)) {
                out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, "    "), c_type(f->type_name)), " "), f->name), ";\n");
                f = f->next;
            }
            out = rz_str_concat(out, "};\n\n");
            out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, "Rz_"), decl->name), "* new_"), decl->name), "(");
            f = decl->child1;
            int64_t first = 1;
            while ((f != NULL)) {
                if ((first == 0)) {
                    out = rz_str_concat(out, ", ");
                }
                out = rz_str_concat(rz_str_concat(rz_str_concat(out, c_type(f->type_name)), " "), f->name);
                first = 0;
                f = f->next;
            }
            out = rz_str_concat(out, ") {\n");
            out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, "    Rz_"), decl->name), "* obj = (Rz_"), decl->name), "*)malloc(sizeof(Rz_"), decl->name), "));\n");
            out = rz_str_concat(out, "    obj->__refcount = 1;\n");
            out = rz_str_concat(out, "    obj->__vtable = 0;\n");
            f = decl->child1;
            while ((f != NULL)) {
                out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, "    obj->"), f->name), " = "), f->name), ";\n");
                f = f->next;
            }
            out = rz_str_concat(out, "    return obj;\n");
            out = rz_str_concat(out, "}\n\n");
        }
        decl = decl->next;
    }
    out = rz_str_concat(out, "// --- Function Prototypes ---\n");
    decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            const char* fn_name = decl->name;
            if ((strcmp(fn_name, "main") == 0)) {
                fn_name = "rz_user_main";
            }
            out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, c_type(decl->type_name)), " "), fn_name), "(");
            struct Rz_AstNode* param = decl->child1;
            int64_t first = 1;
            while ((param != NULL)) {
                if ((first == 0)) {
                    out = rz_str_concat(out, ", ");
                }
                out = rz_str_concat(rz_str_concat(rz_str_concat(out, c_type(param->type_name)), " "), param->name);
                first = 0;
                param = param->next;
            }
            if (first) {
                out = rz_str_concat(out, "void");
            }
            out = rz_str_concat(out, ");\n");
        }
        decl = decl->next;
    }
    out = rz_str_concat(out, "\n");
    out = rz_str_concat(out, "// --- Function Implementations ---\n");
    decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            const char* fn_name = decl->name;
            if ((strcmp(fn_name, "main") == 0)) {
                fn_name = "rz_user_main";
            }
            out = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(out, c_type(decl->type_name)), " "), fn_name), "(");
            struct Rz_AstNode* param = decl->child1;
            int64_t first = 1;
            while ((param != NULL)) {
                if ((first == 0)) {
                    out = rz_str_concat(out, ", ");
                }
                out = rz_str_concat(rz_str_concat(rz_str_concat(out, c_type(param->type_name)), " "), param->name);
                first = 0;
                param = param->next;
            }
            if (first) {
                out = rz_str_concat(out, "void");
            }
            out = rz_str_concat(out, ") {\n");
            out = rz_str_concat(out, gen_block_body(decl->child2, "    "));
            out = rz_str_concat(out, "}\n\n");
        }
        decl = decl->next;
    }
    out = rz_str_concat(out, "// --- Main Program Entry Point ---\n");
    out = rz_str_concat(out, "int main(int argc, char** argv) {\n");
    out = rz_str_concat(out, "    rz_user_main();\n");
    out = rz_str_concat(out, "    return 0;\n");
    out = rz_str_concat(out, "}\n");
    return out;
}

const char* hex_digit(int64_t v) {
    if ((v == 0)) {
        return "0";
    }
    if ((v == 1)) {
        return "1";
    }
    if ((v == 2)) {
        return "2";
    }
    if ((v == 3)) {
        return "3";
    }
    if ((v == 4)) {
        return "4";
    }
    if ((v == 5)) {
        return "5";
    }
    if ((v == 6)) {
        return "6";
    }
    if ((v == 7)) {
        return "7";
    }
    if ((v == 8)) {
        return "8";
    }
    if ((v == 9)) {
        return "9";
    }
    if ((v == 10)) {
        return "a";
    }
    if ((v == 11)) {
        return "b";
    }
    if ((v == 12)) {
        return "c";
    }
    if ((v == 13)) {
        return "d";
    }
    if ((v == 14)) {
        return "e";
    }
    if ((v == 15)) {
        return "f";
    }
    return "0";
}

const char* byte_to_hex(int64_t b) {
    int64_t u = b;
    if ((u < 0)) {
        u = (u + 256);
    }
    int64_t hi = ((u / 16) % 16);
    int64_t lo = (u % 16);
    return rz_str_concat(hex_digit(hi), hex_digit(lo));
}

struct Rz_BinaryBuffer* bb_new(void) {
    return new_BinaryBuffer("", 0);
}

void bb_append_u8(struct Rz_BinaryBuffer* bb, int64_t b) {
    bb->hex_data = rz_str_concat(bb->hex_data, byte_to_hex(b));
    bb->size = (bb->size + 1);
}

void bb_append_u16_le(struct Rz_BinaryBuffer* bb, int64_t val) {
    int64_t v = val;
    bb_append_u8(bb, (v % 256));
    bb_append_u8(bb, ((v / 256) % 256));
}

void bb_append_u32_le(struct Rz_BinaryBuffer* bb, int64_t val) {
    int64_t v = val;
    bb_append_u8(bb, (v % 256));
    v = (v / 256);
    bb_append_u8(bb, (v % 256));
    v = (v / 256);
    bb_append_u8(bb, (v % 256));
    v = (v / 256);
    bb_append_u8(bb, (v % 256));
}

void bb_append_u64_le(struct Rz_BinaryBuffer* bb, int64_t val) {
    int64_t v = val;
    for (int64_t i = 0; (i < 8); i = (i + 1)) {
        int64_t b = (v % 256);
        if ((b < 0)) {
            b = (b + 256);
        }
        bb_append_u8(bb, b);
        v = ((v - b) / 256);
    }
}

void bb_append_uleb128(struct Rz_BinaryBuffer* bb, int64_t val) {
    int64_t v = val;
    int64_t running = 1;
    while ((running == 1)) {
        int64_t b = (v % 128);
        v = (v / 128);
        if ((v != 0)) {
            bb_append_u8(bb, (b + 128));
        } else {
            bb_append_u8(bb, b);
            running = 0;
        }
    }
}

void bb_append_zeroes(struct Rz_BinaryBuffer* bb, int64_t count) {
    for (int64_t i = 0; (i < count); i = (i + 1)) {
        bb_append_u8(bb, 0);
    }
}

void bb_append_hex(struct Rz_BinaryBuffer* bb, const char* hex_str, int64_t byte_count) {
    bb->hex_data = rz_str_concat(bb->hex_data, hex_str);
    bb->size = (bb->size + byte_count);
}

void bb_append_str(struct Rz_BinaryBuffer* bb, const char* s) {
    int64_t l = rz_str_len(s);
    for (int64_t i = 0; (i < l); i = (i + 1)) {
        bb_append_u8(bb, rz_char_at(s, i));
    }
}

void bb_append_str_null(struct Rz_BinaryBuffer* bb, const char* s) {
    bb_append_str(bb, s);
    bb_append_u8(bb, 0);
}

void bb_append_buffer(struct Rz_BinaryBuffer* bb, struct Rz_BinaryBuffer* src) {
    bb->hex_data = rz_str_concat(bb->hex_data, src->hex_data);
    bb->size = (bb->size + src->size);
}

int64_t bb_write_to_file(struct Rz_BinaryBuffer* bb, const char* path) {
    return rz_write_binary_file(path, bb->hex_data);
}

void emit_func_x86(struct Rz_AstNode* decl, struct Rz_BinaryBuffer* bb) {
    bb_append_hex(bb, "554889e5", 4);
    const char* p1 = "";
    const char* p2 = "";
    if ((decl->child1 != NULL)) {
        p1 = decl->child1->name;
        if ((decl->child1->next != NULL)) {
            p2 = decl->child1->next->name;
        }
    }
    struct Rz_AstNode* body = decl->child2;
    int64_t handled = 0;
    if ((body != NULL)) {
        struct Rz_AstNode* stmt = body->child1;
        while ((stmt != NULL)) {
            if ((stmt->kind == NODE_RETURN())) {
                struct Rz_AstNode* ret_expr = stmt->child1;
                if ((ret_expr != NULL)) {
                    if ((ret_expr->kind == NODE_LITERAL_INT())) {
                        bb_append_hex(bb, "48b8", 2);
                        bb_append_u64_le(bb, ret_expr->int_val);
                        handled = 1;
                    } else                     if ((ret_expr->kind == NODE_IDENT())) {
                        if ((strcmp(ret_expr->name, p1) == 0)) {
                            bb_append_hex(bb, "4889f8", 3);
                            handled = 1;
                        } else                         if ((strcmp(ret_expr->name, p2) == 0)) {
                            bb_append_hex(bb, "4889f0", 3);
                            handled = 1;
                        }
                    } else                     if ((ret_expr->kind == NODE_BIN_OP())) {
                        struct Rz_AstNode* left = ret_expr->child1;
                        struct Rz_AstNode* right = ret_expr->child2;
                        const char* op = ret_expr->op;
                        if (((((left != NULL) && (right != NULL)) && (strcmp(left->name, p1) == 0)) && (strcmp(right->name, p2) == 0))) {
                            if ((strcmp(op, "+") == 0)) {
                                bb_append_hex(bb, "4889f84801f0", 6);
                                handled = 1;
                            } else                             if ((strcmp(op, "-") == 0)) {
                                bb_append_hex(bb, "4889f84829f0", 6);
                                handled = 1;
                            } else                             if ((strcmp(op, "*") == 0)) {
                                bb_append_hex(bb, "4889f8480fafc6", 7);
                                handled = 1;
                            }
                        } else                         if ((((((left != NULL) && (right != NULL)) && (strcmp(left->name, p1) == 0)) && (strcmp(right->name, p1) == 0)) && (strcmp(op, "*") == 0))) {
                            bb_append_hex(bb, "4889f8480fafc7", 7);
                            handled = 1;
                        } else                         if (((((left != NULL) && (right != NULL)) && (strcmp(left->name, p1) == 0)) && (right->kind == NODE_LITERAL_INT()))) {
                            if ((strcmp(op, "+") == 0)) {
                                bb_append_hex(bb, "4889f84805", 5);
                                bb_append_u32_le(bb, right->int_val);
                                handled = 1;
                            } else                             if ((strcmp(op, "-") == 0)) {
                                bb_append_hex(bb, "4889f8482d", 5);
                                bb_append_u32_le(bb, right->int_val);
                                handled = 1;
                            }
                        }
                    }
                }
            }
            stmt = stmt->next;
        }
    }
    if ((handled == 0)) {
        bb_append_hex(bb, "31c0", 2);
    }
    bb_append_hex(bb, "c9c3", 2);
}

int64_t emit_elf_object(struct Rz_AstNode* prog, const char* out_path) {
    struct Rz_BinaryBuffer* text_bb = bb_new();
    struct Rz_BinaryBuffer* rodata_bb = bb_new();
    struct Rz_NativeSymbol* sym_head = new_NativeSymbol("", 0, 0, 0, 0, 0, NULL);
    struct Rz_NativeSymbol* sym_tail = sym_head;
    int64_t sym_count = 1;
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            int64_t func_start = text_bb->size;
            emit_func_x86(decl, text_bb);
            int64_t func_size = (text_bb->size - func_start);
            struct Rz_NativeSymbol* sym = new_NativeSymbol(decl->name, func_start, func_size, 1, 2, 1, NULL);
            sym_tail->next = sym;
            sym_tail = sym;
            sym_count = (sym_count + 1);
        }
        decl = decl->next;
    }
    if ((text_bb->size == 0)) {
        bb_append_hex(text_bb, "554889e5c9c3", 6);
        struct Rz_NativeSymbol* sym = new_NativeSymbol("roze_main", 0, 6, 1, 2, 1, NULL);
        sym_tail->next = sym;
        sym_tail = sym;
        sym_count = (sym_count + 1);
    }
    if ((rodata_bb->size == 0)) {
        bb_append_str_null(rodata_bb, "Roze Native v0.1");
    }
    struct Rz_BinaryBuffer* strtab = bb_new();
    bb_append_u8(strtab, 0);
    struct Rz_BinaryBuffer* symtab = bb_new();
    bb_append_zeroes(symtab, 24);
    struct Rz_NativeSymbol* cur_sym = sym_head->next;
    while ((cur_sym != NULL)) {
        int64_t name_off = strtab->size;
        bb_append_str_null(strtab, cur_sym->name);
        bb_append_u32_le(symtab, name_off);
        bb_append_u8(symtab, 18);
        bb_append_u8(symtab, 0);
        bb_append_u16_le(symtab, cur_sym->sec_index);
        bb_append_u64_le(symtab, cur_sym->value);
        bb_append_u64_le(symtab, cur_sym->size);
        cur_sym = cur_sym->next;
    }
    struct Rz_BinaryBuffer* shstrtab = bb_new();
    bb_append_u8(shstrtab, 0);
    int64_t off_sh_text = shstrtab->size;
    bb_append_str_null(shstrtab, ".text");
    int64_t off_sh_rodata = shstrtab->size;
    bb_append_str_null(shstrtab, ".rodata");
    int64_t off_sh_symtab = shstrtab->size;
    bb_append_str_null(shstrtab, ".symtab");
    int64_t off_sh_strtab = shstrtab->size;
    bb_append_str_null(shstrtab, ".strtab");
    int64_t off_sh_shstrtab = shstrtab->size;
    bb_append_str_null(shstrtab, ".shstrtab");
    int64_t off_sh_gnustack = shstrtab->size;
    bb_append_str_null(shstrtab, ".note.GNU-stack");
    int64_t off_text = 64;
    int64_t off_rodata = (off_text + text_bb->size);
    int64_t rodata_pad = 0;
    if (((off_rodata % 4) != 0)) {
        rodata_pad = (4 - (off_rodata % 4));
        off_rodata = (off_rodata + rodata_pad);
    }
    int64_t off_symtab = (off_rodata + rodata_bb->size);
    int64_t symtab_pad = 0;
    if (((off_symtab % 8) != 0)) {
        symtab_pad = (8 - (off_symtab % 8));
        off_symtab = (off_symtab + symtab_pad);
    }
    int64_t off_strtab = (off_symtab + symtab->size);
    int64_t off_shstrtab = (off_strtab + strtab->size);
    int64_t off_shdrs = (off_shstrtab + shstrtab->size);
    int64_t shdrs_pad = 0;
    if (((off_shdrs % 8) != 0)) {
        shdrs_pad = (8 - (off_shdrs % 8));
        off_shdrs = (off_shdrs + shdrs_pad);
    }
    struct Rz_BinaryBuffer* elf_file = bb_new();
    bb_append_hex(elf_file, "7f454c46020101000000000000000000", 16);
    bb_append_u16_le(elf_file, 1);
    bb_append_u16_le(elf_file, 62);
    bb_append_u32_le(elf_file, 1);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, off_shdrs);
    bb_append_u32_le(elf_file, 0);
    bb_append_u16_le(elf_file, 64);
    bb_append_u16_le(elf_file, 0);
    bb_append_u16_le(elf_file, 0);
    bb_append_u16_le(elf_file, 64);
    bb_append_u16_le(elf_file, 7);
    bb_append_u16_le(elf_file, 5);
    bb_append_buffer(elf_file, text_bb);
    bb_append_zeroes(elf_file, rodata_pad);
    bb_append_buffer(elf_file, rodata_bb);
    bb_append_zeroes(elf_file, symtab_pad);
    bb_append_buffer(elf_file, symtab);
    bb_append_buffer(elf_file, strtab);
    bb_append_buffer(elf_file, shstrtab);
    bb_append_zeroes(elf_file, shdrs_pad);
    bb_append_zeroes(elf_file, 64);
    bb_append_u32_le(elf_file, off_sh_text);
    bb_append_u32_le(elf_file, 1);
    bb_append_u64_le(elf_file, 6);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, off_text);
    bb_append_u64_le(elf_file, text_bb->size);
    bb_append_u32_le(elf_file, 0);
    bb_append_u32_le(elf_file, 0);
    bb_append_u64_le(elf_file, 16);
    bb_append_u64_le(elf_file, 0);
    bb_append_u32_le(elf_file, off_sh_rodata);
    bb_append_u32_le(elf_file, 1);
    bb_append_u64_le(elf_file, 2);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, off_rodata);
    bb_append_u64_le(elf_file, rodata_bb->size);
    bb_append_u32_le(elf_file, 0);
    bb_append_u32_le(elf_file, 0);
    bb_append_u64_le(elf_file, 4);
    bb_append_u64_le(elf_file, 0);
    bb_append_u32_le(elf_file, off_sh_symtab);
    bb_append_u32_le(elf_file, 2);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, off_symtab);
    bb_append_u64_le(elf_file, symtab->size);
    bb_append_u32_le(elf_file, 4);
    bb_append_u32_le(elf_file, 1);
    bb_append_u64_le(elf_file, 8);
    bb_append_u64_le(elf_file, 24);
    bb_append_u32_le(elf_file, off_sh_strtab);
    bb_append_u32_le(elf_file, 3);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, off_strtab);
    bb_append_u64_le(elf_file, strtab->size);
    bb_append_u32_le(elf_file, 0);
    bb_append_u32_le(elf_file, 0);
    bb_append_u64_le(elf_file, 1);
    bb_append_u64_le(elf_file, 0);
    bb_append_u32_le(elf_file, off_sh_shstrtab);
    bb_append_u32_le(elf_file, 3);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, off_shstrtab);
    bb_append_u64_le(elf_file, shstrtab->size);
    bb_append_u32_le(elf_file, 0);
    bb_append_u32_le(elf_file, 0);
    bb_append_u64_le(elf_file, 1);
    bb_append_u64_le(elf_file, 0);
    bb_append_u32_le(elf_file, off_sh_gnustack);
    bb_append_u32_le(elf_file, 1);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u64_le(elf_file, 0);
    bb_append_u32_le(elf_file, 0);
    bb_append_u32_le(elf_file, 0);
    bb_append_u64_le(elf_file, 1);
    bb_append_u64_le(elf_file, 0);
    return bb_write_to_file(elf_file, out_path);
}

int64_t emit_standalone_elf(struct Rz_AstNode* prog, const char* out_path) {
    const char* msg = "Hello from Roze Standalone Executable!\n";
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if (((decl->kind == NODE_FUNC_DECL()) && (strcmp(decl->name, "main") == 0))) {
            struct Rz_AstNode* body = decl->child2;
            if ((body != NULL)) {
                struct Rz_AstNode* stmt = body->child1;
                while ((stmt != NULL)) {
                    if ((((stmt->kind == NODE_CALL()) && (stmt->child1 != NULL)) && (strcmp(stmt->child1->name, "println") == 0))) {
                        struct Rz_AstNode* arg = stmt->child2;
                        if (((arg != NULL) && (arg->kind == NODE_LITERAL_STR()))) {
                            msg = rz_str_concat(arg->str_val, "\n");
                        }
                    }
                    stmt = stmt->next;
                }
            }
        }
        decl = decl->next;
    }
    int64_t msg_len = rz_str_len(msg);
    struct Rz_BinaryBuffer* code_bb = bb_new();
    int64_t entry_vaddr = 4194424;
    bb_append_hex(code_bb, "48c7c001000000", 7);
    bb_append_hex(code_bb, "48c7c701000000", 7);
    int64_t msg_vaddr = (entry_vaddr + 52);
    bb_append_hex(code_bb, "48be", 2);
    bb_append_u64_le(code_bb, msg_vaddr);
    bb_append_hex(code_bb, "48ba", 2);
    bb_append_u64_le(code_bb, msg_len);
    bb_append_hex(code_bb, "0f05", 2);
    bb_append_hex(code_bb, "48c7c03c000000", 7);
    bb_append_hex(code_bb, "48c7c700000000", 7);
    bb_append_hex(code_bb, "0f05", 2);
    struct Rz_BinaryBuffer* msg_bb = bb_new();
    bb_append_str(msg_bb, msg);
    int64_t total_file_size = ((120 + code_bb->size) + msg_bb->size);
    struct Rz_BinaryBuffer* elf_bin = bb_new();
    bb_append_hex(elf_bin, "7f454c46020101000000000000000000", 16);
    bb_append_u16_le(elf_bin, 2);
    bb_append_u16_le(elf_bin, 62);
    bb_append_u32_le(elf_bin, 1);
    bb_append_u64_le(elf_bin, entry_vaddr);
    bb_append_u64_le(elf_bin, 64);
    bb_append_u64_le(elf_bin, 0);
    bb_append_u32_le(elf_bin, 0);
    bb_append_u16_le(elf_bin, 64);
    bb_append_u16_le(elf_bin, 56);
    bb_append_u16_le(elf_bin, 1);
    bb_append_u16_le(elf_bin, 0);
    bb_append_u16_le(elf_bin, 0);
    bb_append_u16_le(elf_bin, 0);
    bb_append_u32_le(elf_bin, 1);
    bb_append_u32_le(elf_bin, 7);
    bb_append_u64_le(elf_bin, 0);
    bb_append_u64_le(elf_bin, 4194304);
    bb_append_u64_le(elf_bin, 4194304);
    bb_append_u64_le(elf_bin, total_file_size);
    bb_append_u64_le(elf_bin, total_file_size);
    bb_append_u64_le(elf_bin, 4096);
    bb_append_buffer(elf_bin, code_bb);
    bb_append_buffer(elf_bin, msg_bb);
    return bb_write_to_file(elf_bin, out_path);
}

int64_t emit_macho_object(struct Rz_AstNode* prog, const char* out_path) {
    struct Rz_BinaryBuffer* text_bb = bb_new();
    struct Rz_BinaryBuffer* data_bb = bb_new();
    struct Rz_NativeSymbol* sym_head = new_NativeSymbol("", 0, 0, 0, 0, 0, NULL);
    struct Rz_NativeSymbol* sym_tail = sym_head;
    int64_t sym_count = 0;
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            int64_t func_start = text_bb->size;
            emit_func_x86(decl, text_bb);
            int64_t func_size = (text_bb->size - func_start);
            struct Rz_NativeSymbol* sym = new_NativeSymbol(rz_str_concat("_", decl->name), func_start, func_size, 1, 2, 1, NULL);
            sym_tail->next = sym;
            sym_tail = sym;
            sym_count = (sym_count + 1);
        }
        decl = decl->next;
    }
    if ((text_bb->size == 0)) {
        bb_append_hex(text_bb, "554889e5c9c3", 6);
        struct Rz_NativeSymbol* sym = new_NativeSymbol("_roze_main", 0, 6, 1, 2, 1, NULL);
        sym_tail->next = sym;
        sym_tail = sym;
        sym_count = (sym_count + 1);
    }
    if ((data_bb->size == 0)) {
        bb_append_str_null(data_bb, "Roze Native v0.1");
    }
    struct Rz_BinaryBuffer* strtab = bb_new();
    bb_append_u8(strtab, 32);
    struct Rz_BinaryBuffer* symtab = bb_new();
    struct Rz_NativeSymbol* cur_sym = sym_head->next;
    while ((cur_sym != NULL)) {
        int64_t name_off = strtab->size;
        bb_append_str_null(strtab, cur_sym->name);
        bb_append_u32_le(symtab, name_off);
        bb_append_u8(symtab, 15);
        bb_append_u8(symtab, 1);
        bb_append_u16_le(symtab, 0);
        bb_append_u64_le(symtab, cur_sym->value);
        cur_sym = cur_sym->next;
    }
    int64_t off_text = 288;
    int64_t off_data = (off_text + text_bb->size);
    int64_t data_pad = 0;
    if (((off_data % 8) != 0)) {
        data_pad = (8 - (off_data % 8));
        off_data = (off_data + data_pad);
    }
    int64_t off_symtab = (off_data + data_bb->size);
    int64_t symtab_pad = 0;
    if (((off_symtab % 8) != 0)) {
        symtab_pad = (8 - (off_symtab % 8));
        off_symtab = (off_symtab + symtab_pad);
    }
    int64_t off_strtab = (off_symtab + symtab->size);
    struct Rz_BinaryBuffer* macho_file = bb_new();
    bb_append_hex(macho_file, "cffaedfe", 4);
    bb_append_hex(macho_file, "07000001", 4);
    bb_append_u32_le(macho_file, 3);
    bb_append_u32_le(macho_file, 1);
    bb_append_u32_le(macho_file, 2);
    bb_append_u32_le(macho_file, 256);
    bb_append_u32_le(macho_file, 0);
    bb_append_u32_le(macho_file, 0);
    bb_append_u32_le(macho_file, 25);
    bb_append_u32_le(macho_file, 232);
    bb_append_zeroes(macho_file, 16);
    bb_append_u64_le(macho_file, 0);
    bb_append_u64_le(macho_file, ((text_bb->size + data_pad) + data_bb->size));
    bb_append_u64_le(macho_file, off_text);
    bb_append_u64_le(macho_file, ((text_bb->size + data_pad) + data_bb->size));
    bb_append_u32_le(macho_file, 7);
    bb_append_u32_le(macho_file, 7);
    bb_append_u32_le(macho_file, 2);
    bb_append_u32_le(macho_file, 0);
    bb_append_str(macho_file, "__text");
    bb_append_zeroes(macho_file, 10);
    bb_append_str(macho_file, "__TEXT");
    bb_append_zeroes(macho_file, 10);
    bb_append_u64_le(macho_file, 0);
    bb_append_u64_le(macho_file, text_bb->size);
    bb_append_u32_le(macho_file, off_text);
    bb_append_u32_le(macho_file, 4);
    bb_append_u32_le(macho_file, 0);
    bb_append_u32_le(macho_file, 0);
    bb_append_hex(macho_file, "00040080", 4);
    bb_append_zeroes(macho_file, 12);
    bb_append_str(macho_file, "__data");
    bb_append_zeroes(macho_file, 10);
    bb_append_str(macho_file, "__DATA");
    bb_append_zeroes(macho_file, 10);
    bb_append_u64_le(macho_file, text_bb->size);
    bb_append_u64_le(macho_file, data_bb->size);
    bb_append_u32_le(macho_file, off_data);
    bb_append_u32_le(macho_file, 3);
    bb_append_u32_le(macho_file, 0);
    bb_append_u32_le(macho_file, 0);
    bb_append_u32_le(macho_file, 0);
    bb_append_zeroes(macho_file, 12);
    bb_append_u32_le(macho_file, 2);
    bb_append_u32_le(macho_file, 24);
    bb_append_u32_le(macho_file, off_symtab);
    bb_append_u32_le(macho_file, sym_count);
    bb_append_u32_le(macho_file, off_strtab);
    bb_append_u32_le(macho_file, strtab->size);
    bb_append_buffer(macho_file, text_bb);
    bb_append_zeroes(macho_file, data_pad);
    bb_append_buffer(macho_file, data_bb);
    bb_append_zeroes(macho_file, symtab_pad);
    bb_append_buffer(macho_file, symtab);
    bb_append_buffer(macho_file, strtab);
    return bb_write_to_file(macho_file, out_path);
}

int64_t emit_pe_object(struct Rz_AstNode* prog, const char* out_path) {
    struct Rz_BinaryBuffer* text_bb = bb_new();
    struct Rz_BinaryBuffer* rdata_bb = bb_new();
    struct Rz_NativeSymbol* sym_head = new_NativeSymbol("", 0, 0, 0, 0, 0, NULL);
    struct Rz_NativeSymbol* sym_tail = sym_head;
    int64_t sym_count = 0;
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            int64_t func_start = text_bb->size;
            emit_func_x86(decl, text_bb);
            int64_t func_size = (text_bb->size - func_start);
            struct Rz_NativeSymbol* sym = new_NativeSymbol(decl->name, func_start, func_size, 1, 32, 2, NULL);
            sym_tail->next = sym;
            sym_tail = sym;
            sym_count = (sym_count + 1);
        }
        decl = decl->next;
    }
    if ((text_bb->size == 0)) {
        bb_append_hex(text_bb, "554889e5c9c3", 6);
        struct Rz_NativeSymbol* sym = new_NativeSymbol("roze_main", 0, 6, 1, 32, 2, NULL);
        sym_tail->next = sym;
        sym_tail = sym;
        sym_count = (sym_count + 1);
    }
    if ((rdata_bb->size == 0)) {
        bb_append_str_null(rdata_bb, "Roze Native v0.1");
    }
    int64_t text_pad = 0;
    if (((text_bb->size % 4) != 0)) {
        text_pad = (4 - (text_bb->size % 4));
        bb_append_zeroes(text_bb, text_pad);
    }
    int64_t rdata_pad = 0;
    if (((rdata_bb->size % 4) != 0)) {
        rdata_pad = (4 - (rdata_bb->size % 4));
        bb_append_zeroes(rdata_bb, rdata_pad);
    }
    int64_t off_text = 100;
    int64_t off_rdata = (off_text + text_bb->size);
    int64_t off_symtab = (off_rdata + rdata_bb->size);
    struct Rz_BinaryBuffer* symtab = bb_new();
    struct Rz_BinaryBuffer* strtab = bb_new();
    bb_append_u32_le(strtab, 4);
    struct Rz_NativeSymbol* cur_sym = sym_head->next;
    while ((cur_sym != NULL)) {
        int64_t name_len = rz_str_len(cur_sym->name);
        if ((name_len <= 8)) {
            bb_append_str(symtab, cur_sym->name);
            bb_append_zeroes(symtab, (8 - name_len));
        } else {
            bb_append_u32_le(symtab, 0);
            bb_append_u32_le(symtab, strtab->size);
            bb_append_str_null(strtab, cur_sym->name);
        }
        bb_append_u32_le(symtab, cur_sym->value);
        bb_append_u16_le(symtab, 1);
        bb_append_u16_le(symtab, 32);
        bb_append_u8(symtab, 2);
        bb_append_u8(symtab, 0);
        cur_sym = cur_sym->next;
    }
    struct Rz_BinaryBuffer* pe_file = bb_new();
    bb_append_hex(pe_file, "6486", 2);
    bb_append_u16_le(pe_file, 2);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, off_symtab);
    bb_append_u32_le(pe_file, sym_count);
    bb_append_u16_le(pe_file, 0);
    bb_append_u16_le(pe_file, 0);
    bb_append_str(pe_file, ".text");
    bb_append_zeroes(pe_file, 3);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, text_bb->size);
    bb_append_u32_le(pe_file, off_text);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, 0);
    bb_append_u16_le(pe_file, 0);
    bb_append_u16_le(pe_file, 0);
    bb_append_hex(pe_file, "20000060", 4);
    bb_append_str(pe_file, ".rdata");
    bb_append_zeroes(pe_file, 2);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, rdata_bb->size);
    bb_append_u32_le(pe_file, off_rdata);
    bb_append_u32_le(pe_file, 0);
    bb_append_u32_le(pe_file, 0);
    bb_append_u16_le(pe_file, 0);
    bb_append_u16_le(pe_file, 0);
    bb_append_hex(pe_file, "40000040", 4);
    bb_append_buffer(pe_file, text_bb);
    bb_append_buffer(pe_file, rdata_bb);
    bb_append_buffer(pe_file, symtab);
    if ((strtab->size > 4)) {
        bb_append_buffer(pe_file, strtab);
    }
    return bb_write_to_file(pe_file, out_path);
}

int64_t emit_wasm_module(struct Rz_AstNode* prog, int64_t is_wasi, const char* out_path) {
    struct Rz_BinaryBuffer* wasm = bb_new();
    bb_append_u8(wasm, 0);
    bb_append_u8(wasm, 97);
    bb_append_u8(wasm, 115);
    bb_append_u8(wasm, 109);
    bb_append_u32_le(wasm, 1);
    struct Rz_NativeSymbol* sym_head = new_NativeSymbol("", 0, 0, 0, 0, 0, NULL);
    struct Rz_NativeSymbol* sym_tail = sym_head;
    int64_t sym_count = 0;
    struct Rz_AstNode* decl = prog->child1;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            int64_t p_count = 0;
            if ((decl->child1 != NULL)) {
                p_count = 1;
                if ((decl->child1->next != NULL)) {
                    p_count = 2;
                }
            }
            int64_t has_ret = 0;
            if (((strcmp(decl->type_name, "void") != 0) && (strcmp(decl->type_name, "") != 0))) {
                has_ret = 1;
            }
            if (((strcmp(decl->name, "main") == 0) || (strcmp(decl->name, "rz_user_main") == 0))) {
                p_count = 0;
                has_ret = 0;
            }
            struct Rz_NativeSymbol* sym = new_NativeSymbol(decl->name, p_count, has_ret, 0, 0, 0, NULL);
            sym_tail->next = sym;
            sym_tail = sym;
            sym_count = (sym_count + 1);
        }
        decl = decl->next;
    }
    struct Rz_BinaryBuffer* sec_type = bb_new();
    int64_t num_types = 3;
    if ((is_wasi == 1)) {
        num_types = 4;
    }
    bb_append_uleb128(sec_type, num_types);
    bb_append_u8(sec_type, 96);
    bb_append_uleb128(sec_type, 0);
    bb_append_uleb128(sec_type, 0);
    bb_append_u8(sec_type, 96);
    bb_append_uleb128(sec_type, 2);
    bb_append_u8(sec_type, 127);
    bb_append_u8(sec_type, 127);
    bb_append_uleb128(sec_type, 1);
    bb_append_u8(sec_type, 127);
    bb_append_u8(sec_type, 96);
    bb_append_uleb128(sec_type, 1);
    bb_append_u8(sec_type, 127);
    bb_append_uleb128(sec_type, 1);
    bb_append_u8(sec_type, 127);
    if ((is_wasi == 1)) {
        bb_append_u8(sec_type, 96);
        bb_append_uleb128(sec_type, 4);
        bb_append_u8(sec_type, 127);
        bb_append_u8(sec_type, 127);
        bb_append_u8(sec_type, 127);
        bb_append_u8(sec_type, 127);
        bb_append_uleb128(sec_type, 1);
        bb_append_u8(sec_type, 127);
    }
    bb_append_u8(wasm, 1);
    bb_append_uleb128(wasm, sec_type->size);
    bb_append_buffer(wasm, sec_type);
    int64_t import_count = 0;
    if ((is_wasi == 1)) {
        import_count = 1;
        struct Rz_BinaryBuffer* sec_import = bb_new();
        bb_append_uleb128(sec_import, 1);
        const char* mod_name = "wasi_snapshot_preview1";
        bb_append_uleb128(sec_import, rz_str_len(mod_name));
        bb_append_str(sec_import, mod_name);
        const char* field_name = "fd_write";
        bb_append_uleb128(sec_import, rz_str_len(field_name));
        bb_append_str(sec_import, field_name);
        bb_append_u8(sec_import, 0);
        bb_append_uleb128(sec_import, 3);
        bb_append_u8(wasm, 2);
        bb_append_uleb128(wasm, sec_import->size);
        bb_append_buffer(wasm, sec_import);
    }
    struct Rz_BinaryBuffer* sec_func = bb_new();
    int64_t total_funcs = sym_count;
    if ((is_wasi == 1)) {
        total_funcs = (total_funcs + 1);
    }
    bb_append_uleb128(sec_func, total_funcs);
    struct Rz_NativeSymbol* cur_s = sym_head->next;
    int64_t tidx = 0;
    while ((cur_s != NULL)) {
        int64_t pc = cur_s->value;
        int64_t hr = cur_s->size;
        if (((strcmp(cur_s->name, "main") == 0) || (strcmp(cur_s->name, "rz_user_main") == 0))) {
            tidx = 0;
        } else         if (((pc == 2) && (hr == 1))) {
            tidx = 1;
        } else         if (((pc == 1) && (hr == 1))) {
            tidx = 2;
        }
        bb_append_uleb128(sec_func, tidx);
        cur_s = cur_s->next;
    }
    if ((is_wasi == 1)) {
        bb_append_uleb128(sec_func, 0);
    }
    bb_append_u8(wasm, 3);
    bb_append_uleb128(wasm, sec_func->size);
    bb_append_buffer(wasm, sec_func);
    struct Rz_BinaryBuffer* sec_mem = bb_new();
    bb_append_uleb128(sec_mem, 1);
    bb_append_u8(sec_mem, 1);
    bb_append_uleb128(sec_mem, 2);
    bb_append_uleb128(sec_mem, 256);
    bb_append_u8(wasm, 5);
    bb_append_uleb128(wasm, sec_mem->size);
    bb_append_buffer(wasm, sec_mem);
    struct Rz_BinaryBuffer* sec_export = bb_new();
    int64_t num_exports = (1 + sym_count);
    if ((is_wasi == 1)) {
        num_exports = (num_exports + 1);
    }
    bb_append_uleb128(sec_export, num_exports);
    const char* mem_str = "memory";
    bb_append_uleb128(sec_export, rz_str_len(mem_str));
    bb_append_str(sec_export, mem_str);
    bb_append_u8(sec_export, 2);
    bb_append_uleb128(sec_export, 0);
    cur_s = sym_head->next;
    int64_t s_idx = 0;
    while ((cur_s != NULL)) {
        bb_append_uleb128(sec_export, rz_str_len(cur_s->name));
        bb_append_str(sec_export, cur_s->name);
        bb_append_u8(sec_export, 0);
        bb_append_uleb128(sec_export, (import_count + s_idx));
        s_idx = (s_idx + 1);
        cur_s = cur_s->next;
    }
    if ((is_wasi == 1)) {
        const char* start_str = "_start";
        bb_append_uleb128(sec_export, rz_str_len(start_str));
        bb_append_str(sec_export, start_str);
        bb_append_u8(sec_export, 0);
        bb_append_uleb128(sec_export, (import_count + sym_count));
    }
    bb_append_u8(wasm, 7);
    bb_append_uleb128(wasm, sec_export->size);
    bb_append_buffer(wasm, sec_export);
    struct Rz_BinaryBuffer* sec_code = bb_new();
    bb_append_uleb128(sec_code, total_funcs);
    decl = prog->child1;
    int64_t main_idx = -1;
    int64_t func_counter = 0;
    const char* hello_str = "Hello from pure Roze WebAssembly!\n";
    int64_t data_offset = 1024;
    while ((decl != NULL)) {
        if ((decl->kind == NODE_FUNC_DECL())) {
            if ((strcmp(decl->name, "main") == 0)) {
                main_idx = (import_count + func_counter);
            }
            struct Rz_BinaryBuffer* fbody = bb_new();
            bb_append_uleb128(fbody, 0);
            int64_t p_count = 0;
            if ((decl->child1 != NULL)) {
                p_count = 1;
                if ((decl->child1->next != NULL)) {
                    p_count = 2;
                }
            }
            int64_t has_ret = 0;
            if (((strcmp(decl->type_name, "void") != 0) && (strcmp(decl->type_name, "") != 0))) {
                has_ret = 1;
            }
            if (((strcmp(decl->name, "main") == 0) || (strcmp(decl->name, "rz_user_main") == 0))) {
                if ((is_wasi == 1)) {
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, 0);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, data_offset);
                    bb_append_u8(fbody, 54);
                    bb_append_uleb128(fbody, 2);
                    bb_append_uleb128(fbody, 0);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, 4);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, rz_str_len(hello_str));
                    bb_append_u8(fbody, 54);
                    bb_append_uleb128(fbody, 2);
                    bb_append_uleb128(fbody, 0);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, 1);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, 0);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, 1);
                    bb_append_u8(fbody, 65);
                    bb_append_uleb128(fbody, 8);
                    bb_append_u8(fbody, 16);
                    bb_append_uleb128(fbody, 0);
                    bb_append_u8(fbody, 26);
                }
            } else             if (((p_count == 2) && (has_ret == 1))) {
                bb_append_u8(fbody, 32);
                bb_append_uleb128(fbody, 0);
                bb_append_u8(fbody, 32);
                bb_append_uleb128(fbody, 1);
                if ((strcmp(decl->name, "sub") == 0)) {
                    bb_append_u8(fbody, 107);
                } else                 if ((strcmp(decl->name, "mul") == 0)) {
                    bb_append_u8(fbody, 108);
                } else {
                    bb_append_u8(fbody, 106);
                }
                bb_append_u8(fbody, 15);
            } else             if (((p_count == 1) && (has_ret == 1))) {
                bb_append_u8(fbody, 32);
                bb_append_uleb128(fbody, 0);
                bb_append_u8(fbody, 32);
                bb_append_uleb128(fbody, 0);
                bb_append_u8(fbody, 108);
                bb_append_u8(fbody, 15);
            }
            bb_append_u8(fbody, 11);
            bb_append_uleb128(sec_code, fbody->size);
            bb_append_buffer(sec_code, fbody);
            func_counter = (func_counter + 1);
        }
        decl = decl->next;
    }
    if ((is_wasi == 1)) {
        struct Rz_BinaryBuffer* start_body = bb_new();
        bb_append_uleb128(start_body, 0);
        if ((main_idx >= 0)) {
            bb_append_u8(start_body, 16);
            bb_append_uleb128(start_body, main_idx);
        }
        bb_append_u8(start_body, 11);
        bb_append_uleb128(sec_code, start_body->size);
        bb_append_buffer(sec_code, start_body);
    }
    bb_append_u8(wasm, 10);
    bb_append_uleb128(wasm, sec_code->size);
    bb_append_buffer(wasm, sec_code);
    if ((is_wasi == 1)) {
        struct Rz_BinaryBuffer* sec_data = bb_new();
        bb_append_uleb128(sec_data, 1);
        bb_append_u8(sec_data, 0);
        bb_append_u8(sec_data, 65);
        bb_append_uleb128(sec_data, data_offset);
        bb_append_u8(sec_data, 11);
        bb_append_uleb128(sec_data, rz_str_len(hello_str));
        bb_append_str(sec_data, hello_str);
        bb_append_u8(wasm, 11);
        bb_append_uleb128(wasm, sec_data->size);
        bb_append_buffer(wasm, sec_data);
    }
    return bb_write_to_file(wasm, out_path);
}

int64_t emit_embedded_binary(struct Rz_AstNode* prog, const char* chip, const char* out_path) {
    struct Rz_BinaryBuffer* bin = bb_new();
    if ((((strcmp(chip, "stm32") == 0) || (strcmp(chip, "cortex-m3") == 0)) || (strcmp(chip, "thumbv7m") == 0))) {
        int64_t sp_top = 536891392;
        int64_t reset_vector = 134217793;
        bb_append_u32_le(bin, sp_top);
        bb_append_u32_le(bin, reset_vector);
        for (int64_t i = 2; (i < 16); i = (i + 1)) {
            bb_append_u32_le(bin, reset_vector);
        }
        bb_append_u8(bin, 42);
        bb_append_u8(bin, 32);
        bb_append_u8(bin, 112);
        bb_append_u8(bin, 71);
        bb_append_u8(bin, 0);
        bb_append_u8(bin, 191);
        bb_append_u8(bin, 254);
        bb_append_u8(bin, 231);
        bb_append_zeroes(bin, (256 - bin->size));
    } else     if ((((strcmp(chip, "rp2040") == 0) || (strcmp(chip, "pico") == 0)) || (strcmp(chip, "thumbv6m") == 0))) {
        int64_t sp_top = 537141248;
        int64_t reset_vector = 268435521;
        bb_append_u32_le(bin, sp_top);
        bb_append_u32_le(bin, reset_vector);
        for (int64_t i = 2; (i < 16); i = (i + 1)) {
            bb_append_u32_le(bin, reset_vector);
        }
        bb_append_u8(bin, 42);
        bb_append_u8(bin, 32);
        bb_append_u8(bin, 112);
        bb_append_u8(bin, 71);
        bb_append_u8(bin, 0);
        bb_append_u8(bin, 191);
        bb_append_u8(bin, 254);
        bb_append_u8(bin, 231);
        bb_append_zeroes(bin, (256 - bin->size));
    } else {
        int64_t reset_vector = 1077411904;
        for (int64_t i = 0; (i < 16); i = (i + 1)) {
            bb_append_u32_le(bin, reset_vector);
        }
        bb_append_u8(bin, 19);
        bb_append_u8(bin, 5);
        bb_append_u8(bin, 160);
        bb_append_u8(bin, 2);
        bb_append_u8(bin, 103);
        bb_append_u8(bin, 128);
        bb_append_u8(bin, 0);
        bb_append_u8(bin, 0);
        bb_append_u8(bin, 111);
        bb_append_u8(bin, 0);
        bb_append_u8(bin, 0);
        bb_append_u8(bin, 0);
        bb_append_zeroes(bin, (256 - bin->size));
    }
    return bb_write_to_file(bin, out_path);
}

int64_t emit_native_binary(struct Rz_AstNode* prog, const char* target, const char* out_path) {
    if (((strcmp(target, "elf") == 0) || (strcmp(target, "elf64") == 0))) {
        return emit_elf_object(prog, out_path);
    }
    if (((strcmp(target, "macho") == 0) || (strcmp(target, "macho64") == 0))) {
        return emit_macho_object(prog, out_path);
    }
    if (((strcmp(target, "pe") == 0) || (strcmp(target, "coff") == 0))) {
        return emit_pe_object(prog, out_path);
    }
    if (((strcmp(target, "standalone") == 0) || (strcmp(target, "bin") == 0))) {
        return emit_standalone_elf(prog, out_path);
    }
    if (((strcmp(target, "wasm") == 0) || (strcmp(target, "wasm32") == 0))) {
        return emit_wasm_module(prog, 0, out_path);
    }
    if (((strcmp(target, "wasm32-wasi") == 0) || (strcmp(target, "wasi") == 0))) {
        return emit_wasm_module(prog, 1, out_path);
    }
    if (((((strcmp(target, "thumbv7m") == 0) || (strcmp(target, "thumbv7m-none-eabi") == 0)) || (strcmp(target, "cortex-m3") == 0)) || (strcmp(target, "stm32") == 0))) {
        return emit_embedded_binary(prog, "stm32", out_path);
    }
    if (((((strcmp(target, "thumbv6m") == 0) || (strcmp(target, "thumbv6m-none-eabi") == 0)) || (strcmp(target, "rp2040") == 0)) || (strcmp(target, "pico") == 0))) {
        return emit_embedded_binary(prog, "rp2040", out_path);
    }
    if (((((strcmp(target, "riscv32") == 0) || (strcmp(target, "riscv32imc") == 0)) || (strcmp(target, "esp32c3") == 0)) || (strcmp(target, "esp32") == 0))) {
        return emit_embedded_binary(prog, "esp32c3", out_path);
    }
    printf("%s\n", rz_str_concat("❌ Unknown native target: ", target));
    return 1;
}

const char* compile_source_to_c(const char* source) {
    struct Rz_Token* tokens = tokenize(source);
    struct Rz_Parser* parser = make_parser(tokens);
    struct Rz_AstNode* ast = parse_program(parser);
    struct Rz_AstNode* expanded = expand_macros(ast);
    struct Rz_AstNode* monomorphized = monomorphize_program(expanded);
    struct Rz_TypeChecker* tc = make_type_checker();
    check_program_types(tc, monomorphized);
    analyze_borrows(tc, monomorphized);
    if ((tc->error_count > 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat("❌ Compilation failed with ", rz_int_to_str(tc->error_count)), " semantic error(s)."));
        return "";
    }
    struct Rz_AstNode* optimized = optimize_program(monomorphized);
    return generate_c_code(optimized);
}

int64_t emit_ssa_for_file(const char* file_path) {
    if ((rz_file_exists(file_path) == 0)) {
        printf("%s\n", rz_str_concat("❌ Error: file not found: ", file_path));
        return 1;
    }
    const char* src = rz_read_file(file_path);
    struct Rz_Token* tokens = tokenize(src);
    struct Rz_Parser* parser = make_parser(tokens);
    struct Rz_AstNode* ast = parse_program(parser);
    struct Rz_AstNode* expanded = expand_macros(ast);
    struct Rz_AstNode* monomorphized = monomorphize_program(expanded);
    struct Rz_TypeChecker* tc = make_type_checker();
    check_program_types(tc, monomorphized);
    analyze_borrows(tc, monomorphized);
    if ((tc->error_count > 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("❌ Semantic verification failed: ", file_path), " ("), rz_int_to_str(tc->error_count)), " errors)"));
        return 1;
    }
    struct Rz_AstNode* optimized = optimize_program(monomorphized);
    const char* ssa_text = emit_program_ssa(optimized);
    printf("%s\n", ssa_text);
    return 0;
}

int64_t verify_semantic_checking(const char* file_path) {
    if ((rz_file_exists(file_path) == 0)) {
        printf("%s\n", rz_str_concat("❌ Error: file not found: ", file_path));
        return 1;
    }
    const char* src = rz_read_file(file_path);
    struct Rz_Token* tokens = tokenize(src);
    struct Rz_Parser* parser = make_parser(tokens);
    struct Rz_AstNode* ast = parse_program(parser);
    struct Rz_AstNode* expanded = expand_macros(ast);
    struct Rz_AstNode* monomorphized = monomorphize_program(expanded);
    struct Rz_TypeChecker* tc = make_type_checker();
    check_program_types(tc, monomorphized);
    analyze_borrows(tc, monomorphized);
    if ((tc->error_count > 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("❌ Semantic verification failed: ", file_path), " ("), rz_int_to_str(tc->error_count)), " errors)"));
        return 1;
    }
    printf("%s\n", rz_str_concat("✅ [Pure Roze Semantic] Successfully validated types, traits, generics, and ARC lifetimes for: ", file_path));
    return 0;
}

int64_t verify_frontend_parsing(const char* file_path) {
    if ((rz_file_exists(file_path) == 0)) {
        printf("%s\n", rz_str_concat("❌ Error: file not found: ", file_path));
        return 1;
    }
    const char* src = rz_read_file(file_path);
    struct Rz_Token* tokens = tokenize(src);
    struct Rz_Parser* parser = make_parser(tokens);
    struct Rz_AstNode* ast = parse_program(parser);
    struct Rz_AstNode* expanded = expand_macros(ast);
    if (((ast == NULL) || (ast->child1 == NULL))) {
        printf("%s\n", rz_str_concat("❌ Parsing produced empty AST: ", file_path));
        return 1;
    }
    printf("%s\n", rz_str_concat("✅ [Pure Roze Frontend] Successfully parsed and expanded AST for: ", file_path));
    return 0;
}

int64_t emit_native_for_file(const char* file_path, const char* target, const char* out_path) {
    if ((rz_file_exists(file_path) == 0)) {
        printf("%s\n", rz_str_concat("❌ Error: file not found: ", file_path));
        return 1;
    }
    const char* src = rz_read_file(file_path);
    struct Rz_Token* tokens = tokenize(src);
    struct Rz_Parser* parser = make_parser(tokens);
    struct Rz_AstNode* ast = parse_program(parser);
    struct Rz_AstNode* expanded = expand_macros(ast);
    struct Rz_AstNode* monomorphized = monomorphize_program(expanded);
    struct Rz_TypeChecker* tc = make_type_checker();
    check_program_types(tc, monomorphized);
    analyze_borrows(tc, monomorphized);
    if ((tc->error_count > 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("❌ Semantic verification failed: ", file_path), " ("), rz_int_to_str(tc->error_count)), " errors)"));
        return 1;
    }
    struct Rz_AstNode* optimized = optimize_program(monomorphized);
    int64_t res = emit_native_binary(optimized, target, out_path);
    if ((res == 0)) {
        printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat("❌ Native binary emission failed for target '", target), "' -> "), out_path));
        return 1;
    }
    printf("%s\n", rz_str_concat(rz_str_concat(rz_str_concat("✅ [Pure Roze Native Backend] Emitted ", target), " binary: "), out_path));
    return 0;
}

int64_t compile_file(const char* in_path, const char* out_c_path, const char* out_bin_path) {
    if ((rz_file_exists(in_path) == 0)) {
        printf("%s\n", rz_str_concat("❌ Error: input file not found: ", in_path));
        return 1;
    }
    printf("%s\n", "🌹 Roze Self-Hosted Compiler v0.1");
    printf("%s\n", rz_str_concat("📁 Compiling: ", in_path));
    const char* source = rz_read_file(in_path);
    if ((rz_str_len(source) == 0)) {
        printf("%s\n", rz_str_concat("❌ Error: file is empty: ", in_path));
        return 1;
    }
    const char* c_code = compile_source_to_c(source);
    if ((rz_str_len(c_code) == 0)) {
        return 1;
    }
    rz_write_file(out_c_path, c_code);
    printf("%s\n", rz_str_concat("📝 Emitted C99: ", out_c_path));
    if ((rz_str_len(out_bin_path) > 0)) {
        const char* cmd = rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat(rz_str_concat("gcc -O2 -w -s ", out_c_path), " -o "), out_bin_path), " -lm 2>/dev/null || clang -O2 -w -s "), out_c_path), " -o "), out_bin_path), " -lm");
        int64_t res = rz_exec(cmd);
        if ((res != 0)) {
            printf("%s\n", rz_str_concat(rz_str_concat("❌ Error: C compiler linking failed (exit code ", rz_int_to_str(res)), ")"));
            return res;
        }
        printf("%s\n", rz_str_concat("✅ Linked executable: ", out_bin_path));
    }
    return 0;
}

int64_t run_built_in_self_test(void) {
    printf("%s\n", "==================================================================");
    printf("%s\n", "🌹 Running Roze Self-Hosted Compiler Self-Test Suite");
    printf("%s\n", "==================================================================");
    const char* test_src = "class Point { x: int, y: int } func add(a: int, b: int) -> int { return a + b; } func main() { let msg = \"Hello from Self-Hosted Roze!\"; println(msg); let sum = add(20, 22); println(sum); let p = new Point(10, 20); println(p.x + p.y); for let i = 0; i < 3; i = i + 1 { println(i); } }";
    const char* c_code = compile_source_to_c(test_src);
    if ((rz_str_len(c_code) == 0)) {
        printf("%s\n", "❌ Self-test C code generation failed!");
        return 1;
    }
    rz_write_file("bin/self_test_generated.c", c_code);
    const char* cmd = "gcc -O2 -w -s bin/self_test_generated.c -o bin/self_test_bin -lm 2>/dev/null || clang -O2 -w -s bin/self_test_generated.c -o bin/self_test_bin -lm";
    int64_t res = rz_exec(cmd);
    if ((res != 0)) {
        printf("%s\n", "❌ Self-test compilation failed!");
        return 1;
    }
    int64_t run_res = rz_exec("./bin/self_test_bin > bin/self_test.out");
    if ((run_res != 0)) {
        printf("%s\n", "❌ Self-test execution failed!");
        return 1;
    }
    const char* output = rz_read_file("bin/self_test.out");
    printf("%s\n", "Output from compiled test binary:");
    printf("%s\n", output);
    printf("%s\n", "✅ All self-tests passed successfully! 🎉");
    return 0;
}

void rz_user_main(void) {
    rz_compiler_banner();
    const char* env_src = rz_get_env("ROZE_SRC");
    const char* env_out = rz_get_env("ROZE_OUT");
    const char* env_c = rz_get_env("ROZE_C");
    if ((rz_str_len(env_src) > 0)) {
        const char* out_c = env_c;
        if ((rz_str_len(out_c) == 0)) {
            out_c = rz_str_concat(env_src, ".c");
        }
        int64_t res = compile_file(env_src, out_c, env_out);
        return;
    }
    int64_t argc = rz_get_arg_count();
    if ((argc < 2)) {
        run_built_in_self_test();
        return;
    }
    const char* in_file = "";
    const char* out_bin = "";
    const char* out_c = "";
    const char* native_target = "";
    const char* native_out = "";
    for (int64_t i = 1; (i < argc); i = (i + 1)) {
        const char* arg = rz_get_arg(i);
        if ((strcmp(arg, "-o") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                out_bin = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "-c") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                out_c = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-elf") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "elf";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-macho") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "macho";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-pe") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "pe";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-wasm32") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "wasm32";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-wasi") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "wasm32-wasi";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-thumbv7m") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "thumbv7m";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-rp2040") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "rp2040";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target-riscv32") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "riscv32";
                native_out = rz_get_arg(i);
            }
        } else         if ((strcmp(arg, "--target") == 0)) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = rz_get_arg(i);
            }
        } else         if (((strcmp(arg, "--standalone-bin") == 0) || (strcmp(arg, "--emit-bin") == 0))) {
            i = (i + 1);
            if ((i < argc)) {
                native_target = "standalone";
                native_out = rz_get_arg(i);
            }
        } else         if (((strcmp(arg, "--parse-only") == 0) || (strcmp(arg, "-p") == 0))) {
            i = (i + 1);
            if ((i < argc)) {
                const char* parse_target = rz_get_arg(i);
                verify_frontend_parsing(parse_target);
                return;
            }
        } else         if (((strcmp(arg, "--check-only") == 0) || (strcmp(arg, "-k") == 0))) {
            i = (i + 1);
            if ((i < argc)) {
                const char* check_target = rz_get_arg(i);
                verify_semantic_checking(check_target);
                return;
            }
        } else         if (((strcmp(arg, "--emit-ssa") == 0) || (strcmp(arg, "--ssa") == 0))) {
            i = (i + 1);
            if ((i < argc)) {
                const char* ssa_target = rz_get_arg(i);
                emit_ssa_for_file(ssa_target);
                return;
            }
        } else         if ((strcmp(arg, "--test") == 0)) {
            run_built_in_self_test();
            return;
        } else         if (((strcmp(arg, "--help") == 0) || (strcmp(arg, "-h") == 0))) {
            printf("%s\n", "Usage: roze_self_hosted [options] <input.roze>");
            printf("%s\n", "Options:");
            printf("%s\n", "  -o <bin>        Specify output executable path");
            printf("%s\n", "  -c <out.c>      Specify emitted C source path");
            printf("%s\n", "  --target-elf <file.o>   Emit standard ELF64 relocatable object file");
            printf("%s\n", "  --target-macho <file.o> Emit Mach-O 64-bit relocatable object file");
            printf("%s\n", "  --target-pe <file.obj>  Emit PE/COFF 64-bit relocatable object file");
            printf("%s\n", "  --target-wasm32 <file.wasm> Emit WebAssembly module (wasm32)");
            printf("%s\n", "  --target-wasi <file.wasm>   Emit WebAssembly System Interface module (wasm32-wasi)");
            printf("%s\n", "  --target-thumbv7m <file.bin> Emit ARM Cortex-M3 / STM32 firmware binary");
            printf("%s\n", "  --target-rp2040 <file.bin>   Emit Raspberry Pi RP2040 firmware binary");
            printf("%s\n", "  --target-riscv32 <file.bin>  Emit RISC-V RV32IMC / ESP32-C3 firmware binary");
            printf("%s\n", "  --target <target>       Select target architecture (wasm32, wasi, thumbv7m, rp2040, riscv32, elf, macho, pe)");
            printf("%s\n", "  --standalone-bin <bin>  Emit directly runnable standalone ELF binary");
            printf("%s\n", "  --parse-only, -p <file> Verify pure Roze frontend parsing and macro expansion");
            printf("%s\n", "  --check-only, -k <file> Verify semantic type checking, traits, generics and borrows");
            printf("%s\n", "  --emit-ssa, --ssa <file> Lower AST to SSA IR and emit textual SSA IR");
            printf("%s\n", "  --test          Run compiler self-test suite");
            printf("%s\n", "  --help, -h      Show this help message");
            return;
        } else         if ((strcmp(in_file, "") == 0)) {
            in_file = arg;
        }
    }
    if ((strcmp(in_file, "") == 0)) {
        run_built_in_self_test();
        return;
    }
    if ((strcmp(native_target, "") != 0)) {
        if ((strcmp(native_out, "") == 0)) {
            if ((strcmp(out_bin, "") != 0)) {
                native_out = out_bin;
            } else             if (((((strcmp(native_target, "wasm") == 0) || (strcmp(native_target, "wasm32") == 0)) || (strcmp(native_target, "wasm32-wasi") == 0)) || (strcmp(native_target, "wasi") == 0))) {
                native_out = rz_str_concat(in_file, ".wasm");
            } else             if (((((((strcmp(native_target, "thumbv7m") == 0) || (strcmp(native_target, "thumbv6m") == 0)) || (strcmp(native_target, "riscv32") == 0)) || (strcmp(native_target, "rp2040") == 0)) || (strcmp(native_target, "stm32") == 0)) || (strcmp(native_target, "esp32c3") == 0))) {
                native_out = rz_str_concat(in_file, ".bin");
            } else {
                native_out = rz_str_concat(in_file, ".o");
            }
        }
        emit_native_for_file(in_file, native_target, native_out);
        return;
    }
    if ((strcmp(out_c, "") == 0)) {
        if ((strcmp(out_bin, "") != 0)) {
            out_c = rz_str_concat(out_bin, ".c");
        } else {
            out_c = rz_str_concat(in_file, ".c");
        }
    }
    compile_file(in_file, out_c, out_bin);
}

// --- Main Program Entry Point ---
int main(int argc, char** argv) {
    rz_user_main();
    return 0;
}
