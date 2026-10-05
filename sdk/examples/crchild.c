/*
 * crchild.c -- a program that crashes on purpose (ARCH03 stage 0c).
 *
 * The first character of the command tail selects how it dies; with no tail
 * it takes case 'w', so a bare `#Z` autorun line crashes it too:
 *
 *   'w'  store to the unmapped user address 0xDEAD0000   -> Bus Error
 *        (Data Abort from EL0, translation fault)
 *   'u'  execute an undefined instruction (UDF #0)       -> Illegal Instruction
 *   'x'  jump into the screen memory Physbase() returns,
 *        which is mapped non-executable for EL0          -> Bus Error
 *        (Instruction Abort from EL0, permission fault)
 *   'a'  branch to a misaligned address (text + 2)       -> Address Error
 *   'c'  Cconws((char *)0xDEAD0000): the KERNEL faults on
 *        the bad pointer inside the system call          -> Bus Error, caller killed
 *   'p'  Pexec(0, (char *)0xDEAD0000, "", 0): the same,
 *        in the Pexec path reader                        -> Bus Error, caller killed
 *   'o'  no crash: print a line and Pterm(0)
 *
 * Expected for every crash case (docs/api/bios/_exception-crash.md, EmuTOS
 * `dopanic` + `kill_program`): the crash report on the console, a key wait,
 * then the program is terminated with Pterm(-1) -- the parent's Pexec gets
 * 0x0000FFFF -- and the machine keeps running. The line after the crash
 * ("CRCHILD: survived") must never appear.
 *
 * Build: sdk/build.sh sdk/examples/crchild.c <out>/CRCHILD.NAP
 */
#include <newtos.h>

/* Offset of `p_cmdlin` in the BasePage (tos-types/src/basepage.rs). */
#define P_CMDLIN 0x80

/* Our own BasePage: `*(_sysbase->os_run)` (OSHEADER +0x28, the pointer to
 * the current-PD cell). Since ARCH03 stage 1 the basepage is the start of
 * the TPA block, no longer VA 0. */
static const unsigned char *own_basepage(void)
{
    unsigned long sysbase = *(volatile unsigned int *)0x4F2UL;
    unsigned long run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (const unsigned char *)(unsigned long)*(volatile unsigned int *)run_cell;
}

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

static void target(void)
{
    Cconws("CRCHILD: misaligned target reached?!\r\n");
}

int main(void)
{
    const unsigned char *tail = own_basepage() + P_CMDLIN;
    char c = tail[0] ? (char)tail[1] : 'w';
    char line[] = "CRCHILD: case ? starting\r\n";

    line[14] = c;
    Cconws(line);

    switch (c) {
    case 'w':
        *(volatile unsigned int *)0xDEAD0000UL = 0x12345678;
        break;
    case 'u':
        __asm__ volatile(".inst 0x00000000" ::: "memory"); /* UDF #0 */
        break;
    case 'x': {
        void (*fn)(void) = (void (*)(void))xbios0(2); /* Physbase */
        fn();
        break;
    }
    case 'a': {
        unsigned long to = (unsigned long)&target + 2;
        __asm__ volatile("blr %0" :: "r"(to) : "x30", "memory");
        break;
    }
    case 'c':
        Cconws((const char *)0xDEAD0000UL);
        break;
    case 'p':
        Pexec(0, (const char *)0xDEAD0000UL, "", 0);
        break;
    case 'o':
        Cconws("CRCHILD: clean exit\r\n");
        return 0;
    default:
        Cconws("CRCHILD: unknown case\r\n");
        return 2;
    }
    Cconws("CRCHILD: survived -- FAIL\r\n");
    return 1;
}
