/*
 * test_log.c -- the run log (see core/log.h).
 */
#include "tdtest.h"
#include "log.h"
#include <stdio.h>

int main(int argc, char **argv)
{
	char path[256];
	FILE *f;
	char back[TD_LOG_SIZE + 64];
	size_t n;
	int i;

	if (argc != 2) {
		fprintf(stderr, "usage: test_log OUT.LOG\n");
		return 2;
	}
	snprintf(path, sizeof path, "%s", argv[1]);

	printf("run log: buffering and writing\n");

	td_log_init();
	TD_CHECK_EQ(td_log_len(), 0);
	TD_CHECK_EQ(td_log_truncated(), 0);

	td_log("DUMPVDI build %s\n", "abcd");
	td_log("%s: %u cases, CRC %08lX\n", "VDI.TSV", 11u, 0xB752C38AuL);
	td_log("case %u opcode %d\n", 16u, 35);

	TD_CHECK(td_log_len() > 0);
	TD_CHECK_EQ(td_log_truncated(), 0);
	TD_CHECK(strstr(td_log_text(), "DUMPVDI build abcd") != NULL);
	TD_CHECK(strstr(td_log_text(), "VDI.TSV: 11 cases, CRC B752C38A") != NULL);
	TD_CHECK(strstr(td_log_text(), "case 16 opcode 35") != NULL);

	TD_CHECK_EQ(td_log_write(path), 0);
	f = fopen(path, "rb");
	TD_CHECK(f != NULL);
	if (f) {
		n = fread(back, 1, sizeof back, f);
		fclose(f);
		TD_CHECK_EQ(n, td_log_len());
		TD_CHECK_EQ(memcmp(back, td_log_text(), n), 0);
	}

	/* writing twice is what the dumper does: once after the workstation
	 * opens, once at the end.  The second write must replace, not append. */
	td_log("tail\n");
	TD_CHECK_EQ(td_log_write(path), 0);
	f = fopen(path, "rb");
	TD_CHECK(f != NULL);
	if (f) {
		n = fread(back, 1, sizeof back, f);
		fclose(f);
		TD_CHECK_EQ(n, td_log_len());
	}

	/* Overflow is recorded, never silent, and never a buffer overrun.
	 * Long lines and an early exit keep the echo from flooding the run. */
	td_log_init();
	for (i = 0; i < 1000 && !td_log_truncated(); i++)
		td_log("%04d %s\n", i,
		       "0123456789012345678901234567890123456789012345678901234567890"
		       "1234567890123456789012345678901234567890123456789012345678901"
		       "2345678901234567890123456789012345678901234567890123456789012"
		       "3456789012345678901234567890123456789012345678901234567890123"
		       "4567890123456789012345678901234567890123456789012345678901234"
		       "5678901234567890123456789012345678901234567890123456789012345"
		       "6789012345678901234567890123456789012345678901234567890123456"
		       "789012345678901234567890123456789012345678901234567890123456");
	TD_CHECK_EQ(td_log_truncated(), 1);
	TD_CHECK(td_log_len() <= TD_LOG_SIZE);
	TD_CHECK(strstr(td_log_text(), "truncated") != NULL);
	TD_CHECK_EQ(td_log_text()[td_log_len()], '\0');

	return TD_TEST_RESULT();
}
