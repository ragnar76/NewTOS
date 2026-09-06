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

	/* blank, comment and directive lines carry no call, but they do
	 * consume an id: a case id is a physical line number and nothing may
	 * silently renumber the cases.  '!' is here rather than only in the
	 * AES parser so that host/casefile.c and the VDI and raster parsers
	 * agree with it about which line carries which case id. */
	i = 0;
	while (i < n && (it->buf[begin + i] == ' ' || it->buf[begin + i] == '\t'))
		i++;
	*is_case = !(i == n || it->buf[begin + i] == '#' ||
	             it->buf[begin + i] == '!');
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
/* class 2, sub-reason b: EmuTOS does not say whether TOS 3.06 has it.
 * CONF_WITH_PCGEM gates these four (gemsuper.c:115, :315, :398-403) but does
 * NOT appear in the AES_VERSION ladder (config.h:1345-1354), so the reference
 * cannot tell us either way.  If TOS 3.06 lacks them the dispatcher pops the
 * same modal alert, which on the TT means a run that never finishes and a
 * hang indistinguishable from a real defect.  Refused until a primary source
 * settles it -- Compendium or Profibuch, per CLAUDE.md rule 1's secondary
 * cross-references; EmuTOS alone cannot. */
#define PCGEM  "PC-GEM function whose presence in TOS 3.06 is unestablished "  \
               "(#if CONF_WITH_PCGEM, which is not in the AES_VERSION "        \
               "ladder); if absent, the default arm pops a modal alert: "

static const struct td_aes_opcode aes_opcodes[] = {
	{  10, NULL },                                   /* appl_init      */
	{  11, BLOCKS "appl_read waits on an empty message queue "
	              "(gemsuper.c:100-103, gemaplib.c:88, gemqueue.c:99)" },
	{  12, NULL },                                   /* appl_write     */
	{  13, NULL },                                   /* appl_find      */
	{  14, NULL },                                   /* appl_tplay     */
	{  15, BLOCKS "appl_trecord loops until the operator has filled the "
	              "event buffer (gemaplib.c:216-217)" },
	{  17, PCGEM "appl_yield (gemsuper.c:115-119)" },
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
	{  82, PCGEM "scrp_clear (gemsuper.c:315-319)" },
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
	{ 126, PCGEM "shel_rdef (gemsuper.c:398-401)" },
	{ 127, PCGEM "shel_wdef (gemsuper.c:398, :402-403)" }
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
 * ------------------------------------------------------------------
 * The window-manager guard (round 2)
 * ------------------------------------------------------------------
 *
 * The opcode guard above answers "may this call be issued at all".  Three
 * window-manager opcodes need a second question answered -- "with THESE
 * values?" -- because for them the hazard is in the parameters, not in the
 * opcode:
 *
 * 1. FIELDS THAT TAKE A POINTER OR AN UNCHECKED INDEX.  wind_get and
 *    wind_set select a field with int_in[1], and four of those fields are
 *    unusable from a pointer-free table:
 *      WF_NAME (2), WF_INFO (3) -- the classic binding passes a string
 *        pointer in int_in[2..3] (gemlib a_wind_get.c:344-347,
 *        a_wind_set.c:314-322).  EmuTOS' wm_get has no arm for them and
 *        returns FALSE (gemwmlib.c:1592), but NewTOS copies the title out
 *        through that pointer (tos-api/src/aes/window.rs:2055-2064), and
 *        wm_set stores it in gl_aname.te_ptext where the next frame draw
 *        dereferences it (gemwmlib.c:1657-1666).  The pointer would be
 *        0xDEAD-poison.
 *      WF_COLOR (18), WF_DCOLOR (19) -- gadget = pinwds[0] indexes
 *        w_tcolor[] / gl_wtcolor[] with NO range check (gemwmlib.c:1567-1576
 *        for the get, :1714-1727 for the set).  Poison is -8531.
 *    and one more only on wind_set:
 *      WF_NEWDESK (14) -- stores a tree pointer in gl_newdesk
 *        (gemwmlib.c:1683), which w_drawdesk then walks
 *        (gemwmlib.c:340-350).  wm_new does NOT clear it; only wm_init does
 *        (:1296-1300), so it also survives the reset.
 *    wind_get(WF_NEWDESK) is a different call: EmuTOS has no arm for it and
 *    returns FALSE without touching a pointer, and round 1 measures exactly
 *    that.
 *
 * 2. THE MESSAGE-QUEUE BOUND -- see TD_AES_MAX_OPEN_WINDOWS in cases.h for
 *    the mechanism.  The guard counts the distinct handles the table names
 *    in int_in[0] of every wind_open, over the whole table.
 *
 * 3. THE MOUSE-CONTROL BALANCE.  wind_update mode 2 (END_MCTRL,
 *    aesdefs.h:559-562) reaches fm_own(0) -- wm_update subtracts 2 and
 *    passes the rest on (gemwmlib.c:1789-1795) -- which decrements ml_ocnt
 *    unconditionally (gemfmlib.c:99).
 *    One without a matching mode 3 (BEG_MCTRL) drives it to -1, and wm_new only drains
 *    it `while (ml_ocnt > 0)` (gemwmlib.c:1880) -- so the reset does not
 *    heal it, and from then on every BEG/END pair is off by one: the BEG
 *    that should save gl_mntree and the mouse owner does not, and the END
 *    that should restore them does, with whatever ml_mnhold and ml_pmown
 *    last held.  The screen-sync half (modes 0 and 1) DOES heal, because
 *    unsync returns early at sy_tas <= 0 (gemflag.c:104-108), so it is not
 *    constrained.
 *
 * And one shape rule that makes the other three total: where the guard has
 * to read a value (the field, the handle, the mode), the case must state
 * it.  A case that leaves it poisoned would hide the very thing being
 * checked.
 */

/* wind_get / wind_set field numbers the guard knows by name */
#define WF_NAME     2
#define WF_INFO     3
#define WF_CXYWH    5
#define WF_NEWDESK 14
#define WF_COLOR   18
#define WF_DCOLOR  19

/*
 * A rectangle a case hands to a call that draws.  wind_open and
 * wind_set(WF_CXYWH) both reach draw_change, which does WORD arithmetic on
 * x+w and y+h throughout (rc_intersect / rc_union / mkpiece,
 * gemwrect.c:63-95) and hands the result to gsx_sclip unsorted
 * (gemwmlib.c:1028).  A negative extent or a sum that wraps is a table typo
 * whose damage on screen would look like a defect in the thing being
 * measured.
 *
 * wind_create is deliberately NOT covered: it only stores the rectangle
 * (gemwmlib.c:1369-1378) and draws nothing, so a marshalling case may state
 * fewer values there and let the rest stay poisoned.
 */
static int rect_is_drawable(const int16_t *r)
{
	int i;

	for (i = 0; i < 4; i++)
		if (r[i] < 0)
			return 0;
	if ((long)r[0] + (long)r[2] > 32767L)
		return 0;
	if ((long)r[1] + (long)r[3] > 32767L)
		return 0;
	return 1;
}

/*
 * The per-line half of the window guard: everything decidable from one line
 * on its own.  Applies to preludes exactly as to cases -- a prelude that
 * scribbled through a poisoned pointer would take the machine down just as
 * thoroughly, and it would do it before the case it was meant to set up.
 */
static int aes_line_window_refusal(const struct td_aes_line *l,
                                   const char **what)
{
	int16_t field;

	switch (l->opcode) {
	case 104:                                        /* wind_get */
	case 105:                                        /* wind_set */
		if (l->n_values < 2) {
			*what = "wind_get/wind_set select a field with int_in[1], "
			        "which the guard has to read: state it";
			return TD_CASE_ERR_UNSAFE_FIELD;
		}
		field = l->intin[1];
		if (field == WF_NAME || field == WF_INFO) {
			*what = "WF_NAME/WF_INFO carry a string pointer in "
			        "int_in[2..3] (a_wind_get.c:344-347) and this table "
			        "is pointer-free, so it would be 0xDEAD poison";
			return TD_CASE_ERR_UNSAFE_FIELD;
		}
		if (field == WF_COLOR || field == WF_DCOLOR) {
			*what = "WF_COLOR/WF_DCOLOR index w_tcolor[]/gl_wtcolor[] "
			        "with int_in[2] and EmuTOS does not range-check it "
			        "(gemwmlib.c:1567-1576, :1714-1727)";
			return TD_CASE_ERR_UNSAFE_FIELD;
		}
		if (l->opcode == 105 && field == WF_NEWDESK) {
			*what = "wind_set(WF_NEWDESK) stores a tree pointer in "
			        "gl_newdesk (gemwmlib.c:1683) which w_drawdesk then "
			        "walks, and wm_new does not clear it";
			return TD_CASE_ERR_UNSAFE_FIELD;
		}
		if (l->opcode == 105 && l->intin[0] == 0) {
			*what = "wind_set on handle 0 changes the DESKTOP window's "
			        "own fields, and wm_new sweeps handles >= 1 only "
			        "(gemwmlib.c:1886) -- only wm_init restores them "
			        "(:1275-1281), so nothing in the run undoes it";
			return TD_CASE_ERR_UNSAFE_STATE;
		}
		if (l->opcode == 105 && field == WF_CXYWH) {
			if (l->n_values < 6 || !rect_is_drawable(&l->intin[2])) {
				*what = "wind_set(WF_CXYWH) draws int_in[2..5]: all four "
				        "must be stated, non-negative, and x+w / y+h must "
				        "not wrap a WORD";
				return TD_CASE_ERR_GEOMETRY;
			}
		}
		break;
	case 103:                                        /* wind_delete */
		if (l->n_values < 1) {
			*what = "wind_delete names its handle in int_in[0], which the "
			        "guard has to read: state it";
			return TD_CASE_ERR_UNSAFE_STATE;
		}
		if (l->intin[0] == 0) {
			*what = "wind_delete(0) clears VF_INUSE on the DESKTOP window "
			        "(gemwmlib.c:1477-1481); wm_new only sweeps handles "
			        ">= 1 (:1886), so nothing in the run brings it back";
			return TD_CASE_ERR_UNSAFE_STATE;
		}
		break;
	case 101:                                        /* wind_open */
		if (l->n_values < 1) {
			*what = "wind_open names its handle in int_in[0], which the "
			        "message-queue bound has to read: state it";
			return TD_CASE_ERR_UNSAFE_STATE;
		}
		if (l->intin[0] == 0) {
			*what = "wind_open(0) runs w_obadd(W_TREE, ROOT, ROOT) "
			        "(gemwmlib.c:1421, :186-202), which makes the root its "
			        "own child; the next everyobj walk never terminates";
			return TD_CASE_ERR_UNSAFE_STATE;
		}
		if (l->n_values < 5 || !rect_is_drawable(&l->intin[1])) {
			*what = "wind_open draws int_in[1..4]: all four must be "
			        "stated, non-negative, and x+w / y+h must not wrap "
			        "a WORD";
			return TD_CASE_ERR_GEOMETRY;
		}
		break;
	case 107:                                        /* wind_update */
		if (l->n_values < 1) {
			*what = "wind_update names its mode in int_in[0], which the "
			        "mouse-control balance has to read: state it";
			return TD_CASE_ERR_UNSAFE_STATE;
		}
		break;
	default:
		break;
	}

	return 0;
}

/*
 * The distinct wind_open handles a table names, accumulated over the whole
 * file.  Table-wide rather than per case because the message queue is never
 * drained: what bounds it is how many distinct handles the run ever opens,
 * and the reset only guarantees that a case's handles start again at 1.
 */
struct td_aes_open_set {
	int16_t handle[TD_AES_MAX_OPEN_WINDOWS];
	int     count;
};

static int open_set_add(struct td_aes_open_set *s, int16_t handle)
{
	int i;

	for (i = 0; i < s->count; i++)
		if (s->handle[i] == handle)
			return 0;
	if (s->count >= TD_AES_MAX_OPEN_WINDOWS)
		return -1;
	s->handle[s->count++] = handle;
	return 0;
}


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

	/* the window guard needs the values, so it runs last */
	return aes_line_window_refusal(out, what);
}

/*
 * One "!reset <TAB> name" line.  Refused rather than defaulted on anything
 * it does not recognise: a table that meant to declare a reset and misspelt
 * it would otherwise run round 1's absent one and produce a dump that looks
 * fine and measures the wrong thing.
 */
static int parse_aes_directive(const char *line, size_t len,
                               struct td_aes_case_table *t, int seen_call,
                               const char **what)
{
	const char *col[2];
	size_t col_len[2];
	const char *name;
	size_t name_len;

	if (t->reset >= 0) {
		*what = "the table declares a reset twice";
		return TD_CASE_ERR_DIRECTIVE;
	}
	if (seen_call) {
		*what = "the reset must be declared before the first case or "
		        "prelude; the calls before it would have run unreset";
		return TD_CASE_ERR_DIRECTIVE;
	}

	if (split_columns(line, len, col, col_len, 2) < 2) {
		*what = "a directive is \"!name <TAB> value\"";
		return TD_CASE_ERR_DIRECTIVE;
	}

	name = col[0];
	name_len = col_len[0];
	trim(&name, &name_len);
	name++;                            /* step over the '!' */
	name_len--;
	if (name_len != 5 || memcmp(name, "reset", 5) != 0) {
		*what = "the only directive is \"!reset\"";
		return TD_CASE_ERR_DIRECTIVE;
	}

	name = col[1];
	name_len = col_len[1];
	trim(&name, &name_len);
	if (name_len == 4 && memcmp(name, "none", 4) == 0) {
		t->reset = TD_AES_RESET_NONE;
		return 0;
	}
	if (name_len == 8 && memcmp(name, "wind_new", 8) == 0) {
		t->reset = TD_AES_RESET_WIND_NEW;
		return 0;
	}

	*what = "the reset must be \"none\" or \"wind_new\"; guessing here "
	        "would silently run the wrong round";
	return TD_CASE_ERR_DIRECTIVE;
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
	struct td_aes_open_set opened;
	int is_case;

	memset(t, 0, sizeof *t);
	memset(&opened, 0, sizeof opened);
	/* -1 = not declared yet, so a second "!reset" is an error rather than
	 * an override; it becomes TD_AES_RESET_NONE at the end if absent */
	t->reset = -1;
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

		rc = 0;

		if (!is_case) {
			if (!td_case_line_is_directive(line, line_len))
				continue;
			rc = parse_aes_directive(line, line_len, t,
			                         t->count > 0 || pending > 0, &what);
			if (rc != 0) {
				fail(err, line_no, what);
				return rc;
			}
			continue;
		}

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

		/* The message-queue bound, over the whole table: see
		 * TD_AES_MAX_OPEN_WINDOWS.  Preludes count -- a prelude that
		 * opens a window sends the same WM_REDRAW a case does. */
		if (parsed.opcode == 101 &&
		    open_set_add(&opened, parsed.intin[0]) != 0) {
			fail(err, line_no,
			     "more distinct wind_open handles than the AES message "
			     "queue holds; the dumper is the owner of every WM_REDRAW "
			     "and has no non-blocking way to drain its own queue");
			return TD_CASE_ERR_UNSAFE_STATE;
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

			/*
			 * The mouse-control balance, over this case's calls in the
			 * order the runner issues them: preludes first, then the
			 * measured call.  Per case and not across the table,
			 * because the reset runs in between and drains ml_ocnt --
			 * but only downwards (`while (ml_ocnt > 0)`,
			 * gemwmlib.c:1880), so a count driven negative inside a
			 * case is never healed.  The offending line reported is
			 * the case's: the balance is a property of the case as a
			 * whole, not of any one of its lines.
			 */
			{
				int ocnt = 0;
				uint16_t k;

				for (k = 0; k <= c->prelude_count; k++) {
					int16_t op, mode;

					if (k < c->prelude_count) {
						const struct td_aes_prelude *pp =
							&t->preludes[c->prelude_first + k];
						op   = pp->opcode;
						mode = pp->n_values ? pp->intin[0] : 0;
					} else {
						op   = c->opcode;
						mode = c->n_values ? c->intin[0] : 0;
					}
					if (op == 109) {
						/* wm_new drains ml_ocnt to zero
						 * (gemwmlib.c:1879-1880), so a BEG_MCTRL
						 * before it does not pay for an END_MCTRL
						 * after it */
						ocnt = 0;
						continue;
					}
					if (op != 107)
						continue;
					if (mode == 3)          /* BEG_MCTRL */
						ocnt++;
					else if (mode == 2)     /* END_MCTRL */
						ocnt--;
					if (ocnt < 0)
						break;
				}
				if (ocnt < 0) {
					fail(err, line_no,
					     "wind_update END_MCTRL without a matching "
					     "BEG_MCTRL in the same case: fm_own(0) "
					     "decrements ml_ocnt unconditionally "
					     "(gemfmlib.c:99) and wm_new only drains it "
					     "downwards (gemwmlib.c:1880)");
					return TD_CASE_ERR_UNSAFE_STATE;
				}
			}

			t->count++;
		}
	}

	/* A prelude with nothing to precede would silently never run. */
	if (pending > 0) {
		fail(err, last_prelude, "prelude has no case after it");
		return TD_CASE_ERR_DANGLING;
	}

	/* Absent means round 1's behaviour -- see cases.h for why the default
	 * may not be anything else. */
	if (t->reset < 0)
		t->reset = TD_AES_RESET_NONE;

	return 0;
}

int td_case_line_is_prelude(const char *line, size_t len)
{
	size_t i = 0;

	while (i < len && (line[i] == ' ' || line[i] == '\t'))
		i++;
	return i < len && line[i] == '+';
}

int td_case_line_is_directive(const char *line, size_t len)
{
	size_t i = 0;

	while (i < len && (line[i] == ' ' || line[i] == '\t'))
		i++;
	return i < len && line[i] == '!';
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

/* ------------------------------------------------------------------ */
/* VDI raster table parsing                                            */
/* ------------------------------------------------------------------ */
/*
 * The guard is documented in cases.h.  Its job is not to be clever: it is to
 * make it impossible to state a case that walks off a buffer on a machine
 * whose VDI does no bounds checking of its own.
 */

/* One MFDB column: buffer, fd_w, fd_h, fd_wdwidth, fd_stand, fd_nplanes. */
static int parse_mfdb(const char *s, size_t len, struct td_rast_mfdb *out,
                      const char **what)
{
	int16_t v[6];
	uint16_t n = 0;
	int rc;

	rc = parse_list(s, len, v, 6, &n);
	if (rc != 0) {
		*what = "MFDB column is not a list of numbers";
		return rc;
	}
	if (n != 6) {
		*what = "an MFDB column needs six values: buffer, fd_w, fd_h, "
		        "fd_wdwidth, fd_stand, fd_nplanes";
		return TD_CASE_ERR_COLUMNS;
	}

	out->buffer  = v[0];
	out->w       = v[1];
	out->h       = v[2];
	out->wdwidth = v[3];
	out->stand   = v[4];
	out->nplanes = v[5];

	if (out->buffer != TD_RAST_BUF_A && out->buffer != TD_RAST_BUF_B) {
		*what = "the buffer field of an MFDB is 0 (A) or 1 (B); there is no "
		        "screen MFDB in this table -- the two machines' screens "
		        "differ in size and depth, so nothing measured against one "
		        "could be compared";
		return TD_CASE_ERR_GEOMETRY;
	}
	if (out->w <= 0 || out->h <= 0 || out->wdwidth <= 0 || out->nplanes <= 0) {
		*what = "fd_w, fd_h, fd_wdwidth and fd_nplanes must all be positive";
		return TD_CASE_ERR_GEOMETRY;
	}
	if (out->stand != 0 && out->stand != 1) {
		*what = "fd_stand is 0 (device form) or 1 (standard form)";
		return TD_CASE_ERR_GEOMETRY;
	}
	if (out->nplanes > 16) {
		*what = "fd_nplanes above 16 is past the plane cap the VDI itself "
		        "applies (vdi_raster.c:963), so the call could only ever be "
		        "a no-op -- and the geometry bound below assumes 16";
		return TD_CASE_ERR_GEOMETRY;
	}
	if (out->w > (int16_t)(out->wdwidth * 16)) {
		*what = "fd_w is wider than fd_wdwidth describes; fd_wdwidth is "
		        "(fd_w + 15) / 16";
		return TD_CASE_ERR_GEOMETRY;
	}
	/* The bound that makes a mistyped digit harmless: sixteen WORDs per
	 * sixteen pixels covers the TT's planar reading (fd_nplanes <= 16
	 * interleaved WORDs) and NewTOS' chunky one (one WORD per pixel)
	 * alike.  32-bit arithmetic so the product cannot wrap. */
	if ((int32_t)out->h * (int32_t)out->wdwidth * 16L >
	    (int32_t)TD_RAST_BUF_WORDS) {
		*what = "the form does not fit the dumper's buffer: "
		        "fd_h * fd_wdwidth * 16 must be at most TD_RAST_BUF_WORDS";
		return TD_CASE_ERR_GEOMETRY;
	}
	return 0;
}

/* arb_corner (vdi_raster.c:116-124): the VDI sorts both rectangles into
 * (min,max) order before it uses them, so the guard must check what the VDI
 * will actually address, not what the table happens to spell. */
static void sort_rect(const int16_t *in, int16_t *x1, int16_t *y1,
                      int16_t *x2, int16_t *y2)
{
	*x1 = in[0] < in[2] ? in[0] : in[2];
	*x2 = in[0] < in[2] ? in[2] : in[0];
	*y1 = in[1] < in[3] ? in[1] : in[3];
	*y2 = in[1] < in[3] ? in[3] : in[1];
}

static int check_rect(const int16_t *pts, const struct td_rast_mfdb *m,
                      int16_t ext_w, int16_t ext_h, const char **what)
{
	int16_t x1, y1, x2, y2;

	sort_rect(pts, &x1, &y1, &x2, &y2);
	if (ext_w > 0) {                   /* destination: extent comes from src */
		x2 = (int16_t)(x1 + ext_w - 1);
		y2 = (int16_t)(y1 + ext_h - 1);
	}
	if (x1 < 0 || y1 < 0 || x2 >= m->w || y2 >= m->h) {
		*what = "the rectangle leaves the form it addresses; EmuTOS does "
		        "not bounds-check a memory MFDB (vdi_raster.c:915-950), so "
		        "the blit would scribble over the machine";
		return TD_CASE_ERR_GEOMETRY;
	}
	return 0;
}

static int parse_rast_line(const char **col, const size_t *col_len,
                           struct td_rast_case *c, const char **what)
{
	int16_t fill[2], v;
	uint16_t nfill = 0;
	int rc;

	rc = parse_value(col[0], col_len[0], &v);
	if (rc != 0) {
		*what = "opcode is not a number";
		return rc;
	}
	/* The opcode guard, before anything else about the line matters. */
	if (v != 109 && v != 110 && v != 121) {
		*what = "only the three raster opcodes belong in this table: "
		        "109 vro_cpyfm, 110 vr_trnfm, 121 vrt_cpyfm";
		return TD_CASE_ERR_UNSAFE_OPCODE;
	}
	c->opcode = v;

	rc = parse_list(col[1], col_len[1], c->intin, TD_RAST_INTIN_N,
	                &c->n_intin);
	if (rc != 0) {
		*what = "intin column";
		return rc;
	}
	rc = parse_list(col[2], col_len[2], c->ptsin, TD_RAST_PTSIN_N,
	                &c->n_ptsin);
	if (rc != 0) {
		*what = "ptsin column";
		return rc;
	}
	if (c->n_ptsin & 1) {
		*what = "ptsin holds an odd number of values, so it cannot be a "
		        "list of x,y points";
		return TD_CASE_ERR_ODD_PTSIN;
	}

	rc = parse_mfdb(col[3], col_len[3], &c->src, what);
	if (rc != 0)
		return rc;
	rc = parse_mfdb(col[4], col_len[4], &c->dst, what);
	if (rc != 0)
		return rc;

	rc = parse_list(col[5], col_len[5], fill, 2, &nfill);
	if (rc != 0) {
		*what = "fill column";
		return rc;
	}
	if (nfill != 2) {
		*what = "the fill column is two seeds: buffer A, buffer B";
		return TD_CASE_ERR_COLUMNS;
	}
	c->seed_a = (uint16_t)fill[0];
	c->seed_b = (uint16_t)fill[1];

	/*
	 * The rectangles.  vro_cpyfm and vrt_cpyfm read PTSIN[0..7]; vr_trnfm
	 * reads none.  A short PTSIN would leave the rest at the 0xDEAD poison,
	 * which is -8531 -- a coordinate the VDI would happily blit to.
	 */
	if (c->opcode == 110) {
		if (c->n_ptsin != 0) {
			*what = "vr_trnfm reads no rectangle; leave ptsin empty";
			return TD_CASE_ERR_GEOMETRY;
		}
		/*
		 * vr_trnfm walks fd_nplanes planes of fd_h * fd_wdwidth WORDs,
		 * both taken from the SOURCE MFDB (vdi_raster.c:298-300), and
		 * writes as many into the destination buffer.  No extra bound is
		 * needed: fd_nplanes is capped at 16 and parse_mfdb already
		 * required fd_h * fd_wdwidth * 16 to fit, so the transformed
		 * form fits either buffer by construction.
		 */
		return 0;
	}

	if (c->n_ptsin != 8) {
		*what = "vro_cpyfm and vrt_cpyfm read PTSIN[0..7]; state all eight "
		        "values, or the rest stays poisoned and the blit goes wild";
		return TD_CASE_ERR_GEOMETRY;
	}
	{
		int16_t x1, y1, x2, y2;

		rc = check_rect(c->ptsin, &c->src, 0, 0, what);
		if (rc != 0)
			return rc;
		/* The destination extent is the SOURCE extent, not PTSIN[6..7]
		 * (dont_clip, vdi_raster.c:882-900) -- so that is what the copy
		 * addresses and what has to fit. */
		sort_rect(c->ptsin, &x1, &y1, &x2, &y2);
		rc = check_rect(c->ptsin + 4, &c->dst, (int16_t)(x2 - x1 + 1),
		                (int16_t)(y2 - y1 + 1), what);
		if (rc != 0)
			return rc;
		/* PTSIN[6..7] as WRITTEN has to fit as well, even though the copy
		 * extent does not come from it: the overlap-reversal path starts
		 * from d_xmax/d_ymax, which dont_clip takes literally out of those
		 * two slots (vdi_raster.c:895-898, used at :995-999). */
		rc = check_rect(c->ptsin + 4, &c->dst, 0, 0, what);
		if (rc != 0)
			return rc;
	}
	return 0;
}

int td_rast_cases_parse(struct td_rast_case_table *t, const char *buf,
                        size_t len, struct td_case_error *err)
{
	struct td_line_iter it;
	const char *line;
	size_t line_len;
	uint16_t line_no;
	int is_case;

	memset(t, 0, sizeof *t);
	t->crc = td_crc32((const uint8_t *)buf, len);
	if (err) {
		err->line = 0;
		err->what = NULL;
	}

	td_line_iter_init(&it, buf, len);
	while (td_line_iter_next(&it, &line, &line_len, &line_no, &is_case)) {
		const char *col[TD_RAST_CASE_COLUMNS];
		size_t col_len[TD_RAST_CASE_COLUMNS];
		const char *what = NULL;
		struct td_rast_case *c;
		int rc;

		if (!is_case)
			continue;

		/* No preludes here: the raster opcodes read no attribute the reset
		 * does not already set.  Refused rather than mis-parsed, because
		 * host/casefile.c would skip such a line and the runner would not,
		 * and the two must agree on which line carries which case id. */
		if (td_case_line_is_prelude(line, line_len)) {
			fail(err, line_no, "the raster table has no preludes");
			return TD_CASE_ERR_UNSAFE_OPCODE;
		}

		if (split_columns(line, line_len, col, col_len,
		                  TD_RAST_CASE_COLUMNS) < TD_RAST_CASE_COLUMNS) {
			fail(err, line_no, "needs seven tab separated columns");
			return TD_CASE_ERR_COLUMNS;
		}
		if (t->count >= TD_RAST_MAX_CASES) {
			fail(err, line_no, "more cases than TD_RAST_MAX_CASES");
			return TD_CASE_ERR_TOO_MANY_CASES;
		}

		c = &t->cases[t->count];
		memset(c, 0, sizeof *c);
		c->line = line_no;

		rc = parse_rast_line(col, col_len, c, &what);
		if (rc != 0) {
			fail(err, line_no, what);
			return rc;
		}

		td_case_line_comment_n(line, line_len, TD_RAST_CASE_COLUMNS,
		                       c->comment, TD_COMMENT_LEN);
		t->count++;
	}

	return 0;
}

int td_rast_cases_load(struct td_rast_case_table *t, const char *path,
                       struct td_case_error *err)
{
	char *raw;
	size_t got;
	int rc;

	rc = slurp(path, &raw, &got, err);
	if (rc != 0)
		return rc;

	rc = td_rast_cases_parse(t, raw, got, err);
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
	case TD_CASE_ERR_GEOMETRY:       return "a form or a rectangle would "
	                                        "leave the dumper's buffer";
	case TD_CASE_ERR_DIRECTIVE:      return "a '!' directive line cannot be "
	                                        "honoured";
	case TD_CASE_ERR_UNSAFE_FIELD:   return "the table names a field that "
	                                        "may not be issued";
	case TD_CASE_ERR_UNSAFE_STATE:   return "the table would leave a state "
	                                        "the run cannot come back from";
	default:                         return "unknown error";
	}
}
