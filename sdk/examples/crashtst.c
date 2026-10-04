/*
 * crashtst.c -- parent driver for the ARCH03 stage-0c crash tests.
 *
 * Starts C:\CRCHILD.NAP once per case with `Pexec(PE_LOADGO)` and checks the
 * exit code each child hands back:
 *
 *   w u x a c p   crash cases   -> 0x0000FFFF (Pterm(-1), UWORD rc, D-005)
 *   o             clean exit    -> 0
 *
 * Because the parent is an EL0 program, every child's termination resumes an
 * EL0 parent (`finish_terminate` with a parent TTBR0), and the clean child
 * after the crashes shows that a program starts fine after a crash. Each
 * crash waits for a key (EmuTOS `dopanic`), so a run needs one key per crash
 * case. Prints one line per case and a final PASS/FAIL, then Pterm(0).
 *
 * Build: sdk/build.sh sdk/examples/crashtst.c <out>/CRASHTST.NAP
 */
#include <newtos.h>

static NLONG Pexec(short mode, const char *path, const char *tail, const char *env)
{
    register NLONG x8 __asm__("x8") = 0x4B;
    register NLONG r0 __asm__("x0") = mode;
    register NLONG r1 __asm__("x1") = (NLONG)path;
    register NLONG r2 __asm__("x2") = (NLONG)tail;
    register NLONG r3 __asm__("x3") = (NLONG)env;
    __asm__ volatile("svc #1" : "+r"(r0) : "r"(x8), "r"(r1), "r"(r2), "r"(r3) : "memory");
    return r0;
}

/*
 * noinline + volatile: keeps the compiler from vectorising this and hoisting
 * its SIMD constants into main() across the Pexec `svc` -- the kernel does
 * not preserve v0-v31 across a Pexec/Pterm round trip today (see
 * measurement-2026-10-05-arch03-0c RUNINFO, out-of-scope observation).
 */
static __attribute__((noinline)) void put_hex(unsigned long v)
{
    volatile char buf[19];
    int i;
    buf[0] = '0';
    buf[1] = 'x';
    for (i = 0; i < 16; i++) {
        unsigned d = (unsigned)(v >> (60 - 4 * i)) & 0xF;
        buf[2 + i] = (char)(d < 10 ? '0' + d : 'A' + d - 10);
    }
    buf[18] = '\0';
    Cconws((const char *)buf);
}

int main(void)
{
    static const char cases[] = "wuxacpo";
    char tail[3];
    char line[] = "CRASHTST: case ? rc=";
    int i, fails = 0;

    Cconws("CRASHTST: start\r\n");
    for (i = 0; cases[i]; i++) {
        unsigned long want = cases[i] == 'o' ? 0 : 0xFFFF;
        NLONG rc;

        tail[0] = 1;            /* command-tail length byte */
        tail[1] = cases[i];
        tail[2] = '\0';
        rc = Pexec(0, "C:\\CRCHILD.NAP", tail, 0);

        line[15] = cases[i];
        Cconws(line);
        put_hex((unsigned long)rc);
        if ((unsigned long)rc == want) {
            Cconws(" OK\r\n");
        } else {
            Cconws(" WRONG\r\n");
            fails++;
        }
    }
    Cconws(fails ? "CRASHTST: FAIL\r\n" : "CRASHTST: PASS\r\n");
    return 0;
}
