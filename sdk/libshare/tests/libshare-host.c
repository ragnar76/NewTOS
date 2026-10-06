/*
 * libshare-host.c -- host harness for the libshare allocator as the SDK
 * builds it (sdk/libshare/build-libshare.sh: libshare 0.1.1 + overlay/),
 * TASK/LDG-M3-B2.
 *
 * WHAT IT PROVES. The lifetime property finding B2 is about
 * (docs/analysis/ldg-module-malloc-2026-10-06.md): an LDG module runs once
 * as its own process (Pexec 4); its exit() closes stdio, which mallocs a
 * BUFSIZ buffer and frees it again (mintlib stdio/fclose.c:37-44,
 * internals.c:55-80); the module's Pterm then frees every block the MODULE
 * owns (EmuTOS bdos/proc.c:92-101,136-140); afterwards the client calls
 * module functions that malloc again (POPUP.NLD __get_bkgr). With mintlib's
 * malloc the module's static free list ends up pointing into a freed hunk
 * (measured: measurements/measurement-2026-10-06-ldg-b2). With libshare:
 *
 *   1. exit() closes the two write streams, stderr then stdout
 *      (fcloseall walks __stdio_head = &__stderr -> stdout -> stdin,
 *      stdio-globals.c:68-72), each malloc(BUFSIZ) + free. The first buffer
 *      comes from libshare's static_mblock (the module's BSS, i.e. its TPA,
 *      which the CLIENT owns). Freeing it empties the block, and libshare
 *      then drops the block from its list for good (malloc.c:271-289: the
 *      border chunk is unlinked, the static block is never ldg_Free'd and
 *      static_block_returned stays 1). The second buffer therefore gets a
 *      hunk through sbrk -> ldg_Malloc while the MODULE runs -- but
 *      libshare's free() hands that hunk back with ldg_Free as soon as it is
 *      empty (malloc.c:271-289), i.e. before the Pterm. Measured the same way
 *      on the target: Mxalloc(0x4000) + Mfree of that block inside the
 *      module's Pexec(4) run, free list head 0 at its Pterm
 *      (measurements/measurement-2026-10-06-libshare, ls-instr-on).
 *   2. after the module's Pterm the free list references no freed memory;
 *   3. later mallocs (client context) get their block through ldg_Malloc,
 *      owned by the caller, and the free list stays inside live memory;
 *      a hunk that becomes completely free is returned with ldg_Free;
 *   4. calloc zeroes, realloc keeps the contents (libshare's own files).
 *
 * The system side is modelled here: ldg_Malloc/ldg_Free (libldg, which on
 * the target end in GEMDOS Mxalloc/Mfree) record every block with the
 * "process" that was running when it was requested, and pterm() frees the
 * blocks of a process the way free_all_owned does. Freed blocks are only
 * marked and poisoned (0xDE), never returned to the host allocator, so the
 * checks can look at them without undefined behaviour.
 *
 * libshare's sources are compiled with mintlib's headers (they need
 * <compiler.h>, <osbind.h>, <ldg.h>), this file with the host's; the names
 * malloc/free/calloc/realloc/sbrk/errno/getpagesize are renamed to ut_* on
 * the libshare side (libshare-host.sh) so they do not replace the host C
 * library's. The struct layout both sides agree on is pinned by
 * libshare-host-layout.c, compiled with libshare's own lib.h.
 *
 * Build + run: sdk/libshare/tests/libshare-host.sh (sdk/run-host-tests.sh
 * calls it). Exit status: number of failed checks; 77 = skipped.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* libshare, renamed (libshare-host.sh) */
extern void *ut_malloc (size_t);
extern void ut_free (void *);
extern void *ut_calloc (size_t, size_t);
extern void *ut_realloc (void *, size_t);
extern char static_mblock[];
extern int __sbrk_has_been_called;

/* lib.h:16-25 mirrored; checked against libshare-host-layout.c */
struct ut_chunk
{
  long valid;
  struct ut_chunk *next;
  unsigned long size;
};
extern struct ut_chunk libshare_mchunk_free_list;
extern const unsigned long ut_chunk_size, ut_chunk_off_valid, ut_chunk_off_next,
  ut_chunk_off_size, ut_chunk_align, ut_border_extra;

#define SIZEOF_STATIC_MBLOCK 16384L	/* malloc.c:50 */
#define BUFSIZ_MINTLIB 1024		/* mintlib include/stdio.h:243 */
#define GET_BKGR_SIZE 0x46c0		/* POPUP.NLD __get_bkgr, ldg-b2 run */

/* The mintlib globals libshare reads (lib.h:9-12), set the way the SDK's
   start-up leaves them in a module: heap_init.c, default _stksize = MINKEEP
   -> _split_mem = 1, _heapbase = NULL. */
void *_heapbase = NULL;
short _split_mem = 1;
long _stksize = 64L * 1024L;
int __libc_unix_names = 0;
int ut_errno;
int ut_getpagesize (void) { return 8192; }

void
__assert_fail (const char *a, const char *f, unsigned int l, const char *fn)
{
  printf ("FAIL  libshare assert(%s) at %s:%u (%s)\n", a, f, l, fn ? fn : "?");
  exit (1);
}

/* chdir.c's helpers (not exercised here, but the object is linked whole) */
int _enoent (const char *p) { (void) p; return 0; }
int _unx2dos (const char *u, char *d, size_t n) { strncpy (d, u, n); return 0; }

/* ---- the system side: blocks with owners -------------------------------- */
enum { NONE = 0, MODULE = 1, CLIENT = 2 };
static int run = NONE;		/* EmuTOS `run`: who allocates right now */

struct blk { char *p; long size; int owner; int freed; };
static struct blk blks[64];
static int nblk, n_ldg_malloc, n_ldg_free;

void *
ldg_Malloc (long size)
{
  char *p;
  if (nblk == (int) (sizeof blks / sizeof blks[0]))
    return NULL;
  p = aligned_alloc (4096, (size_t) ((size + 4095) & ~4095L));
  if (!p)
    return NULL;
  memset (p, 0xA5, (size_t) size);	/* not zero: calloc must zero */
  blks[nblk].p = p;
  blks[nblk].size = size;
  blks[nblk].owner = run;
  blks[nblk].freed = 0;
  nblk++;
  n_ldg_malloc++;
  return p;
}

int
ldg_Free (void *m)
{
  int i;
  n_ldg_free++;
  for (i = 0; i < nblk; i++)
    if (blks[i].p == m && !blks[i].freed)
      {
        blks[i].freed = 1;
        memset (blks[i].p, 0xDE, (size_t) blks[i].size);
        return 0;
      }
  printf ("FAIL  ldg_Free(%p): not a live ldg_Malloc block\n", m);
  exit (1);
}

/* Pterm of `owner`: free everything it owns (EmuTOS free_all_owned). */
static void
pterm (int owner)
{
  int i;
  for (i = 0; i < nblk; i++)
    if (blks[i].owner == owner && !blks[i].freed)
      {
        blks[i].freed = 1;
        memset (blks[i].p, 0xDE, (size_t) blks[i].size);
      }
}

static int
in_freed_block (const void *a)
{
  int i;
  for (i = 0; i < nblk; i++)
    if (blks[i].freed && (const char *) a >= blks[i].p
        && (const char *) a < blks[i].p + blks[i].size)
      return 1;
  return 0;
}

static int
live_block_of (const void *a)
{
  int i;
  for (i = 0; i < nblk; i++)
    if (!blks[i].freed && (const char *) a >= blks[i].p
        && (const char *) a < blks[i].p + blks[i].size)
      return i;
  return -1;
}

static int
in_static (const void *a, size_t n)
{
  return (const char *) a >= static_mblock
    && (const char *) a + n <= static_mblock + SIZEOF_STATIC_MBLOCK;
}

/* Every node of libshare's free list must be in the static block or in a
   live ldg_Malloc block; the walk stops at the first node that is not (its
   `next` would be read from freed memory -- the B2 fault). */
static int
free_list_dangling (int *nodes)
{
  const struct ut_chunk *q;
  int n = 0;
  for (q = libshare_mchunk_free_list.next; q; q = q->next)
    {
      if (in_freed_block (q))
        return 1;
      if (!in_static (q, sizeof *q) && live_block_of (q) < 0)
        return 2;
      if (++n > 1000)
        return 3;		/* a cycle */
    }
  if (nodes)
    *nodes = n;
  return 0;
}

static int failures, checks;
#define CHECK(cond, ...) do { checks++; if (cond) { printf ("ok    "); } \
  else { failures++; printf ("FAIL  "); } printf (__VA_ARGS__); printf ("\n"); } while (0)

int
main (void)
{
  char *buf, *bkgr, *small, *c, *r;
  int i, d, nodes = -1, b;

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("libshare-host: skipped -- host is not LP64\n");
      return 77;
    }

  /* layout: harness mirror == libshare's lib.h as compiled */
  CHECK (ut_chunk_size == sizeof (struct ut_chunk)
         && ut_chunk_off_valid == 0 && ut_chunk_off_next == 8
         && ut_chunk_off_size == 16 && ut_border_extra == 32,
         "struct mem_chunk = 24 bytes {valid@0, next@8, size@16}, BORDER_EXTRA 32 (LP64)");
  CHECK (((unsigned long) static_mblock & (ut_chunk_align - 1)) == 0,
         "static_mblock %p aligned to __alignof__(struct mem_chunk) = %lu (overlay malloc.c:51)",
         (void *) static_mblock, ut_chunk_align);

  /* ---- 1. the module's own run (Pexec 4): exit() -> fcloseall: fclose
     (stderr), fclose(stdout) -> __flshfp -> init_stream malloc + free */
  run = MODULE;
  buf = ut_malloc (BUFSIZ_MINTLIB);	/* stderr's buffer */
  CHECK (buf != NULL && in_static (buf, BUFSIZ_MINTLIB),
         "first exit-path stdio buffer (malloc(%d)) = %p lies in static_mblock [%p, +16384)",
         BUFSIZ_MINTLIB, (void *) buf, (void *) static_mblock);
  CHECK (((unsigned long) buf & 7) == 0, "buffer 8-byte aligned (MALLOC_ALIGNMENT)");
  CHECK (n_ldg_malloc == 0 && __sbrk_has_been_called == 0,
         "no system block for it (ldg_Malloc calls: %d, sbrk called: %d)",
         n_ldg_malloc, __sbrk_has_been_called);
  memset (buf, 'x', BUFSIZ_MINTLIB);
  ut_free (buf);			/* fclose: free(stream->__buffer) */
  CHECK (libshare_mchunk_free_list.next == NULL,
         "freeing it empties the static block; libshare drops it from its list (malloc.c:271-289)");
  buf = ut_malloc (BUFSIZ_MINTLIB);	/* stdout's buffer */
  b = buf ? live_block_of (buf) : -1;
  CHECK (buf != NULL && b >= 0 && blks[b].owner == MODULE && n_ldg_malloc == 1,
         "second exit-path buffer = %p from ldg_Malloc while the MODULE runs (owner module)", (void *) buf);
  memset (buf, 'y', BUFSIZ_MINTLIB);
  ut_free (buf);
  CHECK (n_ldg_free == 1 && b >= 0 && blks[b].freed,
         "libshare's free() returns that hunk with ldg_Free before the Pterm (ldg_Free calls: %d)",
         n_ldg_free);
  pterm (MODULE);			/* the module's Pterm */
  d = free_list_dangling (&nodes);
  CHECK (d == 0, "after the module's Pterm the free list references no freed memory (%d nodes, code %d)",
         nodes, d);

  /* ---- 2. the client calls into the module (POPUP __popup -> __get_bkgr) */
  run = CLIENT;
  bkgr = ut_malloc (GET_BKGR_SIZE);
  b = bkgr ? live_block_of (bkgr) : -1;
  CHECK (bkgr != NULL && n_ldg_malloc == 2 && b >= 0 && blks[b].owner == CLIENT,
         "malloc(0x%x) in client context -> %p from ldg_Malloc, owned by the client (%d system block(s))",
         GET_BKGR_SIZE, (void *) bkgr, n_ldg_malloc);
  if (bkgr)
    memset (bkgr, 0x5A, GET_BKGR_SIZE);
  small = ut_malloc (100);
  CHECK (small != NULL && free_list_dangling (NULL) == 0,
         "second malloc(100) = %p, free list consistent", (void *) small);
  ut_free (bkgr);
  ut_free (small);
  CHECK (free_list_dangling (NULL) == 0 && n_ldg_free == 2,
         "freeing both returns the now empty hunk with ldg_Free (ldg_Free calls: %d), list consistent",
         n_ldg_free);

  /* ---- 3. calloc / realloc (libshare calloc.c, realloc.c) ---------------- */
  c = ut_calloc (10, 100);
  for (i = 0; c && i < 1000 && c[i] == 0; i++)
    ;
  CHECK (c != NULL && i == 1000, "calloc(10, 100) returns 1000 zero bytes (overlay calloc.c:25 bzero)");
  memcpy (c, "libshare", 9);
  r = ut_realloc (c, 30000);
  CHECK (r != NULL && memcmp (r, "libshare", 9) == 0,
         "realloc to 30000 keeps the contents");
  ut_free (r);
  CHECK (free_list_dangling (NULL) == 0, "free list consistent at the end");

  /* ---- 4. the client terminates: its blocks go; nothing of the module's
     list may point there any more than the client's own does. */
  pterm (CLIENT);
  printf ("\nlibshare-host: %d checks, %d failed\n", checks, failures);
  return failures;
}
