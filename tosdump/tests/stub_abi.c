/* stub_abi.c -- see stub_abi.h. */
#include <string.h>

#include "stub_abi.h"
#include "abi.h"

struct td_stub_call td_stub_calls[TD_STUB_MAX_CALLS];
int                 td_stub_ncalls;

void td_stub_reset(void)
{
	memset(td_stub_calls, 0, sizeof td_stub_calls);
	td_stub_ncalls = 0;
}

static struct td_stub_call *log_slot(void)
{
	static struct td_stub_call overflow;

	if (td_stub_ncalls < TD_STUB_MAX_CALLS)
		return &td_stub_calls[td_stub_ncalls++];
	td_stub_ncalls++;
	return &overflow;
}

void td_abi_vdi(int16_t *contrl, int16_t *intin, int16_t *ptsin,
                int16_t *intout, int16_t *ptsout)
{
	struct td_stub_call *c = log_slot();
	int i;

	c->kind    = TD_STUB_KIND_VDI;
	c->opcode  = contrl[0];
	c->contrl1 = contrl[1];
	c->contrl3 = contrl[3];
	c->sub     = contrl[5];
	c->handle  = contrl[6];
	c->intin[0] = intin[0];
	c->intin[1] = intin[1];
	c->ptsin[0] = ptsin[0];
	c->ptsin[1] = ptsin[1];

	switch (contrl[0]) {
	case 100:                       /* v_opnvwk */
		contrl[6] = TD_STUB_VWK_HANDLE;
		for (i = 0; i < 45; i++)
			intout[i] = 0;
		intout[0] = 639;
		intout[1] = 479;
		ptsout[0]  = 0;
		ptsout[1]  = TD_STUB_MIN_CHHT;
		ptsout[2]  = 0;
		ptsout[3]  = TD_STUB_MIN_CHHT;
		ptsout[4]  = TD_STUB_MIN_LWID;
		ptsout[5]  = 0;
		ptsout[6]  = 40;
		ptsout[7]  = 0;
		ptsout[8]  = 15;
		ptsout[9]  = TD_STUB_MIN_MKHT;
		ptsout[10] = 120;
		ptsout[11] = TD_STUB_MIN_MKHT * 8;
		contrl[2] = 6;
		contrl[4] = 45;
		break;
	case 101:                       /* v_clsvwk */
		contrl[2] = 0;
		contrl[4] = 0;
		break;
	default:
		contrl[2] = TD_STUB_N_PTSOUT;
		contrl[4] = TD_STUB_N_INTOUT;
		intout[0] = contrl[0];
		intout[1] = contrl[6];
		ptsout[0] = TD_STUB_PTSOUT_0;
		ptsout[1] = TD_STUB_PTSOUT_1;
		/* a real VDI may not do this -- see stub_abi.h */
		intin[0] = (int16_t)TD_STUB_INTIN_MARK;
		ptsin[0] = (int16_t)TD_STUB_PTSIN_MARK;
		break;
	}
}

void td_abi_aes(int16_t *control, int16_t *global, int16_t *int_in,
                int16_t *int_out, void **addr_in, void **addr_out)
{
	struct td_stub_call *c = log_slot();

	(void)global;
	(void)addr_in;
	(void)addr_out;

	c->kind      = TD_STUB_KIND_AES;
	c->opcode    = control[0];
	c->contrl1   = control[1];
	c->contrl3   = control[3];
	c->intin[0]  = int_in[0];
	c->intin[1]  = int_in[1];

	switch (control[0]) {
	case 10:                        /* appl_init */
		int_out[0] = TD_STUB_AP_ID;
		break;
	case 19:                        /* appl_exit */
		int_out[0] = 1;
		break;
	case 77:                        /* graf_handle */
		int_out[0] = TD_STUB_PHYS_HANDLE;
		int_out[1] = 8;
		int_out[2] = 16;
		int_out[3] = 8;
		int_out[4] = 16;
		break;
	default:
		int_out[0] = 0;
		break;
	}
}
