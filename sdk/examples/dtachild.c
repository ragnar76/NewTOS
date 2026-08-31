/*
 * dtachild.c -- a child process that moves *its own* DTA and exits.
 *
 * Exists to prove EmuTOS's per-process DTA rule (`bdos/fsmain.c:396,403-405`:
 * `Fsetdta`/`Fgetdta` act on `run->p_xdta`, the running process's basepage
 * field, not on a system-wide pointer). The test is run by the parent:
 *
 *     Fsetdta(parent_buf);
 *     Pexec(0, "C:\\DTACHILD.NCP", "", 0);   -- this program: Fsetdta(0x2000)
 *     Fgetdta() == parent_buf                -- must still hold
 *
 * With a single global DTA the parent's pointer would come back as 0x2000.
 *
 * 0x2000 is inside this child's own TPA (its image base is 0x1000 and the TPA
 * is 4 MiB), so the address is legal for the child and meaningless for the
 * parent -- exactly what makes the leak visible if there is one.
 */
#include <newtos.h>

/* Fsetdta (GEMDOS 0x1A) -- set this process's DTA pointer. */
static inline NLONG Fsetdta(NLONG dta)
{
    return gemdos1(0x1A, dta);
}

/* Fgetdta (GEMDOS 0x2F) -- read this process's DTA pointer back. */
static inline NLONG Fgetdta(void)
{
    return gemdos0(0x2F);
}

int main(void)
{
    NLONG got;

    Cconws("DTACHILD: Fsetdta(0x2000)\r\n");
    Fsetdta((NLONG)0x2000);
    got = Fgetdta();
    if (got == (NLONG)0x2000)
        Cconws("DTACHILD: own Fgetdta round-trip OK\r\n");
    else
        Cconws("DTACHILD: own Fgetdta round-trip FAILED\r\n");
    return 0;
}
