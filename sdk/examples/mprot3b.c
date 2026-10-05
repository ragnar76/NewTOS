/*
 * mprot3b.c -- driver of the ARCH03 stage-3b memory-protection probe.
 *
 * Allocates three blocks of its own -- PROT_P (Malloc, PRGFLAGS default),
 * PROT_G (Mxalloc mode 0x20|3) and PROT_PR (Mxalloc mode 0x40|3) -- fills
 * them with a pattern and starts C:\MPKID.NAP once per case with the
 * address in the command tail (mpkid.c). It checks the child's exit code
 * (0x0000FFFF = the child was stopped by a protection fault: EmuTOS crash
 * report + Pterm(-1)) and, for the write cases, whether the child's write
 * reached its memory. Afterwards it checks its own blocks once more: a
 * refused child leaves them untouched and this process unharmed.
 *
 * The expected answers depend on the boot argument (FreeMiNT's
 * MEM_PROT / MPF_STRICT), chosen at build time with -DMPROT_MODE:
 *   0  protection on, default (E5: a Pexec 0 child keeps its parent's
 *      blocks) -- MPROT3B.NAP
 *   1  protection on, `memprot=strict` -- MPROTS.NAP
 *   2  `memprot=off` (stage 3a: no protection at all) -- MPROTO.NAP
 * In every mode: PROT_G works across processes, PROT_PR is readable, a
 * Pexec 4 thread shares its caller's memory (E6); with protection on a
 * block given back with Mfree can no longer be written by its former
 * owner (MEM04).
 *
 * Prints one line per case and "MPROT3B: PASS"/"MPROT3B: FAIL".
 *
 * Build: sdk/build.sh -DMPROT_MODE=n sdk/examples/mprot3b.c <out>
 */
#include <newtos.h>

#ifndef MPROT_MODE
#define MPROT_MODE 0
#endif
#define STRICT (MPROT_MODE == 1)
#define OFF (MPROT_MODE == 2)

#define PATTERN 0x50415254UL
#define MARK 0x4B494431UL
#define CRASH 0xFFFFUL

typedef unsigned long UL;

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

static __attribute__((noinline)) void put_hex(UL v)
{
    volatile char buf[11];
    int i;
    buf[0] = '0';
    buf[1] = 'x';
    for (i = 0; i < 8; i++) {
        unsigned d = (unsigned)(v >> (28 - 4 * i)) & 0xF;
        buf[2 + i] = (char)(d < 10 ? '0' + d : 'A' + d - 10);
    }
    buf[10] = '\0';
    Cconws((const char *)buf);
}

static int fails;

static void verdict(char c, const char *what, UL got, UL want)
{
    char line[] = "MPROT3B: case ? ";
    line[14] = c;
    Cconws(line);
    Cconws(what);
    Cconws(" got=");
    put_hex(got);
    Cconws(" want=");
    put_hex(want);
    if (got == want) {
        Cconws(" OK\r\n");
    } else {
        Cconws(" WRONG\r\n");
        fails++;
    }
}

static void make_tail(char *tail, char c, UL addr)
{
    static const char hex[] = "0123456789abcdef";
    int i;
    tail[0] = 10;
    tail[1] = c;
    tail[2] = ' ';
    for (i = 0; i < 8; i++)
        tail[3 + i] = hex[(addr >> ((7 - i) * 4)) & 0xF];
    tail[11] = '\0';
}

static UL run_kid(char c, UL addr)
{
    char tail[12];
    make_tail(tail, c, addr);
    return (UL)Pexec(0, "C:\\MPKID.NAP", tail, 0);
}

int main(void)
{
    volatile unsigned int *priv = (volatile unsigned int *)gemdos1(0x48, 4096);          /* Malloc */
    volatile unsigned int *glob = (volatile unsigned int *)gemdos2(0x44, 4096, 0x20 | 3); /* Mxalloc PROT_G */
    volatile unsigned int *rdon = (volatile unsigned int *)gemdos2(0x44, 4096, 0x40 | 3); /* Mxalloc PROT_PR */
    char tail[12];
    UL rc, pd;

    Cconws(OFF ? "MPROT3B: start (expecting memprot=off)\r\n"
               : STRICT ? "MPROT3B: start (expecting memprot=strict)\r\n"
                        : "MPROT3B: start (expecting protection on, default)\r\n");
    if (!priv || !glob || !rdon) {
        Cconws("MPROT3B: FAIL (allocation)\r\n");
        return 1;
    }
    *priv = PATTERN;
    *glob = PATTERN;
    *rdon = PATTERN;

    /* A child reads / writes our private block: E5 (default) lets it, the
       strict mode stops it (translation fault, "private"). */
    rc = run_kid('p', (UL)priv);
    verdict('p', "child reads parent PROT_P", rc, STRICT ? CRASH : 0);
    rc = run_kid('P', (UL)priv);
    verdict('P', "child writes parent PROT_P", rc, STRICT ? CRASH : 0);
    verdict('P', "  parent block afterwards", *priv, STRICT ? PATTERN : MARK);
    *priv = PATTERN;

    /* PROT_G: everybody, every mode. */
    rc = run_kid('g', (UL)glob);
    verdict('g', "child reads+writes parent PROT_G", rc, 0);
    verdict('g', "  parent block afterwards", *glob, MARK);
    *glob = PATTERN;

    /* PROT_PR: readable by everybody; writable by an owner only (the
       default child inherited owner rights, E5) -- strict: permission fault. */
    rc = run_kid('r', (UL)rdon);
    verdict('r', "child reads parent PROT_PR", rc, 0);
    rc = run_kid('R', (UL)rdon);
    verdict('R', "child writes parent PROT_PR", rc, STRICT ? CRASH : 0);
    verdict('R', "  parent block afterwards", *rdon, STRICT ? PATTERN : MARK);
    *rdon = PATTERN;

    /* Pexec 3 + 4: the child runs as a thread in our memory space (E6) and
       sees our private block in every mode. */
    make_tail(tail, 't', (UL)priv);
    pd = (UL)Pexec(3, "C:\\MPKID.NAP", tail, 0);
    if ((long)pd <= 0) {
        verdict('t', "Pexec 3", pd, 1);
    } else {
        UL env = *(volatile unsigned int *)(pd + 0x2C);
        rc = (UL)Pexec(4, 0, (const char *)pd, 0);
        verdict('t', "Pexec 4 thread reads+writes parent PROT_P", rc, 0);
        verdict('t', "  parent block afterwards", *priv, MARK);
        gemdos1(0x49, (NLONG)env);
        gemdos1(0x49, (NLONG)pd);
    }
    *priv = PATTERN;

    /* MEM04: a block given back with Mfree is nobody's -- writing it ends
       its former owner (translation fault, "free"); without protection the
       write goes through (stage 3a). */
    rc = run_kid('f', 0);
    verdict('f', "write after Mfree", rc, OFF ? 0 : CRASH);

    /* We are unharmed: our blocks still hold what we wrote. */
    verdict('.', "own PROT_P intact", *priv, PATTERN);
    verdict('.', "own PROT_G intact", *glob, PATTERN);
    verdict('.', "own PROT_PR intact", *rdon, PATTERN);
    rc = run_kid('.', 0);
    verdict('.', "clean child", rc, 0);

    Cconws(fails ? "MPROT3B: FAIL\r\n" : "MPROT3B: PASS\r\n");
    return fails;
}
