/*
 * waith-probe.c -- TASK/PORT-WAITH: the <sys/wait.h> status macros on a
 * little-endian LP64 host, compiled with the include path and DEFS of
 * sdk/libc/build-libc.sh, i.e. against the header the SDK installs (the
 * overlay sdk/libc/overlay/include/sys/wait.h once build-libc.sh has applied
 * it). Two halves:
 *
 *   A. Known status words in the layout mintlib's unix/wait4.c stores
 *      (exit code / stop signal in bits 8..15, core flag bit 7, terminating
 *      signal bits 0..6 -- W_EXITCODE/W_STOPCODE in the same header): every
 *      W* macro and the BSD `union wait' members.
 *   B. The real library path: mintlib's UNMODIFIED unix/wait4.c and
 *      unix/waitpid.c, with Pwaitpid answering ENOSYS (EINVFN -- NewTOS and
 *      EmuTOS have no Pwaitpid) so wait4 takes __waitval, which mintlib's
 *      tfork (mintlib/thread.c:146) sets to (pid << 16) | Pexec's return.
 *
 * Results go through waith_report() in waith-host.c (host headers).
 */
#include <sys/types.h>
#include <sys/wait.h>
#include <signal.h>
#include <errno.h>

void waith_report (const char *what, long got, long want);
int waith_skip (const char *why);

/* What wait4.c and lib.h reference outside the two compiled files.  */
int __mint = 0;			/* TOS, as on NewTOS (no MiNT cookie) */
int errno;
long __waitval = -ENOENT;	/* mintlib/thread.c's initial value */
long __waittime = 0;
static long pwaitpid_calls;

long
waith_test_Pwaitpid (int pid, int flag, long *rusage)
{
	(void) pid; (void) flag; (void) rusage;
	pwaitpid_calls++;
	return -ENOSYS;
}

long
waith_test_Pdomain (int arg)
{
	(void) arg;
	return 0;		/* TOS domain */
}

/* The weak_alias lines of wait4.c/waitpid.c/wait.c, as plain calls.  */
pid_t
wait4 (pid_t pid, __WP stat_loc, int options, struct rusage *usage)
{
	return __wait4 (pid, stat_loc, options, usage);
}

pid_t
waitpid (pid_t pid, int *stat_loc, int options)
{
	return __waitpid (pid, stat_loc, options);
}

pid_t
wait (__WP stat_loc)
{
	return __wait (stat_loc);
}

#define R(what, expr, want)	waith_report ((what), (long) (expr), (long) (want))

static void
known (const char *tag, int st, int exited, int code, int sig, int termsig,
       int stopped, int stopsig, int core)
{
	char w[96];
	const char *p;
	int i;
#define T(name)	do { for (i = 0, p = tag; *p && i < 60; ) w[i++] = *p++; \
		     for (p = " " name; *p && i < 95; ) w[i++] = *p++; w[i] = 0; } while (0)
	T ("WIFEXITED");	R (w, WIFEXITED (st) != 0, exited);
	if (exited) {
		T ("WEXITSTATUS");	R (w, WEXITSTATUS (st), code);
	}
	T ("WIFSIGNALED");	R (w, WIFSIGNALED (st) != 0, sig);
	if (sig) {
		T ("WTERMSIG");		R (w, WTERMSIG (st), termsig);
		T ("WCOREDUMP");	R (w, WCOREDUMP (st) != 0, core);
		T ("WIFCOREDUMPED");	R (w, WIFCOREDUMPED (st) != 0, core);
	}
	T ("WIFSTOPPED");	R (w, WIFSTOPPED (st) != 0, stopped);
	if (stopped) {
		T ("WSTOPSIG");		R (w, WSTOPSIG (st), stopsig);
	}
#undef T
}

int
waith_probe (void)
{
	int st;
	pid_t pid;
	union wait u;

#if !defined (__BYTE_ORDER__) || __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__ \
    || !defined (__LP64__)
	return waith_skip ("host is not little-endian LP64 like AArch64");
#endif

	/* A. the layout itself */
	R ("sizeof (union wait) == sizeof (int)", sizeof (union wait), sizeof (int));
	R ("W_EXITCODE (42, 0) == 0x2a00", W_EXITCODE (42, 0), 0x2a00);
	R ("W_STOPCODE (SIGTSTP) == 0x127f", W_STOPCODE (SIGTSTP), 0x127f);

	known ("exit(42) 0x2a00", W_EXITCODE (42, 0), 1, 42, 0, 0, 0, 0, 0);
	known ("exit(0) 0x0000", W_EXITCODE (0, 0), 1, 0, 0, 0, 0, 0, 0);
	known ("exit(1) 0x0100", W_EXITCODE (1, 0), 1, 1, 0, 0, 0, 0, 0);
	known ("exit(127) 0x7f00", W_EXITCODE (127, 0), 1, 127, 0, 0, 0, 0, 0);
	known ("exit(255) 0xff00", W_EXITCODE (255, 0), 1, 255, 0, 0, 0, 0, 0);
	known ("SIGTERM 0x000f", W_EXITCODE (0, SIGTERM), 0, 0, 1, SIGTERM, 0, 0, 0);
	known ("SIGINT 0x0002", SIGINT, 0, 0, 1, SIGINT, 0, 0, 0);
	known ("SIGSEGV+core 0x008b", WCOREFLAG | SIGSEGV, 0, 0, 1, SIGSEGV, 0, 0, 1);
	known ("stopped SIGTSTP 0x127f", W_STOPCODE (SIGTSTP), 0, 0, 0, 0, 1, SIGTSTP, 0);
	known ("stopped SIGSTOP", W_STOPCODE (SIGSTOP), 0, 0, 0, 0, 1, SIGSTOP, 0);

	/* BSD-style union wait, both directions */
	u.w_status = 0x2a00;
	R ("union wait 0x2a00: w_retcode", u.w_retcode, 42);
	R ("union wait 0x2a00: w_termsig", u.w_termsig, 0);
	R ("union wait 0x2a00: w_coredump", u.w_coredump, 0);
	u.w_status = 0x008b;
	R ("union wait 0x008b: w_termsig", u.w_termsig, SIGSEGV);
	R ("union wait 0x008b: w_coredump", u.w_coredump, 1);
	R ("union wait 0x008b: w_retcode", u.w_retcode, 0);
	u.w_status = W_STOPCODE (SIGTSTP);
	R ("union wait stopped: w_stopsig", u.w_stopsig, SIGTSTP);
	u.w_status = 0;
	u.w_retcode = 7;
	R ("union wait: w_retcode = 7 -> w_status", u.w_status, 0x0700);
	u.w_status = 0;
	u.w_termsig = SIGTERM;
	u.w_coredump = 1;
	R ("union wait: w_termsig = 15, w_coredump = 1 -> w_status", u.w_status, 0x008f);

	/* B. mintlib's wait4/waitpid with the value tfork leaves */
	__waitval = (17L << 16) | 42;
	st = -1;
	pid = waitpid (17, &st, 0);
	R ("waitpid (exit 42): returns the pid", pid, 17);
	R ("waitpid (exit 42): took the Pwaitpid -> ENOSYS path", pwaitpid_calls, 1);
	R ("waitpid (exit 42): status word", st, 0x2a00);
	R ("waitpid (exit 42): WIFEXITED", WIFEXITED (st) != 0, 1);
	R ("waitpid (exit 42): WEXITSTATUS", WEXITSTATUS (st), 42);
	R ("waitpid (exit 42): WIFSIGNALED", WIFSIGNALED (st) != 0, 0);
	R ("waitpid (exit 42): WIFSTOPPED", WIFSTOPPED (st) != 0, 0);
	R ("__waitval consumed", __waitval, -ECHILD);

	__waitval = (18L << 16) | 0;
	st = -1;
	pid = wait (&st);
	R ("wait (exit 0): returns the pid", pid, 18);
	R ("wait (exit 0): WIFEXITED", WIFEXITED (st) != 0, 1);
	R ("wait (exit 0): WEXITSTATUS", WEXITSTATUS (st), 0);

	__waitval = (19L << 16) | 7;
	st = -1;
	pid = wait4 (-1, &st, 0, NULL);
	R ("wait4 (exit 7): returns the pid", pid, 19);
	R ("wait4 (exit 7): WEXITSTATUS", WEXITSTATUS (st), 7);

	st = 12345;
	errno = 0;
	pid = waitpid (-1, &st, 0);
	R ("waitpid with no child: -1", pid, -1);
	R ("waitpid with no child: errno ECHILD", errno, ECHILD);
	pid = waitpid (-1, &st, WNOHANG);
	R ("waitpid (WNOHANG) with no child: 0", pid, 0);
	return 0;
}
