/*
 * cases.c -- case table loader (spec section 6).  See cases.h for the format.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cases.h"

/* ------------------------------------------------------------------ */
/* Line iteration -- one rule, used by the parser and by the host tools */
/* ------------------------------------------------------------------ */

void td_line_iter_init(struct td_line_iter *it, const char *buf, size_t len)
{
	it->buf     = buf;
	it->len     = len;
	it->pos     = 0;
	it->line_no = 0;
	it->done    = 0;
}

int td_line_iter_next(struct td_line_iter *it, const char **start,
                      size_t *len, uint16_t *line_no, int *is_case)
{
	size_t begin, n, i;

	if (it->done)
		return 0;

	begin = it->pos;
	while (it->pos < it->len && it->buf[it->pos] != '\n')
		it->pos++;
	n = it->pos - begin;
	if (it->pos < it->len)
		it->pos++;                 /* step over the newline */
	else
		it->done = 1;              /* that was the last line */

	it->line_no++;

	/* a table edited on the Atari carries CRLF */
	if (n > 0 && it->buf[begin + n - 1] == '\r')
		n--;

	*start   = it->buf + begin;
	*len     = n;
	*line_no = it->line_no;

	/* blank and comment lines carry no case, but they do consume an id */
	i = 0;
	while (i < n && (it->buf[begin + i] == ' ' || it->buf[begin + i] == '\t'))
		i++;
	*is_case = !(i == n || it->buf[begin + i] == '#');
	return 1;
}

/* ------------------------------------------------------------------ */
/* Column and value parsing                                            */
/* ------------------------------------------------------------------ */

/*
 * Splits into at most `ncols` columns; the last one keeps its tabs.  The
 * count is per area -- five for the VDI form, seven for the AES form -- and
 * cannot be derived from the line, because the comment column may itself
 * hold tabs.
 */
static int split_columns(const char *line, size_t len,
                         const char **col, size_t *col_len, int ncols)
{
	int n = 0;
	size_t i = 0, start = 0;

	for (;;) {
		if (n == ncols - 1) {
			col[n]     = line + i;
			col_len[n] = len - i;
			return n + 1;
		}
		if (i >= len) {
			col[n]     = line + start;
			col_len[n] = i - start;
			return n + 1;
		}
		if (line[i] == '\t') {
			col[n]     = line + start;
			col_len[n] = i - start;
			n++;
			i++;
			start = i;
			continue;
		}
		i++;
	}
}

static void trim(const char **s, size_t *len)
{
	while (*len > 0 && ((*s)[0] == ' ' || (*s)[0] == '\t')) {
		(*s)++;
		(*len)--;
	}
	while (*len > 0 && ((*s)[*len - 1] == ' ' || (*s)[*len - 1] == '\t'))
		(*len)--;
}

/*
 * Parses one decimal or 0x-hex value.  Returns 0, or an error code.
 * The accepted range is -32768..65535: a case table states bit patterns as
 * often as it states numbers (0xFFFF for an all-ones line-style mask), and
 * both spellings must reach the array as the same 16 bits.
 */
static int parse_value(const char *s, size_t len, int16_t *out)
{
	int negative = 0;
	unsigned long v = 0;
	size_t i = 0;
	int base = 10, digits = 0;

	trim(&s, &len);
	if (len == 0)
		return TD_CASE_ERR_VALUE;

	if (s[0] == '-') {
		negative = 1;
		i = 1;
	} else if (s[0] == '+') {
		i = 1;
	}
	if (len - i > 2 && s[i] == '0' && (s[i + 1] == 'x' || s[i + 1] == 'X')) {
		base = 16;
		i += 2;
	}

	for (; i < len; i++) {
		int d;
		char c = s[i];

		if (c >= '0' && c <= '9')
			d = c - '0';
		else if (base == 16 && c >= 'a' && c <= 'f')
			d = c - 'a' + 10;
		else if (base == 16 && c >= 'A' && c <= 'F')
			d = c - 'A' + 10;
		else
			return TD_CASE_ERR_VALUE;

		v = v * (unsigned long)base + (unsigned long)d;
		digits++;
		if (v > 0x80000000uL)          /* far past anything representable */
			return TD_CASE_ERR_RANGE;
	}
	if (digits == 0)
		return TD_CASE_ERR_VALUE;

	if (negative) {
		if (v > 32768uL)
			return TD_CASE_ERR_RANGE;
		*out = (int16_t)(-(long)v);
	} else {
		if (v > 65535uL)
			return TD_CASE_ERR_RANGE;
		*out = (int16_t)(uint16_t)v;
	}
	return 0;
}

/* "-" means an empty list. */
static int parse_list(const char *s, size_t len, int16_t *dst, uint16_t max,
                      uint16_t *count)
{
	size_t i = 0, start = 0;
	uint16_t n = 0;

	*count = 0;
	trim(&s, &len);
	if (len == 0)
		return TD_CASE_ERR_VALUE;
	if (len == 1 && s[0] == '-')
		return 0;

	for (;;) {
		if (i == len || s[i] == ',') {
			int16_t v;
			int rc;

			if (n >= max)
				return TD_CASE_ERR_TOO_MANY_VALUES;
			rc = parse_value(s + start, i - start, &v);
			if (rc != 0)
				return rc;
			dst[n++] = v;
			if (i == len)
				break;
			i++;
			start = i;
			continue;
		}
		i++;
	}
	*count = n;
	return 0;
}

/* ------------------------------------------------------------------ */
/* Table parsing                                                       */
/* ------------------------------------------------------------------ */

static void fail(struct td_case_error *err, uint16_t line, const char *what)
{
	if (err) {
		err->line = line;
		err->what = what;
	}
}

int td_cases_parse(struct td_case_table *t, const char *buf, size_t len,
                   struct td_case_error *err)
{
	struct td_line_iter it;
	const char *line;
	size_t line_len;
	uint16_t line_no;
	uint16_t pending = 0;      /* preludes seen since the last case */
	uint16_t last_prelude = 0; /* line number of the newest one, for errors */
	int is_case;

	memset(t, 0, sizeof *t);
	t->crc = td_crc32((const uint8_t *)buf, len);
	if (err) {
		err->line = 0;
		err->what = NULL;
	}

	td_line_iter_init(&it, buf, len);
	while (td_line_iter_next(&it, &line, &line_len, &line_no, &is_case)) {
		const char *col[TD_CASE_COLUMNS];
		size_t col_len[TD_CASE_COLUMNS];
		struct td_case *c;
		int16_t v;
		int rc, ncol;

		if (!is_case)
			continue;

		ncol = split_columns(line, line_len, col, col_len,
		                     TD_CASE_COLUMNS);
		if (ncol < TD_CASE_COLUMNS) {
			fail(err, line_no, "needs five tab separated columns");
			return TD_CASE_ERR_COLUMNS;
		}

		/* A '+' opcode marks a prelude: a call issued after the reset and
		 * before the next case's measured call, and not recorded.  It is
		 * what lets a case prove an effect rather than only an echo. */
		{
			const char *o = col[0];
			size_t olen = col_len[0];

			trim(&o, &olen);
			if (td_case_line_is_prelude(line, line_len)) {
				struct td_prelude *p;

				if (pending >= TD_MAX_PRELUDES ||
				    t->prelude_count >= TD_MAX_PRELUDES) {
					fail(err, line_no, "more preludes than TD_MAX_PRELUDES");
					return TD_CASE_ERR_TOO_MANY_PRE;
				}
				p = &t->preludes[t->prelude_count + pending];
				memset(p, 0, sizeof *p);

				rc = parse_value(o + 1, olen - 1, &v);
				if (rc != 0 || v < 0) {
					fail(err, line_no, "prelude opcode is not a number");
					return rc != 0 ? rc : TD_CASE_ERR_RANGE;
				}
				p->opcode = v;

				rc = parse_value(col[1], col_len[1], &v);
				if (rc != 0 || v < 0) {
					fail(err, line_no, "prelude sub-opcode is not a number");
					return rc != 0 ? rc : TD_CASE_ERR_RANGE;
				}
				p->sub = v;

				rc = parse_list(col[2], col_len[2], p->intin,
				                TD_PRELUDE_VALUES, &p->n_intin);
				if (rc != 0) {
					fail(err, line_no, "prelude intin column");
					return rc;
				}
				rc = parse_list(col[3], col_len[3], p->ptsin,
				                TD_PRELUDE_VALUES, &p->n_ptsin);
				if (rc != 0) {
					fail(err, line_no, "prelude ptsin column");
					return rc;
				}
				if (p->n_ptsin & 1) {
					fail(err, line_no, "prelude ptsin holds an odd number of "
					                   "values, so it cannot be a point list");
					return TD_CASE_ERR_ODD_PTSIN;
				}
				pending++;
				last_prelude = line_no;
				continue;
			}
		}

		if (t->count >= TD_MAX_CASES) {
			fail(err, line_no, "more cases than TD_MAX_CASES");
			return TD_CASE_ERR_TOO_MANY_CASES;
		}

		c = &t->cases[t->count];
		memset(c, 0, sizeof *c);
		c->line = line_no;
		c->prelude_first = t->prelude_count;
		c->prelude_count = pending;
		t->prelude_count = (uint16_t)(t->prelude_count + pending);
		pending = 0;

		rc = parse_value(col[0], col_len[0], &v);
		if (rc != 0) {
			fail(err, line_no, "opcode is not a number");
			return rc;
		}
		if (v < 0) {
			fail(err, line_no, "opcode is negative");
			return TD_CASE_ERR_RANGE;
		}
		c->opcode = v;

		rc = parse_value(col[1], col_len[1], &v);
		if (rc != 0) {
			fail(err, line_no, "sub-opcode is not a number");
			return rc;
		}
		if (v < 0) {
			fail(err, line_no, "sub-opcode is negative");
			return TD_CASE_ERR_RANGE;
		}
		c->sub = v;

		rc = parse_list(col[2], col_len[2], c->intin, TD_VDI_INTIN_N,
		                &c->n_intin);
		if (rc != 0) {
			fail(err, line_no, "intin column");
			return rc;
		}

		rc = parse_list(col[3], col_len[3], c->ptsin, TD_VDI_PTSIN_N,
		                &c->n_ptsin);
		if (rc != 0) {
			fail(err, line_no, "ptsin column");
			return rc;
		}
		if (c->n_ptsin & 1) {
			fail(err, line_no, "ptsin holds an odd number of values, so it "
			                   "cannot be a list of x,y points");
			return TD_CASE_ERR_ODD_PTSIN;
		}

		td_case_line_comment(line, line_len, c->comment, TD_COMMENT_LEN);

		t->count++;
	}

	/* A prelude with nothing to precede would silently never run. */
	if (pending > 0) {
		fail(err, last_prelude, "prelude has no case after it");
		return TD_CASE_ERR_DANGLING;
	}

	return 0;
}

/* ------------------------------------------------------------------ */
/* AES table parsing                                                   */
/* ------------------------------------------------------------------ */
/*
 * Seven columns instead of five, and the four count columns are the
 * difference that matters: in the AES every control slot is an input
 * (EmuTOS aes/gemsuper.c:435 copies control[] wholesale onto a local buffer
 * and never writes it back), and the dispatcher's marshalling is driven by
 * those counts alone -- min(IN_LEN, I_SIZE) words in, exactly OUT_LEN words
 * back out (gemsuper.c:436-445).  Stating them separately from the values a
 * case lists is what lets a case measure that rule.
 */

/*
 * ------------------------------------------------------------------
 * The refusal guard
 * ------------------------------------------------------------------
 *
 * A case table is data.  A typo in it must not hang the machine it is
 * carried to, and it must not be mistakable for a real defect once it does.
 * So the loader refuses the whole table -- named reason, non-zero return,
 * the offending physical line -- when a case or a prelude names an opcode
 * from one of three classes:
 *
 *   1. it stops the run finishing.  Most of these wait on the operator; two
 *      (appl_exit, shel_write) end or replace the application instead, which
 *      is the same outcome by a different route.
 *   2. TOS 3.06's AES does not have it.  EmuTOS' own version ladder records
 *      TOS 3.06 as AES 3.20 (include/config.h:1345-1354), and an opcode
 *      compiled out at that level reaches crysbind's default arm, which pops
 *      fm_show(ALNOFUNC) -- a modal alert waiting for a click
 *      (gemsuper.c:411-417).
 *   3. it is not an AES opcode at all.  Same default arm, same alert.
 *      Opcode 0 is the single exception: gemsuper.c:414 skips the alert for
 *      it explicitly ("Ignore zero since some PRGs are this call").
 *
 * There is no override mechanism, deliberately.  Taking an opcode off this
 * list is a source change with a citation, and that friction is the point:
 * the alternative is a data file hanging a machine with no way to tell that
 * apart from a defect in the thing being measured.
 *
 * The table below is also the complete list of opcodes that HAVE an arm in
 * crysbind (aes/gemsuper.c:87-409, 79 of them); `refusal` is NULL for the
 * ones a table may issue and the reason otherwise.  Anything absent from it
 * is class 3.
 */
struct td_aes_opcode {
	int16_t     opcode;
	const char *refusal;      /* NULL = may be issued */
};

/* class 1, sub-reason a: waits on the operator */
#define BLOCKS "blocks on the operator: "
/* class 1, sub-reason b: the run never gets back here */
#define FATAL  "never returns to the run: "
/* class 2 */
#define GATED  "not in TOS 3.06's AES (3.20, config.h:1345-1354); the "        \
               "dispatcher's default arm would pop a modal alert: "

static const struct td_aes_opcode aes_opcodes[] = {
	{  10, NULL },                                   /* appl_init      */
	{  11, BLOCKS "appl_read waits on an empty message queue "
	              "(gemsuper.c:100-103, gemaplib.c:88, gemqueue.c:99)" },
	{  12, NULL },                                   /* appl_write     */
	{  13, NULL },                                   /* appl_find      */
	{  14, NULL },                                   /* appl_tplay     */
	{  15, BLOCKS "appl_trecord loops until the operator has filled the "
	              "event buffer (gemaplib.c:216-217)" },
	{  17, NULL },                                   /* appl_yield     */
	{  19, FATAL  "appl_exit ends the application (gemsuper.c:120-123)" },
	{  20, BLOCKS "evnt_keybd waits for a key (gemsuper.c:127)" },
	{  21, BLOCKS "evnt_button waits for a mouse button (gemevlib.c:90)" },
	{  22, BLOCKS "evnt_mouse waits for the mouse to enter or leave a "
	              "rectangle (gemevlib.c:123-125)" },
	{  23, BLOCKS "evnt_mesag waits for a message (gemevlib.c:100-116, "
	              "gemaplib.c:88)" },
	{  24, NULL },                                   /* evnt_timer     */
	{  25, BLOCKS "evnt_multi waits unless a finite timer is among its "
	              "flags, which the opcode alone cannot guarantee "
	              "(gemevlib.c:152-155)" },
	{  26, NULL },                                   /* evnt_dclick    */
	{  30, NULL },                                   /* menu_bar       */
	{  31, NULL },                                   /* menu_icheck    */
	{  32, NULL },                                   /* menu_ienable   */
	{  33, NULL },                                   /* menu_tnormal   */
	{  34, NULL },                                   /* menu_text      */
	{  35, NULL },                                   /* menu_register  */
	{  36, GATED  "menu_popup is #if CONF_WITH_MENU_EXTENSION, which "
	              "AES_VERSION needs for 0x0330 (gemsuper.c:188-191)" },
	{  37, GATED  "menu_attach is #if CONF_WITH_MENU_EXTENSION "
	              "(gemsuper.c:192-194)" },
	{  38, GATED  "menu_istart is #if CONF_WITH_MENU_EXTENSION "
	              "(gemsuper.c:195-197)" },
	{  39, GATED  "menu_settings is #if CONF_WITH_MENU_EXTENSION "
	              "(gemsuper.c:198-200)" },
	{  40, NULL },                                   /* objc_add       */
	{  41, NULL },                                   /* objc_delete    */
	{  42, NULL },                                   /* objc_draw      */
	{  43, NULL },                                   /* objc_find      */
	{  44, NULL },                                   /* objc_offset    */
	{  45, NULL },                                   /* objc_order     */
	{  46, NULL },                                   /* objc_edit      */
	{  47, NULL },                                   /* objc_change    */
	{  48, GATED  "objc_sysvar is #if CONF_WITH_3D_OBJECTS, which "
	              "AES_VERSION needs for 0x0340 (gemsuper.c:232-236)" },
	{  50, BLOCKS "form_do runs a modal ev_multi(MU_KEYBD|MU_BUTTON) loop "
	              "with a zero timer (gemfmlib.c:312)" },
	{  51, NULL },                                   /* form_dial      */
	{  52, BLOCKS "form_alert ends in fm_do (gemfmalt.c:383)" },
	{  53, BLOCKS "form_error goes fm_show -> fm_alert -> fm_do "
	              "(gemfmlib.c:436, :400)" },
	{  54, NULL },                                   /* form_center    */
	{  55, NULL },                                   /* form_keybd     */
	{  56, BLOCKS "form_button waits for the button to come up, through "
	              "gr_watchbox or ev_button (gemfmlib.c:251, :256)" },
	{  70, BLOCKS "graf_rubbox tracks the mouse until the button comes up "
	              "(gemgrlib.c:211-215)" },
	{  71, BLOCKS "graf_dragbox tracks the mouse until the button comes up "
	              "(gemgrlib.c:257-263)" },
	{  72, NULL },                                   /* graf_mbox      */
	{  73, NULL },                                   /* graf_growbox   */
	{  74, NULL },                                   /* graf_shrinkbox */
	{  75, BLOCKS "graf_watchbox loops on gr_stilldn until the button "
	              "comes up (gemgrlib.c:348-352)" },
	{  76, BLOCKS "graf_slidebox is gr_dragbox (gemgrlib.c:388)" },
	{  77, NULL },                                   /* graf_handle    */
	{  78, NULL },                                   /* graf_mouse     */
	{  79, NULL },                                   /* graf_mkstate   */
	{  80, NULL },                                   /* scrp_read      */
	{  81, NULL },                                   /* scrp_write     */
	{  82, NULL },                                   /* scrp_clear     */
	{  90, BLOCKS "fsel_input runs the modal file selector, fs_input -> "
	              "fm_do (gemfslib.c:718)" },
	{  91, BLOCKS "fsel_exinput runs the modal file selector, fs_input -> "
	              "fm_do (gemfslib.c:718)" },
	{ 100, NULL },                                   /* wind_create    */
	{ 101, NULL },                                   /* wind_open      */
	{ 102, NULL },                                   /* wind_close     */
	{ 103, NULL },                                   /* wind_delete    */
	{ 104, NULL },                                   /* wind_get       */
	{ 105, NULL },                                   /* wind_set       */
	{ 106, NULL },                                   /* wind_find      */
	{ 107, NULL },                                   /* wind_update    */
	{ 108, NULL },                                   /* wind_calc      */
	{ 109, NULL },                                   /* wind_new       */
	{ 110, NULL },                                   /* rsrc_load      */
	{ 111, NULL },                                   /* rsrc_free      */
	{ 112, NULL },                                   /* rsrc_gaddr     */
	{ 113, NULL },                                   /* rsrc_saddr     */
	{ 114, NULL },                                   /* rsrc_obfix     */
	{ 120, NULL },                                   /* shel_read      */
	{ 121, FATAL  "shel_write sets sh_doexec / sh_nextapp, so the AES "
	              "launches something else instead of finishing the run "
	              "(gemshlib.c:157)" },
	{ 122, NULL },                                   /* shel_get       */
	{ 123, NULL },                                   /* shel_put       */
	{ 124, NULL },                                   /* shel_find      */
	{ 125, NULL },                                   /* shel_envrn     */
	{ 126, NULL },                                   /* shel_rdef      */
	{ 127, NULL }                                    /* shel_wdef      */
};

#undef BLOCKS
#undef FATAL
#undef GATED

#define TD_AES_OPCODE_COUNT \
	((int)(sizeof aes_opcodes / sizeof aes_opcodes[0]))

/*
 * NULL if this opcode may be issued, otherwise the reason it may not.
 * Opcode 0 is allowed by name: it is the one unknown opcode the dispatcher
 * handles silently (gemsuper.c:414).
 */
static const char *aes_opcode_refusal(int16_t opcode)
{
	int i;

	if (opcode == 0)
		return NULL;

	for (i = 0; i < TD_AES_OPCODE_COUNT; i++)
		if (aes_opcodes[i].opcode == opcode)
			return aes_opcodes[i].refusal;

	return "not an AES opcode: crysbind has no arm for it, so it would "
	       "reach the default arm and pop a modal alert "
	       "(gemsuper.c:406-417)";
}

/* One AES line's payload, shared by the case and the prelude branches so
 * the two cannot drift apart. */
struct td_aes_line {
	int16_t  opcode;
	int16_t  n_intin;
	int16_t  n_intout;
	int16_t  n_addrin;
	int16_t  n_addrout;
	uint16_t n_values;
	int16_t  intin[TD_AES_INTIN_N];
};

/*
 * One count column.  "-" is accepted only where `dash_value` is not
 * negative -- that is the n_intin column, where it means "as many as the
 * value column lists".  In the other three there is no list to derive a
 * count from and "0" is one character away, so guessing there would
 * silently change what the call declares.
 */
static int parse_count(const char *s, size_t len, int dash_value, int16_t *out)
{
	const char *p = s;
	size_t n = len;
	int16_t v;
	int rc;

	trim(&p, &n);
	if (n == 1 && p[0] == '-') {
		if (dash_value < 0)
			return TD_CASE_ERR_VALUE;
		*out = (int16_t)dash_value;
		return 0;
	}
	rc = parse_value(s, len, &v);
	if (rc != 0)
		return rc;
	if (v < 0)
		return TD_CASE_ERR_RANGE;
	*out = v;
	return 0;
}

/* Columns 1..6 of one AES line.  `opcode_col` already has the '+' of a
 * prelude stripped by the caller. */
static int parse_aes_columns(const char *opcode_col, size_t opcode_len,
                             const char **col, const size_t *col_len,
                             struct td_aes_line *out, const char **what)
{
	int16_t v;
	int rc;

	memset(out, 0, sizeof *out);

	rc = parse_value(opcode_col, opcode_len, &v);
	if (rc != 0) {
		*what = "opcode is not a number";
		return rc;
	}
	if (v < 0) {
		*what = "opcode is negative";
		return TD_CASE_ERR_RANGE;
	}
	out->opcode = v;

	/* The guard, before anything else about the line matters: an opcode
	 * that would hang the machine is refused whether it is a case or a
	 * prelude, and whether or not the rest of the line parses. */
	*what = aes_opcode_refusal(out->opcode);
	if (*what != NULL)
		return TD_CASE_ERR_UNSAFE_OPCODE;

	/* the values first: the n_intin column's "-" is derived from them */
	rc = parse_list(col[5], col_len[5], out->intin, TD_AES_INTIN_N,
	                &out->n_values);
	if (rc != 0) {
		*what = "int_in value column";
		return rc;
	}

	rc = parse_count(col[1], col_len[1], (int)out->n_values, &out->n_intin);
	if (rc != 0) {
		*what = "n_intin column";
		return rc;
	}
	rc = parse_count(col[2], col_len[2], -1, &out->n_intout);
	if (rc != 0) {
		*what = "n_intout column (\"-\" has nothing to be derived from)";
		return rc;
	}
	/*
	 * OUT_LEN is the one count the dispatcher does not clamp:
	 * memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD))
	 * (gemsuper.c:444-445) writes into the CALLER's array, so a larger
	 * value overruns it on the target.  Refuse the table instead of
	 * letting a typo corrupt memory on real hardware.
	 */
	if (out->n_intout > TD_AES_INTOUT_N) {
		*what = "n_intout is larger than the intout array, which the "
		        "dispatcher's copy-back does not clamp";
		return TD_CASE_ERR_OUT_LEN;
	}
	rc = parse_count(col[3], col_len[3], -1, &out->n_addrin);
	if (rc != 0) {
		*what = "n_addrin column (\"-\" has nothing to be derived from)";
		return rc;
	}
	rc = parse_count(col[4], col_len[4], -1, &out->n_addrout);
	if (rc != 0) {
		*what = "n_addrout column (\"-\" has nothing to be derived from)";
		return rc;
	}

	return 0;
}

int td_aes_cases_parse(struct td_aes_case_table *t, const char *buf, size_t len,
                       struct td_case_error *err)
{
	struct td_line_iter it;
	const char *line;
	size_t line_len;
	uint16_t line_no;
	uint16_t pending = 0;      /* preludes seen since the last case */
	uint16_t last_prelude = 0; /* line number of the newest one, for errors */
	int is_case;

	memset(t, 0, sizeof *t);
	t->crc = td_crc32((const uint8_t *)buf, len);
	if (err) {
		err->line = 0;
		err->what = NULL;
	}

	td_line_iter_init(&it, buf, len);
	while (td_line_iter_next(&it, &line, &line_len, &line_no, &is_case)) {
		const char *col[TD_AES_CASE_COLUMNS];
		size_t col_len[TD_AES_CASE_COLUMNS];
		struct td_aes_line parsed;
		const char *what = NULL;
		const char *opcode_col;
		size_t opcode_len;
		int prelude, rc, i;

		if (!is_case)
			continue;

		if (split_columns(line, line_len, col, col_len,
		                  TD_AES_CASE_COLUMNS) < TD_AES_CASE_COLUMNS) {
			fail(err, line_no, "needs seven tab separated columns");
			return TD_CASE_ERR_COLUMNS;
		}

		prelude = td_case_line_is_prelude(line, line_len);
		opcode_col = col[0];
		opcode_len = col_len[0];
		trim(&opcode_col, &opcode_len);
		if (prelude) {                 /* step over the '+' */
			opcode_col++;
			opcode_len--;
		}

		rc = parse_aes_columns(opcode_col, opcode_len, col, col_len,
		                       &parsed, &what);
		if (rc != 0) {
			fail(err, line_no, what);
			return rc;
		}

		/*
		 * A '+' opcode marks a prelude: a call issued before the next
		 * case's measured call, and not recorded.  Round 1 needs none,
		 * but the convention is shared with the VDI table and with
		 * host/casefile.c through td_case_line_is_prelude -- a reader
		 * that counted preludes as cases would index every record
		 * wrongly.
		 */
		if (prelude) {
			struct td_aes_prelude *p;

			if (pending >= TD_MAX_PRELUDES ||
			    t->prelude_count >= TD_MAX_PRELUDES) {
				fail(err, line_no, "more preludes than TD_MAX_PRELUDES");
				return TD_CASE_ERR_TOO_MANY_PRE;
			}
			p = &t->preludes[t->prelude_count + pending];
			memset(p, 0, sizeof *p);
			p->opcode    = parsed.opcode;
			p->n_intin   = parsed.n_intin;
			p->n_intout  = parsed.n_intout;
			p->n_addrin  = parsed.n_addrin;
			p->n_addrout = parsed.n_addrout;
			p->n_values  = parsed.n_values;
			for (i = 0; i < (int)parsed.n_values; i++)
				p->intin[i] = parsed.intin[i];
			pending++;
			last_prelude = line_no;
			continue;
		}

		if (t->count >= TD_MAX_CASES) {
			fail(err, line_no, "more cases than TD_MAX_CASES");
			return TD_CASE_ERR_TOO_MANY_CASES;
		}

		{
			struct td_aes_case *c = &t->cases[t->count];

			memset(c, 0, sizeof *c);
			c->line          = line_no;
			c->prelude_first = t->prelude_count;
			c->prelude_count = pending;
			t->prelude_count = (uint16_t)(t->prelude_count + pending);
			pending = 0;

			c->opcode    = parsed.opcode;
			c->n_intin   = parsed.n_intin;
			c->n_intout  = parsed.n_intout;
			c->n_addrin  = parsed.n_addrin;
			c->n_addrout = parsed.n_addrout;
			c->n_values  = parsed.n_values;
			for (i = 0; i < (int)parsed.n_values; i++)
				c->intin[i] = parsed.intin[i];

			td_case_line_comment_n(line, line_len, TD_AES_CASE_COLUMNS,
			                       c->comment, TD_COMMENT_LEN);
			t->count++;
		}
	}

	/* A prelude with nothing to precede would silently never run. */
	if (pending > 0) {
		fail(err, last_prelude, "prelude has no case after it");
		return TD_CASE_ERR_DANGLING;
	}

	return 0;
}

int td_case_line_is_prelude(const char *line, size_t len)
{
	size_t i = 0;

	while (i < len && (line[i] == ' ' || line[i] == '\t'))
		i++;
	return i < len && line[i] == '+';
}

void td_case_line_comment_n(const char *line, size_t len, int ncols,
                            char *dst, size_t dst_size)
{
	const char *col[TD_AES_CASE_COLUMNS];
	size_t col_len[TD_AES_CASE_COLUMNS];
	size_t n;

	if (dst_size == 0)
		return;
	if (ncols > TD_AES_CASE_COLUMNS ||
	    split_columns(line, len, col, col_len, ncols) < ncols) {
		dst[0] = '\0';
		return;
	}
	n = col_len[ncols - 1];
	if (n >= dst_size)
		n = dst_size - 1;
	memcpy(dst, col[ncols - 1], n);
	dst[n] = '\0';
}

void td_case_line_comment(const char *line, size_t len, char *dst,
                          size_t dst_size)
{
	td_case_line_comment_n(line, len, TD_CASE_COLUMNS, dst, dst_size);
}

/* Slurps a case table.  One reader for both table forms, so a table that
 * loads for one area cannot fail to load for the other. */
static int slurp(const char *path, char **raw_out, size_t *len_out,
                 struct td_case_error *err)
{
	FILE *f;
	long size;
	char *raw;
	size_t got;

	f = fopen(path, "rb");
	if (!f) {
		fail(err, 0, "cannot open the case table");
		return TD_CASE_ERR_IO;
	}
	if (fseek(f, 0, SEEK_END) != 0 || (size = ftell(f)) < 0 ||
	    fseek(f, 0, SEEK_SET) != 0) {
		fclose(f);
		fail(err, 0, "cannot seek the case table");
		return TD_CASE_ERR_IO;
	}
	raw = (char *)malloc((size_t)size + 1);
	if (!raw) {
		fclose(f);
		fail(err, 0, "out of memory reading the case table");
		return TD_CASE_ERR_IO;
	}
	got = fread(raw, 1, (size_t)size, f);
	fclose(f);
	if (got != (size_t)size) {
		free(raw);
		fail(err, 0, "short read on the case table");
		return TD_CASE_ERR_IO;
	}
	raw[got] = '\0';

	*raw_out = raw;
	*len_out = got;
	return 0;
}

int td_cases_load(struct td_case_table *t, const char *path,
                  struct td_case_error *err)
{
	char *raw;
	size_t got;
	int rc;

	rc = slurp(path, &raw, &got, err);
	if (rc != 0)
		return rc;

	rc = td_cases_parse(t, raw, got, err);
	free(raw);
	return rc;
}

int td_aes_cases_load(struct td_aes_case_table *t, const char *path,
                      struct td_case_error *err)
{
	char *raw;
	size_t got;
	int rc;

	rc = slurp(path, &raw, &got, err);
	if (rc != 0)
		return rc;

	rc = td_aes_cases_parse(t, raw, got, err);
	free(raw);
	return rc;
}

const char *td_case_error_text(int rc)
{
	switch (rc) {
	case 0:                          return "ok";
	case TD_CASE_ERR_COLUMNS:        return "fewer than five columns";
	case TD_CASE_ERR_VALUE:          return "a column is not a number";
	case TD_CASE_ERR_RANGE:          return "a value does not fit in 16 bits";
	case TD_CASE_ERR_ODD_PTSIN:      return "ptsin is not a list of points";
	case TD_CASE_ERR_TOO_MANY_VALUES:return "more values than the array holds";
	case TD_CASE_ERR_TOO_MANY_CASES: return "more cases than TD_MAX_CASES";
	case TD_CASE_ERR_IO:             return "the case table could not be read";
	case TD_CASE_ERR_DANGLING:       return "a prelude has no case after it";
	case TD_CASE_ERR_TOO_MANY_PRE:   return "more preludes than TD_MAX_PRELUDES";
	case TD_CASE_ERR_OUT_LEN:        return "n_intout is larger than the "
	                                        "intout array";
	case TD_CASE_ERR_UNSAFE_OPCODE:  return "the table names an opcode that "
	                                        "may not be issued";
	default:                         return "unknown error";
	}
}
