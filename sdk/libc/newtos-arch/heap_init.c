/*
 * heap_init.c -- NewTOS/AArch64 C-runtime heap initialisation.
 *
 * mintlib's malloc (mintlib/malloc.c) draws memory from __sbrk (unix/sbrk.c).
 * __sbrk has two modes selected by the globals _split_mem / _heapbase:
 *
 *   _split_mem == 0 && _heapbase != NULL  -> HeapAlloc(): carve from an
 *                                            in-process arena [_heapbase ..
 *                                            _heapbase + _stksize), growing up
 *                                            towards the downward stack (the
 *                                            classic "keep all of memory"
 *                                            model, crtinit.c _stksize == -1).
 *   otherwise                             -> GEMDOS Malloc() (trap).
 *
 * On NewTOS the program loader (tos-api/src/bdos/process/loader.rs) hands each
 * process a Transient Program Area [0, USER_TPA_TOP=0x400000):
 *
 *     [0x000, 0x100)      BasePage
 *     [0x100, _end)       image + BSS (zeroed by the loader)
 *     [_end, p_hitpa)     free TPA -- heap grows up, stack grows down
 *
 * so we run malloc entirely from that free region: no GEMDOS Malloc dependency
 * and no reliance on the kernel's per-process heap allocator.  We set
 * _heapbase = _end (linker symbol, first free byte) and _stksize to the size
 * of the free region minus a stack reserve, with _split_mem = 0.  __sbrk's
 * HeapAlloc then guards every request against the live stack pointer, so the
 * reserve is only a soft upper bound.
 *
 * crt0 (sdk/libc/crt0.S) calls this once, before main, passing p_hitpa (the
 * mapped TPA top = initial SP) that it already read from the BasePage.
 *
 * loader facts confirmed from tos-api/src/bdos/process/loader.rs:
 *   - USER_TPA_TOP = USER_VA_BASE(0) + TPA_SIZE(4 MiB) = 0x400000
 *   - bp.p_hitpa = USER_TPA_TOP           (loader.rs:453)
 *   - "heap lives between the BSS end (p_bbase+p_blen) and the downward-
 *      growing stack" (loader.rs:394-396)
 *   - USER_STACK_MIN = 0x1_0000 (64 KiB) top-of-TPA stack reserve
 *      (loader.rs:169, validate_segment loader.rs:626)
 */

#include <stddef.h>

/* Provided by the NewTOS linker script (sdk/newtos.ld): first free TPA byte,
   just above the zeroed BSS. */
extern char _end[];

/* Defined in mintlib: heapbase.c (_heapbase), stksiz.c (_stksize),
   globals.c (_split_mem). We only initialise them here. */
extern void *_heapbase;
extern long _stksize;
extern short _split_mem;

/* Keep this much of the top of the TPA for the downward-growing stack; matches
   the loader's USER_STACK_MIN reservation. HeapAlloc's live-SP check is the
   real collision guard, this is only the advertised heap ceiling. */
#define NEWTOS_STACK_RESERVE (64UL * 1024UL)

void
__libc_init_heap (unsigned long p_hitpa)
{
	unsigned long base = (unsigned long) _end;
	unsigned long avail;

	/* Run malloc from the in-TPA arena, never from GEMDOS Malloc. */
	_split_mem = 0;
	_heapbase = (void *) base;

	if (p_hitpa <= base + NEWTOS_STACK_RESERVE)
		avail = 0;                       /* pathologically small TPA */
	else
		avail = p_hitpa - base - NEWTOS_STACK_RESERVE;

	_stksize = (long) avail;
}
