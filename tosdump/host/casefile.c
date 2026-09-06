/*
 * casefile.c -- host-side case table reader (spec section 6).
 *
 * The comparator needs three things from a case table and no more: its CRC32
 * (the identity the record header carries), how many cases it holds, and the
 * comment column so a difference report can name the case.  It deliberately
 * does not parse the value columns: those differ between the VDI/AES form and
 * the BIOS/GEMDOS form, and tosdiff has no business caring which it is.
 *
 * What a line is, and what the comment column is, come from core/cases.c --
 * one rule in one place, so the reader and the runner cannot disagree about
 * which physical line a case id refers to.  The comment's column NUMBER is
 * per area, which is why the caller has to say which area it is reading; see
 * casefile.h.
 *
 * The CRC is over the raw file bytes.  That is the strictest identity
 * available: two dumps may only be compared if they came from the same table,
 * byte for byte.  A case id is a physical line number, so inserting even a
 * comment shifts ids -- and changes the CRC, which is what makes the
 * comparator refuse the stale dump instead of comparing nonsense.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "casefile.h"
#include "cases.h"

/*
 * How many tab separated columns a case line of this area has.  Five for the
 * VDI form and for the BIOS / XBIOS / GEMDOS form (opcode, args, retclass,
 * capture, comment), seven for the AES form (spec section 6) and seven for
 * the VDI raster form (cases.h).
 */
static int columns_for_area(uint16_t area)
{
	switch (area) {
	case TD_AREA_AES:    return TD_AES_CASE_COLUMNS;
	case TD_AREA_RASTER: return TD_RAST_CASE_COLUMNS;
	default:             return TD_CASE_COLUMNS;
	}
}

int td_casefile_read(struct td_casefile *cf, const char *path, uint16_t area,
                     const char **why)
{
	const int ncols = columns_for_area(area);
	struct td_line_iter it;
	const char *line;
	size_t line_len;
	uint16_t line_no;
	int is_case;
	FILE *f;
	long size;
	char *raw;
	size_t got;

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
	raw = (char *)malloc((size_t)size + 1);
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

	cf->crc = td_crc32((const uint8_t *)raw, got);

	td_line_iter_init(&it, raw, got);
	while (td_line_iter_next(&it, &line, &line_len, &line_no, &is_case)) {
		/* A prelude carries no case id; counting one would shift every
		 * record index in the comparison. */
		if (!is_case || td_case_line_is_prelude(line, line_len))
			continue;
		if (cf->count >= TD_MAX_CASES) {
			free(raw);
			if (why) *why = "more cases than TD_MAX_CASES";
			return -2;
		}
		cf->line[cf->count] = line_no;
		td_case_line_comment_n(line, line_len, ncols,
		                       cf->comment[cf->count], TD_COMMENT_LEN);
		cf->count++;
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
