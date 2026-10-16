/*
 * ctrlcpar.c -- Pexec a child that is ended by Ctrl-C and print what Pexec
 * returned (TASK/PROC06).
 *
 * EmuTOS: a Ctrl-C at console input runs `terminate()` = `xterm(-32)`
 * (bdos/console.c:101); `xterm(UWORD rc)` stores the low 16 bits into the
 * parent's saved D0 (bdos/proc.c:601,616), so the parent's Pexec returns
 * 0x0000FFE0 = 65504 -- not -32, which a caller would read as a load error
 * (docs/api/gemdos/process/4c-pterm.md, "Exit-code matrix", row "Ctrl-C at
 * console input"; spec host-test list item 8).
 *
 * Runs C:\PTERMCH.NCP with the command tail "c" (sdk/examples/ptermch.c: wait
 * in Cconin), then prints one verdict line on the console:
 *
 *     CTRLCPAR: Pexec rc=65504 [OK]
 *     CTRLCPAR: Pexec rc=<n> [FAIL: expected 65504]
 *
 * The Ctrl-C has to come from the keyboard: on a headless QEMU run a scratch
 * kernel built with tools/inject-arm.py feeds it, armed on this program's
 * launch (launch:CTRLCPAR). Without a key the child (and with it this
 * program) waits -- so CTRLCPAR is never installed in \AUTO\.
 *
 * Calls used, all against docs/api/:
 *   Pexec   GEMDOS 0x4B  docs/api/gemdos/process/4b-pexec.md
 *   Cconws  GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 */
#include <newtos.h>

/* Pexec (GEMDOS 0x4B): mode in x0, path in x1, tail in x2, env in x3. For
 * mode 0 the result is the child's exit code or a negative load error. */
static NLONG Pexec(NLONG mode, const char *path, const char *tail, const char *env)
{
    register NLONG x8 __asm__("x8") = 0x4B;
    register NLONG r0 __asm__("x0") = mode;
    register NLONG r1 __asm__("x1") = (NLONG)path;
    register NLONG r2 __asm__("x2") = (NLONG)tail;
    register NLONG r3 __asm__("x3") = (NLONG)env;
    __asm__ volatile("svc #1"
                     : "+r"(r0)
                     : "r"(x8), "r"(r1), "r"(r2), "r"(r3)
                     : "memory");
    return r0;
}

/* Append a NUL-terminated string to `dst`, returning the new end index. */
static int puts_at(char *dst, int j, const char *s)
{
    while (*s)
        dst[j++] = *s++;
    dst[j] = '\0';
    return j;
}

/* Append `v` in signed decimal (the full 64-bit x0, so a sign-extended -32
 * shows as -32). */
static int dec_at(char *dst, int j, NLONG v)
{
    char tmp[24];
    int n = 0;
    unsigned long u;

    if (v < 0) {
        dst[j++] = '-';
        u = (unsigned long)(-(v + 1)) + 1UL;
    } else {
        u = (unsigned long)v;
    }
    do {
        tmp[n++] = (char)('0' + (int)(u % 10UL));
        u /= 10UL;
    } while (u);
    while (n)
        dst[j++] = tmp[--n];
    dst[j] = '\0';
    return j;
}

int main(void)
{
    /* Command tail: length byte 1, then "c" (Ctrl-C mode of PTERMCH). */
    static const char tail[] = "\001c";
    char line[96];
    NLONG prc;
    int j;

    Cconws("CTRLCPAR: Pexec(0,C:\\PTERMCH.NCP,\"c\") -- press Ctrl-C\r\n");
    prc = Pexec(0, "C:\\PTERMCH.NCP", tail, 0);

    j = puts_at(line, 0, "CTRLCPAR: Pexec rc=");
    j = dec_at(line, j, prc);
    j = puts_at(line, j, prc == 65504 ? " [OK]\r\n" : " [FAIL: expected 65504]\r\n");
    Cconws(line);
    return 0;
}
