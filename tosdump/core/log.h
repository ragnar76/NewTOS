/*
 * log.h -- run diagnosis, buffered in RAM and written as a file.
 *
 * A run happens on a machine with no debugger and, on a GEM screen, no
 * scrollback: the desktop redraws over the only diagnosis there is.  So every
 * message the dumper emits also goes into a buffer that is written out as a
 * plain text file next to the dump.
 *
 * Buffered, never written per line, for the same reason records are (spec
 * section 7): file I/O between two measurements would touch device state that
 * the measurements are supposed to be isolated from.  td_log_write is called
 * once after the workstation opens -- before any case, so it cannot disturb
 * one, and a run that hangs still leaves its setup on disk -- and once at the
 * end with the complete log.
 *
 * This is diagnosis of the RUN, not interpretation of the RECORDS.  Section 3
 * stands: no record content is ever formatted on the target.
 */
#ifndef TOSDUMP_LOG_H
#define TOSDUMP_LOG_H

#include "record.h"

#define TD_LOG_SIZE 32768

void td_log_init(void);

/* Appends to the buffer and echoes to stdout.  Truncation is recorded in the
 * buffer itself rather than being silent. */
void td_log(const char *fmt, ...);

/* Bytes held, and whether anything was lost. */
size_t td_log_len(void);
int    td_log_truncated(void);
const char *td_log_text(void);

#define TD_LOG_ERR_OPEN  (-1)
#define TD_LOG_ERR_WRITE (-2)
int td_log_write(const char *path);

#endif /* TOSDUMP_LOG_H */
