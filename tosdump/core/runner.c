/*
 * runner.c -- reset, poison, call, record (spec sections 4, 5 and 7).
 */
#include <string.h>

#include "abi.h"
#include "runner.h"

/* ------------------------------------------------------------------ */
/* Reset (spec section 5)                                              */
/* ------------------------------------------------------------------ */
/*
 * "Before every case, reset the state the call could depend on.  For VDI that
 * means at minimum v_clrwk plus re-applying the default attributes."
 *
 * The sequence below is that minimum, made explicit.  It is a fixed sequence
 * of calls, identical on both targets -- not an attempt to restore a
 * particular internal state.  That distinction matters: EmuTOS' vsf_style
 * stores fill_index one lower than init_wk does for the same request
 * (docs/api/vdi/_discrepancies.md D-VDI-8).  Compensating for that here would
 * bake an assumption about the implementation into the measurement, which is
 * precisely what a case in the table is for.
 *
 * The values are the attribute defaults docs/api/vdi/_state.md lists for a
 * workstation opened with the work_in gem.c uses (every index 1, raster
 * coordinates).  Three of them are device-defined and are therefore read back
 * from the SIZ_TAB that v_opnvwk itself returned in ptsout (EmuTOS
 * vdi_control.c:349-352, and vdi_defs.h:100-104 for which slot is which):
 *
 *   line width       DEF_LWID = SIZ_TAB[4] = work_pts[4]
 *   marker height    DEF_MKHT = SIZ_TAB[9] = work_pts[9]
 *   character height DEF_CHHT = SIZ_TAB[1] = work_pts[1]
 *
 * Not reset, and therefore not usable by a case unless the table restores it
 * itself: the user-defined fill pattern (vsf_udpat, 112, whose default is the
 * ROM pattern and cannot be restated portably) and the device palette
 * (vs_color, 14, which is device-global rather than per-workstation).
 */

static int16_t rs_contrl[TD_VDI_CONTRL_N];
static int16_t rs_intin[TD_VDI_INTIN_N];
static int16_t rs_ptsin[TD_VDI_PTSIN_N];
static int16_t rs_intout[TD_VDI_INTOUT_N];
static int16_t rs_ptsout[TD_VDI_PTSOUT_N];

static void rs_call(int16_t opcode, int16_t n_ptsin, int16_t n_intin,
                    int16_t handle)
{
	memset(rs_contrl, 0, sizeof rs_contrl);
	rs_contrl[0] = opcode;
	rs_contrl[1] = n_ptsin;
	rs_contrl[3] = n_intin;
	rs_contrl[5] = 0;
	rs_contrl[6] = handle;
	td_abi_vdi(rs_contrl, rs_intin, rs_ptsin, rs_intout, rs_ptsout);
}

static void rs_int1(int16_t opcode, int16_t v, int16_t handle)
{
	rs_intin[0] = v;
	rs_call(opcode, 0, 1, handle);
}

static void rs_int2(int16_t opcode, int16_t a, int16_t b, int16_t handle)
{
	rs_intin[0] = a;
	rs_intin[1] = b;
	rs_call(opcode, 0, 2, handle);
}

static void rs_pts(int16_t opcode, int16_t x, int16_t y, int16_t handle)
{
	rs_ptsin[0] = x;
	rs_ptsin[1] = y;
	rs_call(opcode, 1, 0, handle);
}

void td_reset_vdi(const struct td_wk *wk)
{
	int16_t h = wk->handle;

	rs_call(3, 0, 0, h);                       /* v_clrwk               */

	rs_int1(32, 1, h);                         /* vswr_mode  replace    */

	rs_int1(15, 1, h);                         /* vsl_type   solid      */
	rs_pts(16, wk->work_pts[4], 0, h);         /* vsl_width  DEF_LWID   */
	rs_int1(17, 1, h);                         /* vsl_color  pen 1      */
	rs_int2(108, 0, 0, h);                     /* vsl_ends   squared    */
	rs_int1(113, (int16_t)0xFFFF, h);          /* vsl_udsty  all ones   */

	rs_int1(18, 1, h);                         /* vsm_type   1          */
	rs_pts(19, 0, wk->work_pts[9], h);         /* vsm_height DEF_MKHT   */
	rs_int1(20, 1, h);                         /* vsm_color  pen 1      */

	rs_int1(23, 1, h);                         /* vsf_interior          */
	rs_int1(24, 1, h);                         /* vsf_style             */
	rs_int1(25, 1, h);                         /* vsf_color  pen 1      */
	rs_int1(104, 1, h);                        /* vsf_perimeter on      */

	rs_int1(21, 1, h);                         /* vst_font   1          */
	rs_pts(12, 0, wk->work_pts[1], h);         /* vst_height DEF_CHHT   */
	rs_int1(13, 0, h);                         /* vst_rotation 0        */
	rs_int1(22, 1, h);                         /* vst_color  pen 1      */
	rs_int1(106, 0, h);                        /* vst_effects none      */
	rs_int2(39, 0, 0, h);                      /* vst_alignment         */

	rs_int1(129, 0, h);                        /* vs_clip    off        */
}

/* ------------------------------------------------------------------ */
/* The run                                                             */
/* ------------------------------------------------------------------ */

static struct td_vdi_arrays run_arrays;
static int16_t              run_before[TD_VDI_CONTRL_N];

int td_run_vdi(const struct td_case_table *ct, const struct td_wk *wk,
               uint8_t *out, size_t out_bytes)
{
	uint16_t i;
	int32_t ret;
	int j;

	if (out_bytes < (size_t)ct->count * TD_VDI_RECORD_SIZE)
		return TD_RUN_ERR_BUFFER;

	for (i = 0; i < ct->count; i++) {
		const struct td_case *c = &ct->cases[i];
		uint8_t *rec = out + (size_t)i * TD_VDI_RECORD_SIZE;

		/* 1. reset the device state this case could otherwise inherit */
		td_reset_vdi(wk);

		/* 1b. the case's preludes: calls that establish the state this case
		 *     measures, issued after the reset so they survive it, and never
		 *     recorded.  Without them a setter case could only ever prove
		 *     its own echo -- see cases.h. */
		for (j = 0; j < (int)c->prelude_count; j++) {
			const struct td_prelude *p =
				&ct->preludes[c->prelude_first + (uint16_t)j];
			int k;

			memset(rs_contrl, 0, sizeof rs_contrl);
			for (k = 0; k < (int)p->n_intin; k++)
				rs_intin[k] = p->intin[k];
			for (k = 0; k < (int)p->n_ptsin; k++)
				rs_ptsin[k] = p->ptsin[k];
			rs_contrl[0] = p->opcode;
			rs_contrl[1] = (int16_t)(p->n_ptsin / 2);
			rs_contrl[3] = (int16_t)p->n_intin;
			rs_contrl[5] = p->sub;
			rs_contrl[6] = wk->handle;
			td_abi_vdi(rs_contrl, rs_intin, rs_ptsin, rs_intout, rs_ptsout);
		}

		/* 2. poison everything (spec section 4) */
		td_poison_vdi(&run_arrays);

		/* 3. lay the case values over the poison; whatever the case does
		 *    not fill stays 0xDEAD, which is what makes an unexpected
		 *    write visible */
		for (j = 0; j < (int)c->n_intin; j++)
			run_arrays.intin[j] = c->intin[j];
		for (j = 0; j < (int)c->n_ptsin; j++)
			run_arrays.ptsin[j] = c->ptsin[j];

		/* 4. contrl: only the input slots.  contrl[2] and contrl[4] are
		 *    outputs and stay poisoned, so a TOS that leaves them alone
		 *    stays distinguishable from one that writes 0 */
		run_arrays.contrl[0] = c->opcode;
		run_arrays.contrl[1] = (int16_t)(c->n_ptsin / 2);   /* points */
		run_arrays.contrl[3] = (int16_t)c->n_intin;
		run_arrays.contrl[5] = c->sub;
		run_arrays.contrl[6] = wk->handle;

		/* 5. snapshot contrl as it stands going in */
		for (j = 0; j < TD_VDI_CONTRL_N; j++)
			run_before[j] = run_arrays.contrl[j];

		/* 6. the call under measurement */
		ret = td_abi_vdi(run_arrays.contrl, run_arrays.intin,
		                 run_arrays.ptsin, run_arrays.intout,
		                 run_arrays.ptsout);

		/* 7. the record.  The input arrays go in as they stand AFTER the
		 *    call: a VDI routine is not allowed to modify them, and
		 *    dumping them afterwards is what makes a violation visible */
		td_vdi_record_write(rec, c->line, (uint16_t)c->opcode, ret,
		                    &run_arrays, run_before);
	}

	return 0;
}
