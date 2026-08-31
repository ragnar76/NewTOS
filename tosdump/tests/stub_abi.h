/*
 * stub_abi.h -- the host-side stub ABI backend (spec section 9, T2).
 *
 * T2's acceptance condition is a host-side test that drives the runner against
 * a stub backend.  The stub logs every call it is handed and answers with a
 * fixed, documented pattern, so the test can assert what the runner did
 * without a TT or a Pi in the room.
 *
 * It deliberately overwrites intin[0] and ptsin[0] on every ordinary VDI call.
 * Nothing in a real VDI is allowed to do that -- which is exactly why the
 * record dumps the input arrays after the call, and the stub proves the runner
 * really captures them afterwards rather than before.
 *
 * The AES side mirrors the dispatcher's marshalling rule rather than reading
 * and writing the caller's arrays directly (EmuTOS aes/gemsuper.c:428-448):
 * int_in is copied onto a local buffer, min(IN_LEN, I_SIZE) words of it; the
 * answer is built in a local int_out and exactly OUT_LEN words are copied
 * back; control[], int_in[], addr_in[] and addr_out[] are never written.
 * That is what lets a test assert that a case declaring fewer output words
 * than the call produces leaves the surplus slots poisoned.
 *
 * Two documented departures, both because a test double may not be
 * undefined:
 *   - xif's local int_in / int_out are UNINITIALISED stack; the stub fills
 *     them with TD_STUB_AES_UNWRITTEN instead, so "the handler never wrote
 *     this slot" is observable rather than being whatever the stack held.
 *     On a real target those slots are genuinely undefined, which is why a
 *     case table may not declare more output words than the call produces.
 *   - the copy-back is clamped to TD_AES_INTOUT_N.  The real one is not
 *     (gemsuper.c:444-445), so a larger OUT_LEN overruns the caller's array
 *     on the target; the case loader refuses such a table, and the stub
 *     refuses to reproduce a heap smash inside the test suite.
 *
 * The AES arm also writes a mark into global[] on every call.  A real AES
 * writes global only at appl_init (gemsuper.c:92-98), but global IS an array
 * the AES may write, it is recorded after every call, and it is the one field
 * the runner must NOT poison -- so a per-call mark is what proves the record
 * captured the live array after the call rather than a poisoned copy.
 */
#ifndef TOSDUMP_STUB_ABI_H
#define TOSDUMP_STUB_ABI_H

#include "record.h"

#define TD_STUB_KIND_VDI  0
#define TD_STUB_KIND_AES  1
#define TD_STUB_MAX_CALLS 4096

/* what the stub answers, so the test and the stub agree in one place */
#define TD_STUB_AP_ID        1
#define TD_STUB_PHYS_HANDLE  7
#define TD_STUB_VWK_HANDLE  42
#define TD_STUB_MIN_CHHT     7    /* work_out pts[1]  */
#define TD_STUB_MIN_LWID     1    /* work_out pts[4]  */
#define TD_STUB_MIN_MKHT    11    /* work_out pts[9]  */
#define TD_STUB_INTIN_MARK  0x5A5A
#define TD_STUB_PTSIN_MARK  0x5A5B
#define TD_STUB_PTSOUT_0    0x0111
#define TD_STUB_PTSOUT_1    0x0222
#define TD_STUB_N_PTSOUT     1
#define TD_STUB_N_INTOUT     2
/* what the stub leaves in the trap's return register, per call kind */
#define TD_STUB_VDI_D0       1    /* flip_y */
#define TD_STUB_AES_D0       0    /* super() returns a hard 0, gemsuper.c:465 */

/* AES side -------------------------------------------------------------- */
#define TD_STUB_AES_UNWRITTEN 0x5AE5  /* stand-in for xif's uninitialised local */
#define TD_STUB_GLOBAL_MARK   0x6E11  /* written into global[] on every call    */
#define TD_STUB_GLOBAL_MARK_AT (TD_AES_GLOBAL_N - 1)
#define TD_STUB_AP_VERSION   0x0340   /* what the stub's appl_init leaves in    */
#define TD_STUB_AP_COUNT          1   /* global[0..2] and global[10], modelled  */
#define TD_STUB_AP_PLANES         4   /* on gemsuper.c:92-98                    */
#define TD_STUB_WCHAR             8   /* graf_handle int_out[1..4]              */
#define TD_STUB_HCHAR            16
#define TD_STUB_WBOX              8
#define TD_STUB_HBOX             16
/* wind_calc: the stub answers int_out[1..4] = int_in[2..5] + these, so a
 * test can tell an output slot apart from the input it came from. */
#define TD_STUB_CALC_DX           1
#define TD_STUB_CALC_DY           2
#define TD_STUB_CALC_DW           3
#define TD_STUB_CALC_DH           4
/* wind_get: int_out[1..4] = handle, field, and these two */
#define TD_STUB_WGET_2       0x0303
#define TD_STUB_WGET_3       0x0404
/* the unsupported-opcode sentinel every AES returns (gemsuper.c:411-417) */
#define TD_STUB_AES_UNSUPPORTED (-1)

struct td_stub_call {
	int16_t kind;
	int16_t opcode;
	int16_t contrl1;   /* VDI: point count in ptsin        */
	int16_t contrl3;   /* VDI: value count in intin        */
	int16_t sub;       /* VDI: contrl[5]                   */
	int16_t handle;    /* VDI: contrl[6]                   */
	int16_t intin[2];  /* as handed in, before any mutation */
	int16_t ptsin[2];
	/* AES only: the whole control array and the whole int_in array as they
	 * stood when the call arrived.  Kept separate from the VDI fields above
	 * so neither side has to guess which shape a logged call has. */
	int16_t aes_control[TD_AES_CONTRL_N];
	int16_t aes_int_in[TD_AES_INTIN_N];
};

extern struct td_stub_call td_stub_calls[TD_STUB_MAX_CALLS];
extern int                 td_stub_ncalls;

void td_stub_reset(void);

#endif /* TOSDUMP_STUB_ABI_H */
