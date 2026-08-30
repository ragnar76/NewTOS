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
 * the same way. */
void td_case_line_comment(const char *line, size_t len, char *dst,
                          size_t dst_size);

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
