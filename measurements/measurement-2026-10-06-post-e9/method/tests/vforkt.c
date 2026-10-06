/*
 * vforkt.c -- ARCH03 stage 2e (TASK/PROC01): mintlib's vfork (AArch64 port of
 * unix/vfork.S, TOS branch = tfork = Pexec(5) + Pexec(104 -> EINVFN) +
 * Pexec(4), mintlib/thread.c) and mintlib's unchanged system().
 *
 *   1. vfork(): the child writes a variable of the parent (shared memory, the
 *      vfork contract) and _exit(42)s; the parent gets the child's pid,
 *      sees the variable, and waitpid() (wait4.c: Pwaitpid -> EINVFN ->
 *      __waitval from tfork) returns the pid with WEXITSTATUS 42.
 *   2. A second vfork in a row (the static save area is reused).
 *   3. The parent's callee-saved registers survive the child's run.
 *   4. system() with $SHELL set takes mintlib's POSIX branch: vfork, the
 *      child execve()s the shell -- on TOS that is Pexec(200), which NewTOS
 *      answers EINVFN like EmuTOS -- and _exit(127)s; system() returns the
 *      waitpid status 127 << 8. (Without $SHELL it takes the TOS branch,
 *      which port10.c covers.)
 *
 * Status words are checked as the number wait4 builds (exit code in bits
 * 8..15, unix/wait4.c), not through WEXITSTATUS: mintlib's <sys/wait.h>
 * reads them through big-endian bit-fields (struct __wait: junk:16 first),
 * which on little-endian AArch64 yield 0 -- a header defect of the port
 * found here (ARCH03 stage 2e, reported, not part of this probe).
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <sys/wait.h>
#include <mint/osbind.h>

static int fails;
static volatile long child_wrote;

static void check(const char *what, int ok, long got)
{
	char line[160];
	snprintf(line, sizeof line, "  %s %s (got %ld)\r\n", ok ? "ok  " : "FAIL", what, got);
	Cconws(line);
	if (!ok)
		fails++;
}

static long __attribute__((noinline)) keep(long v)
{
	/* A value the compiler keeps in a callee-saved register across vfork. */
	return v * 3 + 1;
}

int main(void)
{
	long st0, st1, kept = keep(1000);
	pid_t pid, w;
	int status = -1, rc;

	st0 = (long)Mxalloc(-1L, 0);
	pid = vfork();
	if (pid == 0) {
		child_wrote = 1234;
		_exit(42);
	}
	check("vfork returns a pid in the parent", pid > 0, (long)pid);
	check("the child ran first and wrote our variable", child_wrote == 1234, child_wrote);
	check("our callee-saved value survived the child", kept == 3001, kept);
	w = waitpid(pid, &status, 0);
	check("waitpid returns the child's pid", w == pid, (long)w);
	check("status word == 42 << 8 (exited, code 42)", status == (42 << 8), (long)status);
	{
		char line[96];
		snprintf(line, sizeof line, "  info mintlib WEXITSTATUS(status) = %d (header defect, see top)\r\n",
			 (int)WEXITSTATUS(status));
		Cconws(line);
	}
	check("waitpid again: no child", waitpid(-1, &status, WNOHANG) <= 0, 0);

	child_wrote = 0;
	pid = vfork();
	if (pid == 0) {
		child_wrote = 99;
		_exit(7);
	}
	w = waitpid(pid, &status, 0);
	check("second vfork: child ran", child_wrote == 99, child_wrote);
	check("second vfork: waitpid == pid, status word == 7 << 8", w == pid && status == (7 << 8),
	      (long)status);
	st1 = (long)Mxalloc(-1L, 0);
	{
		char line[96];
		snprintf(line, sizeof line, "[VFORK] ST largest block before %ld after %ld\r\n", st0, st1);
		Cconws(line);
	}
	check("the children's TPAs went back to the pool", st0 - st1 < 0x10000, st0 - st1);

	/* mintlib's POSIX system(): $SHELL makes it use vfork + execve + waitpid. */
	setenv("SHELL", "C:\\VSHELL.NCP", 1);
	rc = system("echo hi");
	check("system() via the POSIX branch: child's exec failed -> status 127 << 8",
	      rc == (127 << 8), (long)rc);
	check("system(NULL) == 1", system(NULL) == 1, 0);

	Cconws(fails ? "VFORK: FAIL\r\n" : "VFORK: PASS\r\n");
	return fails;
}
