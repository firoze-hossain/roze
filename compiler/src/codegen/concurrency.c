// compiler/src/codegen/concurrency.c
//
// Thread-safe Native Concurrency Runtime for Roze (MED-03)
// Provides OS threads (pthread) and bounded/unbounded ring-buffer channels.

#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>

typedef struct {
    int64_t refcount;       // offset 0: unified ARC refcount
    void* (*fn_ptr)(void*); // offset 8: function / trampoline pointer
} RozeClosure;

typedef struct {
    int64_t refcount;       // offset 0: unified ARC refcount
    pthread_t thread;
    RozeClosure* closure;
    int64_t result;
    int joined;
    pthread_mutex_t lock;
} RozeThreadHandle;

void __roze_thread_release(int64_t handle_val);

static void* roze_thread_runner(void* arg) {
    RozeThreadHandle* handle = (RozeThreadHandle*)arg;
    if (handle && handle->closure && handle->closure->fn_ptr) {
        handle->result = (int64_t)handle->closure->fn_ptr(handle->closure);
    }
    if (handle && handle->closure) {
        if (__atomic_fetch_sub(&handle->closure->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            free(handle->closure);
        }
        handle->closure = NULL;
    }
    __roze_thread_release((int64_t)handle);
    return NULL;
}

int64_t __roze_thread_spawn(void* closure_ptr) {
    if (!closure_ptr) return 0;
    RozeThreadHandle* handle = (RozeThreadHandle*)malloc(sizeof(RozeThreadHandle));
    if (!handle) return 0;

    // refcount = 2: 1 for caller handle, 1 for background runner
    handle->refcount = 2;
    handle->closure = (RozeClosure*)closure_ptr;
    handle->result = 0;
    handle->joined = 0;
    pthread_mutex_init(&handle->lock, NULL);

    if (pthread_create(&handle->thread, NULL, roze_thread_runner, handle) != 0) {
        if (__atomic_fetch_sub(&handle->closure->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            free(handle->closure);
        }
        pthread_mutex_destroy(&handle->lock);
        free(handle);
        return 0;
    }
    return (int64_t)handle;
}

int64_t __roze_thread_join(int64_t handle_val) {
    if (!handle_val) return 0;
    RozeThreadHandle* handle = (RozeThreadHandle*)handle_val;
    pthread_mutex_lock(&handle->lock);
    if (!handle->joined) {
        pthread_join(handle->thread, NULL);
        handle->joined = 1;
    }
    int64_t res = handle->result;
    pthread_mutex_unlock(&handle->lock);
    return res;
}

void __roze_thread_sleep(int64_t ms) {
    if (ms <= 0) return;
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, NULL);
}

int64_t __roze_thread_current_id(void) {
    return (int64_t)pthread_self();
}

void __roze_thread_retain(int64_t handle_val) {
    if (!handle_val) return;
    RozeThreadHandle* handle = (RozeThreadHandle*)handle_val;
    __atomic_fetch_add(&handle->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_thread_release(int64_t handle_val) {
    if (!handle_val) return;
    RozeThreadHandle* handle = (RozeThreadHandle*)handle_val;
    if (__atomic_fetch_sub(&handle->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (!handle->joined) {
            pthread_detach(handle->thread);
        }
        pthread_mutex_destroy(&handle->lock);
        free(handle);
    }
}

typedef struct {
    int64_t refcount;       // offset 0: unified ARC refcount
    pthread_mutex_t lock;
    pthread_cond_t not_empty;
    pthread_cond_t not_full;
    int64_t* buffer;
    int64_t capacity;
    int64_t head;
    int64_t tail;
    int64_t count;
    int closed;
} RozeChannel;

int64_t __roze_channel_new(int64_t capacity) {
    if (capacity <= 0) capacity = 64;
    RozeChannel* chan = (RozeChannel*)malloc(sizeof(RozeChannel));
    if (!chan) return 0;

    chan->refcount = 1;
    pthread_mutex_init(&chan->lock, NULL);
    pthread_cond_init(&chan->not_empty, NULL);
    pthread_cond_init(&chan->not_full, NULL);

    chan->buffer = (int64_t*)malloc(sizeof(int64_t) * capacity);
    chan->capacity = capacity;
    chan->head = 0;
    chan->tail = 0;
    chan->count = 0;
    chan->closed = 0;

    return (int64_t)chan;
}

int64_t __roze_channel_send(int64_t chan_val, int64_t val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;

    pthread_mutex_lock(&chan->lock);
    while (chan->count == chan->capacity && !chan->closed) {
        pthread_cond_wait(&chan->not_full, &chan->lock);
    }
    if (chan->closed) {
        pthread_mutex_unlock(&chan->lock);
        return 0; // false: channel closed
    }

    chan->buffer[chan->tail] = val;
    chan->tail = (chan->tail + 1) % chan->capacity;
    chan->count++;

    pthread_cond_signal(&chan->not_empty);
    pthread_mutex_unlock(&chan->lock);
    return 1; // true: sent successfully
}

int64_t __roze_channel_recv(int64_t chan_val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;

    pthread_mutex_lock(&chan->lock);
    while (chan->count == 0 && !chan->closed) {
        pthread_cond_wait(&chan->not_empty, &chan->lock);
    }
    if (chan->count == 0 && chan->closed) {
        pthread_mutex_unlock(&chan->lock);
        return 0;
    }

    int64_t val = chan->buffer[chan->head];
    chan->head = (chan->head + 1) % chan->capacity;
    chan->count--;

    pthread_cond_signal(&chan->not_full);
    pthread_mutex_unlock(&chan->lock);
    return val;
}

void __roze_channel_close(int64_t chan_val) {
    if (!chan_val) return;
    RozeChannel* chan = (RozeChannel*)chan_val;

    pthread_mutex_lock(&chan->lock);
    chan->closed = 1;
    pthread_cond_broadcast(&chan->not_empty);
    pthread_cond_broadcast(&chan->not_full);
    pthread_mutex_unlock(&chan->lock);
}

int64_t __roze_channel_is_closed(int64_t chan_val) {
    if (!chan_val) return 1;
    RozeChannel* chan = (RozeChannel*)chan_val;

    pthread_mutex_lock(&chan->lock);
    int closed = chan->closed;
    pthread_mutex_unlock(&chan->lock);
    return (int64_t)closed;
}

int64_t __roze_channel_len(int64_t chan_val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;

    pthread_mutex_lock(&chan->lock);
    int64_t len = chan->count;
    pthread_mutex_unlock(&chan->lock);
    return len;
}

void __roze_channel_retain(int64_t chan_val) {
    if (!chan_val) return;
    RozeChannel* chan = (RozeChannel*)chan_val;
    __atomic_fetch_add(&chan->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_channel_release(int64_t chan_val) {
    if (!chan_val) return;
    RozeChannel* chan = (RozeChannel*)chan_val;
    if (__atomic_fetch_sub(&chan->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        pthread_mutex_destroy(&chan->lock);
        pthread_cond_destroy(&chan->not_empty);
        pthread_cond_destroy(&chan->not_full);
        if (chan->buffer) free(chan->buffer);
        free(chan);
    }
}

// Freestanding VGA Text Driver Stubs for Hosted Native Target
__attribute__((weak)) void vga_clear(void) {}
__attribute__((weak)) void vga_print(const char* s) { if (s) fputs(s, stdout); }
__attribute__((weak)) void vga_write_char(int64_t row, int64_t col, int64_t ch, int64_t color) { (void)row; (void)col; (void)ch; (void)color; }
__attribute__((weak)) void vga_write_byte(int64_t offset, int64_t byte_val) { (void)offset; (void)byte_val; }
__attribute__((weak)) void vga_set_cursor(int64_t row, int64_t col) { (void)row; (void)col; }
__attribute__((weak)) void vga_set_color(int64_t fg, int64_t bg) { (void)fg; (void)bg; }
__attribute__((weak)) void __roze_vga_print_str(const char* s) { if (s) { fputs(s, stdout); putchar('\n'); } }
__attribute__((weak)) void __roze_vga_print_int(int64_t val) { printf("%lld\n", (long long)val); }
__attribute__((weak)) void __roze_vga_print_bool(int64_t val) { puts(val ? "true" : "false"); }

// Freestanding Kernel & Multiboot2 Stubs for Hosted Native Target
__attribute__((weak)) int64_t multiboot2_get_magic(void) { return 0x36D76289LL; }
__attribute__((weak)) void* multiboot2_get_info_ptr(void) { return NULL; }
__attribute__((weak)) void outb(int64_t port, int64_t val) { (void)port; (void)val; }
__attribute__((weak)) int64_t inb(int64_t port) { (void)port; return 0; }
__attribute__((weak)) void qemu_exit(int64_t code) { (void)code; }
__attribute__((weak)) void serial_print(const char* s) { if (s) fputs(s, stdout); }
__attribute__((weak)) void serial_putchar(int64_t ch) { putchar((int)ch); }

// RozeBoot Enterprise Framework Stubs for Hosted Native Target
#include <time.h>
__attribute__((weak)) void* db_pool_create(const char* url, int64_t max_conns) { (void)url; (void)max_conns; return NULL; }
__attribute__((weak)) void* db_pool_get(void* pool) { (void)pool; return NULL; }
__attribute__((weak)) void db_pool_release(void* pool, void* conn) { (void)pool; (void)conn; }
__attribute__((weak)) void db_pool_close(void* pool) { (void)pool; }
__attribute__((weak)) void* db_pool_stats(void* pool) { (void)pool; return NULL; }
__attribute__((weak)) void* db_pool_query(void* pool, const char* sql) { (void)pool; (void)sql; return NULL; }
__attribute__((weak)) int64_t db_pool_execute(void* pool, const char* sql) { (void)pool; (void)sql; return 0; }

__attribute__((weak)) const char* jwt_sign(void* claims, const char* secret) { (void)claims; (void)secret; return ""; }
__attribute__((weak)) int64_t jwt_verify(const char* token, const char* secret) { (void)token; (void)secret; return 0; }
__attribute__((weak)) void* jwt_decode(const char* token) { (void)token; return NULL; }
__attribute__((weak)) void* jwt_decode_header(const char* token) { (void)token; return NULL; }

__attribute__((weak)) int64_t time_now(void) { return (int64_t)time(NULL); }
__attribute__((weak)) int64_t time_now_millis(void) { return (int64_t)time(NULL) * 1000LL; }
__attribute__((weak)) void* map_keys(void* m) { (void)m; return NULL; }
__attribute__((weak)) const char* substring(const char* s, int64_t start, int64_t end) { (void)start; (void)end; return s ? s : ""; }
__attribute__((weak)) const char* string_substring(const char* s, int64_t start, int64_t end) { (void)start; (void)end; return s ? s : ""; }
__attribute__((weak)) int64_t string_contains(const char* s, const char* sub) { (void)s; (void)sub; return 0; }
__attribute__((weak)) int64_t string_starts_with(const char* s, const char* p) { (void)s; (void)p; return 0; }
__attribute__((weak)) int64_t string_ends_with(const char* s, const char* suffx) { (void)s; (void)suffx; return 0; }
__attribute__((weak)) void* http_request(const char* m, const char* u, void* h, const char* b) { (void)m; (void)u; (void)h; (void)b; return NULL; }



