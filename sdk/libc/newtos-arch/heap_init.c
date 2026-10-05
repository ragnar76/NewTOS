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
 * process a 4-MiB Transient Program Area at its own pool address B (ARCH03
 * stage 1, VA = PA); the image is linked at RESERVED_LOW_END = 0x1000
 * (sdk/newtos.ld) and relocated for B. Relative to B:
 *
 *     [0x000, 0x100)      BasePage
 *     [0x1000, _end)      image + BSS (zeroed by the loader)
 *     [_end, p_hitpa)     free TPA -- heap grows up, stack grows down
 *
 * so we run malloc entirely from that free region: no GEMDOS Malloc dependency
 * and no reliance on the kernel's per-process heap allocator.  We set
 * _heapbase = _end (linker symbol, first free byte) and _stksize to the size
 * of the free region minus a stack reserve, with _split_mem = 0.  __sbrk's
 * HeapAlloc then guards every request against the live stack pointer, so the
 * reserve is only a soft upper bound.
 *
 * crt0 (sdk/libc/crt0.S) calls this once, before main, passing the BasePage
 * address (the base register it read p_hitpa through) and p_hitpa (the
 * mapped TPA top = initial SP).
 *
 * `_base` (mintlib/globals.c:33, `BASEPAGE *_base`) is what mintlib's own
 * m68k crt0 stores first thing at program start (startup/crt0.S:69,
 * `movel %a0, _base`) and what its readers dereference: crtinit.c:167
 * (`bp = _base`, the p_tlen/p_dlen/p_blen sum and p_hitpa for the Mshrink
 * idiom), unix/getpid.c:21 (`(long)_base >> 8` when Pgetpid is absent),
 * unix/getppid.c:22 (`_base->p_parent`), gmon.c:201,237 (`_base->p_tbase`).
 * None of those files runs mintlib's startup here (docs/sdk/07-mintlib-limits.md
 * §5: crtinit.c is not built), so `_base` stayed NULL and TeraDesk's
 * `_BasPag->p_env` (environm.c) only worked because the basepage was at VA 0.
 * Setting it here from the value crt0 hands us makes the pointer true
 * independent of where the loader puts the basepage (TASK/T05) -- since
 * ARCH03 stage 1, the start of the TPA block.
 *
 * loader facts confirmed from tos-api/src/bdos/process/loader.rs:
 *   - bp.p_hitpa = B + TPA_SIZE(4 MiB)    (populate_basepage)
 *   - "heap lives between the BSS end (p_bbase+p_blen) and the downward-
 *      growing stack" (populate_basepage doc comment)
 *   - USER_STACK_MIN = 0x1_0000 (64 KiB) top-of-TPA stack reserve
 *      (loader.rs, validate_segment)
 */

#include <stddef.h>
#include <mint/basepage.h>

/* Provided by the NewTOS linker script (sdk/newtos.ld): first free TPA byte,
   just above the zeroed BSS (== the basepage's p_bbase + p_blen). */
extern char _end[];

/* Defined in mintlib: heapbase.c (_heapbase), stksiz.c (_stksize),
   globals.c (_split_mem, _base). We only initialise them here. */
extern void *_heapbase;
extern long _stksize;
extern short _split_mem;

/* Keep this much of the top of the TPA for the downward-growing stack; matches
   the loader's USER_STACK_MIN reservation. HeapAlloc's live-SP check is the
   real collision guard, this is only the advertised heap ceiling. */
#define NEWTOS_STACK_RESERVE (64UL * 1024UL)

void
__libc_init_heap (unsigned long basepage, unsigned long p_hitpa)
{
	unsigned long base = (unsigned long) _end;
	unsigned long avail;

	/* The BASEPAGE pointer mintlib's readers expect (see the header). */
	_base = (BASEPAGE *) basepage;

	/* Run malloc from the in-TPA arena, never from GEMDOS Malloc. */
	_split_mem = 0;
	_heapbase = (void *) base;

	if (p_hitpa <= base + NEWTOS_STACK_RESERVE)
		avail = 0;                       /* pathologically small TPA */
	else
		avail = p_hitpa - base - NEWTOS_STACK_RESERVE;

	_stksize = (long) avail;
}
