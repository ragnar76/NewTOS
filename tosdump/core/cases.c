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

#define TD_CASE_COLUMNS 5

/* Splits into at most TD_CASE_COLUMNS columns; the last one keeps its tabs. */
static int split_columns(const char *line, size_t len,
                         const char **col, size_t *col_len)
{
	int n = 0;
	size_t i = 0, start = 0;

	for (;;) {
		if (n == TD_CASE_COLUMNS - 1) {
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

		if (t->count >= TD_MAX_CASES) {
			fail(err, line_no, "more cases than TD_MAX_CASES");
			return TD_CASE_ERR_TOO_MANY_CASES;
		}

		ncol = split_columns(line, line_len, col, col_len);
		if (ncol < TD_CASE_COLUMNS) {
			fail(err, line_no, "needs five tab separated columns");
			return TD_CASE_ERR_COLUMNS;
		}

		c = &t->cases[t->count];
		memset(c, 0, sizeof *c);
		c->line = line_no;

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

	return 0;
}

void td_case_line_comment(const char *line, size_t len, char *dst,
                          size_t dst_size)
{
	const char *col[TD_CASE_COLUMNS];
	size_t col_len[TD_CASE_COLUMNS];
	size_t n;

	if (dst_size == 0)
		return;
	if (split_columns(line, len, col, col_len) < TD_CASE_COLUMNS) {
		dst[0] = '\0';
		return;
	}
	n = col_len[TD_CASE_COLUMNS - 1];
	if (n >= dst_size)
		n = dst_size - 1;
	memcpy(dst, col[TD_CASE_COLUMNS - 1], n);
	dst[n] = '\0';
}

int td_cases_load(struct td_case_table *t, const char *path,
                  struct td_case_error *err)
{
	FILE *f;
	long size;
	char *raw;
	size_t got;
	int rc;

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

	rc = td_cases_parse(t, raw, got, err);
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
	default:                         return "unknown error";
	}
}
