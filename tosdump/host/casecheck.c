/*
 * casecheck.c -- parse a case table and print what the runner will do with it.
 *
 * A case table is data that two machines must agree on byte for byte, so it is
 * worth being able to see exactly how it parses -- the case ids, the contrl
 * values each case produces, and the CRC the dump header will carry -- before
 * carrying it to a machine that has no debugger.
 *
 * usage: casecheck [--aes] CASES.TSV
 *
 * --aes reads the seven column AES form instead of the five column VDI one.
 * The two forms cannot be told apart by looking at a line -- a VDI comment may
 * hold tabs -- so which one it is has to be said.
 */
#include <stdio.h>
#include <string.h>

#include "cases.h"

static struct td_case_table     table;
static struct td_aes_case_table aes_table;

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

static void print_error(const char *path, int rc,
                        const struct td_case_error *err)
{
	if (err->line)
		fprintf(stderr, "%s line %u: %s (%s)\n", path, err->line,
		        err->what ? err->what : "", td_case_error_text(rc));
	else
		fprintf(stderr, "%s: %s\n", path, td_case_error_text(rc));
}

static int check_vdi(const char *path)
{
	struct td_case_error err;
	uint16_t i;
	int rc;

	memset(&err, 0, sizeof err);
	rc = td_cases_load(&table, path, &err);
	if (rc != 0) {
		print_error(path, rc, &err);
		return 1;
	}

	printf("%s: %u cases, %u preludes, CRC %08X\n", path, table.count,
	       table.prelude_count, table.crc);
	for (i = 0; i < table.count; i++) {
		const struct td_case *c = &table.cases[i];
		uint16_t k;

		/* Preludes run after the reset and before the case, unrecorded --
		 * print them with the case they belong to, since that is the only
		 * way to see what state the case is actually measuring. */
		for (k = 0; k < c->prelude_count; k++) {
			const struct td_prelude *p =
				&table.preludes[c->prelude_first + k];
			printf("    prelude: opcode %d  contrl[1]=%u contrl[3]=%u "
			       "contrl[5]=%d\n",
			       p->opcode, p->n_ptsin / 2, p->n_intin, p->sub);
			print_list("intin", p->intin, p->n_intin);
			print_list("ptsin", p->ptsin, p->n_ptsin);
		}

		printf("  case %u  opcode %d  contrl[1]=%u contrl[3]=%u contrl[5]=%d"
		       "  %s\n",
		       c->line, c->opcode, c->n_ptsin / 2, c->n_intin, c->sub,
		       c->comment);
		print_list("intin", c->intin, c->n_intin);
		print_list("ptsin", c->ptsin, c->n_ptsin);
	}
	return 0;
}

static int check_aes(const char *path)
{
	struct td_case_error err;
	uint16_t i;
	int rc;

	memset(&err, 0, sizeof err);
	rc = td_aes_cases_load(&aes_table, path, &err);
	if (rc != 0) {
		print_error(path, rc, &err);
		return 1;
	}

	printf("%s: %u cases, %u preludes, CRC %08X\n", path, aes_table.count,
	       aes_table.prelude_count, aes_table.crc);
	for (i = 0; i < aes_table.count; i++) {
		const struct td_aes_case *c = &aes_table.cases[i];
		uint16_t k;

		for (k = 0; k < c->prelude_count; k++) {
			const struct td_aes_prelude *p =
				&aes_table.preludes[c->prelude_first + k];
			printf("    prelude: opcode %d  control[1..4]= %d %d %d %d\n",
			       p->opcode, p->n_intin, p->n_intout, p->n_addrin,
			       p->n_addrout);
			print_list("int_in", p->intin, p->n_values);
		}

		printf("  case %u  opcode %d  control[1..4]= %d %d %d %d  %s\n",
		       c->line, c->opcode, c->n_intin, c->n_intout, c->n_addrin,
		       c->n_addrout, c->comment);
		print_list("int_in", c->intin, c->n_values);
	}
	return 0;
}

int main(int argc, char **argv)
{
	if (argc == 3 && strcmp(argv[1], "--aes") == 0)
		return check_aes(argv[2]);
	if (argc == 2)
		return check_vdi(argv[1]);

	fprintf(stderr, "usage: casecheck [--aes] CASES.TSV\n");
	return 2;
}
