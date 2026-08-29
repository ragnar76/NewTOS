/*
 * casefile.h -- host-side reader for a case table (spec section 6).
 *
 * T1 needs three things from a case table: its CRC32 (the identity the record
 * header carries), how many cases it holds, and the comment column so that a
 * difference report can name the case.  Typed parsing of the value columns is
 * the runner's job and lands with T2.
 */
#ifndef TOSDUMP_CASEFILE_H
#define TOSDUMP_CASEFILE_H

#include "record.h"

struct td_casefile {
	uint32_t crc;                                  /* CRC32 of the raw file  */
	uint16_t count;                                /* cases held             */
	uint16_t line[TD_MAX_CASES];                   /* physical line = case id*/
	char     comment[TD_MAX_CASES][TD_COMMENT_LEN];
};

/* Returns 0 on success, -1 on I/O failure, -2 if the table holds more than
 * TD_MAX_CASES cases.  On failure *why is set to a static message. */
int td_casefile_read(struct td_casefile *cf, const char *path, const char **why);

/* Comment for a case id (physical line number), or "" if the id is unknown. */
const char *td_casefile_comment(const struct td_casefile *cf, uint16_t case_id);

#endif /* TOSDUMP_CASEFILE_H */
