/*
 * abi.h -- the ONLY thing that differs between the two builds.
 *
 * Spec section 2: "two thin ABI backends, and nothing else may differ ...
 * marshal registers, execute, return.  No logic."  If a difference in results
 * can be explained by the backends rather than by the API implementations,
 * the whole exercise is worthless.
 *
 * A backend implements exactly these two functions:
 *
 *   abi/m68k_trap.c    TOS 3.06 on the TT   -- trap #2, d0 = 115 / 200
 *   abi/newtos_svc.c   NewTOS on the Pi 4   -- svc #2, x0 = 115 / 200
 *
 * The arrays are the caller's; a backend builds the target's parameter block
 * around them, executes the trap and returns.  The parameter block layout is
 * the one place where the two targets genuinely differ (32-bit pointers on
 * m68k, 64-bit on AArch64), which is why it lives inside the backend.
 */
#ifndef TOSDUMP_ABI_H
#define TOSDUMP_ABI_H

#include <stdint.h>

/* contrl[] must already carry opcode, counts, sub-opcode and handle. */
void td_abi_vdi(int16_t *contrl, int16_t *intin, int16_t *ptsin,
                int16_t *intout, int16_t *ptsout);

/* control[] must already carry opcode and the four parameter counts. */
void td_abi_aes(int16_t *control, int16_t *global, int16_t *int_in,
                int16_t *int_out, void **addr_in, void **addr_out);

#endif /* TOSDUMP_ABI_H */
