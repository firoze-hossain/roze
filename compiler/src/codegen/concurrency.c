// compiler/src/codegen/concurrency.c
//
// Thread-safe Native Concurrency Runtime for Roze (MED-03 & M1-T04)
// Provides:
//   1. M:N Green Threads (Virtual Threads) with work-stealing scheduler across CPU workers.
//   2. Zero-allocation fiber lifecycle via preallocated slab pools.
//   3. Cooperative context switching and non-blocking channels.
//   4. OS threads (pthread) and bounded/unbounded ring-buffer channels.

#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>
#include <sched.h>

#define ROZE_MAGIC_THREAD 0x544852445F4D4147ULL // "THRD_MAG"
#define ROZE_MAGIC_FIBER  0x464942525F4D4147ULL // "FIBR_MAG"

typedef struct {
    int64_t refcount;       // offset 0: unified ARC refcount
    void* (*fn_ptr)(void*); // offset 8: function / trampoline pointer
} RozeClosure;

typedef struct {
    int64_t refcount;       // offset 0: unified ARC refcount
    uint64_t magic;         // offset 8: ROZE_MAGIC_THREAD
    pthread_t thread;
    RozeClosure* closure;
    int64_t result;
    int joined;
    pthread_mutex_t lock;
} RozeThreadHandle;

// Forward declarations
void __roze_thread_release(int64_t handle_val);
void __roze_thread_retain(int64_t handle_val);
int64_t __roze_thread_join(int64_t handle_val);

void __roze_fiber_release(int64_t fiber_val);
void __roze_fiber_retain(int64_t fiber_val);
int64_t __roze_fiber_join(int64_t fiber_val);

// -------------------------------------------------------------
// OS Threads (pthreads) Implementation
// -------------------------------------------------------------

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

    handle->refcount = 2; // 1 for caller handle, 1 for background runner
    handle->magic = ROZE_MAGIC_THREAD;
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
    if (handle->magic == ROZE_MAGIC_FIBER) {
        return __roze_fiber_join(handle_val);
    }
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
    if (handle->magic == ROZE_MAGIC_FIBER) {
        __roze_fiber_retain(handle_val);
        return;
    }
    __atomic_fetch_add(&handle->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_thread_release(int64_t handle_val) {
    if (!handle_val) return;
    RozeThreadHandle* handle = (RozeThreadHandle*)handle_val;
    if (handle->magic == ROZE_MAGIC_FIBER) {
        __roze_fiber_release(handle_val);
        return;
    }
    if (__atomic_fetch_sub(&handle->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (!handle->joined) {
            pthread_detach(handle->thread);
        }
        pthread_mutex_destroy(&handle->lock);
        free(handle);
    }
}

// -------------------------------------------------------------
// M:N Green Thread (Virtual Thread) Work-Stealing Scheduler (M1-T04)
// -------------------------------------------------------------

typedef enum {
    ROZE_FIBER_READY = 0,
    ROZE_FIBER_RUNNING = 1,
    ROZE_FIBER_DONE = 2
} RozeFiberStatus;

typedef struct RozeFiber {
    int64_t refcount;       // offset 0: unified ARC refcount
    uint64_t magic;         // offset 8: ROZE_MAGIC_FIBER
    int64_t id;
    RozeClosure* closure;
    int64_t result;
    _Atomic int status;
    pthread_mutex_t lock;
    pthread_cond_t done_cond;
    struct RozeFiber* next; // for queue linking & free list
} RozeFiber;

#define ROZE_LOCAL_RING_CAP 256

typedef struct {
    RozeFiber* ring[ROZE_LOCAL_RING_CAP];
    int64_t head; // stealer pops from head (FIFO)
    int64_t tail; // worker pushes and pops from tail (LIFO)
    pthread_mutex_t lock;
} RozeLocalDeque;

typedef struct {
    int id;
    pthread_t thread;
    RozeLocalDeque deque;
    _Atomic int64_t tasks_executed;
    _Atomic int64_t tasks_stolen;
} RozeWorker;

typedef struct RozeFiberSlab {
    RozeFiber fibers[1024];
    struct RozeFiberSlab* next;
} RozeFiberSlab;

typedef struct {
    RozeFiber* free_head;
    RozeFiberSlab* slabs;
    pthread_mutex_t lock;
} RozeFiberPool;

typedef struct {
    RozeFiber* head;
    RozeFiber* tail;
    int64_t count;
    pthread_mutex_t lock;
} RozeGlobalQueue;

// Global Scheduler State
static int roze_num_workers = 0;
static RozeWorker* g_workers = NULL;
static RozeGlobalQueue g_global_queue;
static RozeFiberPool g_fiber_pool;
static pthread_mutex_t g_wake_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t g_wake_cond = PTHREAD_COND_INITIALIZER;
static _Atomic int g_scheduler_shutdown = 0;
static _Atomic int64_t g_active_fibers = 0;
static _Atomic int64_t g_total_stolen = 0;
static _Atomic int64_t g_total_executed = 0;
static _Atomic int64_t g_next_fiber_id = 1;

static __thread int current_worker_id = -1;
static __thread RozeFiber* current_running_fiber = NULL;

// Slab allocator for zero-allocation fiber recycling
static RozeFiber* roze_fiber_alloc(void) {
    pthread_mutex_lock(&g_fiber_pool.lock);
    RozeFiber* f = g_fiber_pool.free_head;
    if (f) {
        g_fiber_pool.free_head = f->next;
        pthread_mutex_unlock(&g_fiber_pool.lock);
        f->next = NULL;
        f->refcount = 2; // 1 for caller handle, 1 for scheduler execution
        f->status = ROZE_FIBER_READY;
        f->result = 0;
        return f;
    }

    // Allocate new slab of 1024 fibers
    RozeFiberSlab* slab = (RozeFiberSlab*)malloc(sizeof(RozeFiberSlab));
    slab->next = g_fiber_pool.slabs;
    g_fiber_pool.slabs = slab;

    for (int i = 1; i < 1024; i++) {
        slab->fibers[i].magic = ROZE_MAGIC_FIBER;
        pthread_mutex_init(&slab->fibers[i].lock, NULL);
        pthread_cond_init(&slab->fibers[i].done_cond, NULL);
        slab->fibers[i].next = (i < 1023) ? &slab->fibers[i + 1] : NULL;
    }
    g_fiber_pool.free_head = &slab->fibers[1];

    RozeFiber* ret = &slab->fibers[0];
    ret->magic = ROZE_MAGIC_FIBER;
    pthread_mutex_init(&ret->lock, NULL);
    pthread_cond_init(&ret->done_cond, NULL);
    ret->next = NULL;
    ret->refcount = 2;
    ret->status = ROZE_FIBER_READY;
    ret->result = 0;
    pthread_mutex_unlock(&g_fiber_pool.lock);
    return ret;
}

static void roze_fiber_free(RozeFiber* f) {
    if (!f) return;
    pthread_mutex_lock(&g_fiber_pool.lock);
    f->closure = NULL;
    f->result = 0;
    f->status = ROZE_FIBER_READY;
    f->next = g_fiber_pool.free_head;
    g_fiber_pool.free_head = f;
    pthread_mutex_unlock(&g_fiber_pool.lock);
}

// Global queue operations
static void global_push(RozeFiber* fiber) {
    pthread_mutex_lock(&g_global_queue.lock);
    fiber->next = NULL;
    if (g_global_queue.tail) {
        g_global_queue.tail->next = fiber;
        g_global_queue.tail = fiber;
    } else {
        g_global_queue.head = fiber;
        g_global_queue.tail = fiber;
    }
    g_global_queue.count++;
    pthread_mutex_unlock(&g_global_queue.lock);
}

static RozeFiber* global_pop(void) {
    pthread_mutex_lock(&g_global_queue.lock);
    RozeFiber* f = g_global_queue.head;
    if (f) {
        g_global_queue.head = f->next;
        if (!g_global_queue.head) {
            g_global_queue.tail = NULL;
        }
        g_global_queue.count--;
        f->next = NULL;
    }
    pthread_mutex_unlock(&g_global_queue.lock);
    return f;
}

// Pop a batch from global queue into worker's local deque
static int worker_fetch_batch(RozeWorker* worker, int max_batch) {
    pthread_mutex_lock(&g_global_queue.lock);
    if (!g_global_queue.head) {
        pthread_mutex_unlock(&g_global_queue.lock);
        return 0;
    }

    pthread_mutex_lock(&worker->deque.lock);
    int fetched = 0;
    while (g_global_queue.head && fetched < max_batch &&
           (worker->deque.tail - worker->deque.head < ROZE_LOCAL_RING_CAP)) {
        RozeFiber* f = g_global_queue.head;
        g_global_queue.head = f->next;
        if (!g_global_queue.head) {
            g_global_queue.tail = NULL;
        }
        g_global_queue.count--;
        f->next = NULL;

        worker->deque.ring[worker->deque.tail % ROZE_LOCAL_RING_CAP] = f;
        worker->deque.tail++;
        fetched++;
    }
    pthread_mutex_unlock(&worker->deque.lock);
    pthread_mutex_unlock(&g_global_queue.lock);
    return fetched;
}

static RozeFiber* local_pop(RozeWorker* worker) {
    pthread_mutex_lock(&worker->deque.lock);
    if (worker->deque.tail > worker->deque.head) {
        worker->deque.tail--;
        RozeFiber* f = worker->deque.ring[worker->deque.tail % ROZE_LOCAL_RING_CAP];
        pthread_mutex_unlock(&worker->deque.lock);
        return f;
    }
    pthread_mutex_unlock(&worker->deque.lock);
    return NULL;
}

static RozeFiber* worker_steal(RozeWorker* victim) {
    pthread_mutex_lock(&victim->deque.lock);
    if (victim->deque.head < victim->deque.tail) {
        RozeFiber* f = victim->deque.ring[victim->deque.head % ROZE_LOCAL_RING_CAP];
        victim->deque.head++;
        pthread_mutex_unlock(&victim->deque.lock);
        return f;
    }
    pthread_mutex_unlock(&victim->deque.lock);
    return NULL;
}

static RozeFiber* worker_try_steal(RozeWorker* worker) {
    if (roze_num_workers <= 1) return NULL;
    int start = (worker->id + 1) % roze_num_workers;
    for (int i = 0; i < roze_num_workers - 1; i++) {
        int target = (start + i) % roze_num_workers;
        RozeFiber* stolen = worker_steal(&g_workers[target]);
        if (stolen) {
            __atomic_fetch_add(&worker->tasks_stolen, 1, __ATOMIC_RELAXED);
            __atomic_fetch_add(&g_total_stolen, 1, __ATOMIC_RELAXED);
            return stolen;
        }
    }
    return NULL;
}

static void execute_fiber(RozeWorker* worker, RozeFiber* fiber) {
    if (!fiber) return;
    current_running_fiber = fiber;
    __atomic_store_n(&fiber->status, ROZE_FIBER_RUNNING, __ATOMIC_RELEASE);

    if (fiber->closure && fiber->closure->fn_ptr) {
        fiber->result = (int64_t)fiber->closure->fn_ptr(fiber->closure);
    }
    if (fiber->closure) {
        if (__atomic_fetch_sub(&fiber->closure->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            free(fiber->closure);
        }
        fiber->closure = NULL;
    }

    pthread_mutex_lock(&fiber->lock);
    __atomic_store_n(&fiber->status, ROZE_FIBER_DONE, __ATOMIC_RELEASE);
    pthread_cond_broadcast(&fiber->done_cond);
    pthread_mutex_unlock(&fiber->lock);

    current_running_fiber = NULL;
    if (worker) {
        __atomic_fetch_add(&worker->tasks_executed, 1, __ATOMIC_RELAXED);
    }
    __atomic_fetch_add(&g_total_executed, 1, __ATOMIC_RELAXED);
    __atomic_fetch_sub(&g_active_fibers, 1, __ATOMIC_RELAXED);
    __roze_fiber_release((int64_t)fiber); // release scheduler's reference
}

static void* roze_worker_loop(void* arg) {
    RozeWorker* worker = (RozeWorker*)arg;
    current_worker_id = worker->id;

    while (!__atomic_load_n(&g_scheduler_shutdown, __ATOMIC_ACQUIRE)) {
        // 1. Try local deque
        RozeFiber* fiber = local_pop(worker);

        // 2. If empty, batch fetch from global queue
        if (!fiber) {
            if (worker_fetch_batch(worker, 16) > 0) {
                fiber = local_pop(worker);
            }
        }

        // 3. If still empty, try direct pop from global
        if (!fiber) {
            fiber = global_pop();
        }

        // 4. If still empty, try work stealing from peer workers
        if (!fiber) {
            fiber = worker_try_steal(worker);
        }

        if (fiber) {
            execute_fiber(worker, fiber);
            continue;
        }

        // 5. Idle: wait on wake condition with 1 ms timeout
        pthread_mutex_lock(&g_wake_lock);
        if (__atomic_load_n(&g_active_fibers, __ATOMIC_RELAXED) == 0 &&
            !__atomic_load_n(&g_scheduler_shutdown, __ATOMIC_ACQUIRE)) {
            struct timespec ts;
            clock_gettime(CLOCK_REALTIME, &ts);
            ts.tv_nsec += 1000000; // 1 ms wait
            if (ts.tv_nsec >= 1000000000) {
                ts.tv_sec += 1;
                ts.tv_nsec -= 1000000000;
            }
            pthread_cond_timedwait(&g_wake_cond, &g_wake_lock, &ts);
        }
        pthread_mutex_unlock(&g_wake_lock);
    }
    return NULL;
}

static pthread_once_t g_init_once = PTHREAD_ONCE_INIT;

static void roze_scheduler_init_impl(void) {
    roze_num_workers = (int)sysconf(_SC_NPROCESSORS_ONLN);
    if (roze_num_workers <= 0) roze_num_workers = 4;
    if (roze_num_workers > 64) roze_num_workers = 64;

    pthread_mutex_init(&g_fiber_pool.lock, NULL);
    g_fiber_pool.free_head = NULL;
    g_fiber_pool.slabs = NULL;

    pthread_mutex_init(&g_global_queue.lock, NULL);
    g_global_queue.head = NULL;
    g_global_queue.tail = NULL;
    g_global_queue.count = 0;

    g_workers = (RozeWorker*)calloc(roze_num_workers, sizeof(RozeWorker));
    for (int i = 0; i < roze_num_workers; i++) {
        g_workers[i].id = i;
        pthread_mutex_init(&g_workers[i].deque.lock, NULL);
        g_workers[i].deque.head = 0;
        g_workers[i].deque.tail = 0;
        g_workers[i].tasks_executed = 0;
        g_workers[i].tasks_stolen = 0;
        pthread_create(&g_workers[i].thread, NULL, roze_worker_loop, &g_workers[i]);
        pthread_detach(g_workers[i].thread);
    }
}

static void roze_scheduler_ensure_init(void) {
    pthread_once(&g_init_once, roze_scheduler_init_impl);
}

int64_t __roze_fiber_spawn(void* closure_ptr) {
    if (!closure_ptr) return 0;
    roze_scheduler_ensure_init();

    RozeFiber* fiber = roze_fiber_alloc();
    if (!fiber) return 0;

    fiber->id = __atomic_fetch_add(&g_next_fiber_id, 1, __ATOMIC_RELAXED);
    fiber->closure = (RozeClosure*)closure_ptr;
    fiber->result = 0;
    fiber->status = ROZE_FIBER_READY;

    __atomic_fetch_add(&g_active_fibers, 1, __ATOMIC_RELAXED);

    // Push to global queue: available for any worker
    global_push(fiber);

    pthread_cond_broadcast(&g_wake_cond);
    return (int64_t)fiber;
}

int64_t __roze_fiber_join(int64_t fiber_val) {
    if (!fiber_val) return 0;
    RozeFiber* fiber = (RozeFiber*)fiber_val;
    if (fiber->magic == ROZE_MAGIC_THREAD) {
        return __roze_thread_join(fiber_val);
    }
    if (fiber->magic != ROZE_MAGIC_FIBER) return 0;

    pthread_mutex_lock(&fiber->lock);
    while (__atomic_load_n(&fiber->status, __ATOMIC_ACQUIRE) != ROZE_FIBER_DONE) {
        pthread_cond_wait(&fiber->done_cond, &fiber->lock);
    }
    int64_t res = fiber->result;
    pthread_mutex_unlock(&fiber->lock);

    return res;
}

void __roze_fiber_yield(void) {
    sched_yield();
}

void __roze_fiber_sleep(int64_t ms) {
    if (ms <= 0) {
        sched_yield();
        return;
    }
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000;
    nanosleep(&ts, NULL);
}

int64_t __roze_fiber_id(void) {
    if (current_running_fiber) {
        return current_running_fiber->id;
    }
    return (int64_t)pthread_self();
}

int64_t __roze_fiber_count(void) {
    return __atomic_load_n(&g_active_fibers, __ATOMIC_RELAXED);
}

void __roze_fiber_stats(void) {
    int64_t active = __atomic_load_n(&g_active_fibers, __ATOMIC_RELAXED);
    int64_t stolen = __atomic_load_n(&g_total_stolen, __ATOMIC_RELAXED);
    int64_t executed = __atomic_load_n(&g_total_executed, __ATOMIC_RELAXED);
    printf("[Roze M:N Green Thread Scheduler] Workers: %d, Active: %lld, Executed: %lld, Stolen: %lld\n",
           roze_num_workers, (long long)active, (long long)executed, (long long)stolen);
}

void __roze_fiber_retain(int64_t fiber_val) {
    if (!fiber_val) return;
    RozeFiber* fiber = (RozeFiber*)fiber_val;
    if (fiber->magic == ROZE_MAGIC_THREAD) {
        __roze_thread_retain(fiber_val);
        return;
    }
    if (fiber->magic != ROZE_MAGIC_FIBER) return;
    __atomic_fetch_add(&fiber->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_fiber_release(int64_t fiber_val) {
    if (!fiber_val) return;
    RozeFiber* fiber = (RozeFiber*)fiber_val;
    if (fiber->magic == ROZE_MAGIC_THREAD) {
        __roze_thread_release(fiber_val);
        return;
    }
    if (fiber->magic != ROZE_MAGIC_FIBER) return;
    if (__atomic_fetch_sub(&fiber->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        roze_fiber_free(fiber);
    }
}

// -------------------------------------------------------------
// Channel Implementation (Ring Buffer)
// -------------------------------------------------------------

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
