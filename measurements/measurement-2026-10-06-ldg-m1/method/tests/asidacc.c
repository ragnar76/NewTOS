/*
 * asidacc.c -- the desk accessory of the ARCH03 stage-4 ASID probe
 * (C:\ACC\ASIDACC.NAC). It wakes on a 1-ms timer, so whenever the running
 * application waits in the AES the scheduler switches to it and back
 * (accessory <-> application address-space switches). On every wake it
 * checks that its page-0 cell still holds its own marker and os_run names
 * itself -- a stale page-0 translation of the application would show the
 * application's marker. Every NWAKE wakes it prints a cumulative verdict
 * line ("ASIDACC: PASS/FAIL wakes=.. bad=..") -- for the whole session (an
 * accessory never ends).
 *
 * Build: sdk/build.sh asidacc.c ASIDACC.NAC
 */
#include "asidcommon.h"

#define NWAKE 300
#define ACCMARK 0xACC00000UL

int main(void)
{
    UL me, mark, wakes = 0, bad = 0;

    if (aes_call(10, 0, 1, 0) < 0) { /* appl_init */
        Cconws("ASIDACC: FAIL (appl_init)\r\n");
        Pterm(1);
    }
    me = os_run_pd();
    mark = ACCMARK | (me & 0xFFFF);
    *CELL = (unsigned int)mark;
    for (;;) {
        evnt_timer(1);
        wakes++;
        if (*CELL != (unsigned int)mark || os_run_pd() != me)
            bad++;
        *CELL = (unsigned int)mark;
        if (wakes % NWAKE == 0) {
            Cconws(bad ? "ASIDACC: FAIL wakes=" : "ASIDACC: PASS wakes=");
            put_dec(wakes);
            Cconws(" bad=");
            put_dec(bad);
            Cconws("\r\n");
        }
    }
}
