/*
 * m68k_trap.c -- ABI backend for TOS 3.06 on the TT.
 *
 * Spec section 2: marshal, execute, return.  No logic.  Everything this file
 * does is build the classic parameter block around the caller's arrays and
 * issue trap #2.
 *
 * The trap sequences are the ones gemlib issues on the m68k, taken from the
 * pristine tree rather than written from memory:
 *
 *   VDI  gem_vdiP.h:47-62   d1 = &VDIPB, d0 = 115, trap #2
 *   AES  gem_aesP.h:38-45   d1 = &AESPB, d0 = 200, trap #2
 *
 * with the same clobber list (d0/d1/d2, a0/a1/a2, memory, cc).  gemlib also
 * fills contrl[] inside its trap macro; this backend must not -- contrl is
 * already laid out by core/, and duplicating that here would be logic in a
 * place where the two builds are required to agree.
 */
#include <stdint.h>

#include "abi.h"

#if !defined(__mc68000__) && !defined(__M68K__)
# error "m68k_trap.c is the TOS 3.06 backend; build it with m68k-atari-mint-gcc"
#endif

/* The classic parameter blocks: five and six 32-bit pointers. */
typedef struct {
	int16_t *contrl;
	int16_t *intin;
	int16_t *ptsin;
	int16_t *intout;
	int16_t *ptsout;
} VDIPB;

typedef struct {
	int16_t *control;
	int16_t *global;
	int16_t *int_in;
	int16_t *int_out;
	void   **addr_in;
	void   **addr_out;
} AESPB;

int32_t td_abi_vdi(int16_t *contrl, int16_t *intin, int16_t *ptsin,
                   int16_t *intout, int16_t *ptsout)
{
	VDIPB pb;
	long ret;

	pb.contrl = contrl;
	pb.intin  = intin;
	pb.ptsin  = ptsin;
	pb.intout = intout;
	pb.ptsout = ptsout;

	/* d0 on return is flip_y (vdi_asm.S:124-125); it is an output of the
	 * call, so it is moved out before the clobber takes effect. */
	__asm__ volatile (
		"move.l %1,%%d1\n\t"      /* &VDIPB          */
		"move.w #115,%%d0\n\t"    /* VDI selector    */
		"trap   #2\n\t"
		"move.l %%d0,%0"
		: "=d"(ret)
		: "g"(&pb)
		: "d0", "d1", "d2", "a0", "a1", "a2", "memory", "cc"
	);
	return (int32_t)ret;
}

int32_t td_abi_aes(int16_t *control, int16_t *global, int16_t *int_in,
                   int16_t *int_out, void **addr_in, void **addr_out)
{
	AESPB pb;
	long ret;

	pb.control  = control;
	pb.global   = global;
	pb.int_in   = int_in;
	pb.int_out  = int_out;
	pb.addr_in  = addr_in;
	pb.addr_out = addr_out;

	__asm__ volatile (
		"move.l %1,%%d1\n\t"      /* &AESPB          */
		"move.w #200,%%d0\n\t"    /* AES selector    */
		"trap   #2\n\t"
		"move.l %%d0,%0"
		: "=d"(ret)
		: "g"(&pb)
		: "d0", "d1", "d2", "a0", "a1", "a2", "memory", "cc"
	);
	return (int32_t)ret;
}
