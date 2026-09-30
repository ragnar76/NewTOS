/*
 * sysinfo.c -- ACC06 part 1: prove that the machine's ARM clock and its die
 * temperature reach an EL0 program.
 *
 * A native NewTOS console program (`.NCP`, ADR-018). It is the proof that
 * part 1 of ACC06 works *before* there is an accessory to show the numbers
 * in: the desk-accessory framework (ACC02/ACC03) is not built yet, but the
 * two values already have to come out of the kernel, and this program is
 * where that is visible.
 *
 * Both values arrive through GEMDOS `Ssystem` (0x154) -- NOT through the
 * cookie jar. A cookie is a value deposited at boot; a temperature changes
 * while you read it. Spec: docs/api/gemdos/154-ssystem.md, section
 * "NewTOS-own subfunctions (2000+)".
 *
 *   Ssystem  GEMDOS 0x154  docs/api/gemdos/154-ssystem.md
 *     mode 2000  S_NEWTOS_CPUCLOCK  ARM core clock, Hz
 *     mode 2001  S_NEWTOS_CPUTEMP   SoC die temperature, 1/1000 degree C
 *   Cconws   GEMDOS 0x09   docs/api/gemdos/console/09-cconws.md
 *   Cconin   GEMDOS 0x01   docs/api/gemdos/console/01-cconin.md
 *
 * Both modes take:
 *     arg1 = pointer to a 32-bit LONG that receives the value, or 0 to have
 *            the value returned in the call's own LONG;
 *     arg2 = reserved, must be 0.
 *
 * and answer ERROR (-1) on a board that cannot measure the value -- QEMU's
 * `virt` machine has no VideoCore mailbox at all, and even QEMU's `raspi4b`
 * firmware model answers only part of what a real Pi 4 does. That ERROR is a
 * result, not a failure: this program prints it as such and invents nothing.
 *
 * Staged by tools/make-sd-image.sh as C:\SYSINFO.NCP, in the root and nowhere
 * else: a double-click is the only way it starts. That is why it ends in
 * newtos_pause() -- this program is TASK/T13's acceptance case ("double-click
 * C:\SYSINFO.NCP, the output stays until a key comes"). The former
 * SD_SYSINFO_AUTO=1 switch, which put a second copy into C:\AUTO\ for a
 * click-free serial run, is gone with that pause: a pausing program in \AUTO\
 * stops the boot. The click-free route is now a `#Z 01 C:\SYSINFO.NCP@` line
 * in C:\NEWDESK.INF, which starts the program through the shell loop after
 * the AES is up, where a key can still arrive.
 */
#include <newtos.h>

/* Ssystem takes three arguments; newtos.h ships gemdos0/1/2 only. Kept local
 * to this example rather than widening the SDK header as a side effect --
 * exactly as curdir.c does for Fread. */
static inline NLONG gemdos3(NLONG fn, NLONG a0, NLONG a1, NLONG a2)
{
    register NLONG x8 __asm__("x8") = fn;
    register NLONG r0 __asm__("x0") = a0;
    register NLONG r1 __asm__("x1") = a1;
    register NLONG r2 __asm__("x2") = a2;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1), "r"(r2)
                     : "memory");
    return r0;
}

#define SSYSTEM            0x154
#define S_NEWTOS_CPUCLOCK  2000
#define S_NEWTOS_CPUTEMP   2001

#define E_OK    0L
#define ERROR  (-1L)

/*
 * The kernel writes the pointer form with a single 32-bit store, so the
 * target is an `int` (32 bit under our LP64 app ABI), never a `long`.
 */
static NLONG Ssystem_ptr(NLONG mode, int *out)
{
    return gemdos3(SSYSTEM, mode, (NLONG)out, 0);
}

static NLONG Ssystem_val(NLONG mode)
{
    return gemdos3(SSYSTEM, mode, 0, 0);
}

/* ---- tiny formatting helpers (no libc in a Stage-1 .NCP) ---- */

/* Append a NUL-terminated string. Returns the new length. */
static int sapp(char *dst, int j, const char *s)
{
    while (*s)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

/* Append a signed decimal. Returns the new length. */
static int napp(char *dst, int j, long v)
{
    char tmp[24];
    int i = 0;
    unsigned long u;

    if (v < 0) {
        dst[j++] = '-';
        u = (unsigned long)(-v);
    } else {
        u = (unsigned long)v;
    }
    do {
        tmp[i++] = (char)('0' + (int)(u % 10));
        u /= 10;
    } while (u);
    while (i)
        dst[j++] = tmp[--i];
    dst[j] = '\0';
    return j;
}

/* Append `v / div` with `frac` decimals, rounding down (no floating point in
 * a freestanding program). */
static int fixapp(char *dst, int j, long v, long div, int frac)
{
    long whole, rest;
    int k;

    /* Take the sign off first: -47200 must print -47.2, not -47.-200. */
    if (v < 0) {
        dst[j++] = '-';
        v = -v;
    }
    whole = v / div;
    rest = v % div;
    j = napp(dst, j, whole);
    if (frac <= 0)
        return j;
    dst[j++] = '.';
    for (k = 0; k < frac; k++) {
        long scale = div;
        int d;
        int m;
        for (m = 0; m <= k; m++)
            scale /= 10;
        d = (int)((rest / (scale ? scale : 1)) % 10);
        dst[j++] = (char)('0' + d);
    }
    dst[j] = '\0';
    return j;
}

static void line(const char *s)
{
    char buf[200];
    int j = sapp(buf, 0, s);
    buf[j++] = '\r';
    buf[j++] = '\n';
    buf[j] = '\0';
    Cconws(buf);
}

/* "SYSINFO: <label>: Ssystem(<mode>) = <rc>  (not available on this board)" */
static void report_absent(const char *label, long mode, long rc)
{
    char buf[200];
    int j = sapp(buf, 0, "SYSINFO: ");
    j = sapp(buf, j, label);
    j = sapp(buf, j, ": Ssystem(");
    j = napp(buf, j, mode);
    j = sapp(buf, j, ") = ");
    j = napp(buf, j, rc);
    j = sapp(buf, j, "  (this board cannot measure it)");
    buf[j++] = '\r';
    buf[j++] = '\n';
    buf[j] = '\0';
    Cconws(buf);
}

int main(void)
{
    char buf[200];
    int hz = 0;
    int mdeg = 0;
    NLONG rc, again;
    int j;

    line("SYSINFO: NewTOS system values through Ssystem (ACC06 part 1)");

    /* ---- 2000: ARM core clock, Hz ---- */
    rc = Ssystem_ptr(S_NEWTOS_CPUCLOCK, &hz);
    if (rc != E_OK) {
        report_absent("ARM clock", S_NEWTOS_CPUCLOCK, rc);
    } else {
        j = sapp(buf, 0, "SYSINFO: ARM clock : ");
        j = fixapp(buf, j, (long)hz, 1000000L, 1);
        j = sapp(buf, j, " MHz  (");
        j = napp(buf, j, (long)hz);
        j = sapp(buf, j, " Hz)");
        buf[j] = '\0';
        line(buf);

        /* The other calling form must agree -- arg1 == 0 returns the value
         * in the call's own LONG. Proving both forms here is why this
         * program exists. */
        again = Ssystem_val(S_NEWTOS_CPUCLOCK);
        j = sapp(buf, 0, "SYSINFO:   value form agrees: ");
        j = sapp(buf, j, (again == (NLONG)hz) ? "yes" : "NO");
        j = sapp(buf, j, " (");
        j = napp(buf, j, (long)again);
        j = sapp(buf, j, ")");
        buf[j] = '\0';
        line(buf);
    }

    /* ---- 2001: SoC die temperature, 1/1000 degree C ---- */
    rc = Ssystem_ptr(S_NEWTOS_CPUTEMP, &mdeg);
    if (rc != E_OK) {
        report_absent("CPU temp ", S_NEWTOS_CPUTEMP, rc);
    } else {
        j = sapp(buf, 0, "SYSINFO: CPU temp  : ");
        j = fixapp(buf, j, (long)mdeg, 1000L, 1);
        j = sapp(buf, j, " C  (");
        j = napp(buf, j, (long)mdeg);
        j = sapp(buf, j, " m C)");
        buf[j] = '\0';
        line(buf);

        again = Ssystem_val(S_NEWTOS_CPUTEMP);
        j = sapp(buf, 0, "SYSINFO:   value form agrees: ");
        j = sapp(buf, j, (again == (NLONG)mdeg) ? "yes" : "NO");
        j = sapp(buf, j, " (");
        j = napp(buf, j, (long)again);
        j = sapp(buf, j, ")");
        buf[j] = '\0';
        line(buf);
    }

    /* ---- the reserved third argument ---- */
    rc = gemdos3(SSYSTEM, S_NEWTOS_CPUCLOCK, 0, 1);
    j = sapp(buf, 0, "SYSINFO: arg2 reserved, Ssystem(2000, 0, 1) = ");
    j = napp(buf, j, (long)rc);
    j = sapp(buf, j, (rc == -25L) ? "  (EINVAL, as specified)" : "  (EXPECTED -25)");
    buf[j] = '\0';
    line(buf);

    line("SYSINFO: done.");
    newtos_pause();
    return 0;
}
