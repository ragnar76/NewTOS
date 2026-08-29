/*
 * test_runner.c -- drives the core runner against the stub backend (T2).
 *
 * The expected reset sequence is written out here rather than read from the
 * implementation: the test states the contract, the implementation has to meet
 * it.  Reading the sequence back out of core/ would assert only that the code
 * equals itself.
 */
#include "tdtest.h"
#include "cases.h"
#include "gem.h"
#include "runner.h"
#include "dump.h"
#include "stub_abi.h"

/*
 * Spec section 5: v_clrwk plus re-applying the default attributes.  The
 * defaults are the ones docs/api/vdi/_state.md lists for a workstation opened
 * with work_in = {1,1,1,1,1,1,1,1,1,1,2}; the three device-defined ones
 * (line width, marker height, character height) come from the SIZ_TAB values
 * v_opnvwk itself returned in ptsout (EmuTOS vdi_control.c:349-352).
 */
static const int16_t expected_reset[] = {
	  3,   /* v_clrwk                                  */
	 32,   /* vswr_mode      replace                   */
	 15,   /* vsl_type       solid                     */
	 16,   /* vsl_width      SIZ_TAB[4]                */
	 17,   /* vsl_color      pen 1                     */
	108,   /* vsl_ends       squared / squared         */
	113,   /* vsl_udsty      0xFFFF                    */
	 18,   /* vsm_type       1                         */
	 19,   /* vsm_height     SIZ_TAB[9]                */
	 20,   /* vsm_color      pen 1                     */
	 23,   /* vsf_interior   1                         */
	 24,   /* vsf_style      1                         */
	 25,   /* vsf_color      pen 1                     */
	104,   /* vsf_perimeter  on                        */
	 21,   /* vst_font       1                         */
	 12,   /* vst_height     SIZ_TAB[1]                */
	 13,   /* vst_rotation   0                         */
	 22,   /* vst_color      pen 1                     */
	106,   /* vst_effects    none                      */
	 39,   /* vst_alignment  left / baseline           */
	129    /* vs_clip        off                       */
};
#define RESET_N ((int)(sizeof expected_reset / sizeof expected_reset[0]))

static const char *table_text =
	"# two cases and a comment line\n"
	"6\t0\t-\t0,0,10,10\tv_pline two points\n"
	"32\t0\t1\t-\tvswr_mode replace\n"
	"11\t7\t-\t5,5,20,20\tGDP circle\n";

static void check_open(struct td_wk *wk)
{
	int rc, i;

	printf("workstation open: appl_init, graf_handle, v_opnvwk\n");

	td_stub_reset();
	rc = td_gem_open(wk);
	TD_CHECK_EQ(rc, 0);

	TD_CHECK_EQ(td_stub_ncalls, 3);
	TD_CHECK_EQ(td_stub_calls[0].kind, TD_STUB_KIND_AES);
	TD_CHECK_EQ(td_stub_calls[0].opcode, 10);          /* appl_init   */
	TD_CHECK_EQ(td_stub_calls[1].kind, TD_STUB_KIND_AES);
	TD_CHECK_EQ(td_stub_calls[1].opcode, 77);          /* graf_handle */
	TD_CHECK_EQ(td_stub_calls[2].kind, TD_STUB_KIND_VDI);
	TD_CHECK_EQ(td_stub_calls[2].opcode, 100);         /* v_opnvwk    */

	/* spec section 5: opened on the handle graf_handle returned */
	TD_CHECK_EQ(td_stub_calls[2].handle, TD_STUB_PHYS_HANDLE);
	TD_CHECK_EQ(td_stub_calls[2].contrl1, 0);          /* no ptsin    */
	TD_CHECK_EQ(td_stub_calls[2].contrl3, 11);         /* 11 intin    */

	TD_CHECK_EQ(wk->ap_id, TD_STUB_AP_ID);
	TD_CHECK_EQ(wk->phys_handle, TD_STUB_PHYS_HANDLE);
	TD_CHECK_EQ(wk->handle, TD_STUB_VWK_HANDLE);
	TD_CHECK_EQ(wk->work_pts[1], TD_STUB_MIN_CHHT);
	TD_CHECK_EQ(wk->work_pts[4], TD_STUB_MIN_LWID);
	TD_CHECK_EQ(wk->work_pts[9], TD_STUB_MIN_MKHT);
	TD_CHECK_EQ(wk->work_out[0], 639);

	/* the work_in every case is measured against, stated once */
	for (i = 0; i < 10; i++)
		TD_CHECK_EQ(wk->work_in[i], 1);
	TD_CHECK_EQ(wk->work_in[10], 2);   /* raster coordinates */
}

static void check_reset_block(int base, const struct td_wk *wk)
{
	int i;

	for (i = 0; i < RESET_N; i++) {
		const struct td_stub_call *c = &td_stub_calls[base + i];
		TD_CHECK_EQ(c->kind, TD_STUB_KIND_VDI);
		TD_CHECK_EQ(c->opcode, expected_reset[i]);
		TD_CHECK_EQ(c->handle, wk->handle);

		switch (c->opcode) {
		case  32: TD_CHECK_EQ(c->intin[0], 1); break;   /* replace       */
		case  15: TD_CHECK_EQ(c->intin[0], 1); break;   /* solid line    */
		case  16: TD_CHECK_EQ(c->ptsin[0], wk->work_pts[4]); break;
		case 108: TD_CHECK_EQ(c->intin[0], 0);
		          TD_CHECK_EQ(c->intin[1], 0); break;
		case 113: TD_CHECK_EQ((uint16_t)c->intin[0], 0xFFFFu); break;
		case  19: TD_CHECK_EQ(c->ptsin[1], wk->work_pts[9]); break;
		case  12: TD_CHECK_EQ(c->ptsin[1], wk->work_pts[1]); break;
		case 104: TD_CHECK_EQ(c->intin[0], 1); break;   /* perimeter on  */
		case 129: TD_CHECK_EQ(c->intin[0], 0); break;   /* clipping off  */
		default: break;
		}
	}
}

static void check_run(const struct td_wk *wk, const struct td_case_table *ct,
                      uint8_t *records)
{
	int rc, c;

	printf("runner: reset, poison, call, record -- per case\n");

	td_stub_reset();
	rc = td_run_vdi(ct, wk, records,
	                (size_t)ct->count * TD_VDI_RECORD_SIZE);
	TD_CHECK_EQ(rc, 0);

	/* every case costs one reset block plus the call under test */
	TD_CHECK_EQ(td_stub_ncalls, ct->count * (RESET_N + 1));

	for (c = 0; c < (int)ct->count; c++) {
		const struct td_case *cs = &ct->cases[c];
		const uint8_t *rec = records + (size_t)c * TD_VDI_RECORD_SIZE;
		int base = c * (RESET_N + 1);
		const struct td_stub_call *call;
		int i;

		check_reset_block(base, wk);

		call = &td_stub_calls[base + RESET_N];
		TD_CHECK_EQ(call->kind, TD_STUB_KIND_VDI);
		TD_CHECK_EQ(call->opcode, cs->opcode);
		TD_CHECK_EQ(call->contrl1, cs->n_ptsin / 2);   /* points, not values */
		TD_CHECK_EQ(call->contrl3, cs->n_intin);
		TD_CHECK_EQ(call->sub, cs->sub);
		TD_CHECK_EQ(call->handle, wk->handle);

		/* --- the record ------------------------------------------- */
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_CASEID), cs->line);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_OPCODE),
		            (uint16_t)cs->opcode);
		TD_CHECK_EQ(td_load_be32(rec + TD_VDI_OFF_RESERVED), 0uL);

		/* contrl as it stood immediately before the call */
		{
			const uint8_t *b = rec + TD_VDI_OFF_CONTRL_BEFORE;
			TD_CHECK_EQ(td_load_be16(b + 0 * 2), (uint16_t)cs->opcode);
			TD_CHECK_EQ(td_load_be16(b + 1 * 2), cs->n_ptsin / 2);
			TD_CHECK_EQ(td_load_be16(b + 3 * 2), cs->n_intin);
			TD_CHECK_EQ(td_load_be16(b + 5 * 2), (uint16_t)cs->sub);
			TD_CHECK_EQ(td_load_be16(b + 6 * 2), (uint16_t)wk->handle);
			/* the output slots are left poisoned so that a TOS which does
			 * not write them is distinguishable from one that writes 0 */
			TD_CHECK_EQ(td_load_be16(b + 2 * 2), 0xDEADu);
			TD_CHECK_EQ(td_load_be16(b + 4 * 2), 0xDEADu);
			TD_CHECK_EQ(td_load_be16(b + 7 * 2), 0xDEADu);
			TD_CHECK_EQ(td_load_be16(b + 11 * 2), 0xDEADu);
		}

		/* contrl after the call, as the backend left it */
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_CONTRL + 2 * 2),
		            TD_STUB_N_PTSOUT);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_CONTRL + 4 * 2),
		            TD_STUB_N_INTOUT);

		/* the input arrays are dumped AFTER the call: the stub overwrote
		 * intin[0] and ptsin[0], and the record has to show that */
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTIN),
		            (uint16_t)TD_STUB_INTIN_MARK);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_PTSIN),
		            (uint16_t)TD_STUB_PTSIN_MARK);

		/* values the case supplied, beyond the one the stub clobbered */
		for (i = 1; i < (int)cs->n_intin; i++)
			TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTIN + i * 2),
			            (uint16_t)cs->intin[i]);
		for (i = 1; i < (int)cs->n_ptsin; i++)
			TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_PTSIN + i * 2),
			            (uint16_t)cs->ptsin[i]);

		/* everything the case did not fill stays poisoned */
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTIN +
		                         (TD_VDI_INTIN_N - 1) * 2), 0xDEADu);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_PTSIN +
		                         (TD_VDI_PTSIN_N - 1) * 2), 0xDEADu);
		if (cs->n_intin < TD_VDI_INTIN_N)
			TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTIN +
			                         cs->n_intin * 2), 0xDEADu);

		/* outputs the backend wrote, and the poison behind them */
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTOUT),
		            (uint16_t)cs->opcode);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTOUT + 2),
		            (uint16_t)wk->handle);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_INTOUT + 4), 0xDEADu);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_PTSOUT), TD_STUB_PTSOUT_0);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_PTSOUT + 2),
		            TD_STUB_PTSOUT_1);
		TD_CHECK_EQ(td_load_be16(rec + TD_VDI_OFF_PTSOUT + 4), 0xDEADu);
	}
}

static void check_too_small(const struct td_wk *wk,
                            const struct td_case_table *ct, uint8_t *records)
{
	printf("runner: refuses a buffer that cannot hold every record\n");
	td_stub_reset();
	TD_CHECK(td_run_vdi(ct, wk, records,
	                    (size_t)ct->count * TD_VDI_RECORD_SIZE - 1) != 0);
	TD_CHECK_EQ(td_stub_ncalls, 0);
}

int main(int argc, char **argv)
{
	static struct td_case_table ct;
	static uint8_t records[8 * TD_VDI_RECORD_SIZE];
	struct td_case_error err;
	struct td_wk wk;
	int rc;

	memset(&err, 0, sizeof err);
	rc = td_cases_parse(&ct, table_text, strlen(table_text), &err);
	TD_CHECK_EQ(rc, 0);
	TD_CHECK_EQ(ct.count, 3);

	check_open(&wk);
	check_run(&wk, &ct, records);
	check_too_small(&wk, &ct, records);

	/* T2 deliverable: a well-formed dump file, written by the same code the
	 * target uses, which tosdiff --validate then has to accept. */
	if (argc == 2) {
		printf("runner: writing %s\n", argv[1]);
		td_stub_reset();
		TD_CHECK_EQ(td_run_vdi(&ct, &wk, records,
		                       (size_t)ct.count * TD_VDI_RECORD_SIZE), 0);
		TD_CHECK_EQ(td_dump_write(argv[1], TD_AREA_VDI, ct.count, "stub",
		                          ct.crc, records,
		                          (size_t)ct.count * TD_VDI_RECORD_SIZE), 0);
	}

	td_gem_close(&wk);
	return TD_TEST_RESULT();
}
