/*
 * log.c -- run diagnosis, buffered in RAM and written as a file.  See log.h.
 */
#include <stdarg.h>
#include <stdio.h>
#include <string.h>

#include "log.h"

static char   log_buf[TD_LOG_SIZE + 1];
static size_t log_len;
static int    log_truncated;

static const char trunc_note[] = "\n*** log truncated ***\n";

void td_log_init(void)
{
	log_buf[0]    = '\0';
	log_len       = 0;
	log_truncated = 0;
}

void td_log(const char *fmt, ...)
{
	char line[512];
	va_list ap;
	int n;
	size_t room;

	va_start(ap, fmt);
	n = vsnprintf(line, sizeof line, fmt, ap);
	va_end(ap);
	if (n < 0)
		return;
	if ((size_t)n >= sizeof line)
		n = (int)sizeof line - 1;

	fputs(line, stdout);

	if (log_truncated)
		return;

	/* keep room for the truncation note, so overflow is never silent */
	room = TD_LOG_SIZE - sizeof trunc_note;
	if (log_len + (size_t)n > room) {
		memcpy(log_buf + log_len, trunc_note, sizeof trunc_note - 1);
		log_len += sizeof trunc_note - 1;
		log_buf[log_len] = '\0';
		log_truncated = 1;
		return;
	}

	memcpy(log_buf + log_len, line, (size_t)n);
	log_len += (size_t)n;
	log_buf[log_len] = '\0';
}

size_t td_log_len(void)       { return log_len; }
int    td_log_truncated(void) { return log_truncated; }
const char *td_log_text(void) { return log_buf; }

int td_log_write(const char *path)
{
	FILE *f = fopen(path, "wb");   /* replace, never append */

	if (!f)
		return TD_LOG_ERR_OPEN;
	if (log_len && fwrite(log_buf, 1, log_len, f) != log_len) {
		fclose(f);
		return TD_LOG_ERR_WRITE;
	}
	if (fclose(f) != 0)
		return TD_LOG_ERR_WRITE;
	return 0;
}
