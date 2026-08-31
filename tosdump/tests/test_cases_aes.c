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


/* ------------------------------------------------------------------ */
/* The refusal guard                                                   */
/* ------------------------------------------------------------------ */
/*
 * A typo in a data file must not hang the user's TT. The loader refuses the
 * whole table -- named reason, non-zero return, the offending line -- if any
 * case or prelude names an opcode that can stop the run finishing, one that
 * TOS 3.06's AES does not have, or one that is not an AES opcode at all.
 * There is no override: taking an opcode off the list is a source change with
 * a citation, and that friction is the point.
 */

static void refused(const char *line, const char *what)
{
	struct td_case_error err;
	char text[256];
	int rc;

	snprintf(text, sizeof text, "%s\n", line);
	rc = parse(&t, text, &err);
	if (rc != TD_CASE_ERR_UNSAFE_OPCODE) {
		printf("  FAIL %s: expected refusal, got %d (%s)\n", what, rc,
		       td_case_error_text(rc));
		td_test_failures++;
		return;
	}
	TD_CHECK_EQ(err.line, 1);
	if (err.what == NULL || err.what[0] == '\0') {
		printf("  FAIL %s: refusal carries no reason\n", what);
		td_test_failures++;
	}
}

static void accepted(const char *line, const char *what)
{
	struct td_case_error err;
	char text[256];
	int rc;

	snprintf(text, sizeof text, "%s\n", line);
	rc = parse(&t, text, &err);
	if (rc != 0) {
		printf("  FAIL %s: expected acceptance, got %d (%s)\n", what, rc,
		       td_case_error_text(rc));
		td_test_failures++;
	}
}

static void test_guard_refuses_run_fatal_opcodes(void)
{
	printf("aes cases: opcodes that stop the run finishing are refused\n");

	/* blocks on the operator */
	refused("11\t2\t1\t3\t0\t0,16\tappl_read",      "appl_read (11)");
	refused("15\t2\t1\t3\t0\t8,0\tappl_trecord",    "appl_trecord (15)");
	refused("20\t0\t1\t0\t0\t-\tevnt_keybd",        "evnt_keybd (20)");
	refused("21\t3\t5\t0\t0\t1,1,0\tevnt_button",   "evnt_button (21)");
	refused("22\t5\t5\t0\t0\t0,0,0,1,1\tevnt_mouse","evnt_mouse (22)");
	refused("23\t0\t1\t1\t0\t-\tevnt_mesag",        "evnt_mesag (23)");
	refused("25\t16\t7\t3\t0\t1\tevnt_multi",       "evnt_multi (25)");
	refused("50\t1\t1\t1\t0\t0\tform_do",           "form_do (50)");
	refused("52\t1\t1\t1\t0\t1\tform_alert",        "form_alert (52)");
	refused("53\t1\t1\t0\t0\t2\tform_error",        "form_error (53)");
	refused("56\t3\t2\t1\t0\t1,1,1\tform_button",   "form_button (56)");
	refused("70\t4\t3\t0\t0\t0,0,1,1\tgraf_rubbox", "graf_rubbox (70)");
	refused("71\t8\t3\t0\t0\t1,1,0,0,0,0,1,1\tgraf_dragbox",
	        "graf_dragbox (71)");
	refused("75\t3\t1\t1\t0\t1,0,1\tgraf_watchbox", "graf_watchbox (75)");
	refused("76\t4\t1\t1\t0\t1,2,1,0\tgraf_slidebox","graf_slidebox (76)");
	refused("90\t0\t2\t2\t0\t-\tfsel_input",        "fsel_input (90)");
	refused("91\t0\t2\t3\t0\t-\tfsel_exinput",      "fsel_exinput (91)");

	/* ends or replaces the application: never returns to the run */
	refused("19\t0\t1\t0\t0\t-\tappl_exit",         "appl_exit (19)");
	refused("121\t3\t1\t2\t0\t0,0,0\tshel_write",   "shel_write (121)");
}

static void test_guard_refuses_opcodes_tos306_does_not_have(void)
{
	printf("aes cases: opcodes above AES 3.20 are refused -- the modal alert "
	       "would hang the TT\n");

	refused("36\t2\t1\t2\t0\t0,0\tmenu_popup",     "menu_popup (36)");
	refused("37\t2\t1\t2\t0\t0,0\tmenu_attach",    "menu_attach (37)");
	refused("38\t3\t1\t1\t0\t0,0,0\tmenu_istart",  "menu_istart (38)");
	refused("39\t1\t1\t1\t0\t0\tmenu_settings",    "menu_settings (39)");
	refused("48\t4\t3\t0\t0\t0,6,0,0\tobjc_sysvar","objc_sysvar (48)");
}

static void test_guard_refuses_pcgem_opcodes(void)
{
	/* Not "gated above 3.20" -- unestablished.  CONF_WITH_PCGEM
	 * (gemsuper.c:115, :315, :398-403) is absent from the AES_VERSION
	 * ladder, so EmuTOS cannot say whether TOS 3.06 implements these.  If it
	 * does not, each one is the modal alert again. */
	printf("aes cases: PC-GEM opcodes are refused -- EmuTOS cannot say "
	       "whether TOS 3.06 has them\n");

	refused("17\t0\t1\t0\t0\t-\tappl_yield", "appl_yield (17)");
	refused("82\t0\t1\t0\t0\t-\tscrp_clear", "scrp_clear (82)");
	refused("126\t0\t1\t2\t0\t-\tshel_rdef", "shel_rdef (126)");
	refused("127\t0\t1\t2\t0\t-\tshel_wdef", "shel_wdef (127)");
}

static void test_guard_refuses_things_that_are_not_opcodes(void)
{
	printf("aes cases: an opcode with no arm in crysbind is refused\n");

	refused("16\t0\t1\t0\t0\t-\tgap in the appl range",  "16");
	refused("18\t0\t1\t0\t0\t-\tappl_search, absent",    "18");
	refused("27\t0\t1\t0\t0\t-\tgap after evnt_dclick",  "27");
	refused("49\t0\t1\t0\t0\t-\tgap after objc_sysvar",  "49");
	refused("57\t0\t1\t0\t0\t-\tgap after form_button",  "57");
	refused("83\t0\t1\t0\t0\t-\tgap after scrp_clear",   "83");
	refused("99\t0\t1\t0\t0\t-\tgap before wind_create", "99");
	refused("115\t0\t1\t0\t0\t-\tgap after rsrc_obfix",  "115");
	refused("128\t0\t1\t0\t0\t-\tpast the last opcode",  "128");
	refused("130\t0\t1\t0\t0\t-\tappl_getinfo, absent",  "130");
	refused("200\t0\t1\t0\t0\t-\tthe selector, not an opcode", "200");

	/* the single deliberate exception */
	accepted("0\t0\t1\t0\t0\t-\topcode 0 is the one safe unknown opcode",
	         "opcode 0");
}

static void test_guard_lets_the_safe_ones_through(void)
{
	printf("aes cases: the calls that do not block are still accepted\n");

	accepted("24\t2\t1\t0\t0\t0,0\tevnt_timer",   "evnt_timer (24)");
	/* evnt_dclick and graf_mbox are NOT in the guard: ev_dclick is a pure
	 * read (gemevlib.c:300-309) and gr_movebox is a finite XOR animation
	 * (gemgrlib.c:287-307).  Both are out of round 1 for other reasons,
	 * which is the ledger's business and not the guard's. */
	accepted("26\t2\t1\t0\t0\t0,0\tevnt_dclick",  "evnt_dclick (26)");
	accepted("72\t6\t1\t0\t0\t1,1,0,0,1,1\tgraf_mbox", "graf_mbox (72)");
	accepted("73\t8\t1\t0\t0\t0,0,1,1,0,0,2,2\tgraf_growbox",
	         "graf_growbox (73)");
	accepted("51\t5\t1\t0\t0\t0,0,0,0,0\tform_dial", "form_dial (51)");
	accepted("55\t4\t3\t1\t0\t0,0,0,0\tform_keybd", "form_keybd (55)");
	accepted("10\t0\t1\t0\t0\t-\tappl_init",      "appl_init (10)");
	accepted("30\t1\t1\t1\t0\t1\tmenu_bar",       "menu_bar (30)");
	accepted("107\t1\t1\t0\t0\t1\twind_update",   "wind_update (107)");
	accepted("112\t2\t1\t0\t1\t0,0\trsrc_gaddr",  "rsrc_gaddr (112)");
	/* shel_wdef (127) used to stand here.  It moved to the PC-GEM refusal
	 * above: EmuTOS gates it behind CONF_WITH_PCGEM, which is not in the
	 * AES_VERSION ladder, so the reference cannot say whether TOS 3.06 has
	 * it -- and if it does not, the run meets the modal alert.  Deliberate
	 * policy change, not a test bent to fit the code. */
}

static void test_guard_covers_preludes_too(void)
{
	struct td_case_error err;

	printf("aes cases: a prelude is guarded exactly like a case\n");

	TD_CHECK_EQ(parse(&t,
		"+50\t1\t1\t1\t0\t0\tform_do as a prelude\n"
		"108\t6\t5\t0\t0\t1,0,0,0,0,0\tthe case\n", &err),
		TD_CASE_ERR_UNSAFE_OPCODE);
	TD_CHECK_EQ(err.line, 1);

	TD_CHECK_EQ(parse(&t,
		"+48\t4\t3\t0\t0\t0,6,0,0\tobjc_sysvar as a prelude\n"
		"108\t6\t5\t0\t0\t1,0,0,0,0,0\tthe case\n", &err),
		TD_CASE_ERR_UNSAFE_OPCODE);
	TD_CHECK_EQ(err.line, 1);
}

static void test_the_shipped_table_still_loads(void)
{
	struct td_case_error err;
	int rc;

	printf("aes cases: the shipped round 1 table passes the guard\n");

	memset(&err, 0, sizeof err);
	rc = td_aes_cases_load(&t, "../cases/aes-r1.tsv", &err);
	if (rc != 0) {
		printf("  FAIL cases/aes-r1.tsv line %u: %s (%s)\n", err.line,
		       err.what ? err.what : "", td_case_error_text(rc));
		td_test_failures++;
		return;
	}
	TD_CHECK_EQ(t.count, 95);
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
	test_guard_refuses_run_fatal_opcodes();
	test_guard_refuses_opcodes_tos306_does_not_have();
	test_guard_refuses_pcgem_opcodes();
	test_guard_refuses_things_that_are_not_opcodes();
	test_guard_lets_the_safe_ones_through();
	test_guard_covers_preludes_too();
	test_the_shipped_table_still_loads();
	return TD_TEST_RESULT();
}
