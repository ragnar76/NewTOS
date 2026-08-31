/*
 * test_runner_aes.c -- drives the AES runner against the stub backend.
 *
 * The counterpart of test_runner.c.  The contract is written out here rather
 * than read back out of core/: the test states what a case does to the
 * arrays, the implementation has to meet it.
 *
 * What the AES runner owes, and none of it is the VDI runner's contract:
 *   - NO reset call between cases (there is no documented AES default state
 *     to restore -- see core/runner_aes.h), so exactly one trap per case
 *     plus whatever preludes the case carries;
 *   - all FIVE control slots laid from the case, none left poisoned: in the
 *     AES every control slot is an input (EmuTOS aes/gemsuper.c:435 copies
 *     control wholesale and never writes it back);
 *   - int_in laid over the poison, so the slots a case does not fill stay
 *     0xDEAD and reach the dispatcher as 0xDEAD;
 *   - exactly OUT_LEN words of int_out delivered, the rest still 0xDEAD;
 *   - global[] never poisoned, handed in live, recorded after the call;
 *   - addr_in / addr_out poisoned and never written (round 1 is
 *     pointer-free).
 */
#include "tdtest.h"
#include "cases.h"
#include "dump.h"
#include "gem.h"
#include "runner_aes.h"
#include "stub_abi.h"

#define REC_AT(recs, i) ((recs) + (size_t)(i) * TD_AES_RECORD_SIZE)

static uint16_t out_w(const uint8_t *rec, int i)
{
	return td_load_be16(rec + TD_AES_OFF_INTOUT + i * 2);
}

static uint16_t in_w(const uint8_t *rec, int i)
{
	return td_load_be16(rec + TD_AES_OFF_INTIN + i * 2);
}

static uint16_t ctl_w(const uint8_t *rec, int i)
{
	return td_load_be16(rec + TD_AES_OFF_CONTRL + i * 2);
}

static uint16_t glob_w(const uint8_t *rec, int i)
{
	return td_load_be16(rec + TD_AES_OFF_GLOBAL + i * 2);
}

/* ------------------------------------------------------------------ */

/*
 * The table the main run uses.  Three cases, chosen so that between them
 * they exercise every branch of the laying rule: a full six-value call, a
 * call with no values at all, and a call whose declared counts are
 * deliberately not the counts its values would imply.
 */
static const char *table_text =
	"# an AES case table\n"
	"108\t6\t5\t0\t0\t1,0x0FEF,100,100,200,150\twind_calc WC_WORK\n"
	"77\t0\t5\t0\t0\t-\tgraf_handle\n"
	"108\t20\t3\t3\t1\t1,0,10,20,30,40\tmarshalling: IN_LEN past I_SIZE\n";

static struct td_aes_case_table ct;
static uint8_t records[8 * TD_AES_RECORD_SIZE];

static void check_open(struct td_wk *wk)
{
	const int16_t *g;

	printf("aes runner: the application context the cases are measured in\n");

	td_stub_reset();
	TD_CHECK_EQ(td_gem_open(wk), 0);
	TD_CHECK_EQ(wk->ap_id, TD_STUB_AP_ID);

	/* global[] is the live AESGLOBAL appl_init filled, and there is exactly
	 * one of it for the whole run (EmuTOS gemsuper.c:92-98) */
	g = td_gem_global();
	TD_CHECK(g != NULL);
	TD_CHECK_EQ((uint16_t)g[0], TD_STUB_AP_VERSION);
	TD_CHECK_EQ(g[1], TD_STUB_AP_COUNT);
	TD_CHECK_EQ(g[2], TD_STUB_AP_ID);
	TD_CHECK_EQ(g[10], TD_STUB_AP_PLANES);
}

static void check_run(struct td_wk *wk)
{
	int rc, c, i;

	printf("aes runner: poison, lay, call, record -- one trap per case\n");

	td_stub_reset();
	rc = td_run_aes(&ct, td_gem_global(), records,
	                (size_t)ct.count * TD_AES_RECORD_SIZE);
	TD_CHECK_EQ(rc, 0);

	/* no reset block: one call per case and not a single call more */
	TD_CHECK_EQ(td_stub_ncalls, (int)ct.count);

	for (c = 0; c < (int)ct.count; c++) {
		const struct td_aes_case *cs = &ct.cases[c];
		const struct td_stub_call *call = &td_stub_calls[c];
		const uint8_t *rec = REC_AT(records, c);

		TD_CHECK_EQ(call->kind, TD_STUB_KIND_AES);
		TD_CHECK_EQ(call->opcode, cs->opcode);

		/* --- all five control slots are inputs, all five are laid --- */
		TD_CHECK_EQ(call->aes_control[0], cs->opcode);
		TD_CHECK_EQ(call->aes_control[1], cs->n_intin);
		TD_CHECK_EQ(call->aes_control[2], cs->n_intout);
		TD_CHECK_EQ(call->aes_control[3], cs->n_addrin);
		TD_CHECK_EQ(call->aes_control[4], cs->n_addrout);

		/* --- int_in: the case's values, poison behind them --------- */
		for (i = 0; i < (int)cs->n_values; i++)
			TD_CHECK_EQ(call->aes_int_in[i], cs->intin[i]);
		for (i = (int)cs->n_values; i < TD_AES_INTIN_N; i++)
			TD_CHECK_EQ((uint16_t)call->aes_int_in[i], 0xDEADu);

		/* --- the record ------------------------------------------- */
		TD_CHECK_EQ(td_load_be16(rec + TD_AES_OFF_CASEID), cs->line);
		TD_CHECK_EQ(td_load_be16(rec + TD_AES_OFF_OPCODE),
		            (uint16_t)cs->opcode);
		/* super() returns a hard 0 for selector 200 (gemsuper.c:465), so
		 * a non-zero d0 here would be a finding, not noise */
		TD_CHECK_EQ((int32_t)td_load_be32(rec + TD_AES_OFF_RET),
		            TD_STUB_AES_D0);

		/* control as it stands after the call: the AES copies it onto a
		 * local buffer and never writes it back, so this is also the
		 * "before" state, and a written slot is a difference */
		TD_CHECK_EQ(ctl_w(rec, 0), (uint16_t)cs->opcode);
		TD_CHECK_EQ(ctl_w(rec, 1), (uint16_t)cs->n_intin);
		TD_CHECK_EQ(ctl_w(rec, 2), (uint16_t)cs->n_intout);
		TD_CHECK_EQ(ctl_w(rec, 3), (uint16_t)cs->n_addrin);
		TD_CHECK_EQ(ctl_w(rec, 4), (uint16_t)cs->n_addrout);

		/* int_in dumped after the call; nothing may have touched it */
		for (i = 0; i < (int)cs->n_values; i++)
			TD_CHECK_EQ(in_w(rec, i), (uint16_t)cs->intin[i]);
		for (i = (int)cs->n_values; i < TD_AES_INTIN_N; i++)
			TD_CHECK_EQ(in_w(rec, i), 0xDEADu);

		/* exactly OUT_LEN words delivered, the rest still poisoned */
		for (i = (int)cs->n_intout; i < TD_AES_INTOUT_N; i++)
			TD_CHECK_EQ(out_w(rec, i), 0xDEADu);

		/* global recorded after the call: live, marked, never poison */
		TD_CHECK_EQ(glob_w(rec, 0), TD_STUB_AP_VERSION);
		TD_CHECK_EQ(glob_w(rec, 2), TD_STUB_AP_ID);
		TD_CHECK_EQ(glob_w(rec, TD_STUB_GLOBAL_MARK_AT),
		            TD_STUB_GLOBAL_MARK);
		for (i = 0; i < TD_AES_GLOBAL_N; i++)
			TD_CHECK(glob_w(rec, i) != 0xDEADu);

		/* round 1 is pointer-free: the address arrays stay poisoned */
		for (i = 0; i < TD_AES_ADDRIN_N; i++)
			TD_CHECK_EQ(td_load_be32(rec + TD_AES_OFF_ADDRIN + i * 4),
			            0xDEADDEADuL);
		for (i = 0; i < TD_AES_ADDROUT_N; i++)
			TD_CHECK_EQ(td_load_be32(rec + TD_AES_OFF_ADDROUT + i * 4),
			            0xDEADDEADuL);
	}

	/* --- case 1: wind_calc, everything declared honestly ------------- */
	{
		const uint8_t *rec = REC_AT(records, 0);
		TD_CHECK_EQ(out_w(rec, 0), 1);                    /* void -> TRUE */
		TD_CHECK_EQ(out_w(rec, 1), 100 + TD_STUB_CALC_DX);
		TD_CHECK_EQ(out_w(rec, 2), 100 + TD_STUB_CALC_DY);
		TD_CHECK_EQ(out_w(rec, 3), 200 + TD_STUB_CALC_DW);
		TD_CHECK_EQ(out_w(rec, 4), 150 + TD_STUB_CALC_DH);
	}

	/* --- case 2: graf_handle, no int_in values at all ---------------- */
	{
		const uint8_t *rec = REC_AT(records, 1);
		TD_CHECK_EQ(out_w(rec, 0), TD_STUB_PHYS_HANDLE);
		TD_CHECK_EQ(out_w(rec, 1), TD_STUB_WCHAR);
		TD_CHECK_EQ(out_w(rec, 4), TD_STUB_HBOX);
		TD_CHECK_EQ(in_w(rec, 0), 0xDEADu);
	}

	/*
	 * --- case 3: the marshalling case -------------------------------
	 * IN_LEN 20 is past I_SIZE, so the dispatcher clamps to 16 and the
	 * six listed values still arrive.  OUT_LEN 3 is short of the five
	 * words wind_calc produces, so int_out[3] and [4] must still be
	 * poisoned -- the surplus was never copied back.
	 */
	{
		const uint8_t *rec = REC_AT(records, 2);
		TD_CHECK_EQ(out_w(rec, 0), 1);
		TD_CHECK_EQ(out_w(rec, 1), 10 + TD_STUB_CALC_DX);
		TD_CHECK_EQ(out_w(rec, 2), 20 + TD_STUB_CALC_DY);
		TD_CHECK_EQ(out_w(rec, 3), 0xDEADu);
		TD_CHECK_EQ(out_w(rec, 4), 0xDEADu);
		TD_CHECK_EQ(ctl_w(rec, 3), 3);      /* AIN_LEN declared ... */
		TD_CHECK_EQ(ctl_w(rec, 4), 1);      /* ... and control[4] too */
	}
}

/*
 * OUT_LEN 0 is the sharpest case in the whole area: the dispatcher copies
 * nothing back at all, so not even the return code reaches the caller and
 * int_out[0] stays poisoned (EmuTOS gemsuper.c:444-445, and
 * docs/api/aes/_dispatcher.md section 3a: "a caller must declare OUT_LEN >= 1
 * to observe the -1").
 */
static void check_out_len_zero(void)
{
	static struct td_aes_case_table t;
	static uint8_t recs[2 * TD_AES_RECORD_SIZE];
	static const char *text =
		"108\t6\t0\t0\t0\t1,0x0FEF,100,100,200,150\tOUT_LEN 0\n"
		"0\t0\t1\t0\t0\t-\topcode 0, the only safe unknown opcode\n";
	struct td_case_error err;
	int i;

	printf("aes runner: OUT_LEN 0 delivers nothing, not even the return code\n");

	memset(&err, 0, sizeof err);
	TD_CHECK_EQ(td_aes_cases_parse(&t, text, strlen(text), &err), 0);
	TD_CHECK_EQ(t.count, 2);

	td_stub_reset();
	TD_CHECK_EQ(td_run_aes(&t, td_gem_global(), recs, sizeof recs), 0);
	TD_CHECK_EQ(td_stub_ncalls, 2);

	for (i = 0; i < TD_AES_INTOUT_N; i++)
		TD_CHECK_EQ(out_w(REC_AT(recs, 0), i), 0xDEADu);

	/* opcode 0 with OUT_LEN 1 does deliver the -1 sentinel */
	TD_CHECK_EQ((int16_t)out_w(REC_AT(recs, 1), 0), TD_STUB_AES_UNSUPPORTED);
	TD_CHECK_EQ(out_w(REC_AT(recs, 1), 1), 0xDEADu);
}

/*
 * A prelude runs before the measured call and is never recorded.  Round 1
 * has no reset, so a prelude is simply an unrecorded call that establishes
 * what the case measures; the mechanism has to be there and has to be the
 * same one the VDI table uses, or the host reader and the runner would
 * disagree about which physical line carries a case id.
 */
static void check_prelude(void)
{
	static struct td_aes_case_table t;
	static uint8_t recs[2 * TD_AES_RECORD_SIZE];
	static const char *text =
		"+108\t6\t5\t0\t0\t1,0,1,2,3,4\ta prelude call\n"
		"77\t0\t5\t0\t0\t-\tthe measured call\n";
	struct td_case_error err;

	printf("aes runner: a prelude runs before the case and is not recorded\n");

	memset(&err, 0, sizeof err);
	TD_CHECK_EQ(td_aes_cases_parse(&t, text, strlen(text), &err), 0);
	TD_CHECK_EQ(t.count, 1);

	td_stub_reset();
	TD_CHECK_EQ(td_run_aes(&t, td_gem_global(), recs, sizeof recs), 0);

	TD_CHECK_EQ(td_stub_ncalls, 2);
	TD_CHECK_EQ(td_stub_calls[0].opcode, 108);
	TD_CHECK_EQ(td_stub_calls[0].aes_control[1], 6);
	TD_CHECK_EQ(td_stub_calls[0].aes_int_in[2], 1);
	TD_CHECK_EQ(td_stub_calls[1].opcode, 77);

	/* one record, and it belongs to the case, not to the prelude */
	TD_CHECK_EQ(td_load_be16(REC_AT(recs, 0) + TD_AES_OFF_OPCODE), 77);
	TD_CHECK_EQ(td_load_be16(REC_AT(recs, 0) + TD_AES_OFF_CASEID), 2);
}

static void check_too_small(void)
{
	printf("aes runner: refuses a buffer that cannot hold every record\n");
	td_stub_reset();
	TD_CHECK(td_run_aes(&ct, td_gem_global(), records,
	                    (size_t)ct.count * TD_AES_RECORD_SIZE - 1) != 0);
	TD_CHECK_EQ(td_stub_ncalls, 0);
}

int main(int argc, char **argv)
{
	struct td_case_error err;
	struct td_wk wk;

	memset(&err, 0, sizeof err);
	TD_CHECK_EQ(td_aes_cases_parse(&ct, table_text, strlen(table_text), &err), 0);
	TD_CHECK_EQ(ct.count, 3);

	check_open(&wk);
	check_run(&wk);
	check_out_len_zero();
	check_prelude();
	check_too_small();

	/*
	 * A well-formed dump file, written by the same code the target uses,
	 * which tosdiff --validate then has to accept -- and, next to it, the
	 * case table it was produced from, so the validation is against a real
	 * table rather than against the dump's own header: same CRC, same case
	 * count, same case id on every record.
	 */
	if (argc == 3) {
		FILE *f = fopen(argv[2], "wb");

		printf("aes runner: writing %s\n", argv[2]);
		TD_CHECK(f != NULL);
		if (f) {
			TD_CHECK_EQ(fwrite(table_text, 1, strlen(table_text), f),
			            strlen(table_text));
			fclose(f);
		}
	}
	if (argc >= 2) {
		printf("aes runner: writing %s\n", argv[1]);
		td_stub_reset();
		TD_CHECK_EQ(td_run_aes(&ct, td_gem_global(), records,
		                       (size_t)ct.count * TD_AES_RECORD_SIZE), 0);
		TD_CHECK_EQ(td_dump_write(argv[1], TD_AREA_AES, ct.count, "stub",
		                          ct.crc, records,
		                          (size_t)ct.count * TD_AES_RECORD_SIZE), 0);
	}

	td_gem_close(&wk);
	return TD_TEST_RESULT();
}
