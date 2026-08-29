/*
 * test_cases.c -- case table parser tests (spec section 6).
 *
 * The case table is the one artefact that must be provably identical for both
 * runs, so the parser is held to the letter of the format: a case id is the
 * physical line number, a malformed line is an error rather than a silently
 * skipped case, and a ptsin column with an odd number of values cannot be a
 * point list and is rejected.
 */
#include "tdtest.h"
#include "cases.h"

static int parse(struct td_case_table *t, const char *text,
                 struct td_case_error *err)
{
	memset(err, 0, sizeof *err);
	return td_cases_parse(t, text, strlen(text), err);
}

static void test_basic(void)
{
	static struct td_case_table t;
	struct td_case_error err;
	int rc;

	printf("case table: the shape of a VDI table\n");

	rc = parse(&t,
	           "# a comment line\n"
	           "\n"
	           "6\t0\t-\t0,0,10,10\tv_pline two points\n"
	           "11\t1\t-\t20,20,30,30\tGDP bar\n"
	           "32\t0\t1\t-\tvswr_mode replace\n",
	           &err);
	TD_CHECK_EQ(rc, 0);
	TD_CHECK_EQ(t.count, 3);

	/* case id is the physical line number: comments and blanks consume one */
	TD_CHECK_EQ(t.cases[0].line, 3);
	TD_CHECK_EQ(t.cases[1].line, 4);
	TD_CHECK_EQ(t.cases[2].line, 5);

	TD_CHECK_EQ(t.cases[0].opcode, 6);
	TD_CHECK_EQ(t.cases[0].sub, 0);
	TD_CHECK_EQ(t.cases[0].n_intin, 0);
	TD_CHECK_EQ(t.cases[0].n_ptsin, 4);
	TD_CHECK_EQ(t.cases[0].ptsin[0], 0);
	TD_CHECK_EQ(t.cases[0].ptsin[3], 10);
	TD_CHECK(strcmp(t.cases[0].comment, "v_pline two points") == 0);

	TD_CHECK_EQ(t.cases[1].opcode, 11);
	TD_CHECK_EQ(t.cases[1].sub, 1);
	TD_CHECK_EQ(t.cases[1].ptsin[0], 20);

	TD_CHECK_EQ(t.cases[2].opcode, 32);
	TD_CHECK_EQ(t.cases[2].n_intin, 1);
	TD_CHECK_EQ(t.cases[2].intin[0], 1);
	TD_CHECK_EQ(t.cases[2].n_ptsin, 0);

	/* the CRC is over the raw bytes and is what the record header carries */
	TD_CHECK(t.crc != 0);
}

static void test_values(void)
{
	static struct td_case_table t;
	struct td_case_error err;

	printf("case table: value column forms\n");

	TD_CHECK_EQ(parse(&t, "15\t0\t-1,0x7FFF,-32768,0\t-\tedges\n", &err), 0);
	TD_CHECK_EQ(t.count, 1);
	TD_CHECK_EQ(t.cases[0].n_intin, 4);
	TD_CHECK_EQ(t.cases[0].intin[0], -1);
	TD_CHECK_EQ(t.cases[0].intin[1], 32767);
	TD_CHECK_EQ(t.cases[0].intin[2], -32768);
	TD_CHECK_EQ(t.cases[0].intin[3], 0);

	/* 0xFFFF is the bit pattern a program writes for an all-ones mask */
	TD_CHECK_EQ(parse(&t, "113\t0\t0xFFFF\t-\tvsl_udsty\n", &err), 0);
	TD_CHECK_EQ((uint16_t)t.cases[0].intin[0], 0xFFFFu);

	/* a hex opcode is accepted too */
	TD_CHECK_EQ(parse(&t, "0x0C\t0\t-\t-\tvst_height\n", &err), 0);
	TD_CHECK_EQ(t.cases[0].opcode, 12);

	/* leading and trailing blanks inside a column do not change the value */
	TD_CHECK_EQ(parse(&t, "6\t0\t 1 , 2 \t-\tspaces\n", &err), 0);
	TD_CHECK_EQ(t.cases[0].n_intin, 2);
	TD_CHECK_EQ(t.cases[0].intin[1], 2);

	/* a table edited on the Atari carries CRLF */
	TD_CHECK_EQ(parse(&t, "6\t0\t-\t1,2\tcrlf\r\n", &err), 0);
	TD_CHECK_EQ(t.count, 1);
	TD_CHECK(strcmp(t.cases[0].comment, "crlf") == 0);

	/* a last line without a newline is still a case */
	TD_CHECK_EQ(parse(&t, "6\t0\t-\t1,2\tno trailing newline", &err), 0);
	TD_CHECK_EQ(t.count, 1);
	TD_CHECK_EQ(t.cases[0].line, 1);
}

static void test_rejections(void)
{
	static struct td_case_table t;
	struct td_case_error err;

	printf("case table: a malformed line is an error, never a skipped case\n");

	TD_CHECK_EQ(parse(&t, "6\t0\t-\n", &err), TD_CASE_ERR_COLUMNS);
	TD_CHECK_EQ(err.line, 1);

	TD_CHECK_EQ(parse(&t, "6\t0\t-\t1,2,3\todd point list\n", &err),
	            TD_CASE_ERR_ODD_PTSIN);
	TD_CHECK_EQ(err.line, 1);

	TD_CHECK_EQ(parse(&t, "six\t0\t-\t-\tbad opcode\n", &err),
	            TD_CASE_ERR_VALUE);
	TD_CHECK_EQ(parse(&t, "6\t0\t1,,2\t-\tempty element\n", &err),
	            TD_CASE_ERR_VALUE);
	TD_CHECK_EQ(parse(&t, "6\t0\t1,zwei\t-\tbad element\n", &err),
	            TD_CASE_ERR_VALUE);
	TD_CHECK_EQ(parse(&t, "6\t0\t65536\t-\tout of 16-bit range\n", &err),
	            TD_CASE_ERR_RANGE);
	TD_CHECK_EQ(parse(&t, "6\t0\t-32769\t-\tout of 16-bit range\n", &err),
	            TD_CASE_ERR_RANGE);
	TD_CHECK_EQ(parse(&t, "-1\t0\t-\t-\tnegative opcode\n", &err),
	            TD_CASE_ERR_RANGE);

	/* the comment column may hold tabs; it is everything from column five on */
	TD_CHECK_EQ(parse(&t, "6\t0\t-\t-\ta\tcomment\twith tabs\n", &err), 0);
	TD_CHECK(strcmp(t.cases[0].comment, "a\tcomment\twith tabs") == 0);

	/* an empty comment column is allowed -- the comment is descriptive only */
	TD_CHECK_EQ(parse(&t, "6\t0\t-\t-\t\n", &err), 0);
	TD_CHECK(strcmp(t.cases[0].comment, "") == 0);
}

static void test_limits(void)
{
	static struct td_case_table t;
	struct td_case_error err;
	static char buf[64 * 1024];
	size_t pos = 0;
	int i;

	printf("case table: array and table limits\n");

	/* intin may fill the array exactly, one more is an error */
	pos = 0;
	pos += (size_t)sprintf(buf + pos, "6\t0\t");
	for (i = 0; i < TD_VDI_INTIN_N; i++)
		pos += (size_t)sprintf(buf + pos, "%s1", i ? "," : "");
	pos += (size_t)sprintf(buf + pos, "\t-\tfull intin\n");
	TD_CHECK_EQ(parse(&t, buf, &err), 0);
	TD_CHECK_EQ(t.cases[0].n_intin, TD_VDI_INTIN_N);

	pos = 0;
	pos += (size_t)sprintf(buf + pos, "6\t0\t");
	for (i = 0; i < TD_VDI_INTIN_N + 1; i++)
		pos += (size_t)sprintf(buf + pos, "%s1", i ? "," : "");
	pos += (size_t)sprintf(buf + pos, "\t-\tintin overflow\n");
	TD_CHECK_EQ(parse(&t, buf, &err), TD_CASE_ERR_TOO_MANY_VALUES);

	pos = 0;
	pos += (size_t)sprintf(buf + pos, "6\t0\t-\t");
	for (i = 0; i < TD_VDI_PTSIN_N + 2; i++)
		pos += (size_t)sprintf(buf + pos, "%s1", i ? "," : "");
	pos += (size_t)sprintf(buf + pos, "\tptsin overflow\n");
	TD_CHECK_EQ(parse(&t, buf, &err), TD_CASE_ERR_TOO_MANY_VALUES);

	pos = 0;
	for (i = 0; i < TD_MAX_CASES + 1; i++)
		pos += (size_t)sprintf(buf + pos, "6\t0\t-\t-\tcase\n");
	TD_CHECK_EQ(parse(&t, buf, &err), TD_CASE_ERR_TOO_MANY_CASES);
}

static void test_crc_identity(void)
{
	static struct td_case_table a, b;
	struct td_case_error err;

	printf("case table: the CRC changes with any byte\n");

	TD_CHECK_EQ(parse(&a, "6\t0\t-\t0,0\tcase\n", &err), 0);
	TD_CHECK_EQ(parse(&b, "6\t0\t-\t0,1\tcase\n", &err), 0);
	TD_CHECK(a.crc != b.crc);

	/* a changed comment is a changed table: the dump it produced is stale */
	TD_CHECK_EQ(parse(&b, "6\t0\t-\t0,0\tCASE\n", &err), 0);
	TD_CHECK(a.crc != b.crc);
}

int main(void)
{
	test_basic();
	test_values();
	test_rejections();
	test_limits();
	test_crc_identity();
	return TD_TEST_RESULT();
}
