// ==============================================================================
// 🌹 Roze Self-Hosted Compiler - Runtime Implementation (ADV-06)
// ==============================================================================

#include "../include/roze_self_hosted.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <sys/stat.h>
#include <unistd.h>

// ------------------------------------------------------------------------------
// 1. Roze ARC String Management
// ------------------------------------------------------------------------------

char* rz_create_string(const char* s, int64_t len) {
    if (len < 0) len = 0;
    static _Thread_local char ring[8][4096];
    static _Thread_local int ring_idx = 0;
    if (len < 4096) {
        char* buf = ring[ring_idx];
        ring_idx = (ring_idx + 1) % 8;
        if (s && len > 0) memcpy(buf, s, len);
        buf[len] = '\0';
        return buf;
    }
    char* buf = (char*)malloc(len + 1);
    if (!buf) return "";
    if (s && len > 0) memcpy(buf, s, len);
    buf[len] = '\0';
    return buf;
}

char* rz_str_concat(const char* a, const char* b) {
    if (!a) a = "";
    if (!b) b = "";
    int64_t len_a = strlen(a);
    int64_t len_b = strlen(b);
    int64_t total = len_a + len_b;
    static _Thread_local char ring[8][8192];
    static _Thread_local int ring_idx = 0;
    if (total < 8192) {
        char* buf = ring[ring_idx];
        ring_idx = (ring_idx + 1) % 8;
        if (len_a > 0) memcpy(buf, a, len_a);
        if (len_b > 0) memcpy(buf + len_a, b, len_b);
        buf[total] = '\0';
        return buf;
    }
    char* buf = (char*)malloc(total + 1);
    if (!buf) return "";
    if (len_a > 0) memcpy(buf, a, len_a);
    if (len_b > 0) memcpy(buf + len_a, b, len_b);
    buf[total] = '\0';
    return buf;
}

int64_t rz_str_len(const char* s) {
    if (!s) return 0;
    return (int64_t)strlen(s);
}

int64_t rz_char_at(const char* s, int64_t index) {
    if (!s || index < 0) return -1;
    unsigned char ch = (unsigned char)s[index];
    if (ch == '\0') return -1;
    return (int64_t)ch;
}

char* rz_substring(const char* s, int64_t start, int64_t len) {
    if (!s || start < 0 || len <= 0) return "";
    int64_t total = strlen(s);
    if (start >= total) return "";
    if (start + len > total) len = total - start;
    static _Thread_local char ring[16][4096];
    static _Thread_local int ring_idx = 0;
    if (len < 4096) {
        char* buf = ring[ring_idx];
        ring_idx = (ring_idx + 1) % 16;
        memcpy(buf, s + start, len);
        buf[len] = '\0';
        return buf;
    }
    char* buf = (char*)malloc(len + 1);
    if (!buf) return "";
    memcpy(buf, s + start, len);
    buf[len] = '\0';
    return buf;
}

char* rz_char_to_str(int64_t ch) {
    static _Thread_local char buf[2];
    buf[0] = (char)ch;
    buf[1] = '\0';
    return buf;
}

char* rz_int_to_str(int64_t val) {
    static _Thread_local char buf[32];
    snprintf(buf, sizeof(buf), "%lld", (long long)val);
    return buf;
}

int64_t rz_str_eq(const char* a, const char* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    return (strcmp(a, b) == 0) ? 1 : 0;
}

// ------------------------------------------------------------------------------
// 2. File IO & Process Execution
// ------------------------------------------------------------------------------

char* rz_read_file(const char* path) {
    if (!path) return "";
    FILE* f = fopen(path, "rb");
    if (!f) return "";

    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);

    char* buf = (char*)malloc(sz + 1);
    if (!buf) {
        fclose(f);
        return "";
    }

    size_t read_bytes = fread(buf, 1, sz, f);
    buf[read_bytes] = '\0';
    fclose(f);
    return buf;
}

int64_t rz_write_file(const char* path, const char* content) {
    if (!path || !content) return 0;

    // Ensure parent directory exists if path contains '/'
    const char* slash = strrchr(path, '/');
    if (slash) {
        char dir[256];
        size_t len = slash - path;
        if (len < sizeof(dir)) {
            strncpy(dir, path, len);
            dir[len] = '\0';
            mkdir(dir, 0755);
        }
    }

    FILE* f = fopen(path, "w");
    if (!f) return 0;

    size_t len = strlen(content);
    size_t written = fwrite(content, 1, len, f);
    fclose(f);
    return (written == len) ? 1 : 0;
}

int64_t rz_file_exists(const char* path) {
    if (!path) return 0;
    return (access(path, F_OK) == 0) ? 1 : 0;
}

int64_t rz_exec(const char* cmd) {
    if (!cmd) return -1;
    int res = system(cmd);
    return (int64_t)res;
}

static int g_argc = 0;
static char* g_argv[64];
static int g_cmdline_init = 0;

static void init_cmdline(void) {
    if (g_cmdline_init) return;
    g_cmdline_init = 1;
    FILE* f = fopen("/proc/self/cmdline", "rb");
    if (!f) return;
    static char buf[8192];
    size_t n = fread(buf, 1, sizeof(buf) - 1, f);
    fclose(f);
    buf[n] = '\0';
    size_t i = 0;
    while (i < n && g_argc < 64) {
        g_argv[g_argc++] = &buf[i];
        while (i < n && buf[i] != '\0') i++;
        i++;
    }
}

int64_t rz_get_arg_count(void) {
    init_cmdline();
    return (int64_t)g_argc;
}

char* rz_get_arg(int64_t index) {
    init_cmdline();
    if (index >= 0 && index < g_argc) {
        return g_argv[index];
    }
    return "";
}

char* rz_get_env(const char* name) {
    if (!name) return "";
    char* val = getenv(name);
    return val ? val : "";
}

// ------------------------------------------------------------------------------
// 3. Diagnostics & Utilities
// ------------------------------------------------------------------------------

void rz_print_str(const char* s) {
    if (s) {
        fputs(s, stdout);
        fflush(stdout);
    }
}

void rz_print_int(int64_t val) {
    printf("%lld\n", (long long)val);
    fflush(stdout);
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
    fflush(stdout);
}
