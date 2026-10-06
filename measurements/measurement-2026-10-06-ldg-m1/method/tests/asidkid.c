/*
 * asidkid.c -- the child of the ARCH03 stage-4 ASID probe (driver
 * asidstr.c). Command tail "<case> <8 hex digits>":
 *
 *   z <m>  page-0 check: its own page 0 is fresh (the loader zeroes it), so
 *          the cell must read 0 -- the parent's marker <m> there would be the
 *          parent's page 0 seen through a stale translation; writes its own
 *          marker, switches away and back once (a Pexec of a child that does
 *          nothing), checks it again. Exit 0 = clean, 2 = it saw the
 *          parent's page 0, 9 = some other non-zero value, 3 = its own cell
 *          changed.
 *   n <d>  a chain of d further generations (each one a Pexec 0 child):
 *          every level writes its marker, starts the next level, and checks
 *          its cell and os_run afterwards -- the switch back lands in a root
 *          whose page-0 entry may still be cached under the child's ASID or,
 *          with asid=tiny, whose ASID went stale in a rollover meanwhile.
 *          Exit 0 = the whole chain clean, else 2.
 *   v      victim of the inactive-ASID narrowing case: Malloc a page, write
 *          and read it (its translation is now cached under this process's
 *          ASID), start "f <page>" as a Pexec 3+4 THREAD in this memory
 *          space, which Mfree's the page while this process is not running,
 *          then write the page again. Protection on: the write must fault
 *          (the page is free = nobody's, I8) -- the EmuTOS crash report,
 *          Pterm(-1), the parent sees 0x0000FFFF. A missed invalidation of
 *          this process's ASID would let the write through (exit 0).
 *   f <a>  Mfree(a) and exit 0 (the thread half of 'v'; EIMBA -> exit 4).
 *
 * Build: sdk/build.sh asidkid.c ASIDKID.NAP
 */
#include "asidcommon.h"

#define P_CMDLIN 0x80
#define KIDMARK 0x4B000000UL

static const unsigned char *own_basepage(void)
{
    return (const unsigned char *)os_run_pd();
}

static int chain(UL d)
{
    char tail[12];
    UL me = os_run_pd();
    UL mark = KIDMARK | (d << 16) | (me & 0xFFFF);
    UL rc;
    int bad = 0;

    if (*CELL != 0)
        bad = 1; /* our page 0 is fresh (the loader zeroes it): anything else
                    is another process's page 0 seen through a stale entry */
    *CELL = (unsigned int)mark;
    if (d > 0) {
        make_tail(tail, 'n', d - 1);
        rc = (UL)Pexec(0, "C:\\ASIDKID.NAP", tail, 0);
        if (rc != 0)
            bad = 1;
    }
    if (*CELL != mark || os_run_pd() != me)
        bad = 1;
    return bad;
}

int main(void)
{
    const unsigned char *tail = own_basepage() + P_CMDLIN;
    char c = (char)tail[1];
    UL arg = hex8(tail + 3);

    switch (c) {
    case 'z': {
        UL me = os_run_pd();
        UL mine = KIDMARK | (me & 0xFFFF);
        if (*CELL != 0) /* fresh page 0; the parent's marker would be arg */
            Pterm(*CELL == (unsigned int)arg ? 2 : 9);
        *CELL = (unsigned int)mine;
        /* One more switch away and back: a child that does nothing. */
        Pexec(0, "C:\\ASIDKID.NAP", "\001.", 0);
        if (*CELL != (unsigned int)mine || os_run_pd() != me)
            Pterm(3);
        Pterm(0);
    }
    case 'n':
        Pterm(chain(arg) ? 2 : 0);
    case 'v': {
        volatile unsigned int *m = (volatile unsigned int *)gemdos1(0x48, 4096); /* Malloc */
        char t[12];
        UL pd, rc;
        if (!m)
            Pterm(5);
        *m = (unsigned int)PATTERN;
        if (*m != (unsigned int)PATTERN)
            Pterm(6);
        make_tail(t, 'f', (UL)m);
        pd = (UL)Pexec(3, "C:\\ASIDKID.NAP", t, 0);
        if ((long)pd <= 0)
            Pterm(7);
        rc = (UL)Pexec(4, 0, (const char *)pd, 0);
        if (rc != 0)
            Pterm(8);
        Cconws("ASIDKID: page freed by the thread, writing it again\r\n");
        *m = (unsigned int)MARK;
        Cconws("ASIDKID: write after the thread's Mfree went through\r\n");
        Pterm(0);
    }
    case 'f':
        Pterm(gemdos1(0x49, (NLONG)arg) == 0 ? 0 : 4); /* Mfree */
    default:
        Pterm(0);
    }
}
