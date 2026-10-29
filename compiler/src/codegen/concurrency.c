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
#include <errno.h>
#include <math.h>
#include <string.h>

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
// Channel Implementation (High-Throughput MPMC Bounded & Unbounded)
// -------------------------------------------------------------

typedef struct {
    int64_t refcount;       // offset 0: unified ARC refcount
    pthread_mutex_t lock;
    pthread_cond_t not_empty;
    pthread_cond_t not_full;
    int64_t* buffer;
    int64_t capacity;       // -1 for unbounded, >0 for bounded
    int64_t alloc_cap;      // current allocated size
    int64_t head;
    int64_t tail;
    int64_t count;
    int closed;
} RozeChannel;

int64_t __roze_channel_new(int64_t capacity) {
    if (capacity <= 0 && capacity != -1) capacity = 64;
    RozeChannel* chan = (RozeChannel*)malloc(sizeof(RozeChannel));
    if (!chan) return 0;

    chan->refcount = 1;
    pthread_mutex_init(&chan->lock, NULL);
    pthread_cond_init(&chan->not_empty, NULL);
    pthread_cond_init(&chan->not_full, NULL);

    int64_t init_alloc = (capacity > 0) ? capacity : 64;
    chan->buffer = (int64_t*)malloc(sizeof(int64_t) * init_alloc);
    chan->capacity = capacity;
    chan->alloc_cap = init_alloc;
    chan->head = 0;
    chan->tail = 0;
    chan->count = 0;
    chan->closed = 0;

    return (int64_t)chan;
}

int64_t __roze_channel_unbounded(void) {
    return __roze_channel_new(-1);
}

int64_t __roze_channel_capacity(int64_t chan_val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;
    return chan->capacity;
}

int64_t __roze_channel_send(int64_t chan_val, int64_t val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;

    pthread_mutex_lock(&chan->lock);
    if (chan->capacity > 0) {
        while (chan->count == chan->capacity && !chan->closed) {
            pthread_cond_wait(&chan->not_full, &chan->lock);
        }
    }
    if (chan->closed) {
        pthread_mutex_unlock(&chan->lock);
        return 0; // false: channel closed
    }

    if (chan->count == chan->alloc_cap) {
        int64_t new_cap = chan->alloc_cap * 2;
        int64_t* new_buf = (int64_t*)malloc(sizeof(int64_t) * new_cap);
        for (int64_t i = 0; i < chan->count; i++) {
            new_buf[i] = chan->buffer[(chan->head + i) % chan->alloc_cap];
        }
        free(chan->buffer);
        chan->buffer = new_buf;
        chan->head = 0;
        chan->tail = chan->count;
        chan->alloc_cap = new_cap;
    }

    chan->buffer[chan->tail] = val;
    chan->tail = (chan->tail + 1) % chan->alloc_cap;
    chan->count++;

    pthread_cond_signal(&chan->not_empty);
    pthread_mutex_unlock(&chan->lock);
    return 1; // true: sent successfully
}

int64_t __roze_channel_try_send(int64_t chan_val, int64_t val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;

    if (pthread_mutex_trylock(&chan->lock) != 0) return 0;
    if (chan->closed || (chan->capacity > 0 && chan->count >= chan->capacity)) {
        pthread_mutex_unlock(&chan->lock);
        return 0;
    }

    if (chan->count == chan->alloc_cap) {
        int64_t new_cap = chan->alloc_cap * 2;
        int64_t* new_buf = (int64_t*)malloc(sizeof(int64_t) * new_cap);
        for (int64_t i = 0; i < chan->count; i++) {
            new_buf[i] = chan->buffer[(chan->head + i) % chan->alloc_cap];
        }
        free(chan->buffer);
        chan->buffer = new_buf;
        chan->head = 0;
        chan->tail = chan->count;
        chan->alloc_cap = new_cap;
    }

    chan->buffer[chan->tail] = val;
    chan->tail = (chan->tail + 1) % chan->alloc_cap;
    chan->count++;

    pthread_cond_signal(&chan->not_empty);
    pthread_mutex_unlock(&chan->lock);
    return 1;
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
    chan->head = (chan->head + 1) % chan->alloc_cap;
    chan->count--;

    if (chan->capacity > 0) {
        pthread_cond_signal(&chan->not_full);
    }
    pthread_mutex_unlock(&chan->lock);
    return val;
}

int64_t __roze_channel_try_recv(int64_t chan_val) {
    if (!chan_val) return 0;
    RozeChannel* chan = (RozeChannel*)chan_val;

    if (pthread_mutex_trylock(&chan->lock) != 0) return 0;
    if (chan->count == 0) {
        pthread_mutex_unlock(&chan->lock);
        return 0;
    }

    int64_t val = chan->buffer[chan->head];
    chan->head = (chan->head + 1) % chan->alloc_cap;
    chan->count--;

    if (chan->capacity > 0) {
        pthread_cond_signal(&chan->not_full);
    }
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

// -------------------------------------------------------------
// Synchronization: Mutex, RwLock, and Atomic Types (M1-T06)
// -------------------------------------------------------------

#define ROZE_MAGIC_MUTEX  0x524F5A4D55544558ULL /* "ROZMUTEX" */
#define ROZE_MAGIC_RWLOCK 0x524F5A52574C4F43ULL /* "ROZRWLOC" */
#define ROZE_MAGIC_ATOMIC 0x524F5A41544F4D49ULL /* "ROZATOMI" */

typedef struct {
    int64_t refcount;
    uint64_t magic;
    pthread_mutex_t lock;
} RozeMutex;

int64_t __roze_mutex_new(void) {
    RozeMutex* m = (RozeMutex*)malloc(sizeof(RozeMutex));
    if (!m) return 0;
    m->refcount = 1;
    m->magic = ROZE_MAGIC_MUTEX;
    pthread_mutex_init(&m->lock, NULL);
    return (int64_t)m;
}

void __roze_mutex_lock(int64_t m_val) {
    if (!m_val) return;
    RozeMutex* m = (RozeMutex*)m_val;
    if (m->magic == ROZE_MAGIC_MUTEX) {
        pthread_mutex_lock(&m->lock);
    }
}

void __roze_mutex_unlock(int64_t m_val) {
    if (!m_val) return;
    RozeMutex* m = (RozeMutex*)m_val;
    if (m->magic == ROZE_MAGIC_MUTEX) {
        pthread_mutex_unlock(&m->lock);
    }
}

int64_t __roze_mutex_try_lock(int64_t m_val) {
    if (!m_val) return 0;
    RozeMutex* m = (RozeMutex*)m_val;
    if (m->magic == ROZE_MAGIC_MUTEX) {
        return pthread_mutex_trylock(&m->lock) == 0 ? 1 : 0;
    }
    return 0;
}

void __roze_mutex_retain(int64_t m_val) {
    if (!m_val) return;
    RozeMutex* m = (RozeMutex*)m_val;
    if (m->magic == ROZE_MAGIC_MUTEX) {
        __atomic_fetch_add(&m->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_mutex_release(int64_t m_val) {
    if (!m_val) return;
    RozeMutex* m = (RozeMutex*)m_val;
    if (m->magic == ROZE_MAGIC_MUTEX) {
        if (__atomic_fetch_sub(&m->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            pthread_mutex_destroy(&m->lock);
            free(m);
        }
    }
}

typedef struct {
    int64_t refcount;
    uint64_t magic;
    pthread_rwlock_t rwlock;
} RozeRwLock;

int64_t __roze_rwlock_new(void) {
    RozeRwLock* rw = (RozeRwLock*)malloc(sizeof(RozeRwLock));
    if (!rw) return 0;
    rw->refcount = 1;
    rw->magic = ROZE_MAGIC_RWLOCK;
    pthread_rwlock_init(&rw->rwlock, NULL);
    return (int64_t)rw;
}

void __roze_rwlock_read_lock(int64_t rw_val) {
    if (!rw_val) return;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        pthread_rwlock_rdlock(&rw->rwlock);
    }
}

void __roze_rwlock_read_unlock(int64_t rw_val) {
    if (!rw_val) return;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        pthread_rwlock_unlock(&rw->rwlock);
    }
}

void __roze_rwlock_write_lock(int64_t rw_val) {
    if (!rw_val) return;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        pthread_rwlock_wrlock(&rw->rwlock);
    }
}

void __roze_rwlock_write_unlock(int64_t rw_val) {
    if (!rw_val) return;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        pthread_rwlock_unlock(&rw->rwlock);
    }
}

int64_t __roze_rwlock_try_read_lock(int64_t rw_val) {
    if (!rw_val) return 0;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        return pthread_rwlock_tryrdlock(&rw->rwlock) == 0 ? 1 : 0;
    }
    return 0;
}

int64_t __roze_rwlock_try_write_lock(int64_t rw_val) {
    if (!rw_val) return 0;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        return pthread_rwlock_trywrlock(&rw->rwlock) == 0 ? 1 : 0;
    }
    return 0;
}

void __roze_rwlock_retain(int64_t rw_val) {
    if (!rw_val) return;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        __atomic_fetch_add(&rw->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_rwlock_release(int64_t rw_val) {
    if (!rw_val) return;
    RozeRwLock* rw = (RozeRwLock*)rw_val;
    if (rw->magic == ROZE_MAGIC_RWLOCK) {
        if (__atomic_fetch_sub(&rw->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            pthread_rwlock_destroy(&rw->rwlock);
            free(rw);
        }
    }
}

typedef struct {
    int64_t refcount;
    uint64_t magic;
    _Atomic int64_t value;
} RozeAtomic;

int64_t __roze_atomic_new(int64_t init) {
    RozeAtomic* a = (RozeAtomic*)malloc(sizeof(RozeAtomic));
    if (!a) return 0;
    a->refcount = 1;
    a->magic = ROZE_MAGIC_ATOMIC;
    __atomic_store_n(&a->value, init, __ATOMIC_SEQ_CST);
    return (int64_t)a;
}

int64_t __roze_atomic_load(int64_t a_val) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_load_n(&a->value, __ATOMIC_SEQ_CST);
}

void __roze_atomic_store(int64_t a_val, int64_t val) {
    if (!a_val) return;
    RozeAtomic* a = (RozeAtomic*)a_val;
    __atomic_store_n(&a->value, val, __ATOMIC_SEQ_CST);
}

int64_t __roze_atomic_swap(int64_t a_val, int64_t val) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_exchange_n(&a->value, val, __ATOMIC_SEQ_CST);
}

int64_t __roze_atomic_cas(int64_t a_val, int64_t expected, int64_t desired) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    int64_t exp = expected;
    return __atomic_compare_exchange_n(&a->value, &exp, desired, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST) ? 1 : 0;
}

int64_t __roze_atomic_add(int64_t a_val, int64_t delta) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_fetch_add(&a->value, delta, __ATOMIC_SEQ_CST);
}

int64_t __roze_atomic_sub(int64_t a_val, int64_t delta) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_fetch_sub(&a->value, delta, __ATOMIC_SEQ_CST);
}

int64_t __roze_atomic_and(int64_t a_val, int64_t val) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_fetch_and(&a->value, val, __ATOMIC_SEQ_CST);
}

int64_t __roze_atomic_or(int64_t a_val, int64_t val) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_fetch_or(&a->value, val, __ATOMIC_SEQ_CST);
}

int64_t __roze_atomic_xor(int64_t a_val, int64_t val) {
    if (!a_val) return 0;
    RozeAtomic* a = (RozeAtomic*)a_val;
    return __atomic_fetch_xor(&a->value, val, __ATOMIC_SEQ_CST);
}

void __roze_atomic_retain(int64_t a_val) {
    if (!a_val) return;
    RozeAtomic* a = (RozeAtomic*)a_val;
    if (a->magic == ROZE_MAGIC_ATOMIC) {
        __atomic_fetch_add(&a->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_atomic_release(int64_t a_val) {
    if (!a_val) return;
    RozeAtomic* a = (RozeAtomic*)a_val;
    if (a->magic == ROZE_MAGIC_ATOMIC) {
        if (__atomic_fetch_sub(&a->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            free(a);
        }
    }
}

// Typed atomic wrappers
int64_t __roze_atomic_bool_new(int64_t val) { return __roze_atomic_new(val ? 1 : 0); }
int64_t __roze_atomic_bool_load(int64_t a) { return __roze_atomic_load(a) ? 1 : 0; }
void __roze_atomic_bool_store(int64_t a, int64_t val) { __roze_atomic_store(a, val ? 1 : 0); }
int64_t __roze_atomic_bool_swap(int64_t a, int64_t val) { return __roze_atomic_swap(a, val ? 1 : 0) ? 1 : 0; }
int64_t __roze_atomic_bool_cas(int64_t a, int64_t exp, int64_t des) { return __roze_atomic_cas(a, exp ? 1 : 0, des ? 1 : 0); }

int64_t __roze_atomic_i64_new(int64_t val) { return __roze_atomic_new(val); }
int64_t __roze_atomic_i64_load(int64_t a) { return __roze_atomic_load(a); }
void __roze_atomic_i64_store(int64_t a, int64_t val) { __roze_atomic_store(a, val); }
int64_t __roze_atomic_i64_swap(int64_t a, int64_t val) { return __roze_atomic_swap(a, val); }
int64_t __roze_atomic_i64_cas(int64_t a, int64_t exp, int64_t des) { return __roze_atomic_cas(a, exp, des); }
int64_t __roze_atomic_i64_add(int64_t a, int64_t delta) { return __roze_atomic_add(a, delta); }
int64_t __roze_atomic_i64_sub(int64_t a, int64_t delta) { return __roze_atomic_sub(a, delta); }
int64_t __roze_atomic_i64_and(int64_t a, int64_t val) { return __roze_atomic_and(a, val); }
int64_t __roze_atomic_i64_or(int64_t a, int64_t val) { return __roze_atomic_or(a, val); }
int64_t __roze_atomic_i64_xor(int64_t a, int64_t val) { return __roze_atomic_xor(a, val); }

int64_t __roze_atomic_usize_new(int64_t val) { return __roze_atomic_new(val); }
int64_t __roze_atomic_usize_load(int64_t a) { return __roze_atomic_load(a); }
void __roze_atomic_usize_store(int64_t a, int64_t val) { __roze_atomic_store(a, val); }
int64_t __roze_atomic_usize_swap(int64_t a, int64_t val) { return __roze_atomic_swap(a, val); }
int64_t __roze_atomic_usize_cas(int64_t a, int64_t exp, int64_t des) { return __roze_atomic_cas(a, exp, des); }
int64_t __roze_atomic_usize_add(int64_t a, int64_t delta) { return __roze_atomic_add(a, delta); }
int64_t __roze_atomic_usize_sub(int64_t a, int64_t delta) { return __roze_atomic_sub(a, delta); }

int64_t __roze_atomic_ptr_new(int64_t val) { return __roze_atomic_new(val); }
int64_t __roze_atomic_ptr_load(int64_t a) { return __roze_atomic_load(a); }
void __roze_atomic_ptr_store(int64_t a, int64_t val) { __roze_atomic_store(a, val); }
int64_t __roze_atomic_ptr_swap(int64_t a, int64_t val) { return __roze_atomic_swap(a, val); }
int64_t __roze_atomic_ptr_cas(int64_t a, int64_t exp, int64_t des) { return __roze_atomic_cas(a, exp, des); }

// ============================================================================
// 4. Async / Await State-Machine Engine & Future<T> Runtime (M1-T05)
// ============================================================================

#define ROZE_MAGIC_FUTURE 0x524F5A4655545552ULL /* "ROZFUTUR" */

#define ROZE_FUTURE_PENDING 0
#define ROZE_FUTURE_READY   1
#define ROZE_FUTURE_ERROR   2

typedef struct RozeFuture {
    uint64_t refcount;
    uint64_t magic;
    _Atomic int status;
    int64_t result;
    pthread_mutex_t lock;
    pthread_cond_t cond;
    struct RozeFuture* next_free;
} RozeFuture;

#define ROZE_FUTURES_PER_SLAB 512

typedef struct RozeFutureSlab {
    RozeFuture futures[ROZE_FUTURES_PER_SLAB];
    struct RozeFutureSlab* next;
} RozeFutureSlab;

typedef struct {
    pthread_mutex_t lock;
    RozeFuture* free_head;
    RozeFutureSlab* slabs;
} RozeFuturePool;

static RozeFuturePool g_future_pool = { PTHREAD_MUTEX_INITIALIZER, NULL, NULL };

static RozeFuture* roze_future_alloc(void) {
    pthread_mutex_lock(&g_future_pool.lock);
    if (!g_future_pool.free_head) {
        RozeFutureSlab* slab = (RozeFutureSlab*)calloc(1, sizeof(RozeFutureSlab));
        if (!slab) {
            pthread_mutex_unlock(&g_future_pool.lock);
            return NULL;
        }
        slab->next = g_future_pool.slabs;
        g_future_pool.slabs = slab;
        for (int i = 0; i < ROZE_FUTURES_PER_SLAB; i++) {
            pthread_mutex_init(&slab->futures[i].lock, NULL);
            pthread_cond_init(&slab->futures[i].cond, NULL);
            slab->futures[i].next_free = g_future_pool.free_head;
            g_future_pool.free_head = &slab->futures[i];
        }
    }
    RozeFuture* f = g_future_pool.free_head;
    g_future_pool.free_head = f->next_free;
    pthread_mutex_unlock(&g_future_pool.lock);

    f->magic = ROZE_MAGIC_FUTURE;
    f->refcount = 1;
    f->status = ROZE_FUTURE_PENDING;
    f->result = 0;
    f->next_free = NULL;
    return f;
}

static void roze_future_recycle(RozeFuture* f) {
    if (!f || f->magic != ROZE_MAGIC_FUTURE) return;
    f->magic = 0;
    pthread_mutex_lock(&g_future_pool.lock);
    f->next_free = g_future_pool.free_head;
    g_future_pool.free_head = f;
    pthread_mutex_unlock(&g_future_pool.lock);
}

void __roze_future_retain(int64_t fut_handle) {
    if (!fut_handle) return;
    RozeFuture* f = (RozeFuture*)fut_handle;
    if (f->magic == ROZE_MAGIC_FUTURE) {
        __atomic_fetch_add(&f->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_future_release(int64_t fut_handle) {
    if (!fut_handle) return;
    RozeFuture* f = (RozeFuture*)fut_handle;
    if (f->magic == ROZE_MAGIC_FUTURE) {
        if (__atomic_fetch_sub(&f->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            roze_future_recycle(f);
        }
    }
}

typedef struct {
    RozeFuture* future;
    RozeClosure* closure;
} RozeFutureSpawnTask;

static void* roze_future_task_worker(void* arg) {
    RozeFutureSpawnTask* task = (RozeFutureSpawnTask*)arg;
    RozeFuture* f = task->future;
    RozeClosure* closure = task->closure;
    free(task);

    int64_t res = 0;
    if (closure && closure->fn_ptr) {
        res = (int64_t)closure->fn_ptr(closure);
    }
    if (closure) {
        if (__atomic_fetch_sub(&closure->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            free(closure);
        }
    }

    pthread_mutex_lock(&f->lock);
    f->result = res;
    __atomic_store_n(&f->status, ROZE_FUTURE_READY, __ATOMIC_RELEASE);
    pthread_cond_broadcast(&f->cond);
    pthread_mutex_unlock(&f->lock);

    __roze_future_release((int64_t)f);
    return NULL;
}

int64_t __roze_async_spawn(void* closure_ptr) {
    if (!closure_ptr) return 0;
    RozeFuture* f = roze_future_alloc();
    if (!f) return 0;
    f->refcount = 2; // caller + worker
    f->status = ROZE_FUTURE_PENDING;

    RozeClosure* closure = (RozeClosure*)closure_ptr;
    __atomic_fetch_add(&closure->refcount, 1, __ATOMIC_SEQ_CST);

    RozeFutureSpawnTask* task = (RozeFutureSpawnTask*)malloc(sizeof(RozeFutureSpawnTask));
    task->future = f;
    task->closure = closure;

    pthread_t th;
    if (pthread_create(&th, NULL, roze_future_task_worker, task) == 0) {
        pthread_detach(th);
    } else {
        roze_future_task_worker(task);
    }

    return (int64_t)f;
}

int64_t __roze_future_await(int64_t fut_handle) {
    if (!fut_handle) return 0;
    RozeFuture* f = (RozeFuture*)fut_handle;
    if (f->magic != ROZE_MAGIC_FUTURE) return fut_handle;

    // Fast path: already ready
    if (__atomic_load_n(&f->status, __ATOMIC_ACQUIRE) == ROZE_FUTURE_READY) {
        return f->result;
    }

    // Cooperative await loop
    while (__atomic_load_n(&f->status, __ATOMIC_ACQUIRE) != ROZE_FUTURE_READY) {
        pthread_mutex_lock(&f->lock);
        if (__atomic_load_n(&f->status, __ATOMIC_ACQUIRE) == ROZE_FUTURE_READY) {
            pthread_mutex_unlock(&f->lock);
            break;
        }
        struct timespec ts;
        clock_gettime(CLOCK_REALTIME, &ts);
        ts.tv_nsec += 250000; // 0.25 ms sleep per wait
        if (ts.tv_nsec >= 1000000000) {
            ts.tv_sec += 1;
            ts.tv_nsec -= 1000000000;
        }
        pthread_cond_timedwait(&f->cond, &f->lock, &ts);
        pthread_mutex_unlock(&f->lock);
        sched_yield();
    }

    return f->result;
}

int64_t __roze_future_poll(int64_t fut_handle) {
    if (!fut_handle) return 1;
    RozeFuture* f = (RozeFuture*)fut_handle;
    if (f->magic != ROZE_MAGIC_FUTURE) return 1;
    return __atomic_load_n(&f->status, __ATOMIC_ACQUIRE) == ROZE_FUTURE_READY ? 1 : 0;
}

int64_t __roze_future_ready(int64_t val) {
    RozeFuture* f = roze_future_alloc();
    if (!f) return 0;
    f->refcount = 1;
    f->status = ROZE_FUTURE_READY;
    f->result = val;
    return (int64_t)f;
}

typedef struct {
    RozeFuture* future;
    int64_t ms;
} RozeSleepTask;

static void* roze_sleep_worker(void* arg) {
    RozeSleepTask* task = (RozeSleepTask*)arg;
    RozeFuture* f = task->future;
    int64_t ms = task->ms;
    free(task);

    struct timespec req;
    req.tv_sec = ms / 1000;
    req.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&req, NULL);

    pthread_mutex_lock(&f->lock);
    f->result = ms;
    __atomic_store_n(&f->status, ROZE_FUTURE_READY, __ATOMIC_RELEASE);
    pthread_cond_broadcast(&f->cond);
    pthread_mutex_unlock(&f->lock);

    __roze_future_release((int64_t)f);
    return NULL;
}

int64_t __roze_async_sleep(int64_t ms) {
    RozeFuture* f = roze_future_alloc();
    if (!f) return 0;
    f->refcount = 2; // caller + worker
    f->status = ROZE_FUTURE_PENDING;

    RozeSleepTask* task = (RozeSleepTask*)malloc(sizeof(RozeSleepTask));
    task->future = f;
    task->ms = ms;

    pthread_t th;
    if (pthread_create(&th, NULL, roze_sleep_worker, task) == 0) {
        pthread_detach(th);
    } else {
        roze_sleep_worker(task);
    }

    return (int64_t)f;
}

// ============================================================================
// 5. Structured Concurrency: Nursery, Task Groups & Cancelation (M1-T07)
// ============================================================================

#define ROZE_MAGIC_CANCEL_TOKEN 0x524F5A43414E434CULL /* "ROZCANCL" */
#define ROZE_MAGIC_TASK_GROUP   0x524F5A54534B4750ULL /* "ROZTSKGP" */
#define ROZE_MAGIC_SUBTASK      0x524F5A535542544BULL /* "ROZSUBTK" */

#define ROZE_TG_POLICY_ALL                 0
#define ROZE_TG_POLICY_SHUTDOWN_ON_FAILURE 1
#define ROZE_TG_POLICY_SHUTDOWN_ON_SUCCESS 2

// ----------------------------------------------------------------------------
// CancellationToken
// ----------------------------------------------------------------------------

typedef struct {
    int64_t refcount;
    uint64_t magic;
    _Atomic int64_t cancelled;
} RozeCancellationToken;

int64_t __roze_cancel_token_new(void) {
    RozeCancellationToken* tok = (RozeCancellationToken*)malloc(sizeof(RozeCancellationToken));
    if (!tok) return 0;
    tok->refcount = 1;
    tok->magic = ROZE_MAGIC_CANCEL_TOKEN;
    __atomic_store_n(&tok->cancelled, 0, __ATOMIC_SEQ_CST);
    return (int64_t)tok;
}

void __roze_cancel_token_cancel(int64_t token_val) {
    if (!token_val) return;
    RozeCancellationToken* tok = (RozeCancellationToken*)token_val;
    if (tok->magic == ROZE_MAGIC_CANCEL_TOKEN) {
        __atomic_store_n(&tok->cancelled, 1, __ATOMIC_SEQ_CST);
    }
}

int64_t __roze_cancel_token_is_cancelled(int64_t token_val) {
    if (!token_val) return 0;
    RozeCancellationToken* tok = (RozeCancellationToken*)token_val;
    if (tok->magic == ROZE_MAGIC_CANCEL_TOKEN) {
        return __atomic_load_n(&tok->cancelled, __ATOMIC_SEQ_CST);
    }
    return 0;
}

void __roze_cancel_token_retain(int64_t token_val) {
    if (!token_val) return;
    RozeCancellationToken* tok = (RozeCancellationToken*)token_val;
    if (tok->magic == ROZE_MAGIC_CANCEL_TOKEN) {
        __atomic_fetch_add(&tok->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_cancel_token_release(int64_t token_val) {
    if (!token_val) return;
    RozeCancellationToken* tok = (RozeCancellationToken*)token_val;
    if (tok->magic == ROZE_MAGIC_CANCEL_TOKEN) {
        if (__atomic_fetch_sub(&tok->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            free(tok);
        }
    }
}

// ----------------------------------------------------------------------------
// Subtask & TaskGroup / Nursery
// ----------------------------------------------------------------------------

typedef struct RozeTaskGroup RozeTaskGroup;

typedef struct {
    int64_t refcount;
    uint64_t magic;
    RozeClosure* closure;
    int64_t result;
    _Atomic int64_t state; // 0: PENDING/RUNNING, 1: SUCCESS, 2: FAILED, 3: CANCELLED
    int64_t fiber_or_thread;
    RozeTaskGroup* parent_tg;
    pthread_mutex_t lock;
    pthread_cond_t cond;
} RozeSubtask;

struct RozeTaskGroup {
    int64_t refcount;
    uint64_t magic;
    int64_t policy;
    RozeCancellationToken* token;
    _Atomic int64_t cancelled;
    int64_t timeout_ms;
    pthread_t timeout_thread;
    _Atomic int64_t timeout_active;

    pthread_mutex_t lock;
    pthread_cond_t cond;

    int64_t task_capacity;
    int64_t task_count;
    RozeSubtask** tasks;

    _Atomic int64_t completed_count;
    _Atomic int64_t failed_count;
    _Atomic int64_t success_result;
    _Atomic int64_t has_success_result;
    int joined;
};

void __roze_subtask_retain(int64_t sub_val);
void __roze_subtask_release(int64_t sub_val);
void __roze_nursery_retain(int64_t tg_val);
void __roze_nursery_release(int64_t tg_val);

int64_t __roze_subtask_get(int64_t sub_val) {
    if (!sub_val) return 0;
    RozeSubtask* sub = (RozeSubtask*)sub_val;
    if (sub->magic != ROZE_MAGIC_SUBTASK) return 0;

    pthread_mutex_lock(&sub->lock);
    while (__atomic_load_n(&sub->state, __ATOMIC_ACQUIRE) == 0) {
        pthread_cond_wait(&sub->cond, &sub->lock);
    }
    int64_t res = sub->result;
    pthread_mutex_unlock(&sub->lock);
    return res;
}

int64_t __roze_subtask_is_done(int64_t sub_val) {
    if (!sub_val) return 0;
    RozeSubtask* sub = (RozeSubtask*)sub_val;
    if (sub->magic != ROZE_MAGIC_SUBTASK) return 0;
    return __atomic_load_n(&sub->state, __ATOMIC_ACQUIRE) != 0 ? 1 : 0;
}

int64_t __roze_subtask_is_cancelled(int64_t sub_val) {
    if (!sub_val) return 0;
    RozeSubtask* sub = (RozeSubtask*)sub_val;
    if (sub->magic != ROZE_MAGIC_SUBTASK) return 0;
    return __atomic_load_n(&sub->state, __ATOMIC_ACQUIRE) == 3 ? 1 : 0;
}

void __roze_subtask_retain(int64_t sub_val) {
    if (!sub_val) return;
    RozeSubtask* sub = (RozeSubtask*)sub_val;
    if (sub->magic == ROZE_MAGIC_SUBTASK) {
        __atomic_fetch_add(&sub->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_subtask_release(int64_t sub_val) {
    if (!sub_val) return;
    RozeSubtask* sub = (RozeSubtask*)sub_val;
    if (sub->magic == ROZE_MAGIC_SUBTASK) {
        if (__atomic_fetch_sub(&sub->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            pthread_mutex_destroy(&sub->lock);
            pthread_cond_destroy(&sub->cond);
            free(sub);
        }
    }
}

// Timeout Watcher Thread
static void* roze_timeout_watcher_thread(void* arg) {
    RozeTaskGroup* tg = (RozeTaskGroup*)arg;
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    ts.tv_sec += tg->timeout_ms / 1000;
    ts.tv_nsec += (tg->timeout_ms % 1000) * 1000000;
    if (ts.tv_nsec >= 1000000000) {
        ts.tv_sec += 1;
        ts.tv_nsec -= 1000000000;
    }

    pthread_mutex_lock(&tg->lock);
    while (__atomic_load_n(&tg->timeout_active, __ATOMIC_ACQUIRE) && !tg->joined) {
        int r = pthread_cond_timedwait(&tg->cond, &tg->lock, &ts);
        if (r == ETIMEDOUT) {
            if (__atomic_load_n(&tg->timeout_active, __ATOMIC_ACQUIRE) && !tg->joined) {
                if (tg->token) {
                    __roze_cancel_token_cancel((int64_t)tg->token);
                }
                __atomic_store_n(&tg->cancelled, 1, __ATOMIC_SEQ_CST);
                pthread_cond_broadcast(&tg->cond);
            }
            break;
        }
    }
    pthread_mutex_unlock(&tg->lock);
    __roze_nursery_release((int64_t)tg);
    return NULL;
}

int64_t __roze_nursery_create(int64_t policy, int64_t timeout_ms) {
    RozeTaskGroup* tg = (RozeTaskGroup*)malloc(sizeof(RozeTaskGroup));
    if (!tg) return 0;
    tg->refcount = 1;
    tg->magic = ROZE_MAGIC_TASK_GROUP;
    tg->policy = policy;
    tg->token = (RozeCancellationToken*)__roze_cancel_token_new();
    __atomic_store_n(&tg->cancelled, 0, __ATOMIC_SEQ_CST);
    tg->timeout_ms = timeout_ms;
    tg->timeout_active = 0;
    pthread_mutex_init(&tg->lock, NULL);
    pthread_cond_init(&tg->cond, NULL);

    tg->task_capacity = 16;
    tg->task_count = 0;
    tg->tasks = (RozeSubtask**)malloc(sizeof(RozeSubtask*) * tg->task_capacity);

    __atomic_store_n(&tg->completed_count, 0, __ATOMIC_SEQ_CST);
    __atomic_store_n(&tg->failed_count, 0, __ATOMIC_SEQ_CST);
    __atomic_store_n(&tg->success_result, 0, __ATOMIC_SEQ_CST);
    __atomic_store_n(&tg->has_success_result, 0, __ATOMIC_SEQ_CST);
    tg->joined = 0;

    if (timeout_ms > 0) {
        __atomic_store_n(&tg->timeout_active, 1, __ATOMIC_SEQ_CST);
        __roze_nursery_retain((int64_t)tg);
        pthread_create(&tg->timeout_thread, NULL, roze_timeout_watcher_thread, tg);
    }
    return (int64_t)tg;
}

int64_t __roze_nursery_new(void) {
    return __roze_nursery_create(ROZE_TG_POLICY_ALL, 0);
}

int64_t __roze_task_group_new(void) {
    return __roze_nursery_create(ROZE_TG_POLICY_ALL, 0);
}

int64_t __roze_nursery_on_failure(void) {
    return __roze_nursery_create(ROZE_TG_POLICY_SHUTDOWN_ON_FAILURE, 0);
}

int64_t __roze_task_group_on_failure(void) {
    return __roze_nursery_create(ROZE_TG_POLICY_SHUTDOWN_ON_FAILURE, 0);
}

int64_t __roze_nursery_on_success(void) {
    return __roze_nursery_create(ROZE_TG_POLICY_SHUTDOWN_ON_SUCCESS, 0);
}

int64_t __roze_task_group_on_success(void) {
    return __roze_nursery_create(ROZE_TG_POLICY_SHUTDOWN_ON_SUCCESS, 0);
}

int64_t __roze_nursery_with_timeout(int64_t timeout_ms) {
    return __roze_nursery_create(ROZE_TG_POLICY_ALL, timeout_ms);
}

int64_t __roze_task_group_with_timeout(int64_t timeout_ms) {
    return __roze_nursery_create(ROZE_TG_POLICY_ALL, timeout_ms);
}

// Subtask worker runner
static void* roze_subtask_runner(void* arg) {
    RozeSubtask* sub = (RozeSubtask*)arg;
    RozeTaskGroup* tg = sub->parent_tg;

    if (__roze_cancel_token_is_cancelled((int64_t)tg->token)) {
        pthread_mutex_lock(&sub->lock);
        __atomic_store_n(&sub->state, 3, __ATOMIC_RELEASE); // CANCELLED
        pthread_cond_broadcast(&sub->cond);
        pthread_mutex_unlock(&sub->lock);

        __atomic_fetch_add(&tg->failed_count, 1, __ATOMIC_SEQ_CST);
        __atomic_fetch_add(&tg->completed_count, 1, __ATOMIC_SEQ_CST);

        pthread_mutex_lock(&tg->lock);
        pthread_cond_broadcast(&tg->cond);
        pthread_mutex_unlock(&tg->lock);

        __roze_subtask_release((int64_t)sub);
        __roze_nursery_release((int64_t)tg);
        return NULL;
    }

    int64_t res = 0;
    if (sub->closure && sub->closure->fn_ptr) {
        res = (int64_t)sub->closure->fn_ptr(sub->closure);
    }

    pthread_mutex_lock(&sub->lock);
    if (__roze_cancel_token_is_cancelled((int64_t)tg->token)) {
        __atomic_store_n(&sub->state, 3, __ATOMIC_RELEASE); // CANCELLED
        __atomic_fetch_add(&tg->failed_count, 1, __ATOMIC_SEQ_CST);
    } else {
        sub->result = res;
        __atomic_store_n(&sub->state, 1, __ATOMIC_RELEASE); // SUCCESS

        if (tg->policy == ROZE_TG_POLICY_SHUTDOWN_ON_SUCCESS) {
            int64_t expected = 0;
            if (__atomic_compare_exchange_n(&tg->has_success_result, &expected, 1, 0, __ATOMIC_SEQ_CST, __ATOMIC_SEQ_CST)) {
                __atomic_store_n(&tg->success_result, res, __ATOMIC_SEQ_CST);
                __roze_cancel_token_cancel((int64_t)tg->token);
            }
        }
    }
    pthread_cond_broadcast(&sub->cond);
    pthread_mutex_unlock(&sub->lock);

    __atomic_fetch_add(&tg->completed_count, 1, __ATOMIC_SEQ_CST);

    pthread_mutex_lock(&tg->lock);
    pthread_cond_broadcast(&tg->cond);
    pthread_mutex_unlock(&tg->lock);

    __roze_subtask_release((int64_t)sub);
    __roze_nursery_release((int64_t)tg);
    return NULL;
}

int64_t __roze_nursery_fork(int64_t tg_val, void* closure_ptr) {
    if (!tg_val || !closure_ptr) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic != ROZE_MAGIC_TASK_GROUP) return 0;

    RozeSubtask* sub = (RozeSubtask*)malloc(sizeof(RozeSubtask));
    if (!sub) return 0;
    sub->refcount = 2; // 1 for caller, 1 for runner
    sub->magic = ROZE_MAGIC_SUBTASK;
    sub->closure = (RozeClosure*)closure_ptr;
    sub->result = 0;
    sub->state = 0;
    sub->parent_tg = tg;
    pthread_mutex_init(&sub->lock, NULL);
    pthread_cond_init(&sub->cond, NULL);

    pthread_mutex_lock(&tg->lock);
    if (tg->task_count == tg->task_capacity) {
        int64_t new_cap = tg->task_capacity * 2;
        tg->tasks = (RozeSubtask**)realloc(tg->tasks, sizeof(RozeSubtask*) * new_cap);
        tg->task_capacity = new_cap;
    }
    tg->tasks[tg->task_count++] = sub;
    __roze_subtask_retain((int64_t)sub);
    pthread_mutex_unlock(&tg->lock);

    __roze_nursery_retain((int64_t)tg);

    pthread_t th;
    pthread_create(&th, NULL, roze_subtask_runner, sub);
    sub->fiber_or_thread = (int64_t)th;
    pthread_detach(th);

    return (int64_t)sub;
}

int64_t __roze_task_group_fork(int64_t tg_val, void* closure_ptr) {
    return __roze_nursery_fork(tg_val, closure_ptr);
}

int64_t __roze_nursery_join(int64_t tg_val) {
    if (!tg_val) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic != ROZE_MAGIC_TASK_GROUP) return 0;

    pthread_mutex_lock(&tg->lock);
    while (1) {
        if (tg->policy == ROZE_TG_POLICY_SHUTDOWN_ON_SUCCESS && __atomic_load_n(&tg->has_success_result, __ATOMIC_ACQUIRE)) {
            break;
        }
        if (tg->policy == ROZE_TG_POLICY_SHUTDOWN_ON_FAILURE && __atomic_load_n(&tg->failed_count, __ATOMIC_ACQUIRE) > 0) {
            break;
        }
        if (__atomic_load_n(&tg->completed_count, __ATOMIC_ACQUIRE) >= tg->task_count) {
            break;
        }
        pthread_cond_wait(&tg->cond, &tg->lock);
    }
    tg->joined = 1;
    __atomic_store_n(&tg->timeout_active, 0, __ATOMIC_RELEASE);
    pthread_cond_broadcast(&tg->cond);
    pthread_mutex_unlock(&tg->lock);

    if (__atomic_load_n(&tg->has_success_result, __ATOMIC_ACQUIRE)) {
        return __atomic_load_n(&tg->success_result, __ATOMIC_ACQUIRE);
    }
    return 0;
}

int64_t __roze_task_group_join(int64_t tg_val) {
    return __roze_nursery_join(tg_val);
}

void __roze_nursery_cancel(int64_t tg_val) {
    if (!tg_val) return;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        if (tg->token) {
            __roze_cancel_token_cancel((int64_t)tg->token);
        }
        __atomic_store_n(&tg->cancelled, 1, __ATOMIC_SEQ_CST);
        pthread_mutex_lock(&tg->lock);
        pthread_cond_broadcast(&tg->cond);
        pthread_mutex_unlock(&tg->lock);
    }
}

void __roze_task_group_cancel(int64_t tg_val) {
    __roze_nursery_cancel(tg_val);
}

int64_t __roze_nursery_is_cancelled(int64_t tg_val) {
    if (!tg_val) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        return __roze_cancel_token_is_cancelled((int64_t)tg->token);
    }
    return 0;
}

int64_t __roze_task_group_is_cancelled(int64_t tg_val) {
    return __roze_nursery_is_cancelled(tg_val);
}

int64_t __roze_nursery_result(int64_t tg_val) {
    if (!tg_val) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        return __atomic_load_n(&tg->success_result, __ATOMIC_ACQUIRE);
    }
    return 0;
}

int64_t __roze_task_group_result(int64_t tg_val) {
    return __roze_nursery_result(tg_val);
}

int64_t __roze_nursery_token(int64_t tg_val) {
    if (!tg_val) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        return (int64_t)tg->token;
    }
    return 0;
}

int64_t __roze_task_group_token(int64_t tg_val) {
    return __roze_nursery_token(tg_val);
}

int64_t __roze_nursery_task_count(int64_t tg_val) {
    if (!tg_val) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        return tg->task_count;
    }
    return 0;
}

int64_t __roze_task_group_task_count(int64_t tg_val) {
    return __roze_nursery_task_count(tg_val);
}

int64_t __roze_nursery_completed_count(int64_t tg_val) {
    if (!tg_val) return 0;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        return __atomic_load_n(&tg->completed_count, __ATOMIC_ACQUIRE);
    }
    return 0;
}

int64_t __roze_task_group_completed_count(int64_t tg_val) {
    return __roze_nursery_completed_count(tg_val);
}

void __roze_nursery_retain(int64_t tg_val) {
    if (!tg_val) return;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        __atomic_fetch_add(&tg->refcount, 1, __ATOMIC_SEQ_CST);
    }
}

void __roze_task_group_retain(int64_t tg_val) {
    __roze_nursery_retain(tg_val);
}

void __roze_nursery_release(int64_t tg_val) {
    if (!tg_val) return;
    RozeTaskGroup* tg = (RozeTaskGroup*)tg_val;
    if (tg->magic == ROZE_MAGIC_TASK_GROUP) {
        if (__atomic_fetch_sub(&tg->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
            for (int64_t i = 0; i < tg->task_count; i++) {
                __roze_subtask_release((int64_t)tg->tasks[i]);
            }
            free(tg->tasks);
            if (tg->token) {
                __roze_cancel_token_release((int64_t)tg->token);
            }
            pthread_mutex_destroy(&tg->lock);
            pthread_cond_destroy(&tg->cond);
            free(tg);
        }
    }
}

void __roze_task_group_release(int64_t tg_val) {
    __roze_nursery_release(tg_val);
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

// =============================================================
// Fixed-Width Numeric Suite (M2-T01) Runtime
// =============================================================

typedef struct {
    int64_t refcount;
    __int128 val;
} RozeI128;

typedef struct {
    int64_t refcount;
    unsigned __int128 val;
} RozeU128;

int64_t __roze_i128_from_str(const char* s) {
    RozeI128* obj = (RozeI128*)malloc(sizeof(RozeI128));
    if (!obj) return 0;
    obj->refcount = 1;
    __int128 val = 0;
    int neg = 0;
    if (s) {
        if (*s == '-') { neg = 1; s++; }
        while (*s >= '0' && *s <= '9') {
            val = val * 10 + (*s - '0');
            s++;
        }
        if (neg) val = -val;
    }
    obj->val = val;
    return (int64_t)obj;
}

int64_t __roze_u128_from_str(const char* s) {
    RozeU128* obj = (RozeU128*)malloc(sizeof(RozeU128));
    if (!obj) return 0;
    obj->refcount = 1;
    unsigned __int128 val = 0;
    if (s) {
        while (*s >= '0' && *s <= '9') {
            val = val * 10 + (*s - '0');
            s++;
        }
    }
    obj->val = val;
    return (int64_t)obj;
}

int64_t __roze_i128_from_i64(int64_t v) {
    RozeI128* obj = (RozeI128*)malloc(sizeof(RozeI128));
    if (!obj) return 0;
    obj->refcount = 1;
    obj->val = (__int128)v;
    return (int64_t)obj;
}

int64_t __roze_u128_from_u64(uint64_t v) {
    RozeU128* obj = (RozeU128*)malloc(sizeof(RozeU128));
    if (!obj) return 0;
    obj->refcount = 1;
    obj->val = (unsigned __int128)v;
    return (int64_t)obj;
}

int64_t __roze_i128_to_i64(int64_t h) {
    if (!h) return 0;
    return (int64_t)((RozeI128*)h)->val;
}

int64_t __roze_u128_to_u64(int64_t h) {
    if (!h) return 0;
    return (int64_t)((RozeU128*)h)->val;
}

void __roze_i128_retain(int64_t h) {
    if (h) {
        RozeI128* obj = (RozeI128*)h;
        obj->refcount++;
    }
}

void __roze_i128_release(int64_t h) {
    if (h) {
        RozeI128* obj = (RozeI128*)h;
        if (--obj->refcount <= 0) {
            free(obj);
        }
    }
}

void __roze_u128_retain(int64_t h) {
    if (h) {
        RozeU128* obj = (RozeU128*)h;
        obj->refcount++;
    }
}

void __roze_u128_release(int64_t h) {
    if (h) {
        RozeU128* obj = (RozeU128*)h;
        if (--obj->refcount <= 0) {
            free(obj);
        }
    }
}

void __roze_i128_print(int64_t h) {
    if (!h) { puts("0"); return; }
    RozeI128* obj = (RozeI128*)h;
    __int128 val = obj->val;
    if (val == 0) { puts("0"); return; }
    char buf[64];
    int idx = 63;
    buf[idx--] = '\0';
    int neg = 0;
    unsigned __int128 uval;
    if (val < 0) {
        neg = 1;
        uval = (unsigned __int128)(-(val + 1)) + 1;
    } else {
        uval = (unsigned __int128)val;
    }
    while (uval > 0) {
        buf[idx--] = (char)('0' + (uval % 10));
        uval /= 10;
    }
    if (neg) buf[idx--] = '-';
    puts(&buf[idx + 1]);
}

void __roze_u128_print(int64_t h) {
    if (!h) { puts("0"); return; }
    RozeU128* obj = (RozeU128*)h;
    unsigned __int128 uval = obj->val;
    if (uval == 0) { puts("0"); return; }
    char buf[64];
    int idx = 63;
    buf[idx--] = '\0';
    while (uval > 0) {
        buf[idx--] = (char)('0' + (uval % 10));
        uval /= 10;
    }
    puts(&buf[idx + 1]);
}

int64_t __roze_i128_min(void) {
    RozeI128* obj = (RozeI128*)malloc(sizeof(RozeI128));
    obj->refcount = 1;
    obj->val = ((__int128)1) << 127;
    return (int64_t)obj;
}

int64_t __roze_i128_max(void) {
    RozeI128* obj = (RozeI128*)malloc(sizeof(RozeI128));
    obj->refcount = 1;
    obj->val = ~(((__int128)1) << 127);
    return (int64_t)obj;
}

int64_t __roze_u128_min(void) {
    RozeU128* obj = (RozeU128*)malloc(sizeof(RozeU128));
    obj->refcount = 1;
    obj->val = 0;
    return (int64_t)obj;
}

int64_t __roze_u128_max(void) {
    RozeU128* obj = (RozeU128*)malloc(sizeof(RozeU128));
    obj->refcount = 1;
    obj->val = ~((unsigned __int128)0);
    return (int64_t)obj;
}

int64_t __roze_i128_wrapping_add(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    res->val = (__int128)((unsigned __int128)va + (unsigned __int128)vb);
    return (int64_t)res;
}

int64_t __roze_i128_wrapping_sub(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    res->val = (__int128)((unsigned __int128)va - (unsigned __int128)vb);
    return (int64_t)res;
}

int64_t __roze_i128_wrapping_mul(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    res->val = (__int128)((unsigned __int128)va * (unsigned __int128)vb);
    return (int64_t)res;
}

int64_t __roze_i128_saturating_add(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    __int128 min_val = ((__int128)1) << 127;
    __int128 max_val = ~(((__int128)1) << 127);
    if (vb > 0 && va > max_val - vb) res->val = max_val;
    else if (vb < 0 && va < min_val - vb) res->val = min_val;
    else res->val = va + vb;
    return (int64_t)res;
}

int64_t __roze_i128_saturating_sub(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    __int128 min_val = ((__int128)1) << 127;
    __int128 max_val = ~(((__int128)1) << 127);
    if (vb < 0 && va > max_val + vb) res->val = max_val;
    else if (vb > 0 && va < min_val + vb) res->val = min_val;
    else res->val = va - vb;
    return (int64_t)res;
}

int64_t __roze_i128_saturating_mul(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    __int128 min_val = ((__int128)1) << 127;
    __int128 max_val = ~(((__int128)1) << 127);
    if (va == 0 || vb == 0) {
        res->val = 0;
    } else if (va > 0) {
        if (vb > 0) {
            res->val = (va > max_val / vb) ? max_val : va * vb;
        } else {
            res->val = (vb < min_val / va) ? min_val : va * vb;
        }
    } else {
        if (vb > 0) {
            res->val = (va < min_val / vb) ? min_val : va * vb;
        } else {
            res->val = (va < max_val / vb) ? max_val : va * vb;
        }
    }
    return (int64_t)res;
}

int64_t __roze_u128_wrapping_add(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    res->val = va + vb;
    return (int64_t)res;
}

int64_t __roze_u128_wrapping_sub(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    res->val = va - vb;
    return (int64_t)res;
}

int64_t __roze_u128_wrapping_mul(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    res->val = va * vb;
    return (int64_t)res;
}

int64_t __roze_u128_saturating_add(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    unsigned __int128 sum = va + vb;
    res->val = (sum < va) ? ~((unsigned __int128)0) : sum;
    return (int64_t)res;
}

int64_t __roze_u128_saturating_sub(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    res->val = (va < vb) ? 0 : va - vb;
    return (int64_t)res;
}

int64_t __roze_u128_saturating_mul(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    if (va == 0 || vb == 0) {
        res->val = 0;
    } else {
        unsigned __int128 max_val = ~((unsigned __int128)0);
        res->val = (va > max_val / vb) ? max_val : va * vb;
    }
    return (int64_t)res;
}

int64_t __roze_i128_sdiv(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    res->val = vb != 0 ? (va / vb) : 0;
    return (int64_t)res;
}

int64_t __roze_i128_srem(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    RozeI128* res = (RozeI128*)malloc(sizeof(RozeI128));
    res->refcount = 1;
    res->val = vb != 0 ? (va % vb) : 0;
    return (int64_t)res;
}

int64_t __roze_i128_cmp(int64_t a, int64_t b) {
    __int128 va = a ? ((RozeI128*)a)->val : 0;
    __int128 vb = b ? ((RozeI128*)b)->val : 0;
    if (va < vb) return -1;
    if (va > vb) return 1;
    return 0;
}

int64_t __roze_u128_udiv(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    res->val = vb != 0 ? (va / vb) : 0;
    return (int64_t)res;
}

int64_t __roze_u128_urem(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    RozeU128* res = (RozeU128*)malloc(sizeof(RozeU128));
    res->refcount = 1;
    res->val = vb != 0 ? (va % vb) : 0;
    return (int64_t)res;
}

int64_t __roze_u128_cmp(int64_t a, int64_t b) {
    unsigned __int128 va = a ? ((RozeU128*)a)->val : 0;
    unsigned __int128 vb = b ? ((RozeU128*)b)->val : 0;
    if (va < vb) return -1;
    if (va > vb) return 1;
    return 0;
}

// Fixed-width integer wrapping & saturating arithmetic
int64_t __roze_wrapping_add_i8(int64_t a, int64_t b) { return (int8_t)((uint8_t)a + (uint8_t)b); }
int64_t __roze_wrapping_sub_i8(int64_t a, int64_t b) { return (int8_t)((uint8_t)a - (uint8_t)b); }
int64_t __roze_wrapping_mul_i8(int64_t a, int64_t b) { return (int8_t)((uint8_t)a * (uint8_t)b); }
int64_t __roze_saturating_add_i8(int64_t a, int64_t b) {
    int64_t res = (int8_t)a + (int8_t)b;
    if (res > 127) return 127;
    if (res < -128) return -128;
    return res;
}
int64_t __roze_saturating_sub_i8(int64_t a, int64_t b) {
    int64_t res = (int8_t)a - (int8_t)b;
    if (res > 127) return 127;
    if (res < -128) return -128;
    return res;
}
int64_t __roze_saturating_mul_i8(int64_t a, int64_t b) {
    int64_t res = (int8_t)a * (int8_t)b;
    if (res > 127) return 127;
    if (res < -128) return -128;
    return res;
}

int64_t __roze_wrapping_add_u8(int64_t a, int64_t b) { return (uint8_t)((uint8_t)a + (uint8_t)b); }
int64_t __roze_wrapping_sub_u8(int64_t a, int64_t b) { return (uint8_t)((uint8_t)a - (uint8_t)b); }
int64_t __roze_wrapping_mul_u8(int64_t a, int64_t b) { return (uint8_t)((uint8_t)a * (uint8_t)b); }
int64_t __roze_saturating_add_u8(int64_t a, int64_t b) {
    uint64_t res = (uint8_t)a + (uint8_t)b;
    return res > 255 ? 255 : res;
}
int64_t __roze_saturating_sub_u8(int64_t a, int64_t b) {
    return ((uint8_t)a < (uint8_t)b) ? 0 : (uint8_t)a - (uint8_t)b;
}
int64_t __roze_saturating_mul_u8(int64_t a, int64_t b) {
    uint64_t res = (uint64_t)(uint8_t)a * (uint64_t)(uint8_t)b;
    return res > 255 ? 255 : res;
}

int64_t __roze_wrapping_add_i16(int64_t a, int64_t b) { return (int16_t)((uint16_t)a + (uint16_t)b); }
int64_t __roze_wrapping_sub_i16(int64_t a, int64_t b) { return (int16_t)((uint16_t)a - (uint16_t)b); }
int64_t __roze_wrapping_mul_i16(int64_t a, int64_t b) { return (int16_t)((uint16_t)a * (uint16_t)b); }
int64_t __roze_saturating_add_i16(int64_t a, int64_t b) {
    int64_t res = (int16_t)a + (int16_t)b;
    if (res > 32767) return 32767;
    if (res < -32768) return -32768;
    return res;
}
int64_t __roze_saturating_sub_i16(int64_t a, int64_t b) {
    int64_t res = (int16_t)a - (int16_t)b;
    if (res > 32767) return 32767;
    if (res < -32768) return -32768;
    return res;
}
int64_t __roze_saturating_mul_i16(int64_t a, int64_t b) {
    int64_t res = (int16_t)a * (int16_t)b;
    if (res > 32767) return 32767;
    if (res < -32768) return -32768;
    return res;
}

int64_t __roze_wrapping_add_u16(int64_t a, int64_t b) { return (uint16_t)((uint16_t)a + (uint16_t)b); }
int64_t __roze_wrapping_sub_u16(int64_t a, int64_t b) { return (uint16_t)((uint16_t)a - (uint16_t)b); }
int64_t __roze_wrapping_mul_u16(int64_t a, int64_t b) { return (uint16_t)((uint16_t)a * (uint16_t)b); }
int64_t __roze_saturating_add_u16(int64_t a, int64_t b) {
    uint64_t res = (uint16_t)a + (uint16_t)b;
    return res > 65535 ? 65535 : res;
}
int64_t __roze_saturating_sub_u16(int64_t a, int64_t b) {
    return ((uint16_t)a < (uint16_t)b) ? 0 : (uint16_t)a - (uint16_t)b;
}
int64_t __roze_saturating_mul_u16(int64_t a, int64_t b) {
    uint64_t res = (uint64_t)(uint16_t)a * (uint64_t)(uint16_t)b;
    return res > 65535 ? 65535 : res;
}

int64_t __roze_wrapping_add_i32(int64_t a, int64_t b) { return (int32_t)((uint32_t)a + (uint32_t)b); }
int64_t __roze_wrapping_sub_i32(int64_t a, int64_t b) { return (int32_t)((uint32_t)a - (uint32_t)b); }
int64_t __roze_wrapping_mul_i32(int64_t a, int64_t b) { return (int32_t)((uint32_t)a * (uint32_t)b); }
int64_t __roze_saturating_add_i32(int64_t a, int64_t b) {
    int64_t res = (int32_t)a + (int32_t)b;
    if (res > 2147483647LL) return 2147483647LL;
    if (res < -2147483648LL) return -2147483648LL;
    return res;
}
int64_t __roze_saturating_sub_i32(int64_t a, int64_t b) {
    int64_t res = (int32_t)a - (int32_t)b;
    if (res > 2147483647LL) return 2147483647LL;
    if (res < -2147483648LL) return -2147483648LL;
    return res;
}
int64_t __roze_saturating_mul_i32(int64_t a, int64_t b) {
    int64_t res = (int64_t)(int32_t)a * (int64_t)(int32_t)b;
    if (res > 2147483647LL) return 2147483647LL;
    if (res < -2147483648LL) return -2147483648LL;
    return res;
}

int64_t __roze_wrapping_add_u32(int64_t a, int64_t b) { return (uint32_t)((uint32_t)a + (uint32_t)b); }
int64_t __roze_wrapping_sub_u32(int64_t a, int64_t b) { return (uint32_t)((uint32_t)a - (uint32_t)b); }
int64_t __roze_wrapping_mul_u32(int64_t a, int64_t b) { return (uint32_t)((uint32_t)a * (uint32_t)b); }
int64_t __roze_saturating_add_u32(int64_t a, int64_t b) {
    uint64_t res = (uint64_t)(uint32_t)a + (uint64_t)(uint32_t)b;
    return res > 4294967295ULL ? 4294967295ULL : res;
}
int64_t __roze_saturating_sub_u32(int64_t a, int64_t b) {
    return ((uint32_t)a < (uint32_t)b) ? 0 : (uint32_t)a - (uint32_t)b;
}
int64_t __roze_saturating_mul_u32(int64_t a, int64_t b) {
    uint64_t res = (uint64_t)(uint32_t)a * (uint64_t)(uint32_t)b;
    return res > 4294967295ULL ? 4294967295ULL : res;
}

int64_t __roze_wrapping_add_i64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
int64_t __roze_wrapping_sub_i64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
int64_t __roze_wrapping_mul_i64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }
int64_t __roze_saturating_add_i64(int64_t a, int64_t b) {
    __int128 res = (__int128)a + (__int128)b;
    if (res > 9223372036854775807LL) return 9223372036854775807LL;
    if (res < (-9223372036854775807LL - 1)) return (-9223372036854775807LL - 1);
    return (int64_t)res;
}
int64_t __roze_saturating_sub_i64(int64_t a, int64_t b) {
    __int128 res = (__int128)a - (__int128)b;
    if (res > 9223372036854775807LL) return 9223372036854775807LL;
    if (res < (-9223372036854775807LL - 1)) return (-9223372036854775807LL - 1);
    return (int64_t)res;
}
int64_t __roze_saturating_mul_i64(int64_t a, int64_t b) {
    __int128 res = (__int128)a * (__int128)b;
    if (res > 9223372036854775807LL) return 9223372036854775807LL;
    if (res < (-9223372036854775807LL - 1)) return (-9223372036854775807LL - 1);
    return (int64_t)res;
}

int64_t __roze_wrapping_add_u64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a + (uint64_t)b); }
int64_t __roze_wrapping_sub_u64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a - (uint64_t)b); }
int64_t __roze_wrapping_mul_u64(int64_t a, int64_t b) { return (int64_t)((uint64_t)a * (uint64_t)b); }
int64_t __roze_saturating_add_u64(int64_t a, int64_t b) {
    uint64_t ua = (uint64_t)a, ub = (uint64_t)b;
    uint64_t res = ua + ub;
    return (res < ua) ? ~0ULL : res;
}
int64_t __roze_saturating_sub_u64(int64_t a, int64_t b) {
    uint64_t ua = (uint64_t)a, ub = (uint64_t)b;
    return (ua < ub) ? 0ULL : ua - ub;
}
int64_t __roze_saturating_mul_u64(int64_t a, int64_t b) {
    if (a == 0 || b == 0) return 0;
    uint64_t ua = (uint64_t)a, ub = (uint64_t)b;
    if (ua > ~0ULL / ub) return ~0ULL;
    return (int64_t)(ua * ub);
}

// ============================================================================
// Hardware SIMD Vector Types & Intrinsics Engine (M2-T02)
// Vector operations leveraging GCC/Clang vector extensions with automatic
// AVX-512, AVX2, SSE and ARM NEON code generation.
// ============================================================================

typedef float __attribute__((vector_size(16))) v4sf;
typedef float __attribute__((vector_size(32))) v8sf;
typedef int32_t __attribute__((vector_size(16))) v4si;
typedef uint8_t __attribute__((vector_size(16))) v16u8;

typedef struct RozeVec4f {
    int64_t refcount;
    v4sf data;
} RozeVec4f;

typedef struct RozeVec8f {
    int64_t refcount;
    v8sf data;
} RozeVec8f;

typedef struct RozeVec4i {
    int64_t refcount;
    v4si data;
} RozeVec4i;

typedef struct RozeVec16u8 {
    int64_t refcount;
    v16u8 data;
} RozeVec16u8;

static void __roze_print_single_float(float x) {
    if (x == (long)x) {
        printf("%ld.0", (long)x);
    } else {
        printf("%g", x);
    }
}

// ---- RozeVec4f ----
RozeVec4f* __roze_vec4f_new(float x, float y, float z, float w) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (v4sf){x, y, z, w};
    return v;
}

RozeVec4f* __roze_vec4f_splat(float val) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (v4sf){val, val, val, val};
    return v;
}

RozeVec4f* __roze_vec4f_zero(void) {
    return __roze_vec4f_splat(0.0f);
}

void __roze_vec4f_retain(RozeVec4f* v) {
    if (v) { v->refcount++; }
}

void __roze_vec4f_release(RozeVec4f* v) {
    if (v && --v->refcount <= 0) {
        free(v);
    }
}

RozeVec4f* __roze_vec4f_add(RozeVec4f* a, RozeVec4f* b) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = a->data + b->data;
    return v;
}

RozeVec4f* __roze_vec4f_sub(RozeVec4f* a, RozeVec4f* b) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = a->data - b->data;
    return v;
}

RozeVec4f* __roze_vec4f_mul(RozeVec4f* a, RozeVec4f* b) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = a->data * b->data;
    return v;
}

RozeVec4f* __roze_vec4f_div(RozeVec4f* a, RozeVec4f* b) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = a->data / b->data;
    return v;
}

float __roze_vec4f_dot(RozeVec4f* a, RozeVec4f* b) {
    v4sf p = a->data * b->data;
    return p[0] + p[1] + p[2] + p[3];
}

float __roze_vec4f_sum(RozeVec4f* a) {
    return a->data[0] + a->data[1] + a->data[2] + a->data[3];
}

RozeVec4f* __roze_vec4f_min(RozeVec4f* a, RozeVec4f* b) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (v4sf){
        a->data[0] < b->data[0] ? a->data[0] : b->data[0],
        a->data[1] < b->data[1] ? a->data[1] : b->data[1],
        a->data[2] < b->data[2] ? a->data[2] : b->data[2],
        a->data[3] < b->data[3] ? a->data[3] : b->data[3]
    };
    return v;
}

RozeVec4f* __roze_vec4f_max(RozeVec4f* a, RozeVec4f* b) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (v4sf){
        a->data[0] > b->data[0] ? a->data[0] : b->data[0],
        a->data[1] > b->data[1] ? a->data[1] : b->data[1],
        a->data[2] > b->data[2] ? a->data[2] : b->data[2],
        a->data[3] > b->data[3] ? a->data[3] : b->data[3]
    };
    return v;
}

RozeVec4f* __roze_vec4f_sqrt(RozeVec4f* a) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (v4sf){sqrtf(a->data[0]), sqrtf(a->data[1]), sqrtf(a->data[2]), sqrtf(a->data[3])};
    return v;
}

RozeVec4f* __roze_vec4f_abs(RozeVec4f* a) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (v4sf){fabsf(a->data[0]), fabsf(a->data[1]), fabsf(a->data[2]), fabsf(a->data[3])};
    return v;
}

RozeVec4f* __roze_vec4f_fma(RozeVec4f* a, RozeVec4f* b, RozeVec4f* c) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = (a->data * b->data) + c->data;
    return v;
}

float __roze_vec4f_get(RozeVec4f* a, int64_t idx) {
    if (idx < 0 || idx >= 4) return 0.0f;
    return a->data[idx];
}

RozeVec4f* __roze_vec4f_set(RozeVec4f* a, int64_t idx, float val) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    v->data = a->data;
    if (idx >= 0 && idx < 4) {
        v->data[idx] = val;
    }
    return v;
}

RozeVec4f* __roze_vec4f_load(float* ptr) {
    RozeVec4f* v = (RozeVec4f*)malloc(sizeof(RozeVec4f));
    v->refcount = 1;
    memcpy(&v->data, ptr, sizeof(v4sf));
    return v;
}

void __roze_vec4f_store(float* ptr, RozeVec4f* a) {
    if (ptr && a) {
        memcpy(ptr, &a->data, sizeof(v4sf));
    }
}

void __roze_vec4f_print(RozeVec4f* a) {
    if (!a) { printf("null\n"); return; }
    printf("Vec4f(");
    for (int i = 0; i < 4; i++) {
        __roze_print_single_float(a->data[i]);
        if (i < 3) printf(", ");
    }
    printf(")\n");
}

int64_t __roze_vec4f_eq(RozeVec4f* a, RozeVec4f* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    for (int i = 0; i < 4; i++) {
        if (a->data[i] != b->data[i]) return 0;
    }
    return 1;
}

// ---- RozeVec8f ----
RozeVec8f* __roze_vec8f_new(float f0, float f1, float f2, float f3, float f4, float f5, float f6, float f7) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (v8sf){f0, f1, f2, f3, f4, f5, f6, f7};
    return v;
}

RozeVec8f* __roze_vec8f_splat(float val) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (v8sf){val, val, val, val, val, val, val, val};
    return v;
}

RozeVec8f* __roze_vec8f_zero(void) {
    return __roze_vec8f_splat(0.0f);
}

void __roze_vec8f_retain(RozeVec8f* v) {
    if (v) { v->refcount++; }
}

void __roze_vec8f_release(RozeVec8f* v) {
    if (v && --v->refcount <= 0) {
        free(v);
    }
}

RozeVec8f* __roze_vec8f_add(RozeVec8f* a, RozeVec8f* b) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = a->data + b->data;
    return v;
}

RozeVec8f* __roze_vec8f_sub(RozeVec8f* a, RozeVec8f* b) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = a->data - b->data;
    return v;
}

RozeVec8f* __roze_vec8f_mul(RozeVec8f* a, RozeVec8f* b) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = a->data * b->data;
    return v;
}

RozeVec8f* __roze_vec8f_div(RozeVec8f* a, RozeVec8f* b) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = a->data / b->data;
    return v;
}

float __roze_vec8f_dot(RozeVec8f* a, RozeVec8f* b) {
    v8sf p = a->data * b->data;
    return p[0] + p[1] + p[2] + p[3] + p[4] + p[5] + p[6] + p[7];
}

float __roze_vec8f_sum(RozeVec8f* a) {
    return a->data[0] + a->data[1] + a->data[2] + a->data[3] + a->data[4] + a->data[5] + a->data[6] + a->data[7];
}

RozeVec8f* __roze_vec8f_min(RozeVec8f* a, RozeVec8f* b) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (v8sf){
        a->data[0] < b->data[0] ? a->data[0] : b->data[0],
        a->data[1] < b->data[1] ? a->data[1] : b->data[1],
        a->data[2] < b->data[2] ? a->data[2] : b->data[2],
        a->data[3] < b->data[3] ? a->data[3] : b->data[3],
        a->data[4] < b->data[4] ? a->data[4] : b->data[4],
        a->data[5] < b->data[5] ? a->data[5] : b->data[5],
        a->data[6] < b->data[6] ? a->data[6] : b->data[6],
        a->data[7] < b->data[7] ? a->data[7] : b->data[7]
    };
    return v;
}

RozeVec8f* __roze_vec8f_max(RozeVec8f* a, RozeVec8f* b) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (v8sf){
        a->data[0] > b->data[0] ? a->data[0] : b->data[0],
        a->data[1] > b->data[1] ? a->data[1] : b->data[1],
        a->data[2] > b->data[2] ? a->data[2] : b->data[2],
        a->data[3] > b->data[3] ? a->data[3] : b->data[3],
        a->data[4] > b->data[4] ? a->data[4] : b->data[4],
        a->data[5] > b->data[5] ? a->data[5] : b->data[5],
        a->data[6] > b->data[6] ? a->data[6] : b->data[6],
        a->data[7] > b->data[7] ? a->data[7] : b->data[7]
    };
    return v;
}

RozeVec8f* __roze_vec8f_sqrt(RozeVec8f* a) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (v8sf){
        sqrtf(a->data[0]), sqrtf(a->data[1]), sqrtf(a->data[2]), sqrtf(a->data[3]),
        sqrtf(a->data[4]), sqrtf(a->data[5]), sqrtf(a->data[6]), sqrtf(a->data[7])
    };
    return v;
}

RozeVec8f* __roze_vec8f_abs(RozeVec8f* a) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (v8sf){
        fabsf(a->data[0]), fabsf(a->data[1]), fabsf(a->data[2]), fabsf(a->data[3]),
        fabsf(a->data[4]), fabsf(a->data[5]), fabsf(a->data[6]), fabsf(a->data[7])
    };
    return v;
}

RozeVec8f* __roze_vec8f_fma(RozeVec8f* a, RozeVec8f* b, RozeVec8f* c) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = (a->data * b->data) + c->data;
    return v;
}

float __roze_vec8f_get(RozeVec8f* a, int64_t idx) {
    if (idx < 0 || idx >= 8) return 0.0f;
    return a->data[idx];
}

RozeVec8f* __roze_vec8f_set(RozeVec8f* a, int64_t idx, float val) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    v->data = a->data;
    if (idx >= 0 && idx < 8) {
        v->data[idx] = val;
    }
    return v;
}

RozeVec8f* __roze_vec8f_load(float* ptr) {
    RozeVec8f* v = (RozeVec8f*)malloc(sizeof(RozeVec8f));
    v->refcount = 1;
    memcpy(&v->data, ptr, sizeof(v8sf));
    return v;
}

void __roze_vec8f_store(float* ptr, RozeVec8f* a) {
    if (ptr && a) {
        memcpy(ptr, &a->data, sizeof(v8sf));
    }
}

void __roze_vec8f_print(RozeVec8f* a) {
    if (!a) { printf("null\n"); return; }
    printf("Vec8f(");
    for (int i = 0; i < 8; i++) {
        __roze_print_single_float(a->data[i]);
        if (i < 7) printf(", ");
    }
    printf(")\n");
}

int64_t __roze_vec8f_eq(RozeVec8f* a, RozeVec8f* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    for (int i = 0; i < 8; i++) {
        if (a->data[i] != b->data[i]) return 0;
    }
    return 1;
}

// ---- RozeVec4i ----
RozeVec4i* __roze_vec4i_new(int64_t a, int64_t b, int64_t c, int64_t d) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = (v4si){(int32_t)a, (int32_t)b, (int32_t)c, (int32_t)d};
    return v;
}

RozeVec4i* __roze_vec4i_splat(int64_t val) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    int32_t iv = (int32_t)val;
    v->data = (v4si){iv, iv, iv, iv};
    return v;
}

RozeVec4i* __roze_vec4i_zero(void) {
    return __roze_vec4i_splat(0);
}

void __roze_vec4i_retain(RozeVec4i* v) {
    if (v) { v->refcount++; }
}

void __roze_vec4i_release(RozeVec4i* v) {
    if (v && --v->refcount <= 0) {
        free(v);
    }
}

RozeVec4i* __roze_vec4i_add(RozeVec4i* a, RozeVec4i* b) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = a->data + b->data;
    return v;
}

RozeVec4i* __roze_vec4i_sub(RozeVec4i* a, RozeVec4i* b) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = a->data - b->data;
    return v;
}

RozeVec4i* __roze_vec4i_mul(RozeVec4i* a, RozeVec4i* b) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = a->data * b->data;
    return v;
}

RozeVec4i* __roze_vec4i_div(RozeVec4i* a, RozeVec4i* b) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = (v4si){
        b->data[0] != 0 ? a->data[0] / b->data[0] : 0,
        b->data[1] != 0 ? a->data[1] / b->data[1] : 0,
        b->data[2] != 0 ? a->data[2] / b->data[2] : 0,
        b->data[3] != 0 ? a->data[3] / b->data[3] : 0
    };
    return v;
}

int64_t __roze_vec4i_sum(RozeVec4i* a) {
    return (int64_t)a->data[0] + a->data[1] + a->data[2] + a->data[3];
}

RozeVec4i* __roze_vec4i_min(RozeVec4i* a, RozeVec4i* b) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = (v4si){
        a->data[0] < b->data[0] ? a->data[0] : b->data[0],
        a->data[1] < b->data[1] ? a->data[1] : b->data[1],
        a->data[2] < b->data[2] ? a->data[2] : b->data[2],
        a->data[3] < b->data[3] ? a->data[3] : b->data[3]
    };
    return v;
}

RozeVec4i* __roze_vec4i_max(RozeVec4i* a, RozeVec4i* b) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = (v4si){
        a->data[0] > b->data[0] ? a->data[0] : b->data[0],
        a->data[1] > b->data[1] ? a->data[1] : b->data[1],
        a->data[2] > b->data[2] ? a->data[2] : b->data[2],
        a->data[3] > b->data[3] ? a->data[3] : b->data[3]
    };
    return v;
}

RozeVec4i* __roze_vec4i_abs(RozeVec4i* a) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = (v4si){abs(a->data[0]), abs(a->data[1]), abs(a->data[2]), abs(a->data[3])};
    return v;
}

int64_t __roze_vec4i_get(RozeVec4i* a, int64_t idx) {
    if (idx < 0 || idx >= 4) return 0;
    return a->data[idx];
}

RozeVec4i* __roze_vec4i_set(RozeVec4i* a, int64_t idx, int64_t val) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    v->data = a->data;
    if (idx >= 0 && idx < 4) {
        v->data[idx] = (int32_t)val;
    }
    return v;
}

RozeVec4i* __roze_vec4i_load(int32_t* ptr) {
    RozeVec4i* v = (RozeVec4i*)malloc(sizeof(RozeVec4i));
    v->refcount = 1;
    memcpy(&v->data, ptr, sizeof(v4si));
    return v;
}

void __roze_vec4i_store(int32_t* ptr, RozeVec4i* a) {
    if (ptr && a) {
        memcpy(ptr, &a->data, sizeof(v4si));
    }
}

void __roze_vec4i_print(RozeVec4i* a) {
    if (!a) { printf("null\n"); return; }
    printf("Vec4i(%d, %d, %d, %d)\n", a->data[0], a->data[1], a->data[2], a->data[3]);
}

int64_t __roze_vec4i_eq(RozeVec4i* a, RozeVec4i* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    for (int i = 0; i < 4; i++) {
        if (a->data[i] != b->data[i]) return 0;
    }
    return 1;
}

// ---- RozeVec16u8 ----
RozeVec16u8* __roze_vec16u8_new(
    int64_t u0, int64_t u1, int64_t u2, int64_t u3,
    int64_t u4, int64_t u5, int64_t u6, int64_t u7,
    int64_t u8, int64_t u9, int64_t u10, int64_t u11,
    int64_t u12, int64_t u13, int64_t u14, int64_t u15
) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    v->data = (v16u8){
        (uint8_t)u0, (uint8_t)u1, (uint8_t)u2, (uint8_t)u3,
        (uint8_t)u4, (uint8_t)u5, (uint8_t)u6, (uint8_t)u7,
        (uint8_t)u8, (uint8_t)u9, (uint8_t)u10, (uint8_t)u11,
        (uint8_t)u12, (uint8_t)u13, (uint8_t)u14, (uint8_t)u15
    };
    return v;
}

RozeVec16u8* __roze_vec16u8_splat(int64_t val) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    uint8_t b = (uint8_t)val;
    v->data = (v16u8){
        b, b, b, b, b, b, b, b,
        b, b, b, b, b, b, b, b
    };
    return v;
}

RozeVec16u8* __roze_vec16u8_zero(void) {
    return __roze_vec16u8_splat(0);
}

void __roze_vec16u8_retain(RozeVec16u8* v) {
    if (v) { v->refcount++; }
}

void __roze_vec16u8_release(RozeVec16u8* v) {
    if (v && --v->refcount <= 0) {
        free(v);
    }
}

RozeVec16u8* __roze_vec16u8_add(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    v->data = a->data + b->data;
    return v;
}

RozeVec16u8* __roze_vec16u8_sub(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    v->data = a->data - b->data;
    return v;
}

RozeVec16u8* __roze_vec16u8_mul(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    v->data = a->data * b->data;
    return v;
}

RozeVec16u8* __roze_vec16u8_div(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    for (int i = 0; i < 16; i++) {
        v->data[i] = b->data[i] != 0 ? (a->data[i] / b->data[i]) : 0;
    }
    return v;
}

RozeVec16u8* __roze_vec16u8_saturating_add(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    for (int i = 0; i < 16; i++) {
        uint32_t r = (uint32_t)a->data[i] + (uint32_t)b->data[i];
        v->data[i] = (uint8_t)(r > 255 ? 255 : r);
    }
    return v;
}

RozeVec16u8* __roze_vec16u8_saturating_sub(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    for (int i = 0; i < 16; i++) {
        v->data[i] = (a->data[i] < b->data[i]) ? 0 : (a->data[i] - b->data[i]);
    }
    return v;
}

int64_t __roze_vec16u8_sum(RozeVec16u8* a) {
    int64_t s = 0;
    for (int i = 0; i < 16; i++) {
        s += a->data[i];
    }
    return s;
}

RozeVec16u8* __roze_vec16u8_min(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    for (int i = 0; i < 16; i++) {
        v->data[i] = a->data[i] < b->data[i] ? a->data[i] : b->data[i];
    }
    return v;
}

RozeVec16u8* __roze_vec16u8_max(RozeVec16u8* a, RozeVec16u8* b) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    for (int i = 0; i < 16; i++) {
        v->data[i] = a->data[i] > b->data[i] ? a->data[i] : b->data[i];
    }
    return v;
}

int64_t __roze_vec16u8_get(RozeVec16u8* a, int64_t idx) {
    if (idx < 0 || idx >= 16) return 0;
    return a->data[idx];
}

RozeVec16u8* __roze_vec16u8_set(RozeVec16u8* a, int64_t idx, int64_t val) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    v->data = a->data;
    if (idx >= 0 && idx < 16) {
        v->data[idx] = (uint8_t)val;
    }
    return v;
}

RozeVec16u8* __roze_vec16u8_load(uint8_t* ptr) {
    RozeVec16u8* v = (RozeVec16u8*)malloc(sizeof(RozeVec16u8));
    v->refcount = 1;
    memcpy(&v->data, ptr, sizeof(v16u8));
    return v;
}

void __roze_vec16u8_store(uint8_t* ptr, RozeVec16u8* a) {
    if (ptr && a) {
        memcpy(ptr, &a->data, sizeof(v16u8));
    }
}

void __roze_vec16u8_print(RozeVec16u8* a) {
    if (!a) { printf("null\n"); return; }
    printf("Vec16u8(");
    for (int i = 0; i < 16; i++) {
        printf("%u", (unsigned int)a->data[i]);
        if (i < 15) printf(", ");
    }
    printf(")\n");
}

int64_t __roze_vec16u8_eq(RozeVec16u8* a, RozeVec16u8* b) {
    if (a == b) return 1;
    if (!a || !b) return 0;
    return memcmp(&a->data, &b->data, 16) == 0 ? 1 : 0;
}

// =============================================================
// Advanced High-Performance Standard Collections (M2-T03)
// =============================================================

extern int64_t __roze_list_new(void);
extern int64_t __roze_list_push(int64_t list_ptr, int64_t item);

static inline int roze_compare_keys(int64_t a, int64_t b, int is_str_key) {
    if (is_str_key) {
        if (!a && !b) return 0;
        if (!a) return -1;
        if (!b) return 1;
        return strcmp((const char*)a, (const char*)b);
    } else {
        if (a < b) return -1;
        if (a > b) return 1;
        return 0;
    }
}

static inline uint64_t roze_hash_key(int64_t key, int is_str_key) {
    if (is_str_key && key) {
        const char* s = (const char*)key;
        uint64_t h = 14695981039346656037ULL;
        while (*s) {
            h ^= (uint64_t)(unsigned char)(*s++);
            h *= 1099511628211ULL;
        }
        return h;
    } else {
        return ((uint64_t)key) * 11400714819323198485ULL;
    }
}

// -------------------------------------------------------------
// 1. VecDeque (Growable Circular Ring-Buffer Deque)
// -------------------------------------------------------------

typedef struct {
    int64_t refcount;
    int64_t* buffer;
    int64_t capacity;
    int64_t head;
    int64_t count;
} RozeVecDeque;

static void vec_deque_ensure_cap(RozeVecDeque* vd) {
    if (vd->count >= vd->capacity) {
        int64_t new_cap = vd->capacity ? vd->capacity * 2 : 8;
        int64_t* new_buf = (int64_t*)malloc(sizeof(int64_t) * new_cap);
        for (int64_t i = 0; i < vd->count; i++) {
            new_buf[i] = vd->buffer[(vd->head + i) % vd->capacity];
        }
        if (vd->buffer) free(vd->buffer);
        vd->buffer = new_buf;
        vd->head = 0;
        vd->capacity = new_cap;
    }
}

int64_t __roze_vec_deque_new(void) {
    RozeVecDeque* vd = (RozeVecDeque*)malloc(sizeof(RozeVecDeque));
    vd->refcount = 1;
    vd->capacity = 8;
    vd->buffer = (int64_t*)malloc(sizeof(int64_t) * 8);
    vd->head = 0;
    vd->count = 0;
    return (int64_t)vd;
}

int64_t __roze_vec_deque_with_capacity(int64_t cap) {
    if (cap <= 0) cap = 8;
    RozeVecDeque* vd = (RozeVecDeque*)malloc(sizeof(RozeVecDeque));
    vd->refcount = 1;
    vd->capacity = cap;
    vd->buffer = (int64_t*)malloc(sizeof(int64_t) * cap);
    vd->head = 0;
    vd->count = 0;
    return (int64_t)vd;
}

void __roze_vec_deque_retain(int64_t vd_val) {
    if (!vd_val) return;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    __atomic_fetch_add(&vd->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_vec_deque_release(int64_t vd_val) {
    if (!vd_val) return;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (__atomic_fetch_sub(&vd->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (vd->buffer) free(vd->buffer);
        free(vd);
    }
}

void __roze_vec_deque_push_back(int64_t vd_val, int64_t val) {
    if (!vd_val) return;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    vec_deque_ensure_cap(vd);
    vd->buffer[(vd->head + vd->count) % vd->capacity] = val;
    vd->count++;
}

void __roze_vec_deque_push_front(int64_t vd_val, int64_t val) {
    if (!vd_val) return;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    vec_deque_ensure_cap(vd);
    vd->head = (vd->head - 1 + vd->capacity) % vd->capacity;
    vd->buffer[vd->head] = val;
    vd->count++;
}

int64_t __roze_vec_deque_pop_front(int64_t vd_val) {
    if (!vd_val) return 0;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (vd->count == 0) return 0;
    int64_t val = vd->buffer[vd->head];
    vd->head = (vd->head + 1) % vd->capacity;
    vd->count--;
    return val;
}

int64_t __roze_vec_deque_pop_back(int64_t vd_val) {
    if (!vd_val) return 0;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (vd->count == 0) return 0;
    int64_t idx = (vd->head + vd->count - 1) % vd->capacity;
    int64_t val = vd->buffer[idx];
    vd->count--;
    return val;
}

int64_t __roze_vec_deque_front(int64_t vd_val) {
    if (!vd_val) return 0;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (vd->count == 0) return 0;
    return vd->buffer[vd->head];
}

int64_t __roze_vec_deque_back(int64_t vd_val) {
    if (!vd_val) return 0;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (vd->count == 0) return 0;
    return vd->buffer[(vd->head + vd->count - 1) % vd->capacity];
}

int64_t __roze_vec_deque_get(int64_t vd_val, int64_t idx) {
    if (!vd_val) return 0;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (idx < 0 || idx >= vd->count) return 0;
    return vd->buffer[(vd->head + idx) % vd->capacity];
}

void __roze_vec_deque_set(int64_t vd_val, int64_t idx, int64_t val) {
    if (!vd_val) return;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    if (idx < 0 || idx >= vd->count) return;
    vd->buffer[(vd->head + idx) % vd->capacity] = val;
}

int64_t __roze_vec_deque_len(int64_t vd_val) {
    if (!vd_val) return 0;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    return vd->count;
}

int64_t __roze_vec_deque_is_empty(int64_t vd_val) {
    if (!vd_val) return 1;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    return vd->count == 0;
}

void __roze_vec_deque_clear(int64_t vd_val) {
    if (!vd_val) return;
    RozeVecDeque* vd = (RozeVecDeque*)vd_val;
    vd->head = 0;
    vd->count = 0;
}

// -------------------------------------------------------------
// 2. BTreeMap (Ordered AVL Balanced Search Tree)
// -------------------------------------------------------------

typedef struct RozeBTreeNode {
    int64_t key;
    int64_t value;
    int height;
    struct RozeBTreeNode* left;
    struct RozeBTreeNode* right;
} RozeBTreeNode;

typedef struct {
    int64_t refcount;
    RozeBTreeNode* root;
    int64_t count;
    int is_str_key;
} RozeBTreeMap;

static inline int btree_height(RozeBTreeNode* n) {
    return n ? n->height : 0;
}

static inline int btree_max(int a, int b) {
    return (a > b) ? a : b;
}

static inline void btree_update_height(RozeBTreeNode* n) {
    if (n) {
        n->height = 1 + btree_max(btree_height(n->left), btree_height(n->right));
    }
}

static inline int btree_balance_factor(RozeBTreeNode* n) {
    return n ? (btree_height(n->left) - btree_height(n->right)) : 0;
}

static RozeBTreeNode* btree_rotate_right(RozeBTreeNode* y) {
    RozeBTreeNode* x = y->left;
    RozeBTreeNode* T2 = x->right;
    x->right = y;
    y->left = T2;
    btree_update_height(y);
    btree_update_height(x);
    return x;
}

static RozeBTreeNode* btree_rotate_left(RozeBTreeNode* x) {
    RozeBTreeNode* y = x->right;
    RozeBTreeNode* T2 = y->left;
    y->left = x;
    x->right = T2;
    btree_update_height(x);
    btree_update_height(y);
    return y;
}

static RozeBTreeNode* btree_rebalance(RozeBTreeNode* node) {
    if (!node) return NULL;
    btree_update_height(node);
    int bf = btree_balance_factor(node);
    if (bf > 1) {
        if (btree_balance_factor(node->left) < 0) {
            node->left = btree_rotate_left(node->left);
        }
        return btree_rotate_right(node);
    }
    if (bf < -1) {
        if (btree_balance_factor(node->right) > 0) {
            node->right = btree_rotate_right(node->right);
        }
        return btree_rotate_left(node);
    }
    return node;
}

static RozeBTreeNode* btree_insert_node(RozeBTreeNode* node, int64_t key, int64_t value, int is_str, int* inserted) {
    if (!node) {
        RozeBTreeNode* n = (RozeBTreeNode*)malloc(sizeof(RozeBTreeNode));
        n->key = key;
        n->value = value;
        n->height = 1;
        n->left = NULL;
        n->right = NULL;
        *inserted = 1;
        return n;
    }
    int cmp = roze_compare_keys(key, node->key, is_str);
    if (cmp == 0) {
        node->value = value;
        *inserted = 0;
        return node;
    }
    if (cmp < 0) {
        node->left = btree_insert_node(node->left, key, value, is_str, inserted);
    } else {
        node->right = btree_insert_node(node->right, key, value, is_str, inserted);
    }
    return btree_rebalance(node);
}

static RozeBTreeNode* btree_min_node(RozeBTreeNode* node) {
    RozeBTreeNode* curr = node;
    while (curr && curr->left) {
        curr = curr->left;
    }
    return curr;
}

static RozeBTreeNode* btree_remove_node(RozeBTreeNode* node, int64_t key, int is_str, int64_t* removed_val, int* removed) {
    if (!node) return NULL;
    int cmp = roze_compare_keys(key, node->key, is_str);
    if (cmp < 0) {
        node->left = btree_remove_node(node->left, key, is_str, removed_val, removed);
    } else if (cmp > 0) {
        node->right = btree_remove_node(node->right, key, is_str, removed_val, removed);
    } else {
        *removed = 1;
        *removed_val = node->value;
        if (!node->left || !node->right) {
            RozeBTreeNode* temp = node->left ? node->left : node->right;
            free(node);
            return temp;
        } else {
            RozeBTreeNode* succ = btree_min_node(node->right);
            node->key = succ->key;
            node->value = succ->value;
            int dummy_rem = 0;
            int64_t dummy_val = 0;
            node->right = btree_remove_node(node->right, succ->key, is_str, &dummy_val, &dummy_rem);
        }
    }
    return btree_rebalance(node);
}

static void btree_free_tree(RozeBTreeNode* node) {
    if (!node) return;
    btree_free_tree(node->left);
    btree_free_tree(node->right);
    free(node);
}

static void btree_inorder_keys(RozeBTreeNode* node, int64_t list) {
    if (!node) return;
    btree_inorder_keys(node->left, list);
    __roze_list_push(list, node->key);
    btree_inorder_keys(node->right, list);
}

static void btree_inorder_values(RozeBTreeNode* node, int64_t list) {
    if (!node) return;
    btree_inorder_values(node->left, list);
    __roze_list_push(list, node->value);
    btree_inorder_values(node->right, list);
}

int64_t __roze_btree_map_new(void) {
    RozeBTreeMap* m = (RozeBTreeMap*)malloc(sizeof(RozeBTreeMap));
    m->refcount = 1;
    m->root = NULL;
    m->count = 0;
    m->is_str_key = 0;
    return (int64_t)m;
}

int64_t __roze_btree_map_new_str(void) {
    RozeBTreeMap* m = (RozeBTreeMap*)malloc(sizeof(RozeBTreeMap));
    m->refcount = 1;
    m->root = NULL;
    m->count = 0;
    m->is_str_key = 1;
    return (int64_t)m;
}

void __roze_btree_map_retain(int64_t m_val) {
    if (!m_val) return;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    __atomic_fetch_add(&m->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_btree_map_release(int64_t m_val) {
    if (!m_val) return;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    if (__atomic_fetch_sub(&m->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        btree_free_tree(m->root);
        free(m);
    }
}

void __roze_btree_map_insert(int64_t m_val, int64_t key, int64_t value) {
    if (!m_val) return;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    int inserted = 0;
    m->root = btree_insert_node(m->root, key, value, m->is_str_key, &inserted);
    if (inserted) m->count++;
}

int64_t __roze_btree_map_get(int64_t m_val, int64_t key) {
    if (!m_val) return 0;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    RozeBTreeNode* curr = m->root;
    while (curr) {
        int cmp = roze_compare_keys(key, curr->key, m->is_str_key);
        if (cmp == 0) return curr->value;
        if (cmp < 0) curr = curr->left;
        else curr = curr->right;
    }
    return 0;
}

int64_t __roze_btree_map_contains_key(int64_t m_val, int64_t key) {
    if (!m_val) return 0;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    RozeBTreeNode* curr = m->root;
    while (curr) {
        int cmp = roze_compare_keys(key, curr->key, m->is_str_key);
        if (cmp == 0) return 1;
        if (cmp < 0) curr = curr->left;
        else curr = curr->right;
    }
    return 0;
}

int64_t __roze_btree_map_remove(int64_t m_val, int64_t key) {
    if (!m_val) return 0;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    int removed = 0;
    int64_t val = 0;
    m->root = btree_remove_node(m->root, key, m->is_str_key, &val, &removed);
    if (removed) m->count--;
    return val;
}

int64_t __roze_btree_map_len(int64_t m_val) {
    if (!m_val) return 0;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    return m->count;
}

int64_t __roze_btree_map_is_empty(int64_t m_val) {
    if (!m_val) return 1;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    return m->count == 0;
}

int64_t __roze_btree_map_first_key(int64_t m_val) {
    if (!m_val) return 0;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    RozeBTreeNode* min_n = btree_min_node(m->root);
    return min_n ? min_n->key : 0;
}

int64_t __roze_btree_map_last_key(int64_t m_val) {
    if (!m_val) return 0;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    RozeBTreeNode* curr = m->root;
    while (curr && curr->right) {
        curr = curr->right;
    }
    return curr ? curr->key : 0;
}

int64_t __roze_btree_map_keys(int64_t m_val) {
    int64_t list = __roze_list_new();
    if (m_val) {
        RozeBTreeMap* m = (RozeBTreeMap*)m_val;
        btree_inorder_keys(m->root, list);
    }
    return list;
}

int64_t __roze_btree_map_values(int64_t m_val) {
    int64_t list = __roze_list_new();
    if (m_val) {
        RozeBTreeMap* m = (RozeBTreeMap*)m_val;
        btree_inorder_values(m->root, list);
    }
    return list;
}

void __roze_btree_map_clear(int64_t m_val) {
    if (!m_val) return;
    RozeBTreeMap* m = (RozeBTreeMap*)m_val;
    btree_free_tree(m->root);
    m->root = NULL;
    m->count = 0;
}

// -------------------------------------------------------------
// 3. BTreeSet (Ordered Unique Set)
// -------------------------------------------------------------

typedef struct {
    int64_t refcount;
    RozeBTreeMap* map;
} RozeBTreeSet;

int64_t __roze_btree_set_new(void) {
    RozeBTreeSet* s = (RozeBTreeSet*)malloc(sizeof(RozeBTreeSet));
    s->refcount = 1;
    s->map = (RozeBTreeMap*)__roze_btree_map_new();
    return (int64_t)s;
}

int64_t __roze_btree_set_new_str(void) {
    RozeBTreeSet* s = (RozeBTreeSet*)malloc(sizeof(RozeBTreeSet));
    s->refcount = 1;
    s->map = (RozeBTreeMap*)__roze_btree_map_new_str();
    return (int64_t)s;
}

void __roze_btree_set_retain(int64_t s_val) {
    if (!s_val) return;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    __atomic_fetch_add(&s->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_btree_set_release(int64_t s_val) {
    if (!s_val) return;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    if (__atomic_fetch_sub(&s->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        __roze_btree_map_release((int64_t)s->map);
        free(s);
    }
}

int64_t __roze_btree_set_insert(int64_t s_val, int64_t val) {
    if (!s_val) return 0;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    if (__roze_btree_map_contains_key((int64_t)s->map, val)) {
        return 0;
    }
    __roze_btree_map_insert((int64_t)s->map, val, 1);
    return 1;
}

int64_t __roze_btree_set_contains(int64_t s_val, int64_t val) {
    if (!s_val) return 0;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    return __roze_btree_map_contains_key((int64_t)s->map, val);
}

int64_t __roze_btree_set_remove(int64_t s_val, int64_t val) {
    if (!s_val) return 0;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    if (!__roze_btree_map_contains_key((int64_t)s->map, val)) {
        return 0;
    }
    __roze_btree_map_remove((int64_t)s->map, val);
    return 1;
}

int64_t __roze_btree_set_len(int64_t s_val) {
    if (!s_val) return 0;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    return __roze_btree_map_len((int64_t)s->map);
}

int64_t __roze_btree_set_is_empty(int64_t s_val) {
    if (!s_val) return 1;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    return __roze_btree_map_is_empty((int64_t)s->map);
}

int64_t __roze_btree_set_first(int64_t s_val) {
    if (!s_val) return 0;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    return __roze_btree_map_first_key((int64_t)s->map);
}

int64_t __roze_btree_set_last(int64_t s_val) {
    if (!s_val) return 0;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    return __roze_btree_map_last_key((int64_t)s->map);
}

int64_t __roze_btree_set_to_list(int64_t s_val) {
    if (!s_val) return __roze_list_new();
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    return __roze_btree_map_keys((int64_t)s->map);
}

void __roze_btree_set_clear(int64_t s_val) {
    if (!s_val) return;
    RozeBTreeSet* s = (RozeBTreeSet*)s_val;
    __roze_btree_map_clear((int64_t)s->map);
}

// -------------------------------------------------------------
// 4. BinaryHeap (Max-Heap Priority Queue)
// -------------------------------------------------------------

typedef struct {
    int64_t refcount;
    int64_t* data;
    int64_t count;
    int64_t capacity;
} RozeBinaryHeap;

int64_t __roze_binary_heap_new(void) {
    RozeBinaryHeap* h = (RozeBinaryHeap*)malloc(sizeof(RozeBinaryHeap));
    h->refcount = 1;
    h->capacity = 8;
    h->data = (int64_t*)malloc(sizeof(int64_t) * 8);
    h->count = 0;
    return (int64_t)h;
}

void __roze_binary_heap_retain(int64_t h_val) {
    if (!h_val) return;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    __atomic_fetch_add(&h->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_binary_heap_release(int64_t h_val) {
    if (!h_val) return;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    if (__atomic_fetch_sub(&h->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (h->data) free(h->data);
        free(h);
    }
}

void __roze_binary_heap_push(int64_t h_val, int64_t val) {
    if (!h_val) return;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    if (h->count >= h->capacity) {
        h->capacity = h->capacity ? h->capacity * 2 : 8;
        h->data = (int64_t*)realloc(h->data, sizeof(int64_t) * h->capacity);
    }
    int64_t i = h->count;
    h->data[i] = val;
    h->count++;
    // sift-up
    while (i > 0) {
        int64_t parent = (i - 1) / 2;
        if (h->data[i] > h->data[parent]) {
            int64_t tmp = h->data[i];
            h->data[i] = h->data[parent];
            h->data[parent] = tmp;
            i = parent;
        } else {
            break;
        }
    }
}

int64_t __roze_binary_heap_pop(int64_t h_val) {
    if (!h_val) return 0;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    if (h->count == 0) return 0;
    int64_t top = h->data[0];
    h->count--;
    h->data[0] = h->data[h->count];
    // sift-down
    int64_t i = 0;
    while (2 * i + 1 < h->count) {
        int64_t left = 2 * i + 1;
        int64_t right = 2 * i + 2;
        int64_t largest = (right < h->count && h->data[right] > h->data[left]) ? right : left;
        if (h->data[largest] > h->data[i]) {
            int64_t tmp = h->data[i];
            h->data[i] = h->data[largest];
            h->data[largest] = tmp;
            i = largest;
        } else {
            break;
        }
    }
    return top;
}

int64_t __roze_binary_heap_peek(int64_t h_val) {
    if (!h_val) return 0;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    return (h->count > 0) ? h->data[0] : 0;
}

int64_t __roze_binary_heap_len(int64_t h_val) {
    if (!h_val) return 0;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    return h->count;
}

int64_t __roze_binary_heap_is_empty(int64_t h_val) {
    if (!h_val) return 1;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    return h->count == 0;
}

void __roze_binary_heap_clear(int64_t h_val) {
    if (!h_val) return;
    RozeBinaryHeap* h = (RozeBinaryHeap*)h_val;
    h->count = 0;
}

// -------------------------------------------------------------
// 5. IndexMap (Insertion-Ordered Hash Map with Indexed Access)
// -------------------------------------------------------------

typedef struct {
    int64_t key;
    int64_t value;
} RozeIndexEntry;

typedef struct {
    int64_t refcount;
    RozeIndexEntry* entries;
    int64_t count;
    int64_t entry_capacity;
    int64_t* buckets;
    int64_t bucket_capacity;
    int is_str_key;
} RozeIndexMap;

static int64_t index_map_find(RozeIndexMap* m, int64_t key) {
    if (m->count == 0 || m->bucket_capacity == 0) return -1;
    uint64_t h = roze_hash_key(key, m->is_str_key);
    int64_t mask = m->bucket_capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (m->buckets[idx] != -1) {
        int64_t e_idx = m->buckets[idx];
        if (roze_compare_keys(m->entries[e_idx].key, key, m->is_str_key) == 0) {
            return e_idx;
        }
        idx = (idx + 1) & mask;
    }
    return -1;
}

static void index_map_rehash(RozeIndexMap* m) {
    int64_t new_cap = m->bucket_capacity * 2;
    int64_t* new_b = (int64_t*)malloc(sizeof(int64_t) * new_cap);
    memset(new_b, -1, sizeof(int64_t) * new_cap);
    int64_t mask = new_cap - 1;
    for (int64_t i = 0; i < m->count; i++) {
        uint64_t h = roze_hash_key(m->entries[i].key, m->is_str_key);
        int64_t idx = (int64_t)(h & (uint64_t)mask);
        while (new_b[idx] != -1) {
            idx = (idx + 1) & mask;
        }
        new_b[idx] = i;
    }
    free(m->buckets);
    m->buckets = new_b;
    m->bucket_capacity = new_cap;
}

int64_t __roze_index_map_new(void) {
    RozeIndexMap* m = (RozeIndexMap*)malloc(sizeof(RozeIndexMap));
    m->refcount = 1;
    m->count = 0;
    m->entry_capacity = 16;
    m->entries = (RozeIndexEntry*)malloc(sizeof(RozeIndexEntry) * 16);
    m->bucket_capacity = 32;
    m->buckets = (int64_t*)malloc(sizeof(int64_t) * 32);
    memset(m->buckets, -1, sizeof(int64_t) * 32);
    m->is_str_key = 0;
    return (int64_t)m;
}

int64_t __roze_index_map_new_str(void) {
    RozeIndexMap* m = (RozeIndexMap*)malloc(sizeof(RozeIndexMap));
    m->refcount = 1;
    m->count = 0;
    m->entry_capacity = 16;
    m->entries = (RozeIndexEntry*)malloc(sizeof(RozeIndexEntry) * 16);
    m->bucket_capacity = 32;
    m->buckets = (int64_t*)malloc(sizeof(int64_t) * 32);
    memset(m->buckets, -1, sizeof(int64_t) * 32);
    m->is_str_key = 1;
    return (int64_t)m;
}

void __roze_index_map_retain(int64_t m_val) {
    if (!m_val) return;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    __atomic_fetch_add(&m->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_index_map_release(int64_t m_val) {
    if (!m_val) return;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    if (__atomic_fetch_sub(&m->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (m->entries) free(m->entries);
        if (m->buckets) free(m->buckets);
        free(m);
    }
}

void __roze_index_map_insert(int64_t m_val, int64_t key, int64_t value) {
    if (!m_val) return;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    int64_t existing = index_map_find(m, key);
    if (existing != -1) {
        m->entries[existing].value = value;
        return;
    }
    if (m->count >= m->entry_capacity) {
        m->entry_capacity *= 2;
        m->entries = (RozeIndexEntry*)realloc(m->entries, sizeof(RozeIndexEntry) * m->entry_capacity);
    }
    if ((double)m->count >= (double)m->bucket_capacity * 0.70) {
        index_map_rehash(m);
    }
    int64_t e_idx = m->count;
    m->entries[e_idx].key = key;
    m->entries[e_idx].value = value;
    m->count++;

    uint64_t h = roze_hash_key(key, m->is_str_key);
    int64_t mask = m->bucket_capacity - 1;
    int64_t idx = (int64_t)(h & (uint64_t)mask);
    while (m->buckets[idx] != -1) {
        idx = (idx + 1) & mask;
    }
    m->buckets[idx] = e_idx;
}

int64_t __roze_index_map_get(int64_t m_val, int64_t key) {
    if (!m_val) return 0;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    int64_t idx = index_map_find(m, key);
    return (idx != -1) ? m->entries[idx].value : 0;
}

int64_t __roze_index_map_get_index_key(int64_t m_val, int64_t idx) {
    if (!m_val) return 0;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    return (idx >= 0 && idx < m->count) ? m->entries[idx].key : 0;
}

int64_t __roze_index_map_get_index_value(int64_t m_val, int64_t idx) {
    if (!m_val) return 0;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    return (idx >= 0 && idx < m->count) ? m->entries[idx].value : 0;
}

int64_t __roze_index_map_contains_key(int64_t m_val, int64_t key) {
    if (!m_val) return 0;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    return index_map_find(m, key) != -1;
}

int64_t __roze_index_map_remove(int64_t m_val, int64_t key) {
    if (!m_val) return 0;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    int64_t e_idx = index_map_find(m, key);
    if (e_idx == -1) return 0;
    int64_t old_val = m->entries[e_idx].value;
    for (int64_t i = e_idx; i < m->count - 1; i++) {
        m->entries[i] = m->entries[i + 1];
    }
    m->count--;
    // rebuild buckets to reflect shifted entry indices
    memset(m->buckets, -1, sizeof(int64_t) * m->bucket_capacity);
    int64_t mask = m->bucket_capacity - 1;
    for (int64_t i = 0; i < m->count; i++) {
        uint64_t h = roze_hash_key(m->entries[i].key, m->is_str_key);
        int64_t idx = (int64_t)(h & (uint64_t)mask);
        while (m->buckets[idx] != -1) {
            idx = (idx + 1) & mask;
        }
        m->buckets[idx] = i;
    }
    return old_val;
}

int64_t __roze_index_map_len(int64_t m_val) {
    if (!m_val) return 0;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    return m->count;
}

int64_t __roze_index_map_is_empty(int64_t m_val) {
    if (!m_val) return 1;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    return m->count == 0;
}

int64_t __roze_index_map_keys(int64_t m_val) {
    int64_t list = __roze_list_new();
    if (m_val) {
        RozeIndexMap* m = (RozeIndexMap*)m_val;
        for (int64_t i = 0; i < m->count; i++) {
            __roze_list_push(list, m->entries[i].key);
        }
    }
    return list;
}

int64_t __roze_index_map_values(int64_t m_val) {
    int64_t list = __roze_list_new();
    if (m_val) {
        RozeIndexMap* m = (RozeIndexMap*)m_val;
        for (int64_t i = 0; i < m->count; i++) {
            __roze_list_push(list, m->entries[i].value);
        }
    }
    return list;
}

void __roze_index_map_clear(int64_t m_val) {
    if (!m_val) return;
    RozeIndexMap* m = (RozeIndexMap*)m_val;
    m->count = 0;
    memset(m->buckets, -1, sizeof(int64_t) * m->bucket_capacity);
}

// -------------------------------------------------------------
// 6. RingBuffer (Fixed-Capacity Circular Buffer)
// -------------------------------------------------------------

typedef struct {
    int64_t refcount;
    int64_t* buffer;
    int64_t capacity;
    int64_t head;
    int64_t count;
} RozeRingBuffer;

int64_t __roze_ring_buffer_new(int64_t capacity) {
    if (capacity <= 0) capacity = 16;
    RozeRingBuffer* rb = (RozeRingBuffer*)malloc(sizeof(RozeRingBuffer));
    rb->refcount = 1;
    rb->capacity = capacity;
    rb->buffer = (int64_t*)malloc(sizeof(int64_t) * capacity);
    rb->head = 0;
    rb->count = 0;
    return (int64_t)rb;
}

void __roze_ring_buffer_retain(int64_t rb_val) {
    if (!rb_val) return;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    __atomic_fetch_add(&rb->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_ring_buffer_release(int64_t rb_val) {
    if (!rb_val) return;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    if (__atomic_fetch_sub(&rb->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (rb->buffer) free(rb->buffer);
        free(rb);
    }
}

int64_t __roze_ring_buffer_push(int64_t rb_val, int64_t val) {
    if (!rb_val) return 0;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    if (rb->count >= rb->capacity) return 0; // full
    rb->buffer[(rb->head + rb->count) % rb->capacity] = val;
    rb->count++;
    return 1;
}

void __roze_ring_buffer_push_overwrite(int64_t rb_val, int64_t val) {
    if (!rb_val) return;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    if (rb->count >= rb->capacity) {
        rb->buffer[rb->head] = val;
        rb->head = (rb->head + 1) % rb->capacity;
    } else {
        rb->buffer[(rb->head + rb->count) % rb->capacity] = val;
        rb->count++;
    }
}

int64_t __roze_ring_buffer_pop(int64_t rb_val) {
    if (!rb_val) return 0;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    if (rb->count == 0) return 0;
    int64_t val = rb->buffer[rb->head];
    rb->head = (rb->head + 1) % rb->capacity;
    rb->count--;
    return val;
}

int64_t __roze_ring_buffer_peek(int64_t rb_val) {
    if (!rb_val) return 0;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    return (rb->count > 0) ? rb->buffer[rb->head] : 0;
}

int64_t __roze_ring_buffer_len(int64_t rb_val) {
    if (!rb_val) return 0;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    return rb->count;
}

int64_t __roze_ring_buffer_capacity(int64_t rb_val) {
    if (!rb_val) return 0;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    return rb->capacity;
}

int64_t __roze_ring_buffer_is_empty(int64_t rb_val) {
    if (!rb_val) return 1;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    return rb->count == 0;
}

int64_t __roze_ring_buffer_is_full(int64_t rb_val) {
    if (!rb_val) return 0;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    return rb->count >= rb->capacity;
}

void __roze_ring_buffer_clear(int64_t rb_val) {
    if (!rb_val) return;
    RozeRingBuffer* rb = (RozeRingBuffer*)rb_val;
    rb->head = 0;
    rb->count = 0;
}

// -------------------------------------------------------------
// 7. BitSet (Dense 64-Bit Word Bit Vector)
// -------------------------------------------------------------

typedef struct {
    int64_t refcount;
    uint64_t* words;
    int64_t num_words;
} RozeBitSet;

static void bit_set_ensure_word(RozeBitSet* bs, int64_t word_idx) {
    if (word_idx >= bs->num_words) {
        int64_t new_words = word_idx + 4;
        uint64_t* new_arr = (uint64_t*)realloc(bs->words, sizeof(uint64_t) * new_words);
        for (int64_t i = bs->num_words; i < new_words; i++) {
            new_arr[i] = 0;
        }
        bs->words = new_arr;
        bs->num_words = new_words;
    }
}

int64_t __roze_bit_set_new(void) {
    RozeBitSet* bs = (RozeBitSet*)malloc(sizeof(RozeBitSet));
    bs->refcount = 1;
    bs->num_words = 1;
    bs->words = (uint64_t*)calloc(1, sizeof(uint64_t));
    return (int64_t)bs;
}

int64_t __roze_bit_set_with_capacity(int64_t num_bits) {
    int64_t words = (num_bits > 0) ? (num_bits + 63) / 64 : 1;
    if (words <= 0) words = 1;
    RozeBitSet* bs = (RozeBitSet*)malloc(sizeof(RozeBitSet));
    bs->refcount = 1;
    bs->num_words = words;
    bs->words = (uint64_t*)calloc(words, sizeof(uint64_t));
    return (int64_t)bs;
}

void __roze_bit_set_retain(int64_t bs_val) {
    if (!bs_val) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    __atomic_fetch_add(&bs->refcount, 1, __ATOMIC_SEQ_CST);
}

void __roze_bit_set_release(int64_t bs_val) {
    if (!bs_val) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    if (__atomic_fetch_sub(&bs->refcount, 1, __ATOMIC_SEQ_CST) == 1) {
        if (bs->words) free(bs->words);
        free(bs);
    }
}

void __roze_bit_set_set(int64_t bs_val, int64_t idx) {
    if (!bs_val || idx < 0) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    int64_t w_idx = idx / 64;
    bit_set_ensure_word(bs, w_idx);
    bs->words[w_idx] |= (1ULL << (idx % 64));
}

void __roze_bit_set_clear_bit(int64_t bs_val, int64_t idx) {
    if (!bs_val || idx < 0) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    int64_t w_idx = idx / 64;
    if (w_idx >= bs->num_words) return;
    bs->words[w_idx] &= ~(1ULL << (idx % 64));
}

int64_t __roze_bit_set_test(int64_t bs_val, int64_t idx) {
    if (!bs_val || idx < 0) return 0;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    int64_t w_idx = idx / 64;
    if (w_idx >= bs->num_words) return 0;
    return (bs->words[w_idx] >> (idx % 64)) & 1ULL;
}

void __roze_bit_set_flip(int64_t bs_val, int64_t idx) {
    if (!bs_val || idx < 0) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    int64_t w_idx = idx / 64;
    bit_set_ensure_word(bs, w_idx);
    bs->words[w_idx] ^= (1ULL << (idx % 64));
}

int64_t __roze_bit_set_count_ones(int64_t bs_val) {
    if (!bs_val) return 0;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    int64_t total = 0;
    for (int64_t i = 0; i < bs->num_words; i++) {
        total += __builtin_popcountll(bs->words[i]);
    }
    return total;
}

int64_t __roze_bit_set_len(int64_t bs_val) {
    if (!bs_val) return 0;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    return bs->num_words * 64;
}

int64_t __roze_bit_set_is_empty(int64_t bs_val) {
    if (!bs_val) return 1;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    for (int64_t i = 0; i < bs->num_words; i++) {
        if (bs->words[i] != 0) return 0;
    }
    return 1;
}

void __roze_bit_set_union(int64_t bs_val, int64_t other_val) {
    if (!bs_val || !other_val) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    RozeBitSet* other = (RozeBitSet*)other_val;
    if (other->num_words > bs->num_words) {
        bit_set_ensure_word(bs, other->num_words - 1);
    }
    for (int64_t i = 0; i < other->num_words; i++) {
        bs->words[i] |= other->words[i];
    }
}

void __roze_bit_set_intersect(int64_t bs_val, int64_t other_val) {
    if (!bs_val || !other_val) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    RozeBitSet* other = (RozeBitSet*)other_val;
    int64_t min_w = (bs->num_words < other->num_words) ? bs->num_words : other->num_words;
    for (int64_t i = 0; i < min_w; i++) {
        bs->words[i] &= other->words[i];
    }
    for (int64_t i = min_w; i < bs->num_words; i++) {
        bs->words[i] = 0;
    }
}

void __roze_bit_set_difference(int64_t bs_val, int64_t other_val) {
    if (!bs_val || !other_val) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    RozeBitSet* other = (RozeBitSet*)other_val;
    int64_t min_w = (bs->num_words < other->num_words) ? bs->num_words : other->num_words;
    for (int64_t i = 0; i < min_w; i++) {
        bs->words[i] &= ~(other->words[i]);
    }
}

void __roze_bit_set_clear(int64_t bs_val) {
    if (!bs_val) return;
    RozeBitSet* bs = (RozeBitSet*)bs_val;
    memset(bs->words, 0, sizeof(uint64_t) * bs->num_words);
}



