/*
 * cases.h -- case table loader (spec section 6).
 *
 * Format, for cases/vdi.tsv and cases/aes.tsv:
 *
 *   opcode <TAB> sub <TAB> intin values <TAB> ptsin values <TAB> comment
 *
 * Values are comma separated, decimal or 0x-hex, and "-" means none.  A line
 * whose first non-blank character is '#' is a comment, and so is an empty
 * line; both still consume a line number, because **a case id is the physical
 * 1-based line number** and nothing may silently renumber the cases.  The
 * comment column is everything from the fifth column on, tabs included.
 *
 * A line whose opcode field starts with '+' is a PRELUDE, not a case: a call
 * issued after the reset and before the next case's measured call, and not
 * recorded. That is what makes an effect measurable rather than only an echo
 * -- a setter's echo proves the echo, and only a read-back after a
 * non-default set proves the effect. Preludes stack; several may precede one
 * case. A prelude consumes a line number like any other line, and a prelude
 * with no case after it is an error.
 *
 * A malformed line is an error, never a skipped case: a table that half-loads
 * would produce a dump that compares against the wrong cases.
 *
 * The table is loaded at runtime from the same directory as the executable and
 * is never compiled in -- the table must be provably identical for both runs,
 * and recompiling between runs makes that unprovable.
 */
#ifndef TOSDUMP_CASES_H
#define TOSDUMP_CASES_H

#include "record.h"

/*
 * Tab separated columns of one case line, per table form.  The comment is
 * everything from the last column on, tabs included, so the count is what
 * tells a reader where the comment starts -- and it cannot be derived from
 * a line, because a comment may itself hold tabs.
 */
#define TD_CASE_COLUMNS      5   /* VDI form, and the BIOS/GEMDOS form   */

/* Values a prelude may carry.  Small on purpose: a prelude sets one piece of
 * state, and the target holds the whole table in static memory. */
#define TD_PRELUDE_VALUES  16
#define TD_MAX_PRELUDES   256

struct td_prelude {
	int16_t  opcode;
	int16_t  sub;
	uint16_t n_intin;
	uint16_t n_ptsin;
	int16_t  intin[TD_PRELUDE_VALUES];
	int16_t  ptsin[TD_PRELUDE_VALUES];
};

struct td_case {
	uint16_t line;                     /* physical line number = case id  */
	uint16_t prelude_first;            /* index into the table's pool     */
	uint16_t prelude_count;            /* calls issued before this case   */
	int16_t  opcode;                   /* contrl[0]                       */
	int16_t  sub;                      /* contrl[5]                       */
	uint16_t n_intin;                  /* values, becomes contrl[3]       */
	uint16_t n_ptsin;                  /* values; contrl[1] = n_ptsin / 2 */
	int16_t  intin[TD_VDI_INTIN_N];
	int16_t  ptsin[TD_VDI_PTSIN_N];
	char     comment[TD_COMMENT_LEN];
};

struct td_case_table {
	uint32_t          crc;             /* CRC32 over the raw file bytes   */
	uint16_t          count;
	uint16_t          prelude_count;
	struct td_case    cases[TD_MAX_CASES];
	struct td_prelude preludes[TD_MAX_PRELUDES];
};

#define TD_CASE_ERR_COLUMNS         (-1)   /* fewer than five columns      */
#define TD_CASE_ERR_VALUE           (-2)   /* a column is not a number     */
#define TD_CASE_ERR_RANGE           (-3)   /* a value does not fit         */
#define TD_CASE_ERR_ODD_PTSIN       (-4)   /* ptsin is not a point list    */
#define TD_CASE_ERR_TOO_MANY_VALUES (-5)   /* more values than the array   */
#define TD_CASE_ERR_TOO_MANY_CASES  (-6)   /* more than TD_MAX_CASES       */
#define TD_CASE_ERR_IO              (-7)   /* file could not be read       */
#define TD_CASE_ERR_DANGLING        (-8)   /* prelude with no case after   */
#define TD_CASE_ERR_TOO_MANY_PRE    (-9)   /* more than TD_MAX_PRELUDES    */
#define TD_CASE_ERR_OUT_LEN        (-10)  /* n_intout > the intout array  */
#define TD_CASE_ERR_UNSAFE_OPCODE  (-11)  /* opcode may not be issued     */
#define TD_CASE_ERR_GEOMETRY       (-12)  /* a form or rect leaves the buf*/
#define TD_CASE_ERR_DIRECTIVE      (-13)  /* a '!' line cannot be honoured*/
#define TD_CASE_ERR_UNSAFE_FIELD   (-14)  /* a field may not be issued    */
#define TD_CASE_ERR_UNSAFE_STATE   (-15)  /* the run could not finish     */

struct td_case_error {
	uint16_t    line;                  /* physical line, 0 if not a line  */
	const char *what;                  /* static description              */
};

int td_cases_parse(struct td_case_table *t, const char *buf, size_t len,
                   struct td_case_error *err);
int td_cases_load(struct td_case_table *t, const char *path,
                  struct td_case_error *err);

const char *td_case_error_text(int rc);

/* Copies the comment column -- everything from column five on, tabs included
 * -- of one case line.  Shared so the runner and the host tools name a case
 * the same way.  This is the FIVE column (VDI) rule; see
 * td_case_line_comment_n for tables with a different column count. */
void td_case_line_comment(const char *line, size_t len, char *dst,
                          size_t dst_size);

/*
 * The same rule for a table with `ncols` columns: the comment is everything
 * from column `ncols` on, tabs included.  The column count is per area (five
 * for the VDI form, seven for the AES form), which is why it cannot be
 * derived from the line: a VDI comment may itself hold tabs, so counting the
 * tabs on a line does not tell you which form it is.
 */
void td_case_line_comment_n(const char *line, size_t len, int ncols,
                            char *dst, size_t dst_size);

/* Is this line a prelude rather than a case?  Shared so every reader agrees
 * on what carries a case id -- a comparator that counted preludes as cases
 * would index every record wrongly. */
int td_case_line_is_prelude(const char *line, size_t len);

/* Is this line a '!' directive?  A directive carries no call at all, so the
 * shared line iterator already reports it as not-a-case; this is how the
 * parser that owns the directive picks it back out. */
int td_case_line_is_directive(const char *line, size_t len);

/* ------------------------------------------------------------------ */
/* AES case tables (cases/aes-*.tsv)                                   */
/* ------------------------------------------------------------------ */
/*
 * Format, seven tab separated columns:
 *
 *   opcode <TAB> n_intin <TAB> n_intout <TAB> n_addrin <TAB> n_addrout
 *          <TAB> intin values <TAB> comment
 *
 * The four count columns become control[1..4] and are stated EXPLICITLY
 * rather than derived from the value list.  That is the whole point of the
 * form: in the AES all five control slots are inputs, and IN_LEN / OUT_LEN
 * being settable independently of the values actually listed is what lets a
 * case measure the dispatcher's marshalling rule -- xif copies
 * min(IN_LEN, I_SIZE) words in and exactly OUT_LEN words back out
 * (EmuTOS aes/gemsuper.c:428-448, include/aesdefs.h:121-134).  A case that
 * declares fewer output words than the call produces leaves the surplus
 * slots poisoned, and that absence is the measurement.
 *
 * A "-" in the n_intin column means "as many as the value column lists".
 * In the other three count columns "-" is an ERROR: there is no value list
 * to derive them from, and 0 is one character away.  Guessing there would
 * silently change what the call declares.
 *
 * n_intout is refused above TD_AES_INTOUT_N.  xif's copy-back is
 * memcpy(pcrys_blk->intout, int_out, OUT_LEN*sizeof(WORD)) with NO clamp
 * (gemsuper.c:444-445), so a larger OUT_LEN overruns the caller's own
 * intout array on the target.  The other three counts are clamped by the
 * dispatcher (min(IN_LEN,I_SIZE), min(AIN_LEN,AI_SIZE)) or never read at
 * all (control[4]), so they are only refused when negative.
 *
 * Everything else -- physical line number as the case id, "#" comments,
 * blank lines, the "+" prelude convention, malformed line is an error -- is
 * identical to the VDI form above, and goes through the same line iterator
 * and the same value parser so the two cannot drift apart.
 */
#define TD_AES_CASE_COLUMNS 7

/* ------------------------------------------------------------------ */
/* The reset directive (spec section 5, for the AES)                   */
/* ------------------------------------------------------------------ */
/*
 * A line whose first non-blank character is '!' is a DIRECTIVE: it carries
 * no case and no call, but it does consume a line number like every other
 * line, so case ids stay the physical line numbers.  One directive exists:
 *
 *   !reset <TAB> none | wind_new
 *
 * It must appear before the first case or prelude, at most once, and its
 * value must be one of the names below -- an unknown name is refused rather
 * than defaulting, because defaulting would silently run a round-2 table
 * with round 1's absent reset.
 *
 * WHY THE TABLE DECLARES IT AND NOT THE PROGRAM.  Spec section 5 puts the
 * reset sequence in core/ so both targets run the identical one, and it is
 * in core/ here too -- td_reset_aes(), one fixed call.  What the table
 * chooses is only WHICH named reset runs, and that has to be the table's:
 * the dumper is double-clicked on the TT with no arguments and always reads
 * AES.TSV, so the file is the only thing that can tell two rounds apart, and
 * the file's CRC is what the dump header carries.  A reset selected by a
 * command line or a rebuild would not be provable from the dump.
 *
 * ABSENT MEANS `none`, and that is deliberate rather than lazy: round 1's
 * table (cases/aes-r1.tsv, CRC 33C13C8F) is frozen and its dumps are taken.
 * Any other default would silently change what re-running it measures, and
 * the comparator would then compare a reset run against an unreset reference
 * and call the difference a defect.
 *
 * `wind_new` issues opcode 109 with control[1..4] = 0, exactly as gemlib
 * lays it (a_wind_new.c:25).  It is the AES' OWN reset: wm_new closes and
 * deletes every window with a handle >= 1 and drains the wind_update mouse
 * and screen locks (gemwmlib.c:1875-1893).  EmuTOS issues it itself after
 * every graphics-mode program the shell runs (gemshlib.c:676-677), so a run
 * that issues it leaves nothing behind that the shell would not have swept
 * anyway.
 */
#define TD_AES_RESET_NONE      0
#define TD_AES_RESET_WIND_NEW  1

/*
 * How many DISTINCT window handles one table may open.  This is a hang
 * bound, not a style limit.
 *
 * Every wind_open sends the window's owner one WM_REDRAW (w_redraw,
 * gemwmlib.c:887-908, reached through draw_change -> w_update), and the
 * dumper is that owner.  It never reads its own message queue: every AES
 * call that would drain one blocks on an empty queue, so there is no
 * non-blocking drain to put in the reset.  doq unions successive WM_REDRAWs
 * for the same handle (gemqueue.c:45-56), so what accumulates is one message
 * per distinct handle; p_queue holds QUEUE_SIZE = 128 bytes = eight 16-byte
 * messages (struct.h:56, :152), and aqueue BLOCKS the sender when the next
 * message does not fit (gemqueue.c:96, :124).  Here the sender and the
 * receiver are the same process, so that block never ends.
 *
 * The reset frees every handle, so wm_create hands out handle 1 again
 * (gemwmlib.c:1369-1378) and the distinct-handle count over a whole run is
 * the largest number any single case opens.  Four leaves half the queue
 * free for the control manager's own messages, which an operator touching
 * the mouse during the run would generate.
 */
#define TD_AES_MAX_OPEN_WINDOWS 4

struct td_aes_prelude {
	int16_t  opcode;
	int16_t  n_intin;                  /* control[1], IN_LEN              */
	int16_t  n_intout;                 /* control[2], OUT_LEN             */
	int16_t  n_addrin;                 /* control[3], AIN_LEN             */
	int16_t  n_addrout;                /* control[4]                      */
	uint16_t n_values;                 /* int_in values actually listed   */
	int16_t  intin[TD_AES_INTIN_N];
};

struct td_aes_case {
	uint16_t line;                     /* physical line number = case id  */
	uint16_t prelude_first;            /* index into the table's pool     */
	uint16_t prelude_count;            /* calls issued before this case   */
	int16_t  opcode;                   /* control[0]                      */
	int16_t  n_intin;                  /* control[1], IN_LEN              */
	int16_t  n_intout;                 /* control[2], OUT_LEN             */
	int16_t  n_addrin;                 /* control[3], AIN_LEN             */
	int16_t  n_addrout;                /* control[4]                      */
	uint16_t n_values;                 /* int_in values actually listed   */
	int16_t  intin[TD_AES_INTIN_N];
	char     comment[TD_COMMENT_LEN];
};

struct td_aes_case_table {
	uint32_t              crc;         /* CRC32 over the raw file bytes   */
	int16_t               reset;       /* TD_AES_RESET_*, from "!reset"   */
	uint16_t              count;
	uint16_t              prelude_count;
	struct td_aes_case    cases[TD_MAX_CASES];
	struct td_aes_prelude preludes[TD_MAX_PRELUDES];
};

int td_aes_cases_parse(struct td_aes_case_table *t, const char *buf,
                       size_t len, struct td_case_error *err);
int td_aes_cases_load(struct td_aes_case_table *t, const char *path,
                      struct td_case_error *err);

/* ------------------------------------------------------------------ */
/* VDI raster case tables (cases/vdi-raster.tsv)                       */
/* ------------------------------------------------------------------ */
/*
 * Format, seven tab separated columns:
 *
 *   opcode <TAB> intin <TAB> ptsin <TAB> src <TAB> dst <TAB> fill <TAB> comment
 *
 * `src` and `dst` each describe one MFDB, as six comma separated values:
 *
 *   buffer , fd_w , fd_h , fd_wdwidth , fd_stand , fd_nplanes
 *
 * `buffer` is 0 for buffer A and 1 for buffer B -- the two fixed pixel
 * buffers the dumper owns.  fd_addr is not in the table because it cannot
 * be: it is an address, and the whole point of the table is that it is the
 * same file on two machines.  Naming a buffer instead of an address is also
 * what lets both MFDBs name the SAME buffer, which is how the in-place
 * vr_trnfm cases are stated.
 *
 * `fill` is two values, the seeds the two buffers are laid with before the
 * call (td_rast_fill).  Two different seeds give a source and a destination
 * that no logic op can confuse.
 *
 * There is no sub-opcode column: none of the three raster opcodes is an
 * escape.  There are no preludes: the raster opcodes read no attribute the
 * reset does not already set, and a '+' line is refused rather than silently
 * treated as a case.
 *
 * ------------------------------------------------------------------
 * THE GUARD
 * ------------------------------------------------------------------
 *
 * The AES loader refuses opcodes that would hang the machine (see above).
 * The raster loader has the same duty for a different hazard: EmuTOS does
 * NOT bounds-check a memory MFDB.  setup_info takes fd_addr, fd_wdwidth and
 * fd_nplanes at face value (vdi_raster.c:915-950) and bit_blt then walks
 * whatever they describe; vdi_vr_trnfm does not even check fd_addr for NULL
 * (vdi_raster.c:295-297).  A mistyped digit in this file would therefore
 * scribble over a running TT, and the damage would look like a defect in
 * the thing being measured.
 *
 * So the loader refuses the whole table -- named reason, non-zero return,
 * the offending line -- unless every case satisfies all of:
 *
 *   1. the opcode is one of 109, 110, 121.  Nothing else belongs in a
 *      table whose record carries two MFDBs and two pixel buffers.
 *   2. both MFDBs name a real buffer.  A NULL MFDB pointer, or an MFDB with
 *      fd_addr == 0 (the screen), is not expressible here at all: EmuTOS
 *      dereferences the MFDB pointer unconditionally, and the screen is a
 *      different size and depth on the two machines, so nothing measured
 *      against it could be compared anyway.
 *   3. the form fits the buffer under EITHER machine's reading of it:
 *      fd_h * fd_wdwidth * 16 <= TD_RAST_BUF_WORDS.  Sixteen, not
 *      fd_nplanes, because NewTOS reads a memory MFDB as one WORD per pixel
 *      (raster.rs) and the TT reads it as fd_nplanes interleaved planes
 *      (s_nxwd = fd_nplanes * 2, vdi_raster.c:922); sixteen words per
 *      sixteen pixels bounds both, and fd_nplanes is separately capped at
 *      16 (the CONF_WITH_VDI_16BIT plane limit, vdi_raster.c:963).
 *   4. fd_w <= fd_wdwidth * 16, fd_stand is 0 or 1, and all of fd_w, fd_h,
 *      fd_wdwidth, fd_nplanes are positive.
 *   5. for 109 and 121, PTSIN holds exactly eight values.  Fewer would
 *      leave the rest of the rectangle poisoned -- 0xDEAD is -8531, and a
 *      blit to it is exactly the scribble this guard exists to prevent.
 *      Both rectangles, after the arb_corner sort the VDI itself applies
 *      (vdi_raster.c:1336-1337), must lie inside the form they address.
 *   6. for 110, PTSIN is empty: vr_trnfm reads no rectangle.
 *
 * As in the AES table there is no override.  A case that cannot pass is a
 * case that is wrong.
 */
#define TD_RAST_CASE_COLUMNS 7

/* Which of the two buffers an MFDB addresses. */
#define TD_RAST_BUF_A 0
#define TD_RAST_BUF_B 1

struct td_rast_mfdb {
	int16_t buffer;                    /* TD_RAST_BUF_A / _B              */
	int16_t w;                         /* fd_w                            */
	int16_t h;                         /* fd_h                            */
	int16_t wdwidth;                   /* fd_wdwidth                      */
	int16_t stand;                     /* fd_stand                        */
	int16_t nplanes;                   /* fd_nplanes                      */
};

struct td_rast_case {
	uint16_t line;                     /* physical line number = case id  */
	int16_t  opcode;                   /* contrl[0]: 109, 110 or 121      */
	uint16_t n_intin;                  /* values, becomes contrl[3]       */
	uint16_t n_ptsin;                  /* values; contrl[1] = n_ptsin / 2 */
	int16_t  intin[TD_RAST_INTIN_N];
	int16_t  ptsin[TD_RAST_PTSIN_N];
	struct td_rast_mfdb src;
	struct td_rast_mfdb dst;
	uint16_t seed_a;                   /* buffer A fill seed              */
	uint16_t seed_b;                   /* buffer B fill seed              */
	char     comment[TD_COMMENT_LEN];
};

struct td_rast_case_table {
	uint32_t            crc;           /* CRC32 over the raw file bytes   */
	uint16_t            count;
	struct td_rast_case cases[TD_RAST_MAX_CASES];
};

int td_rast_cases_parse(struct td_rast_case_table *t, const char *buf,
                        size_t len, struct td_case_error *err);
int td_rast_cases_load(struct td_rast_case_table *t, const char *path,
                       struct td_case_error *err);

/*
 * Shared line iterator.  One rule for what a line is, used by the parser and
 * by the host-side reader, so the two cannot drift apart.  Returns 0 when the
 * buffer is exhausted; *line_no counts every physical line including comments.
 */
struct td_line_iter {
	const char *buf;
	size_t      len;
	size_t      pos;
	uint16_t    line_no;
	int         done;
};

void td_line_iter_init(struct td_line_iter *it, const char *buf, size_t len);
/* On return, start and len describe the line with any CR stripped, line_no is
 * its physical number, and is_case says whether it carries a case. */
int  td_line_iter_next(struct td_line_iter *it, const char **start,
                       size_t *len, uint16_t *line_no, int *is_case);

#endif /* TOSDUMP_CASES_H */
