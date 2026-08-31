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

int32_t td_abi_vdi(int16_t *contrl, int16_t *intin, int16_t *ptsin,
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
	return TD_STUB_VDI_D0;
}

int32_t td_abi_aes(int16_t *control, int16_t *global, int16_t *int_in,
                   int16_t *int_out, void **addr_in, void **addr_out)
{
	struct td_stub_call *c = log_slot();
	int16_t local_in[TD_AES_INTIN_N];
	int16_t local_out[TD_AES_INTOUT_N];
	int16_t in_len, out_len;
	int i, n;

	(void)addr_in;
	(void)addr_out;

	c->kind      = TD_STUB_KIND_AES;
	c->opcode    = control[0];
	c->contrl1   = control[1];
	c->contrl3   = control[3];
	c->intin[0]  = int_in[0];
	c->intin[1]  = int_in[1];
	for (i = 0; i < TD_AES_CONTRL_N; i++)
		c->aes_control[i] = control[i];
	for (i = 0; i < TD_AES_INTIN_N; i++)
		c->aes_int_in[i] = int_in[i];

	/* --- marshal in (gemsuper.c:435-439) ---------------------------- */
	for (i = 0; i < TD_AES_INTIN_N; i++)
		local_in[i] = (int16_t)TD_STUB_AES_UNWRITTEN;
	in_len = control[1];
	if (in_len > 0) {
		n = (in_len < TD_AES_INTIN_N) ? in_len : TD_AES_INTIN_N;
		for (i = 0; i < n; i++)
			local_in[i] = int_in[i];
	}
	for (i = 0; i < TD_AES_INTOUT_N; i++)
		local_out[i] = (int16_t)TD_STUB_AES_UNWRITTEN;

	/* --- the "handler" ---------------------------------------------- */
	/* crysbind starts with ret = TRUE, so a void handler leaves 1
	 * (gemsuper.c:85, docs/api/aes/_dispatcher.md section 2). */
	local_out[0] = 1;
	switch (control[0]) {
	case 10:                        /* appl_init */
		local_out[0] = TD_STUB_AP_ID;
		global[0] = TD_STUB_AP_VERSION;
		global[1] = TD_STUB_AP_COUNT;
		global[2] = TD_STUB_AP_ID;
		global[10] = TD_STUB_AP_PLANES;
		break;
	case 19:                        /* appl_exit */
		local_out[0] = 1;
		break;
	case 24:                        /* evnt_timer -- void handler */
		break;
	case 77:                        /* graf_handle */
		local_out[0] = TD_STUB_PHYS_HANDLE;
		local_out[1] = TD_STUB_WCHAR;
		local_out[2] = TD_STUB_HCHAR;
		local_out[3] = TD_STUB_WBOX;
		local_out[4] = TD_STUB_HBOX;
		break;
	case 79:                        /* graf_mkstate -- void handler */
		break;
	case 104:                       /* wind_get */
		local_out[1] = local_in[0];         /* handle */
		local_out[2] = local_in[1];         /* field  */
		local_out[3] = TD_STUB_WGET_2;
		local_out[4] = TD_STUB_WGET_3;
		break;
	case 106:                       /* wind_find */
		local_out[0] = (int16_t)(local_in[0] + local_in[1]);
		break;
	case 108:                       /* wind_calc -- void handler */
		local_out[1] = (int16_t)(local_in[2] + TD_STUB_CALC_DX);
		local_out[2] = (int16_t)(local_in[3] + TD_STUB_CALC_DY);
		local_out[3] = (int16_t)(local_in[4] + TD_STUB_CALC_DW);
		local_out[4] = (int16_t)(local_in[5] + TD_STUB_CALC_DH);
		break;
	default:                        /* unsupported opcode, incl. 0 */
		local_out[0] = TD_STUB_AES_UNSUPPORTED;
		break;
	}

	/* the mark that proves the record captured global AFTER the call */
	global[TD_STUB_GLOBAL_MARK_AT] = (int16_t)TD_STUB_GLOBAL_MARK;

	/* --- marshal out (gemsuper.c:444-445), clamped: see stub_abi.h --- */
	out_len = control[2];
	if (out_len > 0) {
		n = (out_len < TD_AES_INTOUT_N) ? out_len : TD_AES_INTOUT_N;
		for (i = 0; i < n; i++)
			int_out[i] = local_out[i];
	}

	return TD_STUB_AES_D0;
}
