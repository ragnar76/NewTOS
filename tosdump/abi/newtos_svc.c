/*
 * newtos_svc.c -- ABI backend for NewTOS on the Raspberry Pi 4 (AArch64).
 *
 * Spec section 2: marshal, execute, return.  No logic.  The symmetric
 * counterpart of abi/m68k_trap.c -- same two functions, same parameter blocks,
 * only the instruction and the pointer width differ.  Section 8 rule 5: any
 * logic that appears in one backend and not the other is a bug, however well
 * justified it looks, so nothing beyond the block setup lives here either.
 *
 * The register ABI is the one the kernel dispatcher defines, taken from the
 * already-retargeted gemlib primitives rather than re-derived:
 *
 *   sdk/gemlib/overlay/_gc_vdi.c   x0 = 115, x1 = &VDIPB, svc #2
 *   sdk/gemlib/overlay/_gc_aes.c   x0 = 200, x1 = &AESPB, svc #2
 *
 * verified there against kernel/src/arch/aarch64/exceptions.rs (the GEM branch
 * of handle_svc dispatches on x0 and passes x1) and tos-api/src/vdi/mod.rs /
 * aes/mod.rs (Vdipb::from_raw / Aespb::from_raw read consecutive native
 * pointers, i.e. the LP64 struct layout below).  The SVC entry preserves
 * x1..x29 and only overwrites x0, so this clobbers x0 and memory and nothing
 * else, and x0 on return is what the kernel left there -- the counterpart of
 * the m68k trap's d0.  Whether NewTOS produces a flip_y equivalent at all is
 * an open question in docs/api/vdi/_working-list.md (D-080); the dumper does
 * not assume an answer, it records what is there.
 */
#include <stdint.h>

#include "abi.h"

#ifndef __aarch64__
# error "newtos_svc.c is the NewTOS backend; build it for aarch64"
#endif

/* The classic parameter blocks.  Same fields as on the m68k; the pointers are
 * 64-bit here, which is the one genuine difference between the two builds. */
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
	register long x0 __asm__("x0") = 115;      /* VDI selector */
	register long x1 __asm__("x1");

	pb.contrl = contrl;
	pb.intin  = intin;
	pb.ptsin  = ptsin;
	pb.intout = intout;
	pb.ptsout = ptsout;
	x1 = (long)&pb;

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)
		: "r"(x1)
		: "memory", "cc"
	);
	return (int32_t)x0;
}

int32_t td_abi_aes(int16_t *control, int16_t *global, int16_t *int_in,
                   int16_t *int_out, void **addr_in, void **addr_out)
{
	AESPB pb;
	register long x0 __asm__("x0") = 200;      /* AES selector */
	register long x1 __asm__("x1");

	pb.control  = control;
	pb.global   = global;
	pb.int_in   = int_in;
	pb.int_out  = int_out;
	pb.addr_in  = addr_in;
	pb.addr_out = addr_out;
	x1 = (long)&pb;

	__asm__ __volatile__ (
		"svc	#2"
		: "+r"(x0)
		: "r"(x1)
		: "memory", "cc"
	);
	return (int32_t)x0;
}
