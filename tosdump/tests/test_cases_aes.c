/*
 * test_cases_aes.c -- the AES case table loader (spec section 6).
 *
 * The contract is stated here, not read out of core/cases.c: the test says
 * what the seven column form means, the implementation has to meet it.
 *
 * Format under test:
 *   opcode <TAB> n_intin <TAB> n_intout <TAB> n_addrin <TAB> n_addrout
 *          <TAB> intin values <TAB> comment
 */
#include "tdtest.h"
#include "cases.h"

static struct td_aes_case_table t;

static int parse(struct td_aes_case_table *tab, const char *text,
                 struct td_case_error *err)
{
	memset(err, 0, sizeof *err);
	return td_aes_cases_parse(tab, text, strlen(text), err);
}

/* ------------------------------------------------------------------ */

static void test_basic(void)
{
	struct td_case_error err;

	printf("aes cases: the seven columns become control[0..4] and int_in\n");

	TD_CHECK_EQ(parse(&t,
		"# wind_calc, the border/work conversion\n"
		"108\t6\t5\t0\t0\t1,0x0FEF,100,100,200,150\twind_calc WC_WORK\n"
		"\n"
		"77\t0\t5\t0\t0\t-\tgraf_handle\n", &err), 0);
	TD_CHECK_EQ(t.count, 2);

	/* a case id is the PHYSICAL line number; comments and blanks count */
	TD_CHECK_EQ(t.cases[0].line, 2);
	TD_CHECK_EQ(t.cases[1].line, 4);

	TD_CHECK_EQ(t.cases[0].opcode, 108);
	TD_CHECK_EQ(t.cases[0].n_intin, 6);
	TD_CHECK_EQ(t.cases[0].n_intout, 5);
	TD_CHECK_EQ(t.cases[0].n_addrin, 0);
	TD_CHECK_EQ(t.cases[0].n_addrout, 0);
	TD_CHECK_EQ(t.cases[0].n_values, 6);
	TD_CHECK_EQ(t.cases[0].intin[0], 1);
	TD_CHECK_EQ((uint16_t)t.cases[0].intin[1], 0x0FEFu);
	TD_CHECK_EQ(t.cases[0].intin[2], 100);
	TD_CHECK_EQ(t.cases[0].intin[5], 150);
	TD_CHECK(strcmp(t.cases[0].comment, "wind_calc WC_WORK") == 0);

	/* "-" in the value column means no values at all */
	TD_CHECK_EQ(t.cases[1].opcode, 77);
	TD_CHECK_EQ(t.cases[1].n_values, 0);
	TD_CHECK_EQ(t.cases[1].n_intin, 0);
	TD_CHECK_EQ(t.cases[1].n_intout, 5);
	TD_CHECK(strcmp(t.cases[1].comment, "graf_handle") == 0);
}

static void test_counts_are_independent_of_the_values(void)
{
	struct td_case_error err;

	printf("aes cases: the declared counts are NOT derived from the values\n");

	/* This is the whole reason the counts are explicit: a case that
	 * declares more input words than it lists, and fewer output words
	 * than the call produces, is how the marshalling rule gets measured. */
	TD_CHECK_EQ(parse(&t, "108\t20\t3\t3\t1\t1,0,0,0,0,0\tmarshalling\n",
	                  &err), 0);
	TD_CHECK_EQ(t.count, 1);
	TD_CHECK_EQ(t.cases[0].n_intin, 20);    /* beyond I_SIZE on purpose */
	TD_CHECK_EQ(t.cases[0].n_intout, 3);
	TD_CHECK_EQ(t.cases[0].n_addrin, 3);
	TD_CHECK_EQ(t.cases[0].n_addrout, 1);
	TD_CHECK_EQ(t.cases[0].n_values, 6);

	/* a declared count of zero with values listed stays zero */
	TD_CHECK_EQ(parse(&t, "24\t0\t1\t0\t0\t7,8\tIN_LEN 0 with values\n",
	                  &err), 0);
	TD_CHECK_EQ(t.cases[0].n_intin, 0);
	TD_CHECK_EQ(t.cases[0].n_values, 2);
	TD_CHECK_EQ(t.cases[0].intin[0], 7);
	TD_CHECK_EQ(t.cases[0].intin[1], 8);
}

static void test_dash_in_the_n_intin_column(void)
{
	struct td_case_error err;

	printf("aes cases: \"-\" means \"as many as the value column lists\", "
	       "and only in n_intin\n");

	TD_CHECK_EQ(parse(&t, "106\t-\t1\t0\t0\t100,200\twind_find\n", &err), 0);
	TD_CHECK_EQ(t.cases[0].n_intin, 2);
	TD_CHECK_EQ(t.cases[0].n_values, 2);

	TD_CHECK_EQ(parse(&t, "77\t-\t5\t0\t0\t-\tno values at all\n", &err), 0);
	TD_CHECK_EQ(t.cases[0].n_intin, 0);

	/* The other three have no value list to be derived from.  Guessing
	 * would silently change what the call declares, so it is an error. */
	TD_CHECK(parse(&t, "108\t6\t-\t0\t0\t1,0,0,0,0,0\tn_intout dash\n",
	               &err) != 0);
	TD_CHECK_EQ(err.line, 1);
	TD_CHECK(parse(&t, "108\t6\t5\t-\t0\t1,0,0,0,0,0\tn_addrin dash\n",
	               &err) != 0);
	TD_CHECK(parse(&t, "108\t6\t5\t0\t-\t1,0,0,0,0,0\tn_addrout dash\n",
	               &err) != 0);
}

static void test_out_len_is_bounded(void)
{
	struct td_case_error err;

	printf("aes cases: n_intout above the intout array is refused\n");

	/*
	 * xif copies OUT_LEN words back with no clamp
	 * (memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD)),
	 * EmuTOS aes/gemsuper.c:444-445), so a table that declared more would
	 * overrun the caller's own intout array on the target.  The loader
	 * refuses rather than letting a typo corrupt memory on real hardware.
	 */
	TD_CHECK_EQ(parse(&t, "108\t6\t7\t0\t0\t1,0,0,0,0,0\tOUT_LEN at O_SIZE\n",
	                  &err), 0);
	TD_CHECK_EQ(t.cases[0].n_intout, TD_AES_INTOUT_N);

	TD_CHECK_EQ(parse(&t, "108\t6\t8\t0\t0\t1,0,0,0,0,0\tone too many\n",
	                  &err), TD_CASE_ERR_OUT_LEN);
	TD_CHECK_EQ(err.line, 1);
}

static void test_negative_counts_are_refused(void)
{
	struct td_case_error err;

	printf("aes cases: a negative count column is refused\n");

	/* A negative IN_LEN passes xif's `if (IN_LEN)` and then reaches
	 * min(IN_LEN, I_SIZE) as the memcpy length (gemsuper.c:436-437). */
	TD_CHECK(parse(&t, "108\t-1\t5\t0\t0\t1,0,0,0,0,0\tneg IN_LEN\n",
	               &err) != 0);
	TD_CHECK(parse(&t, "108\t6\t-1\t0\t0\t1,0,0,0,0,0\tneg OUT_LEN\n",
	               &err) != 0);
	TD_CHECK(parse(&t, "108\t6\t5\t-2\t0\t1,0,0,0,0,0\tneg AIN_LEN\n",
	               &err) != 0);
	TD_CHECK(parse(&t, "108\t6\t5\t0\t-2\t1,0,0,0,0,0\tneg control[4]\n",
	               &err) != 0);
	TD_CHECK(parse(&t, "-1\t0\t1\t0\t0\t-\tnegative opcode\n", &err) != 0);
}

static void test_malformed_lines(void)
{
	struct td_case_error err;

	printf("aes cases: a malformed line is an error, never a skipped case\n");

	/* six columns is not seven */
	TD_CHECK_EQ(parse(&t, "108\t6\t5\t0\t0\t1,0,0,0,0,0\n", &err),
	            TD_CASE_ERR_COLUMNS);
	TD_CHECK_EQ(err.line, 1);

	TD_CHECK_EQ(parse(&t, "abc\t0\t1\t0\t0\t-\tnot a number\n", &err),
	            TD_CASE_ERR_VALUE);
	TD_CHECK_EQ(parse(&t, "108\tx\t5\t0\t0\t-\tnot a number\n", &err),
	            TD_CASE_ERR_VALUE);
	TD_CHECK_EQ(parse(&t, "108\t6\t5\t0\t0\t1,zz\tnot a number\n", &err),
	            TD_CASE_ERR_VALUE);

	/* more values than the int_in array holds */
	TD_CHECK_EQ(parse(&t,
		"108\t17\t1\t0\t0\t1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17\ttoo many\n",
		&err), TD_CASE_ERR_TOO_MANY_VALUES);

	/* exactly the array size is fine */
	TD_CHECK_EQ(parse(&t,
		"108\t16\t1\t0\t0\t1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16\tfull\n",
		&err), 0);
	TD_CHECK_EQ(t.cases[0].n_values, TD_AES_INTIN_N);
	TD_CHECK_EQ(t.cases[0].intin[TD_AES_INTIN_N - 1], 16);
}

static void test_comment_column(void)
{
	struct td_case_error err;

	printf("aes cases: the comment is everything from column seven on\n");

	TD_CHECK_EQ(parse(&t, "108\t6\t5\t0\t0\t1,0,0,0,0,0\ta\tcomment\twith tabs\n",
	                  &err), 0);
	TD_CHECK(strcmp(t.cases[0].comment, "a\tcomment\twith tabs") == 0);

	TD_CHECK_EQ(parse(&t, "108\t6\t5\t0\t0\t1,0,0,0,0,0\t\n", &err), 0);
	TD_CHECK(strcmp(t.cases[0].comment, "") == 0);
}

static void test_preludes(void)
{
	struct td_case_error err;

	printf("aes cases: a \"+\" line is a prelude, carries no case id\n");

	TD_CHECK_EQ(parse(&t,
		"+26\t2\t1\t0\t0\t3,1\tevnt_dclick set index 3\n"
		"26\t2\t1\t0\t0\t0,0\tevnt_dclick reads it back\n", &err), 0);
	TD_CHECK_EQ(t.count, 1);
	TD_CHECK_EQ(t.prelude_count, 1);
	TD_CHECK_EQ(t.cases[0].line, 2);          /* the prelude consumed line 1 */
	TD_CHECK_EQ(t.cases[0].prelude_first, 0);
	TD_CHECK_EQ(t.cases[0].prelude_count, 1);
	TD_CHECK_EQ(t.preludes[0].opcode, 26);
	TD_CHECK_EQ(t.preludes[0].n_intin, 2);
	TD_CHECK_EQ(t.preludes[0].n_intout, 1);
	TD_CHECK_EQ(t.preludes[0].n_values, 2);
	TD_CHECK_EQ(t.preludes[0].intin[0], 3);
	TD_CHECK_EQ(t.preludes[0].intin[1], 1);

	/* preludes stack */
	TD_CHECK_EQ(parse(&t,
		"+26\t2\t1\t0\t0\t3,1\tone\n"
		"+26\t2\t1\t0\t0\t2,1\ttwo\n"
		"26\t2\t1\t0\t0\t0,0\tthe case\n", &err), 0);
	TD_CHECK_EQ(t.count, 1);
	TD_CHECK_EQ(t.cases[0].prelude_count, 2);
	TD_CHECK_EQ(t.preludes[1].intin[0], 2);

	/* a prelude with no case after it would silently never run */
	TD_CHECK_EQ(parse(&t,
		"26\t2\t1\t0\t0\t0,0\tthe case\n"
		"+26\t2\t1\t0\t0\t3,1\tdangling\n", &err), TD_CASE_ERR_DANGLING);
	TD_CHECK_EQ(err.line, 2);

	/* the same rule the host reader uses to skip them */
	TD_CHECK(td_case_line_is_prelude("+26\t2\t1\t0\t0\t3,1\tx", 20));
	TD_CHECK(!td_case_line_is_prelude("26\t2\t1\t0\t0\t3,1\tx", 19));
}

static void test_crc_is_over_the_raw_bytes(void)
{
	static struct td_aes_case_table u;
	struct td_case_error err;

	printf("aes cases: the CRC covers the raw file, comments included\n");

	TD_CHECK_EQ(parse(&t, "108\t6\t5\t0\t0\t1,0,0,0,0,0\tone\n", &err), 0);
	TD_CHECK_EQ(parse(&u, "108\t6\t5\t0\t0\t1,0,0,0,0,0\ttwo\n", &err), 0);
	TD_CHECK(t.crc != u.crc);
}

int main(void)
{
	test_basic();
	test_counts_are_independent_of_the_values();
	test_dash_in_the_n_intin_column();
	test_out_len_is_bounded();
	test_negative_counts_are_refused();
	test_malformed_lines();
	test_comment_column();
	test_preludes();
	test_crc_is_over_the_raw_bytes();
	return TD_TEST_RESULT();
}
