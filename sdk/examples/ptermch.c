/*
 * ptermch.c -- a Pexec child that ends with the exit code its command tail
 * asks for (TASK/PROC06).
 *
 * Exists to prove what the *parent's* Pexec receives for a child's exit code:
 * EmuTOS `xterm(UWORD rc)` stores `run->p_dreg[0] = rc` (bdos/proc.c:601,616),
 * so the parent's Pexec (modes 0/4/6) sees `rc & 0xFFFF`, zero-extended --
 * never a negative value (docs/api/gemdos/process/4c-pterm.md, "Exit-code
 * matrix"; 4b-pexec.md, native ABI).
 *
 * The first character of the command tail (BasePage p_cmdlin, 0x80: length
 * byte, then the characters) selects the ending:
 *
 *     m   Pterm(-1)       parent expects 65535   (APITEST T33.1)
 *     h   Pterm(0x8000)   parent expects 32768   (APITEST T33.2)
 *     z   Pterm0()        parent expects 0       (APITEST T33.3)
 *     c   wait in Cconin for a key: a Ctrl-C ends the child through the
 *         console's terminate() = xterm(-32) (bdos/console.c:101), parent
 *         expects 65504 (CTRLCPAR.NCP, sdk/examples/ctrlcpar.c). Any other
 *         key ends it with Pterm(1).
 *     other / empty tail: Pterm(2).
 *
 * Deliberately **no** key wait in the m/h/z modes (TASK/T13): its caller in
 * those modes is C:\APITEST.NCP (case T33, sdk/apps/apitest/apitest.c), which
 * Pexecs it at boot and waits for it to end. Only mode `c` waits, and only
 * CTRLCPAR.NCP uses it.
 *
 * Calls used, all against docs/api/:
 *   Cconws  GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 *   Cconin  GEMDOS 0x01  docs/api/gemdos/console/01-cconin.md
 *   Pterm   GEMDOS 0x4C  docs/api/gemdos/process/4c-pterm.md
 *   Pterm0  GEMDOS 0x00  docs/api/gemdos/process/00-pterm0.md
 */
#include <newtos.h>

/* Offset of `p_cmdlin` in the BasePage (tos-types/src/basepage.rs). */
#define P_CMDLIN 0x80

/* Our own BasePage: `*(_sysbase->os_run)` (OSHEADER +0x28, the pointer to
 * the current-PD cell; docs/api/structures/osheader.md) -- as
 * sdk/examples/cmdline.c. */
static const unsigned char *own_basepage(void)
{
    unsigned long sysbase = *(volatile unsigned int *)0x4F2UL;
    unsigned long run_cell = *(volatile unsigned int *)(sysbase + 0x28);
    return (const unsigned char *)(unsigned long)*(volatile unsigned int *)run_cell;
}

int main(void)
{
    const unsigned char *tail = own_basepage() + P_CMDLIN;
    char mode = tail[0] ? (char)tail[1] : '\0';

    switch (mode) {
    case 'm':
        Cconws("PTERMCH: Pterm(-1)\r\n");
        Pterm(-1);
    case 'h':
        Cconws("PTERMCH: Pterm(0x8000)\r\n");
        Pterm(0x8000);
    case 'z':
        Cconws("PTERMCH: Pterm0()\r\n");
        Pterm0();
    case 'c':
        Cconws("PTERMCH: waiting in Cconin (Ctrl-C ends me with -32)\r\n");
        Cconin();
        Cconws("PTERMCH: Cconin returned a key, Pterm(1)\r\n");
        Pterm(1);
    default:
        Cconws("PTERMCH: unknown mode, Pterm(2)\r\n");
        Pterm(2);
    }
}
