/*
 * doomgeneric_gem_kbdvec.h -- the IKBD keyboard vector hook shared between
 * doomgeneric_gem.c (installs it, reads what it recorded) and
 * doomgeneric_gem_kbdvec.c (the hook itself). TASK/KEYS02.
 *
 * Copyright (C) 2026 the NewTOS project. GPLv2 or later.
 *
 * The hook sits in the `kbdvec` slot right before the table Kbdvbase()
 * returns (TOS >= 2.0, EmuTOS bios/aciavecs.S:129 "must be right before
 * _kbdvecs"; NewTOS docs/api/xbios/22-kbdvbase.md B1/C1). It sees every IKBD
 * keyboard byte, make AND break (bit 7), puts it into kbdvec_ring and chains
 * to the previous vector, so the BIOS queue, Kbshift and the AES keep
 * working (22-kbdvbase.md A6.3).
 *
 * The ring is single-producer (the hook: an interrupt on a 680x0, an EL0
 * upcall at a syscall return on NewTOS) / single-consumer (DG_GetKey). The
 * hook writes the byte, then publishes it by advancing kbdvec_w; the reader
 * reads the byte, then frees it by advancing kbdvec_r. Both indices are one
 * byte, so each store is a single, indivisible access on both CPUs. A full
 * ring drops the newest byte, as the kernel's own ring does (D3a).
 */
#ifndef DOOMGENERIC_GEM_KBDVEC_H
#define DOOMGENERIC_GEM_KBDVEC_H

/*
 * THE ONE PLACE WHERE THE TWO ABIs DIFFER: the width of a vector slot.
 *
 *   680x0 TOS: every slot is a 4-byte pointer (`.ds.l`, aciavecs.S:126-141),
 *              so kbdvec is the long at Kbdvbase() - 4.
 *   NewTOS:    LP64, every slot is an 8-byte pointer (22-kbdvbase.md B1/C1:
 *              Kbdvbase() = 0x9B0, kbdvec at 0x9A8), kbdvec at Kbdvbase() - 8.
 *
 * In both cases a slot is exactly as wide as a pointer and as `unsigned
 * long` (ILP32 m68k: 4/4, LP64 AArch64: 8/8), so "the slot before the table"
 * is ((kbdslot_t *)Kbdvbase())[-1] on both, and the check below stops a
 * build for an ABI where that does not hold.
 */
typedef unsigned long kbdslot_t;
typedef char kbdslot_is_pointer_wide[sizeof(kbdslot_t) == sizeof(void *) ? 1 : -1];

#define KBDVEC_RING	64		/* power of two */

extern volatile unsigned char kbdvec_ring[KBDVEC_RING];
extern volatile unsigned char kbdvec_w;	/* written by the hook only */
extern volatile unsigned char kbdvec_r;	/* written by the reader only */

/* The value the slot held before the hook went in; the hook chains to it. */
extern volatile kbdslot_t kbdvec_old;
/* The slot the hook sits in (0 = not installed). */
extern kbdslot_t *volatile kbdvec_slot;

/* The hook. Never called from C: only its address goes into the slot.
 * NewTOS: an AAPCS64 function, x0 = byte, x1 = IOREC (0) (22-kbdvbase.md B2).
 * 680x0: register arguments d0.b / a0 (A4), so no C prototype describes it. */
#ifdef __m68k__
void kbdvec_hook(void);
#else
void kbdvec_hook(long scancode, long iorec);
#endif

#ifdef __m68k__
/*
 * 680x0 only: the etv_term handler (0x408, Setexc 0x102) that takes the hook
 * out again when the program ends without running its atexit() handlers
 * (a crash, a Pterm from somewhere else). TOS calls etv_term on every
 * process termination (EmuTOS bdos/proc.c:606-607) and never touches the
 * kbdvec slot itself (22-kbdvbase.md A8) -- without this a dead program's
 * hook would stay in the slot and the next key would jump into freed memory.
 * NewTOS needs none: the hook vanishes with the process (D8, 22-kbdvbase.md
 * C5).
 */
extern volatile kbdslot_t kbdvec_etv_old;
void kbdvec_etv_term(void);
#endif

#endif /* DOOMGENERIC_GEM_KBDVEC_H */
