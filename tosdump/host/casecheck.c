/*
 * casecheck.c -- parse a case table and print what the runner will do with it.
 *
 * A case table is data that two machines must agree on byte for byte, so it is
 * worth being able to see exactly how it parses -- the case ids, the contrl
 * values each case produces, and the CRC the dump header will carry -- before
 * carrying it to a machine that has no debugger.
 *
 * usage: casecheck CASES.TSV
 */
#include <stdio.h>
#include <string.h>

#include "cases.h"

static struct td_case_table table;

static void print_list(const char *name, const int16_t *v, uint16_t n)
{
	uint16_t i;

	if (n == 0)
		return;
	printf("      %s[%u] =", name, n);
	for (i = 0; i < n && i < 12; i++)
		printf(" %d", v[i]);
	if (n > 12)
		printf(" ... (%u more)", n - 12);
	printf("\n");
}

int main(int argc, char **argv)
{
	struct td_case_error err;
	uint16_t i;
	int rc;

	if (argc != 2) {
		fprintf(stderr, "usage: casecheck CASES.TSV\n");
		return 2;
	}

	memset(&err, 0, sizeof err);
	rc = td_cases_load(&table, argv[1], &err);
	if (rc != 0) {
		if (err.line)
			fprintf(stderr, "%s line %u: %s (%s)\n", argv[1], err.line,
			        err.what ? err.what : "", td_case_error_text(rc));
		else
			fprintf(stderr, "%s: %s\n", argv[1], td_case_error_text(rc));
		return 1;
	}

	printf("%s: %u cases, CRC %08X\n", argv[1], table.count, table.crc);
	for (i = 0; i < table.count; i++) {
		const struct td_case *c = &table.cases[i];

		printf("  case %u  opcode %d  contrl[1]=%u contrl[3]=%u contrl[5]=%d"
		       "  %s\n",
		       c->line, c->opcode, c->n_ptsin / 2, c->n_intin, c->sub,
		       c->comment);
		print_list("intin", c->intin, c->n_intin);
		print_list("ptsin", c->ptsin, c->n_ptsin);
	}
	return 0;
}
