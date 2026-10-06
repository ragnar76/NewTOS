/*
 * heap_init.c -- NewTOS/AArch64 C-runtime start-up: mintlib's crtinit.c
 * `_crtinit` / `_acc_main` for the TOS case.
 *
 * mintlib/crtinit.c does not compile for AArch64 (m68k inline asm for the
 * "Mshrink onto a new stack" idiom, docs/sdk/07-mintlib-limits.md), so
 * sdk/libc/crt0.S calls `__libc_crtinit` instead, on a start-up stack in the
 * reserved gap above the BasePage, and moves the stack to the address it
 * returns. Everything else is mintlib's own rules, cited per line.
 *
 * THE TPA. The NewTOS loader gives a program the LARGEST free block of the
 * pool its PRGFLAGS choose, as EmuTOS alloc_tpa does (bdos/proc.c:460-502;
 * ARCH03 E9), with the BasePage at its start B and p_hitpa at its end. The
 * image is linked at B + 0x1000 (sdk/newtos.ld), so "the program sans
 * stack/heap" -- mintlib's tlen + dlen + blen + sizeof(BASEPAGE)
 * (crtinit.c:173) -- is `_end - B` here: the same bytes plus the reserved gap.
 * On top of it come, as in mintlib (crtinit.c:167-172), the bytes parseargs
 * lays out just above the BSS -- environ[], the split command line, argv[]
 * (newtos-arch/cmdargs.c, TASK/PORT09) -- so heap and stack start above them.
 *
 * THE POLICY (crtinit.c:6-25,176-222), by mintlib's `_stksize`
 * (stksiz.c: default MINKEEP = 64 KiB) and `_initial_stack` (inistack.c):
 *
 *     _stksize      keep                                 malloc from
 *       -1          all of memory but MINFREE            own heap above the image
 *        0          MINKEEP stack                        GEMDOS Malloc (_split_mem)
 *      1/2/3        1/4, 1/2, 3/4 of the free memory     GEMDOS Malloc (_split_mem)
 *      other        that many bytes                      GEMDOS Malloc (_split_mem)
 *     < -1          |_stksize|                           own heap above the image
 *
 * then `Mshrink(B, m)` with m = program + stack, and the stack at B + m
 * (crtinit.c:212-247). With the default, malloc goes to GEMDOS Malloc first
 * and to nothing else (`_heapbase` stays NULL): unix/sbrk.c:54-73.
 *
 * ACCESSORIES. The AES side shrinks an accessory's TPA to "code+data+bss plus
 * basepage" right after loading it (EmuTOS pgmld, util/gemdos.c:29-47;
 * kernel/src/accs.rs), and the basepage's p_hitpa keeps naming the old top.
 * mintlib recognises an accessory in startup/crt0.S:46-62: the basepage in
 * a0 (for a program a0 is 0) AND `p_parent == 0` -- an accessory is loaded
 * with Pexec(PE_LOAD) and never started by proc_go, the only writer of
 * p_parent (EmuTOS bdos/proc.c:370,541). NewTOS hands the BasePage over in
 * x0 on every path, so the first half always holds and the test is
 * `p_parent == 0`; since TASK/PPARENT every other start has a non-zero
 * p_parent, as on EmuTOS (docs/api/gemdos/process/_parent-and-environment.md).
 * An accessory then goes straight to what `_acc_main` does
 * (crtinit.c:112-149): a Malloc'ed stack of `_stksize` (MINKEEP for 0 and
 * -1), `_app = 0`, malloc through GEMDOS Malloc, argv = { "", NULL } -- no
 * parseargs, no Mshrink.
 *
 * `_base` (mintlib/globals.c:33) is what mintlib's m68k crt0 stores first
 * (startup/crt0.S:58,69) and what its readers dereference: crtinit.c:167,
 * unix/getpid.c:21, unix/getppid.c:22, gmon.c:201,237, TeraDesk's
 * `_BasPag->p_env` (environm.c).
 */

#include <stddef.h>
#include <stdlib.h>
#include <mint/basepage.h>
#include <mint/osbind.h>

/* mintlib/stksiz.h */
#define MINFREE	(8L * 1024L)		/* free at least this much mem on top */
#define MINKEEP (64L * 1024L)		/* keep at least this much mem on stack */

/* First byte past the image, BSS included (sdk/newtos.ld). */
extern char _end[];

/* mintlib: heapbase.c, stksiz.c, inistack.c, globals.c. */
extern void *_heapbase;
extern long _stksize;
extern long _initial_stack;
extern short _split_mem;
extern short _app;
extern unsigned long _PgmSize;
extern char **environ;

/* newtos-arch/cmdargs.c */
extern long __libc_parseargs (unsigned long basepage, char *area,
			      const char *limit);
extern char **__libc_argv;

/* crtinit.c:104 -- an accessory has no name and no arguments. */
static char *acc_argv[] = { "", NULL };

/* AAPCS64 wants a 16-byte aligned stack; mintlib rounds to 4 (m68k). */
#define STACK_ALIGN 15L

/* crtinit.c:112-149 `_acc_main`, up to the stack switch: returns the top of
   a Malloc'ed stack of `stksize` (the program's `_stksize`). */
static unsigned long
acc_main (long stksize)
{
	char *s;

	_stksize = stksize;
	if (_stksize == 0 || _stksize == -1L)
		_stksize = MINKEEP;
	if (_stksize < 0)
		_stksize = -_stksize;
	if ((s = getenv ("STACKSIZE")) != 0)
		_stksize = atoi (s);
	_stksize = (_stksize + STACK_ALIGN) & ~STACK_ALIGN;

	/* Not the application model: malloc from GEMDOS Malloc
	   (globals.c / heapbase.c defaults). */
	_split_mem = 0;
	_heapbase = NULL;

	s = (char *) Malloc (_stksize);
	if (s == NULL || (long) s < 0) {
		(void) Cconws ("Fatal error: insufficient memory\r\n");
		Pterm (-1);
	}

	/* this is an accessory */
	_app = 0;
	__libc_argv = acc_argv;
	environ = acc_argv;

	return ((unsigned long) s + (unsigned long) _stksize) & ~(unsigned long) STACK_ALIGN;
}

unsigned long
__libc_crtinit (unsigned long basepage)
{
	BASEPAGE *bp = (BASEPAGE *) basepage;
	long m, freemem;
	char *s;

	/* startup/crt0.S:52-58: `_base` first, for both kinds */
	_base = bp;

	/* startup/crt0.S:52-56: "test parent basepage pointer -- for accs, it
	   must be 0" (the a0 half is always true here, see the header) */
	if (bp->p_parent == 0)
		return acc_main (_stksize);

	/* its an application */
	_app = 1;

	/* m = # bytes used by environment + args (crtinit.c:167-168): parseargs
	   lays environ[], the split command line and argv[] out above the BSS,
	   from the 16-byte aligned `_end` (cmdargs.c "WHERE THE RESULT LIVES");
	   -1 = they do not fit below p_hitpa. */
	{
		unsigned long area = ((unsigned long) _end + STACK_ALIGN) & ~(unsigned long) STACK_ALIGN;
		long used = __libc_parseargs (basepage, (char *) area,
					      (const char *) (unsigned long) bp->p_hitpa);

		if (used < 0)
			goto notenough;
		/* m = the program sans stack/heap (crtinit.c:170-171) */
		m = (long) (area - basepage) + used;
	}
	m = (m + STACK_ALIGN) & ~STACK_ALIGN;

	/* freemem the amount of free mem accounting for MINFREE at top */
	if ((freemem = (long) bp->p_hitpa - (long) bp - MINFREE - m) <= 0L)
		goto notenough;

	if (_initial_stack) {
		_heapbase = (void *) ((long) bp + m);
		_stksize = _initial_stack;
	} else {
		if (_stksize >= -1L)
			/* malloc from Malloc first, then from own heap */
			_split_mem = 1;
	}

	if (_stksize == -1L) {
		_stksize = freemem;
		_heapbase = (void *) ((long) bp + m);
	} else if (_stksize == 0L) {	/* free all but MINKEEP */
		_stksize = MINKEEP;
	} else if (_stksize == 1L) {	/* keep 1/4, free 3/4 */
		_stksize = freemem >> 2;
	} else if (_stksize == 2L) {	/* keep 1/2, free 1/2 */
		_stksize = freemem >> 1;
	} else if (_stksize == 3L) {	/* keep 3/4, free 1/4 */
		_stksize = freemem - (freemem >> 2);
	} else {
		if (_stksize < -1L) {	/* keep |_stksize|, use heap for mallocs */
			_stksize = -_stksize;
			_heapbase = (void *) ((long) bp + m);
		}
	}

	if ((s = getenv ("STACKSIZE")) != 0)
		_stksize = atoi (s);

	/* make m the total number of bytes including stack */
	_stksize = _stksize & ~STACK_ALIGN;
	m += _stksize;

	/* make sure there's enough room for the stack */
	if (((long) bp + m) > ((long) bp->p_hitpa - MINFREE))
		goto notenough;

	/* shrink the TPA (crtinit.c:227-247; mintlib ignores the result) */
	(void) Mshrink (bp, m);

	/* keep length of program area */
	_PgmSize = m;

	/* the new stack: bp + m, minus mintlib's "some unused space for buggy
	   OS" (crtinit.c:236) */
	return (basepage + (unsigned long) m - 64) & ~(unsigned long) STACK_ALIGN;

notenough:
	(void) Cconws ("Fatal error: insufficient memory\r\n");
	Pterm (-1);
	return 0;	/* not reached */
}
