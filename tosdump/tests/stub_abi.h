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

struct td_stub_call {
	int16_t kind;
	int16_t opcode;
	int16_t contrl1;   /* VDI: point count in ptsin        */
	int16_t contrl3;   /* VDI: value count in intin        */
	int16_t sub;       /* VDI: contrl[5]                   */
	int16_t handle;    /* VDI: contrl[6]                   */
	int16_t intin[2];  /* as handed in, before any mutation */
	int16_t ptsin[2];
};

extern struct td_stub_call td_stub_calls[TD_STUB_MAX_CALLS];
extern int                 td_stub_ncalls;

void td_stub_reset(void);

#endif /* TOSDUMP_STUB_ABI_H */
