/*
 * asidstr.c -- ARCH03 stage-4 stress probe: many address-space switches,
 * each followed by a check that the process sees ITS OWN page 0 (the one VA
 * that maps to a different page in every process) and, with protection on,
 * that a page another thread of its memory space freed while it was not
 * running is gone for it too (an invalidation owed to an INACTIVE ASID).
 *
 * Parts (one line per part, "ASIDSTR: PASS"/"FAIL" at the end):
 *   A  NA x: own marker into the page-0 cell, Pexec 0 C:\ASIDKID.NAP "z"
 *      (the child must find a fresh page 0 and keeps its own across one more
 *      switch), then the parent's cell and os_run must be its own again.
 *      Every child gets a slot whose previous tenant's ASID was retired.
 *   B  NB x a chain of DEPTH generations (ASIDKID "n"): with asid=tiny the
 *      ASIDs roll over inside the chain, so the switches back land in roots
 *      whose ASID went stale and is re-assigned.
 *   C  (not in ASIDPERF) the inactive-ASID narrowing case (ASIDKID "v"):
 *      protection on -> the victim must be stopped (0x0000FFFF), off -> its
 *      write goes through (0).
 *   D  GEM: appl_init, ND x { own marker; evnt_timer(1) -- the AES gives
 *      the CPU to a runnable accessory (C:\ACC\ASIDACC.NAC, which checks its
 *      own page 0 on every wake); cell/os_run/hz_200 checks }, appl_exit.
 *      Without the AES (virt, launched before it) part D is skipped.
 * Timing: "<part> begin/end" lines, timestamped on the host
 * (drive.py --stamp) -- QEMU wall-clock, only indicative (QEMU has no real
 * TLB cost model).
 *
 * Build variants (-D via a prepended #define, build-asid.sh):
 *   ASID_OFF 0/1  expectation of part C (protection on or strict / off)
 *   ASIDPERF 0/1  1 = no part C (no crash prompt; for the product kernel),
 *                 more iterations, prints "ASIDPERF:" instead of "ASIDSTR:"
 */
#include "asidcommon.h"

#ifndef ASID_OFF
#define ASID_OFF 0
#endif
#ifndef ASIDPERF
#define ASIDPERF 0
#endif

#if ASIDPERF
#define TAG "ASIDPERF: "
#define NA 64
#define NB 4
#define DEPTH 6
#define ND 400
#else
#define TAG "ASIDSTR: "
#define NA 32
#define NB 4
#define DEPTH 6
#define ND 200
#endif

#define PARENTMARK 0xA5000000UL

static int fails;

static void verdict(const char *what, UL got, UL want)
{
    Cconws(TAG);
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

/* Part boundaries for the host-side timestamps (drive.py --stamp): the
   page-0 _hz_200 copy is refreshed only while its process runs, so it cannot
   time a loop of switches from inside one process. */
static void mark(const char *what, UL n)
{
    Cconws(TAG);
    Cconws(what);
    Cconws(" n=");
    put_dec(n);
    Cconws("\r\n");
}

int main(void)
{
    char tail[12];
    UL me = os_run_pd();
    UL i, rc, bad, badrc;

    Cconws(TAG);
    Cconws(ASID_OFF ? "start (part C expects memprot=off)\r\n"
                    : "start (part C expects protection on/strict)\r\n");

    /* A: parent <-> fresh child, page 0 both ways. */
    bad = 0;
    badrc = 0;
    mark("A begin", NA);
    for (i = 0; i < NA; i++) {
        UL m = PARENTMARK | i;
        *CELL = (unsigned int)m;
        make_tail(tail, 'z', m);
        rc = (UL)Pexec(0, "C:\\ASIDKID.NAP", tail, 0);
        if (rc != 0) {
            badrc++;
            if (badrc == 1)
                verdict("A first bad child exit", rc, 0);
        }
        if (*CELL != (unsigned int)m || os_run_pd() != me)
            bad++;
    }
    mark("A end", NA);
    verdict("A children that saw a foreign page 0", badrc, 0);
    verdict("A parent page-0 cell/os_run wrong after the child", bad, 0);

    /* B: chains. */
    bad = 0;
    mark("B begin", NB * DEPTH);
    for (i = 0; i < NB; i++) {
        UL m = PARENTMARK | 0x100 | i;
        *CELL = (unsigned int)m;
        make_tail(tail, 'n', DEPTH - 1);
        rc = (UL)Pexec(0, "C:\\ASIDKID.NAP", tail, 0);
        if (rc != 0 || *CELL != (unsigned int)m || os_run_pd() != me)
            bad++;
    }
    mark("B end", NB * DEPTH);
    verdict("B chains with a wrong page 0", bad, 0);

#if !ASIDPERF
    /* C: a page freed by a thread of our space while the victim slept. */
    rc = (UL)Pexec(0, "C:\\ASIDKID.NAP", "\001v", 0);
    verdict(ASID_OFF ? "C write after the thread's Mfree (off: goes through)"
                     : "C write after the thread's Mfree (stopped)",
            rc, ASID_OFF ? 0 : CRASH);
    verdict("C own page-0 cell after", *CELL, PARENTMARK | 0x100 | (NB - 1));
#endif

    /* D: GEM, an accessory gets the CPU while we wait. */
    if (aes_call(10, 0, 1, 0) >= 0 && aes_global[2] >= 0) { /* appl_init */
        UL h0, h1, backwards = 0;
        bad = 0;
        mark("D begin", ND);
        h0 = HZ200;
        for (i = 0; i < ND; i++) {
            UL m = PARENTMARK | 0x200 | i;
            *CELL = (unsigned int)m;
            evnt_timer(1);
            if (*CELL != (unsigned int)m || os_run_pd() != me)
                bad++;
            h1 = HZ200;
            if (h1 < h0)
                backwards++; /* another process's (older) _hz_200 copy */
            h0 = h1;
        }
        mark("D end", ND);
        verdict("D page-0 cell/os_run wrong after a wait", bad, 0);
        verdict("D _hz_200 went backwards", backwards, 0);
        aes_call(19, 0, 1, 0); /* appl_exit */
    } else {
        Cconws(TAG);
        Cconws("D skipped (no AES)\r\n");
    }

    Cconws(fails ? TAG "FAIL\r\n" : TAG "PASS\r\n");
    return fails;
}
