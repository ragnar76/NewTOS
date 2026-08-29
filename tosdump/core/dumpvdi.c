/*
 * dumpvdi.c -- DUMPVDI, the VDI reference dumper (spec section 7).
 *
 * Portable.  The same source builds for TOS 3.06 on the TT and for NewTOS on
 * the Pi 4; only the ABI backend underneath differs.
 *
 * usage: DUMPVDI [CASES.TSV [OUT.BIN]]
 *        defaults: VDI.TSV in the current directory, VDI.BIN beside it.
 *
 * A human starts this from a desktop on both machines, so it waits for RETURN
 * before returning -- otherwise the desktop redraws over the only diagnosis
 * there is.  The wait is after every measurement and cannot influence one.
 *
 * The case table is read at runtime and never compiled in (spec section 6):
 * it must be provably the same table for both runs, and recompiling between
 * runs makes that unprovable.  Records are buffered in RAM and written once
 * at the end (spec section 7).
 */
#include <stdio.h>
#include <string.h>

#include "cases.h"
#include "dump.h"
#include "gem.h"
#include "runner.h"

#ifndef TD_BUILD_ID
#define TD_BUILD_ID "----"
#endif

static struct td_case_table table;
static uint8_t records[(size_t)TD_MAX_CASES * TD_VDI_RECORD_SIZE];

/* Outside the measurement: every case has already been run and recorded. */
static int finish(int rc)
{
	printf("\npress RETURN\n");
	while (getchar() == EOF)
		break;
	return rc;
}

int main(int argc, char **argv)
{
	const char *table_path = (argc > 1) ? argv[1] : "VDI.TSV";
	const char *out_path   = (argc > 2) ? argv[2] : "VDI.BIN";
	struct td_case_error err;
	struct td_wk wk;
	size_t bytes;
	int rc;

	printf("DUMPVDI build %s\n", TD_BUILD_ID);

	memset(&err, 0, sizeof err);
	rc = td_cases_load(&table, table_path, &err);
	if (rc != 0) {
		if (err.line)
			printf("%s line %u: %s (%s)\n", table_path, err.line,
			       err.what ? err.what : "", td_case_error_text(rc));
		else
			printf("%s: %s\n", table_path, td_case_error_text(rc));
		return finish(1);
	}
	printf("%s: %u cases, CRC %08lX\n", table_path, table.count,
	       (unsigned long)table.crc);
	if (table.count == 0) {
		printf("nothing to do\n");
		return finish(1);
	}

	rc = td_gem_open(&wk);
	if (rc != 0) {
		printf("%s\n", td_gem_error_text(rc));
		return finish(1);
	}
	printf("workstation %d on physical handle %d, %dx%d\n",
	       wk.handle, wk.phys_handle, wk.work_out[0] + 1, wk.work_out[1] + 1);

	bytes = (size_t)table.count * TD_VDI_RECORD_SIZE;
	rc = td_run_vdi(&table, &wk, records, sizeof records);
	td_gem_close(&wk);
	if (rc != 0) {
		printf("runner failed (%d)\n", rc);
		return finish(1);
	}

	rc = td_dump_write(out_path, TD_AREA_VDI, table.count, TD_BUILD_ID,
	                   table.crc, records, bytes);
	if (rc != 0) {
		printf("cannot write %s (%d)\n", out_path, rc);
		return finish(1);
	}

	printf("%s: %u records, %lu bytes\n", out_path, table.count,
	       (unsigned long)(TD_HEADER_SIZE + bytes));
	return finish(0);
}
