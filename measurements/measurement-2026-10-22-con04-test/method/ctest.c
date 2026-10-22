/*
 * ctest.c -- CON04 acceptance: a console program started from the desktop.
 * Prints a banner through Cconws, waits ~14 s without Cconin (so the scripted
 * mouse injector keeps running) by polling Tgettime (GEMDOS 0x2C), then returns.
 */
#include <newtos.h>

int main(void)
{
    long t0, t;
    int n = 0;

    Cconws("CTEST: console program started from the desktop\r\n");
    Cconws("line two\r\nline three\r\n> ");
    t0 = gemdos0(0x2C);
    while (n < 14) {
        t = gemdos0(0x2C);
        if (t != t0) {
            t0 = t;
            n += 2;
        }
    }
    Cconws("done\r\n");
    return 0;
}
