/* DIAGNOSTIC ONLY (PORT06 measurement, scratch): the delivered build does not
 * link because libc's system() drags in spawnvp/vfork/wait4 gaps
 * (docs/sdk/07-mintlib-limits.md). DOOM reaches system() only from
 * ZenityErrorBox, which I_Error skips while stdout is a tty (always, on
 * NewTOS). This refuses loudly instead of pretending to run anything. */
#include <stdio.h>
#include <errno.h>
int system(const char *cmd)
{
	fprintf(stderr, "DIAG: system(\"%s\") refused -- not available in this diagnostic build\n", cmd ? cmd : "(null)");
	errno = ENOSYS;
	return -1;
}
