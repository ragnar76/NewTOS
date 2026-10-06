/*
 * libshare-host-layout.c -- compiled with libshare's own lib.h (mintlib's
 * include path) next to the overlaid libshare sources, so the host harness
 * (libshare-host.c, host headers) can check that its mirror of
 * `struct mem_chunk` and the static block size match what libshare was
 * compiled with. Part of sdk/libshare/tests/libshare-host.sh.
 */
#include <stddef.h>
#include "lib.h"

const unsigned long ut_chunk_size = sizeof (struct mem_chunk);
const unsigned long ut_chunk_off_valid = offsetof (struct mem_chunk, valid);
const unsigned long ut_chunk_off_next = offsetof (struct mem_chunk, next);
const unsigned long ut_chunk_off_size = offsetof (struct mem_chunk, size);
const unsigned long ut_chunk_align = __alignof__ (struct mem_chunk);
const unsigned long ut_border_extra = BORDER_EXTRA;
