/*
 * malloc-host.c -- host harness for the hunk schedule of mintlib's malloc
 * with the NewTOS overlay (TASK/MEM05 part B, user decision E6, design
 * docs/analysis/mem05-design-2026-10-13.md section 4).
 *
 * mintlib's __malloc (mintlib/malloc.c) asks the OS for memory in hunks: one
 * __sbrk = one GEMDOS Malloc = one memory descriptor (unix/sbrk.c). A hunk is
 * MINHUNK bytes (8 KiB at the start) and MINHUNK doubles after each hunk
 * until it reaches MAXHUNK (32 KiB in mintlib); a request larger than MINHUNK
 * gets a hunk of its own size. The overlay sdk/libc/overlay/mintlib/malloc.c
 * adds __newtos_malloc_maxhunk(siz), which sets only MAXHUNK; the C start-up
 * (newtos-arch/heap_init.c) calls it with 16 MiB.
 *
 * malloc-host.sh links the overlay malloc.c (or the file named by
 * MALLOC_SRC), built with the include path and DEFS of build-libc.sh, with
 * this file, which plays __sbrk (records every hunk size, hands out host
 * memory), getpagesize (8192, mintlib's DEF_PAGESIZE for TOS,
 * unix/getpagesize.c) and Mfree. The global state of malloc.c (MINHUNK,
 * MAXHUNK, the free list) cannot be reset, so every scenario runs in its own
 * process: `malloc-host <scenario>'.
 *
 * Scenarios (malloc(32) = 56 bytes per block with its LP64 mem_chunk):
 *   pristine   no call: hunks 8, 16, 32, 32, ... KiB; hunk counts after
 *              100/1000/10000/50000/100000/147605/147606 blocks equal the
 *              post-E9 QEMU measurement 1/3/19/87/173/254/255
 *              (measurements/measurement-2026-10-06-post-e9/RUNINFO.TXT,
 *              "MD PROBE") -- calibrates the harness against the target.
 *   maxhunk    __newtos_malloc_maxhunk(16 MiB) as the start-up does: hunks
 *              8 KiB << i up to 16 MiB, then 16 MiB each; counts after
 *              100/1000/10000/147605/1000000 = 1/3/7/10/14 (design table
 *              section 4.4).
 *   acc        the same as maxhunk with the accessory settings of acc_main
 *              (_split_mem = 0, _heapbase = NULL) -- the same schedule.
 *   small      maxhunk set: a hello-world's allocations (a 1024-byte console
 *              buffer + a few small blocks) take exactly ONE 8 KiB hunk; a
 *              32 KiB file buffer then gets a hunk of its own size and does
 *              not advance the doubling (the next small hunk is 16 KiB).
 *   chunksize  maxhunk set, then _mallocChunkSize(64 KiB): every hunk is
 *              64 KiB (mintlib's own tuning path still overrides).
 *
 * Exit status: number of failed checks; 77 = host is not LP64.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* the overlay malloc.c, renamed by malloc-host.sh (-D) so the host's own
   malloc/free stay untouched */
void *mh_malloc (size_t);
void mh_mallocchunksize (size_t);
void __newtos_malloc_maxhunk (size_t);

/* what malloc.c reads from mintlib's globals.c / heapbase.c / stksiz.c */
void *_heapbase = NULL;
short _split_mem = 1;		/* heap_init.c, application, default _stksize */
long _stksize = 0;

#define MAXH 8192
static unsigned long hunk[MAXH];
static int nhunks;
static int failed, checks;

void *__sbrk (intptr_t n);
void *
__sbrk (intptr_t n)
{
  void *p;

  if (nhunks >= MAXH)
    return (void *) -1L;
  p = malloc ((size_t) n);
  if (p == NULL)
    return (void *) -1L;
  hunk[nhunks++] = (unsigned long) n;
  return p;
}

int mh_getpagesize (void);
int
mh_getpagesize (void)
{
  return 8192;
}

long mh_Mfree (void *);
long
mh_Mfree (void *p)
{
  (void) p;
  return 0;
}

static void
check (int ok, const char *what, long got, long want)
{
  checks++;
  if (!ok)
    failed++;
  printf ("%s  %s: got %ld, want %ld\n", ok ? "PASS" : "FAIL", what, got,
	  want);
}

struct point { unsigned long blocks; int hunks; };

/* malloc(32) up to the last point; the hunk count at each point */
static void
series (const char *tag, const struct point *pt, int npt)
{
  unsigned long n = 0;
  int i;
  char what[96];

  for (i = 0; i < npt; i++)
    {
      while (n < pt[i].blocks)
	{
	  char *p = mh_malloc (32);
	  if (p == NULL)
	    {
	      check (0, "malloc(32) returned NULL", (long) n, -1);
	      return;
	    }
	  p[0] = 1;
	  n++;
	}
      snprintf (what, sizeof what, "%s: hunks after %lu x malloc(32)", tag,
		pt[i].blocks);
      check (nhunks == pt[i].hunks, what, nhunks, pt[i].hunks);
    }
}

/* hunk i must be min(8 KiB << i, ceiling) */
static void
schedule (const char *tag, unsigned long ceiling)
{
  int i, bad = -1;

  for (i = 0; i < nhunks; i++)
    {
      unsigned long want = i < 20 ? (8192UL << i) : ceiling;
      if (want > ceiling)
	want = ceiling;
      if (hunk[i] != want)
	{
	  bad = i;
	  break;
	}
    }
  char what[96];
  snprintf (what, sizeof what, "%s: hunk sizes 8 KiB doubling to %lu (first bad index)",
	    tag, ceiling);
  check (bad < 0, what, bad, -1);
  if (nhunks > 0)
    {
      snprintf (what, sizeof what, "%s: last hunk size", tag);
      check (hunk[nhunks - 1] == ceiling, what, (long) hunk[nhunks - 1],
	     (long) ceiling);
    }
}

int
main (int argc, char **argv)
{
  const char *s = argc > 1 ? argv[1] : "";

  if (sizeof (long) != 8 || sizeof (void *) != 8)
    {
      printf ("malloc-host: skipped -- host is not LP64\n");
      return 77;
    }

  if (!strcmp (s, "pristine"))
    {
      static const struct point pt[] = {
	{ 100, 1 }, { 1000, 3 }, { 10000, 19 }, { 50000, 87 },
	{ 100000, 173 }, { 147605, 254 }, { 147606, 255 } };
      series ("pristine", pt, 7);
      schedule ("pristine", 32768);
    }
  else if (!strcmp (s, "maxhunk") || !strcmp (s, "acc"))
    {
      static const struct point pt[] = {
	{ 100, 1 }, { 1000, 3 }, { 10000, 7 }, { 147605, 10 },
	{ 1000000, 14 } };
      if (!strcmp (s, "acc"))
	_split_mem = 0;		/* acc_main: _split_mem = 0, _heapbase = NULL */
      __newtos_malloc_maxhunk (16UL << 20);
      series (s, pt, 5);
      schedule (s, 16UL << 20);
    }
  else if (!strcmp (s, "small"))
    {
      __newtos_malloc_maxhunk (16UL << 20);
      /* a hello-world: the console stream buffer (BUFSIZ 1024,
	 docs/sdk/07-mintlib-limits.md) and a few small blocks */
      (void) mh_malloc (1024);
      (void) mh_malloc (100);
      (void) mh_malloc (32);
      (void) mh_malloc (2000);
      check (nhunks == 1, "small: hunks for a hello-world's blocks", nhunks, 1);
      check (nhunks >= 1 && hunk[0] == 8192, "small: its size", (long) hunk[0],
	     8192);
      /* a 32 KiB file buffer (do_fstat.c st_blksize): its own hunk, exact
	 size rounded to the page (+ border), no doubling consumed */
      (void) mh_malloc (32768);
      check (nhunks == 2, "small: hunks after a 32 KiB buffer", nhunks, 2);
      check (nhunks >= 2 && hunk[1] == 32768 + 8192,
	     "small: size of that hunk (32 KiB + border, page-rounded)",
	     (long) hunk[1], 32768 + 8192);
      /* use up the first hunk; the next small hunk is MINHUNK = 16 KiB */
      while (nhunks == 2)
	(void) mh_malloc (32);
      check (hunk[2] == 16384, "small: next small hunk", (long) hunk[2],
	     16384);
    }
  else if (!strcmp (s, "chunksize"))
    {
      static const struct point pt[] = { { 10000, 9 } };
      __newtos_malloc_maxhunk (16UL << 20);
      mh_mallocchunksize (65536);
      series ("chunksize", pt, 1);
      int i, bad = -1;
      for (i = 0; i < nhunks; i++)
	if (hunk[i] != 65536)
	  {
	    bad = i;
	    break;
	  }
      check (bad < 0, "chunksize: every hunk 64 KiB (first bad index)", bad,
	     -1);
    }
  else
    {
      fprintf (stderr, "usage: malloc-host pristine|maxhunk|acc|small|chunksize\n");
      return 1;
    }

  printf ("%s: %d checks, %d failed\n", s, checks, failed);
  return failed;
}
