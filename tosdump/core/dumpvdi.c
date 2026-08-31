/*
 * dumpvdi.c -- DUMPVDI, the VDI reference dumper (spec section 7).
 *
 * Portable.  The same source builds for TOS 3.06 on the TT and for NewTOS on
 * the Pi 4; only the ABI backend underneath differs.
 *
 * usage: DUMPVDI [CASES.TSV [OUT.BIN [OUT.LOG]]]
 *        defaults: VDI.TSV in the current directory, VDI.BIN and VDI.LOG
 *        beside it.
 *
 * The case table is read at runtime and never compiled in (spec section 6):
 * it must be provably the same table for both runs, and recompiling between
 * runs makes that unprovable.  Records are buffered in RAM and written once at
 * the end (spec section 7), and so is the run log -- file I/O between two
 * measurements would touch device state the measurements are isolated from.
 *
 * Everything the program says goes to the log file as well as the screen: a
 * run happens on a machine with no debugger, and on a GEM screen the desktop
 * redraws over the only diagnosis there is.  The log is written twice -- once
 * after the workstation opens, before any case, so a run that hangs still
 * leaves its setup on disk, and once at the end.
 *
 * It does NOT wait for a keypress at the end, and that is deliberate.  An
 * earlier version did, so an operator could read the output before the
 * desktop repainted over it.  That predates the log file, which records the
 * same text on disk on both machines and made the pause redundant.  It also
 * hung NewTOS outright: the blocking console read waits on the PL011 RX
 * interrupt, which is never delivered on real Pi 4 hardware (QUIRK-002), so
 * the machine sat in `wfi` until it was power-cycled.
 */
#include <stdio.h>
#include <string.h>

#include "cases.h"
#include "dump.h"
#include "gem.h"
#include "log.h"
#include "runner.h"

#ifndef TD_BUILD_ID
#define TD_BUILD_ID "----"
#endif

static struct td_case_table table;
static uint8_t records[(size_t)TD_MAX_CASES * TD_VDI_RECORD_SIZE];

static const char *log_path = "VDI.LOG";

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
	const char *table_path = (argc > 1) ? argv[1] : "VDI.TSV";
	const char *out_path   = (argc > 2) ? argv[2] : "VDI.BIN";
	struct td_case_error err;
	struct td_wk wk;
	size_t bytes;
	int rc, i;

	if (argc > 3)
		log_path = argv[3];

	td_log_init();
	td_log("DUMPVDI build %s, record format %d\n", TD_BUILD_ID,
	       TD_FORMAT_VERSION);

	memset(&err, 0, sizeof err);
	rc = td_cases_load(&table, table_path, &err);
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
	td_log("work_in ");
	for (i = 0; i < TD_WORK_IN_N; i++)
		td_log("%d%s", wk.work_in[i], i + 1 < TD_WORK_IN_N ? "," : "\n");
	/* DEV_TAB and SIZ_TAB as this machine reports them: the reset reads its
	 * three device-defined values out of SIZ_TAB, so a difference there
	 * explains differences later without any guessing. */
	td_log("DEV_TAB ");
	for (i = 0; i < TD_WORK_OUT_N; i++)
		td_log("%d%s", wk.work_out[i], i + 1 < TD_WORK_OUT_N ? "," : "\n");
	td_log("SIZ_TAB ");
	for (i = 0; i < TD_WORK_PTS_N; i++)
		td_log("%d%s", wk.work_pts[i], i + 1 < TD_WORK_PTS_N ? "," : "\n");

	/* Written before the first case: a run that hangs still leaves this. */
	td_log("running %u cases\n", table.count);
	td_log_write(log_path);

	bytes = (size_t)table.count * TD_VDI_RECORD_SIZE;
	rc = td_run_vdi(&table, &wk, records, sizeof records);
	td_gem_close(&wk);
	if (rc != 0) {
		td_log("runner failed (%d)\n", rc);
		return finish(1);
	}
	td_log("all %u cases ran\n", table.count);

	rc = td_dump_write(out_path, TD_AREA_VDI, table.count, TD_BUILD_ID,
	                   table.crc, records, bytes);
	if (rc != 0) {
		td_log("cannot write %s (%d)\n", out_path, rc);
		return finish(1);
	}

	td_log("%s: %u records, %lu bytes\n", out_path, table.count,
	       (unsigned long)(TD_HEADER_SIZE + bytes));
	return finish(0);
}
