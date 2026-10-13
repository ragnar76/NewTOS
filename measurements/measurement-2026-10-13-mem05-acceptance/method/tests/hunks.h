/*
 * hunks.h -- MEM05 part B probes: list mintlib malloc's hunks from inside the
 * program. Every hunk mintlib takes from the OS (one __sbrk = one GEMDOS
 * Malloc, unix/sbrk.c) starts with a VAL_BORDER mem_chunk that stays on the
 * address-ordered free list (_mchunk_free_list) for the hunk's lifetime; the
 * long after it (ALLOC_SIZE) is the hunk's usable size, i.e. the Malloc size
 * minus BORDER_EXTRA (mintlib/malloc.c:99-109, malloc_int.h). The layout is
 * copied from sdk/libc/mintlib/mintlib/malloc_int.h (LP64: 24-byte
 * mem_chunk, BORDER_EXTRA 32). Test code, not product code.
 */
struct mh_chunk { long valid; struct mh_chunk *next; unsigned long size; };
#define MH_VAL_BORDER 0xb04d0abcL
#define MH_BORDER_EXTRA 32UL
extern struct mh_chunk _mchunk_free_list;

static void hk_putdec(unsigned long v)
{
	char b[24];
	int i = 23;
	b[i] = 0;
	do { b[--i] = (char)('0' + v % 10); v /= 10; } while (v);
	Cconws(&b[i]);
}

/* "<tag> hunks=<n> total=<bytes> sizes(KiB, by address)=a,b,c..." -- the
   GEMDOS Malloc size of each hunk (border included). verbose=0: count+total. */
static unsigned long hunk_report(const char *tag, int verbose)
{
	const struct mh_chunk *c;
	unsigned long n = 0, total = 0;
	for (c = _mchunk_free_list.next; c; c = c->next)
		if (c->valid == MH_VAL_BORDER) {
			n++;
			total += *(const long *)(c + 1) + MH_BORDER_EXTRA;
		}
	Cconws(tag);
	Cconws(" hunks=");
	hk_putdec(n);
	Cconws(" total=");
	hk_putdec(total);
	if (verbose) {
		int first = 1;
		Cconws(" sizes(KiB, by address)=");
		for (c = _mchunk_free_list.next; c; c = c->next)
			if (c->valid == MH_VAL_BORDER) {
				if (!first)
					Cconws(",");
				first = 0;
				hk_putdec((*(const long *)(c + 1) + MH_BORDER_EXTRA) >> 10);
			}
	}
	Cconws("\r\n");
	return n;
}
