#include "alloc.h"
#include "tree_sitter/api.h"
#include <inttypes.h>
#include <stdint.h>
#include <stdlib.h>

// THE COUNT OF THE ALLOCATIONS OF ONE THREAD. The default allocator counts the net bytes that each
// thread holds, and the highest value of that count since the last `ts_allocation_reset`. A parse
// that runs away then stops at a ceiling of bytes, and not at the memory of the machine. One parse
// of a 20 KB file took 152 GB on 2026-09-16 before the kernel killed it, and five other processes
// with it.
//
// The count reads the size of a block back from the C library, so a block has no header. A block
// of this allocator is a plain block of the C library, and a pointer from before a call of
// `ts_set_allocator` frees correctly with each of the two. A library with no such function gives
// a count of zero, and the ceiling and the cap then never fire.
#if defined(__linux__)
#include <malloc.h>
#define ts_block_size(pointer) malloc_usable_size(pointer)
#elif defined(__APPLE__)
#include <malloc/malloc.h>
#define ts_block_size(pointer) malloc_size(pointer)
#elif defined(_WIN32)
#include <malloc.h>
#define ts_block_size(pointer) _msize(pointer)
#else
#define ts_block_size(pointer) ((size_t)0)
#endif

#if defined(_MSC_VER)
#define TS_THREAD_LOCAL __declspec(thread)
#else
#define TS_THREAD_LOCAL _Thread_local
#endif

// THE CAP IS THE BACKSTOP OF THE CEILING, AND NOT THE CEILING. A caller reads the peak in its
// progress callback and stops the parse at its own ceiling, which fails one file. The runtime
// calls that callback one time for each 100 parse operations, so an allocation that runs away
// inside one operation passes the ceiling with no callback. The cap catches that one case, and
// it stops the whole process, because a call of `malloc` cannot unwind. The value stays far above
// the ceiling of the corpus tools, so that the callback path catches each parse that the corpus
// shows, and the cap fires only for a true runaway. The largest peak of one parse in the corpus of
// 2026-09-16 was 806,867,464 bytes, for a file of 13,995,652 bytes, and the ceiling of the corpus
// tools is 64 MiB plus 512 bytes for each byte of the source.
#define TS_ALLOCATION_CAP_DEFAULT ((uint64_t)16 << 30)

static TS_THREAD_LOCAL int64_t ts_allocation_net = 0;
static TS_THREAD_LOCAL uint64_t ts_allocation_high = 0;
static TS_THREAD_LOCAL const char *ts_allocation_name = NULL;
static uint64_t ts_allocation_cap = TS_ALLOCATION_CAP_DEFAULT;

// Add the bytes of a new block to the count of this thread. Stop the process at the cap.
static void ts_allocation_grow(size_t bytes) {
  ts_allocation_net += (int64_t)bytes;
  if (ts_allocation_net > (int64_t)ts_allocation_high) {
    ts_allocation_high = (uint64_t)ts_allocation_net;
    if (ts_allocation_cap != 0 && ts_allocation_high > ts_allocation_cap) {
      fprintf(
        stderr,
        "tree-sitter: one thread holds %" PRIu64 " bytes of allocations, more than the cap of %" PRIu64
        " bytes, in the parse of %s. The process stops so that the machine does not. "
        "ts_set_allocation_cap changes the cap.\n",
        ts_allocation_high,
        ts_allocation_cap,
        ts_allocation_name ? ts_allocation_name : "an input with no label"
      );
      abort();
    }
  }
}

// Remove the bytes of a released block from the count of this thread. The count goes below zero
// when a thread releases a block that a different thread allocated, or a block from before the
// last reset. The peak reads the growth since the reset, so a negative count is correct.
static void ts_allocation_shrink(size_t bytes) {
  ts_allocation_net -= (int64_t)bytes;
}

void ts_allocation_reset(void) {
  ts_allocation_net = 0;
  ts_allocation_high = 0;
}

uint64_t ts_allocation_peak(void) {
  return ts_allocation_high;
}

void ts_allocation_label(const char *label) {
  ts_allocation_name = label;
}

void ts_set_allocation_cap(uint64_t bytes) {
  ts_allocation_cap = bytes;
}

static void *ts_malloc_default(size_t size) {
  void *result = malloc(size);
  if (size > 0 && !result) {
    fprintf(stderr, "tree-sitter failed to allocate %zu bytes", size);
    abort();
  }
  if (result) ts_allocation_grow(ts_block_size(result));
  return result;
}

static void *ts_calloc_default(size_t count, size_t size) {
  void *result = calloc(count, size);
  if (count > 0 && !result) {
    fprintf(stderr, "tree-sitter failed to allocate %zu bytes", count * size);
    abort();
  }
  if (result) ts_allocation_grow(ts_block_size(result));
  return result;
}

static void *ts_realloc_default(void *buffer, size_t size) {
  // The size of the old block comes first, because the old pointer is not valid after a move.
  size_t before = buffer ? ts_block_size(buffer) : 0;
  void *result = realloc(buffer, size);
  if (size > 0 && !result) {
    fprintf(stderr, "tree-sitter failed to reallocate %zu bytes", size);
    abort();
  }
  if (result) {
    ts_allocation_shrink(before);
    ts_allocation_grow(ts_block_size(result));
  } else {
    // A size of zero releases the block, and the C library gives no block.
    ts_allocation_shrink(before);
  }
  return result;
}

static void ts_free_default(void *buffer) {
  if (buffer) ts_allocation_shrink(ts_block_size(buffer));
  free(buffer);
}

// Allow clients to override allocation functions dynamically
TS_PUBLIC void *(*ts_current_malloc)(size_t) = ts_malloc_default;
TS_PUBLIC void *(*ts_current_calloc)(size_t, size_t) = ts_calloc_default;
TS_PUBLIC void *(*ts_current_realloc)(void *, size_t) = ts_realloc_default;
TS_PUBLIC void (*ts_current_free)(void *) = ts_free_default;

void ts_set_allocator(
  void *(*new_malloc)(size_t size),
  void *(*new_calloc)(size_t count, size_t size),
  void *(*new_realloc)(void *ptr, size_t size),
  void (*new_free)(void *ptr)
) {
  ts_current_malloc = new_malloc ? new_malloc : ts_malloc_default;
  ts_current_calloc = new_calloc ? new_calloc : ts_calloc_default;
  ts_current_realloc = new_realloc ? new_realloc : ts_realloc_default;
  ts_current_free = new_free ? new_free : ts_free_default;
}
