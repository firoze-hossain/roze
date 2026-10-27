// ==============================================================================
// 🌹 Roze Self-Hosted Compiler - Runtime Support Header (ADV-06)
// ==============================================================================

#ifndef ROZE_SELF_HOSTED_H
#define ROZE_SELF_HOSTED_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

// --- Roze ARC String Management ---
// Roze native strings have a 16-byte header preceding the char data:
//   offset -16: int64_t refcount (1 for fresh allocation, -1 for static literal)
//   offset  -8: int64_t length
//   offset   0: char bytes, null-terminated

char*   rz_create_string(const char* s, int64_t len);
char*   rz_str_concat(const char* a, const char* b);
int64_t rz_str_len(const char* s);
int64_t rz_char_at(const char* s, int64_t index);
char*   rz_substring(const char* s, int64_t start, int64_t len);
char*   rz_char_to_str(int64_t ch);
char*   rz_int_to_str(int64_t val);
int64_t rz_str_eq(const char* a, const char* b);

// --- File IO & Process Execution ---
char*   rz_read_file(const char* path);
int64_t rz_write_file(const char* path, const char* content);
int64_t rz_file_exists(const char* path);
int64_t rz_exec(const char* cmd);

// --- Command Line & Environment ---
int64_t rz_get_arg_count(void);
char*   rz_get_arg(int64_t index);
char*   rz_get_env(const char* name);

// --- Diagnostics & Utilities ---
void    rz_print_str(const char* s);
void    rz_print_int(int64_t val);
void    rz_compiler_banner(void);

#ifdef __cplusplus
}
#endif

#endif // ROZE_SELF_HOSTED_H
