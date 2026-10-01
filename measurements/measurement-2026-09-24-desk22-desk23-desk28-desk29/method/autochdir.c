/*
 * autochdir.c -- MEASUREMENT SCAFFOLDING for DESK28 (not a product file).
 *
 * A native NewTOS console program (`.NCP`, ADR-018) that does exactly one
 * thing: change the current directory to `C:\SUB` and say so. Staged as
 * `C:\AUTO\AUTOCHD.NCP` so it runs BEFORE `C:\AUTO\CURDIR.NCP` in the AUTO
 * folder's directory order.
 *
 * Why it exists: on EmuTOS an AUTO program's `Dsetpath` touches only its own
 * basepage copy of `p_curdir` (`bdos/proc.c:400-410`), so the NEXT AUTO
 * program still starts at the root of the boot drive. NewTOS keeps that state
 * in globals, so without DESK28's `set_auto_default_path()` the change leaks
 * into the next program -- which is what the control build shows and the
 * product build must not.
 *
 * Calls used, all against docs/api/:
 *   Dsetpath GEMDOS 0x3B  docs/api/gemdos/directory/3b-dsetpath.md
 *   Cconws   GEMDOS 0x09  docs/api/gemdos/console/09-cconws.md
 */
#include <newtos.h>

static NLONG Dsetpath(const char *p) { return gemdos1(0x3B, (NLONG)p); }

int main(void)
{
    NLONG rc = Dsetpath("\\SUB");

    if (rc == 0)
        Cconws("AUTOCHD: Dsetpath(\"\\SUB\") = 0\r\n");
    else
        Cconws("AUTOCHD: Dsetpath(\"\\SUB\") FAILED\r\n");
    return 0;
}
