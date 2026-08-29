/*
 * casefile.c -- host-side case table reader (spec section 6).
 *
 * The CRC is taken over the raw file bytes.  That is the strictest identity
 * available and it is what the record header carries: two dumps may only be
 * compared if they were produced from the same table, byte for byte.
 *
 * A case id is the physical 1-based line number in the table, so inserting a
 * comment shifts ids -- and changes the CRC, which is exactly what makes the
 * comparator refuse the stale dump instead of comparing nonsense.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "casefile.h"

static void take_comment(char *dst, const char *line, size_t len)
{
	const char *last_tab = NULL;
	const char *start;
	size_t n, i;

	for (i = 0; i < len; i++)
		if (line[i] == '\t')
			last_tab = line + i;

	start = last_tab ? last_tab + 1 : line;
	n = (size_t)(line + len - start);
	while (n > 0 && (start[n - 1] == '\r' || start[n - 1] == ' '))
		n--;
	if (n >= TD_COMMENT_LEN)
		n = TD_COMMENT_LEN - 1;
	memcpy(dst, start, n);
	dst[n] = '\0';
}

int td_casefile_read(struct td_casefile *cf, const char *path, const char **why)
{
	FILE *f;
	long size;
	uint8_t *raw;
	size_t got, pos;
	uint16_t line_no = 0;

	memset(cf, 0, sizeof *cf);

	f = fopen(path, "rb");
	if (!f) {
		if (why) *why = "cannot open";
		return -1;
	}
	if (fseek(f, 0, SEEK_END) != 0 || (size = ftell(f)) < 0 ||
	    fseek(f, 0, SEEK_SET) != 0) {
		fclose(f);
		if (why) *why = "cannot seek";
		return -1;
	}
	raw = (uint8_t *)malloc((size_t)size + 1);
	if (!raw) {
		fclose(f);
		if (why) *why = "out of memory";
		return -1;
	}
	got = fread(raw, 1, (size_t)size, f);
	fclose(f);
	if (got != (size_t)size) {
		free(raw);
		if (why) *why = "short read";
		return -1;
	}
	raw[got] = '\0';

	cf->crc = td_crc32(raw, got);

	pos = 0;
	while (pos <= got) {
		size_t start = pos, len;
		const char *line;

		while (pos < got && raw[pos] != '\n')
			pos++;
		len = pos - start;
		line = (const char *)raw + start;
		pos++;                       /* step over the newline */
		line_no++;

		/* strip a trailing CR so a table edited on the Atari still parses */
		if (len > 0 && line[len - 1] == '\r')
			len--;

		/* blank and comment lines carry no case, but they do consume an id */
		{
			size_t i = 0;
			while (i < len && (line[i] == ' ' || line[i] == '\t'))
				i++;
			if (i == len || line[i] == '#')
				continue;
		}

		if (cf->count >= TD_MAX_CASES) {
			free(raw);
			if (why) *why = "more cases than TD_MAX_CASES";
			return -2;
		}
		cf->line[cf->count] = line_no;
		take_comment(cf->comment[cf->count], line, len);
		cf->count++;

		if (start + len >= got && pos > got)
			break;
	}

	free(raw);
	return 0;
}

const char *td_casefile_comment(const struct td_casefile *cf, uint16_t case_id)
{
	uint16_t i;

	for (i = 0; i < cf->count; i++)
		if (cf->line[i] == case_id)
			return cf->comment[i];
	return "";
}
