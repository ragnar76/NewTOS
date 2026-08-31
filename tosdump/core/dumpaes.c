/*
 * dumpaes.c -- DUMPAES, the AES reference dumper (spec section 7).
 *
 * Portable.  The same source builds for TOS 3.06 on the TT and for NewTOS on
 * the Pi 4; only the ABI backend underneath differs.
 *
 * usage: DUMPAES [CASES.TSV [OUT.BIN [OUT.LOG]]]
 *        defaults: AES.TSV in the current directory, AES.BIN and AES.LOG
 *        beside it.
 *
 * The counterpart of dumpvdi.c, and deliberately the same shape: the case
 * table is read at runtime and never compiled in (spec section 6), records
 * are buffered in RAM and written once at the end (spec section 7), and the
 * run log is written twice -- once after the application context is up, so a
 * run that hangs still leaves its setup on disk, and once at the end.
 *
 * It runs the same application context as DUMPVDI (td_gem_open: appl_init,
 * graf_handle, v_opnvwk on the handle graf_handle returned).  That is not
 * because the AES cases need a virtual workstation -- none of them does --
 * but because the two dumps have to be taken in the same context to be
 * comparable, and appl_init is what fills the AESGLOBAL every AES call is
 * handed (EmuTOS aes/gemsuper.c:92-98).
 *
 * It does NOT wait for a keypress, and it issues no blocking AES call.  Both
 * matter more here than in the VDI dumper: an AES opcode the machine does
 * not implement reaches the dispatcher's default arm, which pops an alert
 * box and waits for a click (fm_show(ALNOFUNC), gemsuper.c:411-417) -- on
 * the TT and on the Pi alike that is a hung run and a power cycle.  Opcode 0
 * is the only unknown opcode that is safe to issue (gemsuper.c:414 skips the
 * alert for it), and the case table may hold no blocking call at all.  That
 * is enforced rather than merely documented: td_aes_cases_load refuses a
 * table naming such an opcode, so the program exits before it ever opens the
 * workstation.  See the refusal guard in core/cases.c for the per-opcode
 * citations and cases/COVERAGE-AES.md for what it does not cover.
 */
#include <stdio.h>
#include <string.h>

#include "cases.h"
#include "dump.h"
#include "gem.h"
#include "log.h"
#include "runner_aes.h"

#ifndef TD_BUILD_ID
#define TD_BUILD_ID "----"
#endif

static struct td_aes_case_table table;
static uint8_t records[(size_t)TD_MAX_CASES * TD_AES_RECORD_SIZE];

static const char *log_path = "AES.LOG";

/* Write the log out.  Outside the measurement: every case has already been
 * run and recorded. */
static int finish(int rc)
{
	int lr = td_log_write(log_path);

	if (lr != 0)
		printf("cannot write %s (%d)\n", log_path, lr);
	else
		printf("%s: %lu bytes%s\n", log_path,
		       (unsigned long)td_log_len(),
		       td_log_truncated() ? " (truncated)" : "");

	return rc;
}

int main(int argc, char **argv)
{
	const char *table_path = (argc > 1) ? argv[1] : "AES.TSV";
	const char *out_path   = (argc > 2) ? argv[2] : "AES.BIN";
	struct td_case_error err;
	struct td_wk wk;
	const int16_t *g;
	size_t bytes;
	int rc, i;

	if (argc > 3)
		log_path = argv[3];

	td_log_init();
	td_log("DUMPAES build %s, record format %d\n", TD_BUILD_ID,
	       TD_FORMAT_VERSION);

	memset(&err, 0, sizeof err);
	rc = td_aes_cases_load(&table, table_path, &err);
	if (rc != 0) {
		if (err.line)
			td_log("%s line %u: %s (%s)\n", table_path, err.line,
			       err.what ? err.what : "", td_case_error_text(rc));
		else
			td_log("%s: %s\n", table_path, td_case_error_text(rc));
		return finish(1);
	}
	td_log("%s: %u cases, CRC %08lX\n", table_path, table.count,
	       (unsigned long)table.crc);
	if (table.count == 0) {
		td_log("nothing to do\n");
		return finish(1);
	}

	rc = td_gem_open(&wk);
	if (rc != 0) {
		td_log("%s\n", td_gem_error_text(rc));
		return finish(1);
	}
	td_log("appl_init id %d, graf_handle %d, v_opnvwk handle %d\n",
	       wk.ap_id, wk.phys_handle, wk.handle);

	/*
	 * The AESGLOBAL as appl_init left it, before any case runs.  Every
	 * record carries global[] afterwards, so this line is what a reader
	 * compares those against -- and ap_version says which AES answered
	 * (aes/gemrslib.h:16-27 for the layout, gemsuper.c:92-98 for the
	 * values appl_init writes).
	 */
	g = td_gem_global();
	td_log("global after appl_init ");
	for (i = 0; i < TD_AES_GLOBAL_N; i++)
		td_log("%d%s", g[i], i + 1 < TD_AES_GLOBAL_N ? "," : "\n");
	td_log("ap_version %04X, ap_count %d, ap_id %d, ap_planes %d\n",
	       (unsigned)(uint16_t)g[0], g[1], g[2], g[10]);

	/* Written before the first case: a run that hangs still leaves this. */
	td_log("running %u cases\n", table.count);
	td_log_write(log_path);

	bytes = (size_t)table.count * TD_AES_RECORD_SIZE;
	rc = td_run_aes(&table, td_gem_global(), records, sizeof records);
	td_gem_close(&wk);
	if (rc != 0) {
		td_log("runner failed (%d)\n", rc);
		return finish(1);
	}
	td_log("all %u cases ran\n", table.count);

	rc = td_dump_write(out_path, TD_AREA_AES, table.count, TD_BUILD_ID,
	                   table.crc, records, bytes);
	if (rc != 0) {
		td_log("cannot write %s (%d)\n", out_path, rc);
		return finish(1);
	}

	td_log("%s: %u records, %lu bytes\n", out_path, table.count,
	       (unsigned long)(TD_HEADER_SIZE + bytes));
	return finish(0);
}
