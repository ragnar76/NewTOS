/*
 * doomgeneric_gem_kbdvec.c -- the IKBD keyboard vector hook of
 * doomgeneric_gem.c: records every keyboard make and break byte, then chains
 * to the previous kbdvec. TASK/KEYS02, spec docs/api/xbios/22-kbdvbase.md.
 *
 * Copyright (C) 2026 the NewTOS project. GPLv2 or later.
 *
 * A file of its own because the hook runs under conditions the rest of the
 * program does not:
 *
 * NewTOS / AArch64 (22-kbdvbase.md B2, C3)
 *   The hook is an AAPCS64 function  void hook(long scancode, long iorec):
 *   x0 = the IKBD byte (bit 7 = break), x1 = 0 (no IOREC, D2a). The kernel
 *   calls it as an EL0 upcall at a return from a syscall; the upcall saves
 *   the interrupted code's general registers, NOT its FP/SIMD registers. So
 *   the hook must not touch a single v/q/d/s/h/b register: this file is
 *   compiled with -mgeneral-regs-only (sdk/apps/build-doom.sh), and
 *   build-doom.sh checks the object with llvm-objdump for any of them.
 *   Chaining is a tail call to the old vector with x0/x1 unchanged.
 *
 * 680x0 TOS (22-kbdvbase.md A4, A6)
 *   ikbdraw jumps into kbdvec (an `rts` trick, aciavecs.S:407-414) from the
 *   ACIA interrupt, with d0.b = the byte and a0 = the keyboard IOREC, and the
 *   vector returns with `rts` straight to ikbdraw's caller. There is no
 *   specification of which registers a vector may change (aciavecs.S:250-252),
 *   so the hook changes none: it saves what it uses, records d0.b, restores,
 *   and chains by pushing the old vector and returning into it -- d0 and a0
 *   reach the old vector untouched, and its `rts` goes where ours would have
 *   (the same chaining EmuTOS' wheel_int uses, vdi/vdi_asm.S:476). Written
 *   in assembly, so no compiler decides about registers.
 */
#include "doomgeneric_gem_kbdvec.h"

volatile unsigned char kbdvec_ring[KBDVEC_RING];
volatile unsigned char kbdvec_w;
volatile unsigned char kbdvec_r;
volatile kbdslot_t kbdvec_old;
kbdslot_t *volatile kbdvec_slot;

#if defined(__aarch64__)

#if defined(__ARM_NEON) || defined(__ARM_FP)
#error "doomgeneric_gem_kbdvec.c must be compiled with -mgeneral-regs-only (the kbdvec upcall saves no FP/SIMD registers)"
#endif

typedef void (*kbdvec_fn)(long scancode, long iorec);

void kbdvec_hook(long scancode, long iorec)
{
	unsigned char w = kbdvec_w;
	unsigned char next = (unsigned char)((w + 1) & (KBDVEC_RING - 1));

	if (next != kbdvec_r) {
		kbdvec_ring[w] = (unsigned char)scancode;	/* the byte first */
		kbdvec_w = next;				/* then publish it */
	}
	((kbdvec_fn)kbdvec_old)(scancode, iorec);		/* chain (A6.3) */
}

#elif defined(__m68k__)

/*
 * The a.out symbols of the C objects above carry the `_` prefix
 * (__USER_LABEL_PREFIX__ of m68k-atari-mint-gcc).
 *
 * kbdvec_hook: in  d0.b = IKBD byte, a0 = IOREC; every register preserved.
 * kbdvec_etv_term: called by TOS from Pterm (supervisor mode); takes the hook
 *   out of its slot if it is still ours, gives etv_term back if it is still
 *   ours, then chains to the previous etv_term. Every register preserved.
 */
__asm__(
"	.text\n"
"	.even\n"
"	.globl	_kbdvec_hook\n"
"_kbdvec_hook:\n"
"	movem.l	%d1-%d2/%a1,-(%sp)\n"
"	moveq	#0,%d1\n"
"	move.b	_kbdvec_w,%d1\n"		/* d1 = write index           */
"	move.l	%d1,%d2\n"
"	addq.b	#1,%d2\n"
"	andi.b	#63,%d2\n"			/* d2 = next (KBDVEC_RING 64) */
"	cmp.b	_kbdvec_r,%d2\n"
"	beq.s	1f\n"				/* full: drop the newest      */
"	lea	_kbdvec_ring,%a1\n"
"	move.b	%d0,0(%a1,%d1.w)\n"		/* the byte first ...         */
"	move.b	%d2,_kbdvec_w\n"		/* ... then publish it        */
"1:	movem.l	(%sp)+,%d1-%d2/%a1\n"
"	move.l	_kbdvec_old,-(%sp)\n"		/* chain, d0/a0 untouched     */
"	rts\n"
"\n"
"	.globl	_kbdvec_etv_term\n"
"_kbdvec_etv_term:\n"
"	movem.l	%d0/%a0,-(%sp)\n"
"	move.l	_kbdvec_slot,%d0\n"
"	beq.s	2f\n"
"	move.l	%d0,%a0\n"
"	move.l	#_kbdvec_hook,%d0\n"
"	cmp.l	(%a0),%d0\n"
"	bne.s	2f\n"				/* someone hooked after us    */
"	move.l	_kbdvec_old,(%a0)\n"
"	clr.l	_kbdvec_slot\n"
"2:	move.l	#_kbdvec_etv_term,%d0\n"
"	cmp.l	0x408,%d0\n"
"	bne.s	3f\n"
"	move.l	_kbdvec_etv_old,0x408\n"
"3:	movem.l	(%sp)+,%d0/%a0\n"
"	tst.l	_kbdvec_etv_old\n"
"	beq.s	4f\n"
"	move.l	_kbdvec_etv_old,-(%sp)\n"	/* chain                      */
"4:	rts\n"
);

volatile kbdslot_t kbdvec_etv_old;

#else
#error "doomgeneric_gem_kbdvec.c: no kbdvec hook for this CPU"
#endif
